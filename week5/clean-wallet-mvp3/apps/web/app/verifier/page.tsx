"use client";

import { useEffect, useState } from "react";
import { demoScenarioOrder, demoScenarios, makeDemoArtifact, type DemoScenarioId } from "@/lib/guided-demo";
import type { Quote } from "@/lib/verify-quote";

type Bundle = { artifact: unknown; quote: Quote };
type CheckResult = { ok: boolean; detail: string };
type VerifyBundleResponse = {
  check1_quoteGenuine: CheckResult;
  check2_quoteBindsArtifact: CheckResult;
  check3_artifactBindsContext: CheckResult;
  finalResult?: "PASS" | "FAIL";
};

type FixtureScenario = "clean" | "dirty";

type DemoFixtures = {
  policy: { value: unknown; source: string; present: boolean };
  depositIntents: Record<FixtureScenario, { value: unknown; source: string; present: boolean }>;
  bundles: Record<FixtureScenario, { value: unknown; source: string; present: boolean }>;
};

export default function VerifierPage() {
  const [scenarioId, setScenarioId] = useState<DemoScenarioId>("clean");
  const [demoVerified, setDemoVerified] = useState(false);
  const [bundleJson, setBundleJson] = useState("");
  const [policyJson, setPolicyJson] = useState("");
  const [intentJson, setIntentJson] = useState("");
  const [check1, setCheck1] = useState<CheckResult | null>(null);
  const [check2, setCheck2] = useState<CheckResult | null>(null);
  const [check3, setCheck3] = useState<CheckResult | null>(null);
  const [finalResult, setFinalResult] = useState<"PASS" | "FAIL" | null>(null);
  const [error, setError] = useState<string>("");
  const [fixtureStatus, setFixtureStatus] = useState<string>("");
  const scenario = demoScenarios[scenarioId];

  useEffect(() => {
    const requested = new URLSearchParams(window.location.search).get("scenario");
    if (requested === "clean" || requested === "sanctioned") setScenarioId(requested);
  }, []);

  function selectScenario(nextScenario: DemoScenarioId) {
    setScenarioId(nextScenario);
    setDemoVerified(false);
  }

  async function verify() {
    setError("");
    setCheck1(null); setCheck2(null); setCheck3(null); setFinalResult(null);
    try {
      const bundle: Bundle = JSON.parse(bundleJson);
      const policy = JSON.parse(policyJson);
      const depositIntent = JSON.parse(intentJson);
      const quoteResp = await fetch("/api/verify-quote", {
        method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(bundle.quote),
      });
      const quoteVerification = await quoteResp.json();
      const bundleResp = await fetch("/api/verify-bundle", {
        method: "POST", headers: { "content-type": "application/json" },
        body: JSON.stringify({ artifact: bundle.artifact, quote: bundle.quote, policy, depositIntent, quoteVerification }),
      });
      if (!bundleResp.ok) throw new Error(`/api/verify-bundle returned ${bundleResp.status}`);
      const result: VerifyBundleResponse = await bundleResp.json();
      setCheck1(result.check1_quoteGenuine); setCheck2(result.check2_quoteBindsArtifact);
      setCheck3(result.check3_artifactBindsContext); setFinalResult(result.finalResult ?? null);
    } catch (e) {
      setError(String(e));
    }
  }

  async function loadVerifierFixture(fixtureScenario: FixtureScenario) {
    setError("");
    setFixtureStatus("Loading verifier fixtures...");
    setCheck1(null); setCheck2(null); setCheck3(null); setFinalResult(null);
    try {
      const resp = await fetch("/api/demo-fixtures");
      if (!resp.ok) throw new Error(`/api/demo-fixtures returned ${resp.status}`);
      const fixtures: DemoFixtures = await resp.json();
      const bundle = fixtures.bundles[fixtureScenario];
      const intent = fixtures.depositIntents[fixtureScenario];
      setPolicyJson(JSON.stringify(fixtures.policy.value, null, 2));
      setIntentJson(JSON.stringify(intent.value, null, 2));
      setBundleJson(bundle.present ? JSON.stringify(bundle.value, null, 2) : "");
      const label = fixtureScenario === "clean" ? "Wallet A" : "Wallet B";
      const bundleNote = bundle.present
        ? `Loaded ${bundle.source}.`
        : `No saved ${bundle.source} yet. Run live screening first, then save the bundle.`;
      const intentNote = intent.present
        ? `Loaded ${intent.source}.`
        : "Generated a placeholder deposit intent; replace it before live verification.";
      setFixtureStatus(`${label} verifier context loaded. ${bundleNote} ${intentNote}`);
    } catch (e) {
      setError(String(e));
      setFixtureStatus("");
    }
  }

  return (
    <main>
      <section className="pageIntro demoIntro">
        <div className="introBadge">Exchange view</div>
        <p className="eyebrow">Exchange verifier</p>
        <h1>Verify the artifact. Decide the deposit.</h1>
        <p>
          The exchange never receives shielded wallet history. It verifies that
          a trusted scanner produced an untampered result for this deposit.
        </p>
      </section>

      <section className="demoSection">
        <div className="sectionHeading">
          <p className="eyebrow">1. Choose an incoming artifact</p>
          <h2>Compare the two exchange decisions</h2>
        </div>
        <div className="scenarioGrid">
          {demoScenarioOrder.map((id) => {
            const option = demoScenarios[id];
            return (
              <button className={`scenarioCard ${scenarioId === id ? "selected" : ""} ${option.result === "FAIL" ? "risk" : ""}`} key={id} onClick={() => selectScenario(id)} type="button">
                <span className="scenarioIcon">{option.result === "PASS" ? "✓" : "!"}</span>
                <span className="scenarioCopy"><small>{option.walletLabel} artifact</small><strong>{option.label}</strong><span>{option.description}</span></span>
              </button>
            );
          })}
        </div>
        <button className="primaryButton actionButton" onClick={() => setDemoVerified(true)} type="button">
          Verify {scenario.walletLabel} artifact
        </button>
        <p className="demoDisclaimer">Guided demo preview: the live verifier for a real Phala quote remains available below.</p>
      </section>

      {demoVerified && (
        <section className="verifierReport">
          <div className={`verdict ${scenario.result === "PASS" ? "pass" : "fail"}`}>
            <span>{scenario.result === "PASS" ? "Deposit approved" : "Deposit rejected"}</span>
            <strong>{scenario.result}</strong>
          </div>
          <div className="decisionExplainer">
            <p className="eyebrow">What the exchange learned</p>
            <h2>{scenario.decision}</h2>
            <p>The exchange learned the decision and integrity checks below. It did not receive transaction-level wallet records.</p>
          </div>
          <div className="checkList">
            <VerificationCheck label="1. Trusted scanner" detail="The artifact came from the approved measured TEE scanner." />
            <VerificationCheck label="2. Untampered artifact" detail="The attestation seal binds the exact screening artifact." />
            <VerificationCheck label="3. Correct deposit context" detail="The artifact is bound to this deposit, policy, and audit range." />
            <VerificationCheck
              label="4. Sanctions screening result"
              detail={scenario.result === "PASS" ? "No sanctioned outgoing recipient was found." : "One sanctioned outgoing recipient was found."}
              emphasis={scenario.result === "FAIL" ? "bad" : "ok"}
            />
          </div>
          <details className="artifactDetails">
            <summary>Show artifact received by exchange</summary>
            <pre>{JSON.stringify(makeDemoArtifact(scenario), null, 2)}</pre>
          </details>
        </section>
      )}

      <details className="advancedPanel">
        <summary>Advanced live mode: verify a real Phala artifact</summary>
        <p className="muted">Paste the real scanner bundle and the exchange-side context. This path calls the real Phala quote verification API.</p>
        <div className="fixtureToolbar" aria-label="Public verifier fixture loader">
          <button type="button" onClick={() => loadVerifierFixture("clean")}>Load Wallet A context</button>
          <button type="button" onClick={() => loadVerifierFixture("dirty")}>Load Wallet B context</button>
          <span>Loads saved public bundles and context from `demo-data` when present.</span>
        </div>
        {fixtureStatus && <p className="notice warn">{fixtureStatus}</p>}
        <div className="workbench verifierGrid">
          <section className="panel inputPanel">
            <div className="panelHeader"><p className="eyebrow">Live verification inputs</p><h2>Paste artifact and context</h2></div>
            <label className="fieldGroup"><span>Bundle: artifact and quote</span><textarea value={bundleJson} onChange={(e) => setBundleJson(e.target.value)} /></label>
            <label className="fieldGroup"><span>Local policy JSON</span><textarea value={policyJson} onChange={(e) => setPolicyJson(e.target.value)} /></label>
            <label className="fieldGroup"><span>Local deposit intent JSON</span><textarea value={intentJson} onChange={(e) => setIntentJson(e.target.value)} /></label>
            <button className="primaryButton" onClick={verify}>Verify real artifact</button>
            {error && <pre className="errorBox">{error}</pre>}
          </section>
          <section className="panel verifierPanel">
            <div className="panelHeader"><p className="eyebrow">Live verification report</p><h2>Exchange decision</h2></div>
            <div className={`verdict ${finalResult === "PASS" ? "pass" : finalResult === "FAIL" ? "fail" : "pending"}`}>
              {finalResult === "PASS" ? "Deposit approved: PASS" : finalResult === "FAIL" ? "Deposit rejected: FAIL" : "Awaiting live artifact"}
            </div>
            <div className="checkList">
              <LiveVerificationCheck label="TDX quote authenticity" value={check1} />
              <LiveVerificationCheck label="Quote binds artifact" value={check2} />
              <LiveVerificationCheck label="Artifact binds context" value={check3} />
            </div>
          </section>
        </div>
      </details>
    </main>
  );
}

function VerificationCheck({ label, detail, emphasis = "ok" }: { label: string; detail: string; emphasis?: "ok" | "bad" }) {
  return <div className="checkItem"><div className={`checkIcon ${emphasis}`}>{emphasis === "ok" ? "✓" : "!"}</div><div><strong>{label}</strong><p>{detail}</p></div></div>;
}

function LiveVerificationCheck({ label, value }: { label: string; value: CheckResult | null }) {
  return <div className="checkItem"><div className={`checkIcon ${value ? (value.ok ? "ok" : "bad") : "idle"}`}>{value ? (value.ok ? "✓" : "!") : "-"}</div><div><strong>{label}</strong><p>{value?.detail ?? "Run live verification to evaluate this check."}</p></div></div>;
}
