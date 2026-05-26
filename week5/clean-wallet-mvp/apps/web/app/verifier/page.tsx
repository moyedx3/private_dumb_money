"use client";
import { useState } from "react";
import type { Quote } from "@/lib/verify-quote";

type Bundle = { artifact: unknown; quote: Quote };

type CheckResult = { ok: boolean; detail: string };

type VerifyBundleResponse = {
  check1_quoteGenuine: CheckResult;
  check2_quoteBindsArtifact: CheckResult;
  check3_artifactBindsContext: CheckResult;
  finalResult?: "PASS" | "FAIL";
};

export default function VerifierPage() {
  const [bundleJson, setBundleJson] = useState("");
  const [policyJson, setPolicyJson] = useState("");
  const [intentJson, setIntentJson] = useState("");
  const [check1, setCheck1] = useState<CheckResult | null>(null);
  const [check2, setCheck2] = useState<CheckResult | null>(null);
  const [check3, setCheck3] = useState<CheckResult | null>(null);
  const [finalResult, setFinalResult] = useState<"PASS" | "FAIL" | null>(null);
  const [error, setError] = useState<string>("");

  async function verify() {
    setError("");
    setCheck1(null); setCheck2(null); setCheck3(null); setFinalResult(null);
    try {
      const bundle: Bundle = JSON.parse(bundleJson);
      const policy = JSON.parse(policyJson);
      const depositIntent = JSON.parse(intentJson);

      // Step 1: verify the quote against dstack-verifier
      const quoteResp = await fetch("/api/verify-quote", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(bundle.quote),
      });
      const quoteVerification = await quoteResp.json();

      // Step 2: verify the full bundle (passes quoteVerification through)
      const bundleResp = await fetch("/api/verify-bundle", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          artifact: bundle.artifact,
          quote: bundle.quote,
          policy,
          depositIntent,
          quoteVerification,
        }),
      });
      if (!bundleResp.ok) throw new Error(`/api/verify-bundle returned ${bundleResp.status}`);
      const result: VerifyBundleResponse = await bundleResp.json();

      setCheck1(result.check1_quoteGenuine);
      setCheck2(result.check2_quoteBindsArtifact);
      setCheck3(result.check3_artifactBindsContext);
      setFinalResult(result.finalResult ?? null);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <main>
      <h1>Verifier (Exchange)</h1>

      <h2>Inputs</h2>
      <label>Bundle (artifact + quote):
        <textarea value={bundleJson} onChange={(e) => setBundleJson(e.target.value)} />
      </label>
      <label>Local Policy JSON:
        <textarea value={policyJson} onChange={(e) => setPolicyJson(e.target.value)} />
      </label>
      <label>Local DepositIntent JSON:
        <textarea value={intentJson} onChange={(e) => setIntentJson(e.target.value)} />
      </label>
      <p><button onClick={verify}>Verify</button></p>

      {error && <pre className="fail">{error}</pre>}

      {check1 && (
        <pre className={check1.ok ? "pass" : "fail"}>
          {check1.ok ? "✅" : "❌"} Check 1: {check1.detail}
        </pre>
      )}
      {check2 && (
        <pre className={check2.ok ? "pass" : "fail"}>
          {check2.ok ? "✅" : "❌"} Check 2: {check2.detail}
        </pre>
      )}
      {check3 && (
        <pre className={check3.ok ? "pass" : "fail"}>
          {check3.ok ? "✅" : "❌"} Check 3: {check3.detail}
        </pre>
      )}

      {finalResult === "PASS" && (
        <p className="pass" style={{ fontSize: "1.4rem", marginTop: "1.5rem" }}>
          RESULT: PASS — deposit accepted by policy.
        </p>
      )}
      {finalResult === "FAIL" && (
        <p className="fail" style={{ fontSize: "1.4rem", marginTop: "1.5rem" }}>
          RESULT: FAIL — sanctioned recipient found.
        </p>
      )}
    </main>
  );
}
