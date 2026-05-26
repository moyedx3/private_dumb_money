"use client";
import { useState } from "react";
import type { Policy, DepositIntent } from "@/lib/policy";
import { fetchAttestation, postScreen } from "@/lib/scanner-client";

const DEFAULT_SCANNER = process.env.NEXT_PUBLIC_SCANNER_URL ?? "http://localhost:8080";

export default function ProverPage() {
  const [scannerUrl, setScannerUrl] = useState(DEFAULT_SCANNER);
  const [policyJson, setPolicyJson] = useState("");
  const [intentJson, setIntentJson] = useState("");
  const [ufvk, setUfvk] = useState("");
  const [attestationStatus, setAttestationStatus] = useState<string>("");
  const [scannerMeasurement, setScannerMeasurement] = useState<string>("");
  const [result, setResult] = useState<string>("");
  const [error, setError] = useState<string>("");

  async function checkAttestation() {
    setError(""); setAttestationStatus("checking…");
    try {
      const q = await fetchAttestation(scannerUrl);
      const measurement = (q.vm_config as { measurement?: string })?.measurement
        ?? "(unknown — verify via dstack-verifier)";
      setScannerMeasurement(measurement);
      setAttestationStatus("scanner returned a quote; verify the code measurement below matches your policy.expectedScannerCodeMeasurement before uploading your UFVK.");
    } catch (e) {
      setError(String(e));
      setAttestationStatus("");
    }
  }

  async function submitScreen() {
    setError(""); setResult("submitting…");
    try {
      const policy: Policy = JSON.parse(policyJson);
      const depositIntent: DepositIntent = JSON.parse(intentJson);
      const out = await postScreen(scannerUrl, { ufvk, policy, depositIntent });
      setResult(JSON.stringify(out, null, 2));
    } catch (e) {
      setError(String(e));
      setResult("");
    }
  }

  return (
    <main>
      <h1>Prover (User)</h1>

      <h2>1. Pre-flight: verify the scanner</h2>
      <label>Scanner URL: <input value={scannerUrl} onChange={(e) => setScannerUrl(e.target.value)} size={50} /></label>
      <p><button onClick={checkAttestation}>Fetch attestation</button></p>
      {attestationStatus && <p className="warn">{attestationStatus}</p>}
      {scannerMeasurement && (
        <pre>scanner code measurement: {scannerMeasurement}</pre>
      )}

      <h2>2. Provide inputs</h2>
      <label>UFVK: <textarea value={ufvk} onChange={(e) => setUfvk(e.target.value)} /></label>
      <label>Policy JSON: <textarea value={policyJson} onChange={(e) => setPolicyJson(e.target.value)} /></label>
      <label>DepositIntent JSON: <textarea value={intentJson} onChange={(e) => setIntentJson(e.target.value)} /></label>
      <p><button onClick={submitScreen}>Submit screening request</button></p>

      {error && <pre className="fail">{error}</pre>}
      {result && (
        <>
          <h2>3. Copy this blob to the exchange</h2>
          <textarea readOnly value={result} style={{ minHeight: "16rem" }} />
        </>
      )}
    </main>
  );
}
