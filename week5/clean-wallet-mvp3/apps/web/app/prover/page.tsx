"use client";

import Link from "next/link";
import { useState } from "react";
import { demoScenarioOrder, demoScenarios, makeDemoArtifact, type DemoScenarioId } from "@/lib/guided-demo";
import type { Policy, DepositIntent } from "@/lib/policy";
import { fetchAttestation, postScreen } from "@/lib/scanner-client";

const DEFAULT_SCANNER = process.env.NEXT_PUBLIC_SCANNER_URL ?? "http://localhost:8080";

type FixtureScenario = "clean" | "dirty";

type DemoFixtures = {
  policy: { value: unknown; source: string; present: boolean };
  wallets: Record<FixtureScenario, { value: string; source: string; present: boolean; mainnetReady: boolean }>;
  depositIntents: Record<FixtureScenario, { value: unknown; source: string; present: boolean }>;
};

export default function ProverPage() {
  const [scenarioId, setScenarioId] = useState<DemoScenarioId>("clean");
  const [demoRan, setDemoRan] = useState(false);
  const [scannerUrl, setScannerUrl] = useState(DEFAULT_SCANNER);
  const [policyJson, setPolicyJson] = useState("");
  const [intentJson, setIntentJson] = useState("");
  const [ufvk, setUfvk] = useState("");
  const [attestationStatus, setAttestationStatus] = useState<string>("");
  const [scannerMeasurement, setScannerMeasurement] = useState<string>("");
  const [result, setResult] = useState<string>("");
  const [error, setError] = useState<string>("");
  const [fixtureStatus, setFixtureStatus] = useState<string>("");
  const scenario = demoScenarios[scenarioId];

  function selectScenario(nextScenario: DemoScenarioId) {
    setScenarioId(nextScenario);
    setDemoRan(false);
  }

  function runGuidedDemo() {
    setDemoRan(true);
  }

  async function checkAttestation() {
    setError("");
    setAttestationStatus("Checking live scanner attestation...");
    try {
      const resp = await fetchAttestation(scannerUrl);
      setScannerMeasurement(resp.code_measurement);
      setAttestationStatus("Live Phala CVM attestation received. Compare this measurement with the policy before uploading a UFVK.");
    } catch (e) {
      setError(String(e));
      setAttestationStatus("");
    }
  }

  async function submitScreen() {
    setError("");
    setResult("Submitting live screening request...");
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

  async function loadLiveFixture(fixtureScenario: FixtureScenario) {
    setError("");
    setResult("");
    setFixtureStatus("Loading public demo fixtures...");
    try {
      const resp = await fetch("/api/demo-fixtures");
      if (!resp.ok) throw new Error(`/api/demo-fixtures returned ${resp.status}`);
      const fixtures: DemoFixtures = await resp.json();
      const wallet = fixtures.wallets[fixtureScenario];
      const intent = fixtures.depositIntents[fixtureScenario];
      setPolicyJson(JSON.stringify(fixtures.policy.value, null, 2));
      setIntentJson(JSON.stringify(intent.value, null, 2));
      setUfvk(wallet.value);
      const label = fixtureScenario === "clean" ? "Wallet A" : "Wallet B";
      const readiness = wallet.mainnetReady
        ? "Mainnet UFVK detected."
        : "Not live-ready yet: this UFVK is missing or still starts with uviewtest.";
      const intentNote = intent.present
        ? `Loaded ${intent.source}.`
        : "Generated a placeholder deposit intent; replace it before claiming a live run.";
      setFixtureStatus(`${label} fixtures loaded. ${readiness} ${intentNote}`);
    } catch (e) {
      setError(String(e));
      setFixtureStatus("");
    }
  }

  return (
    <main>
      <section className="pageIntro demoIntro">
        <div className="introBadge">Guided demo</div>
        <p className="eyebrow">User-side private screening</p>
        <h1>Choose a wallet story. See the screening decision.</h1>
        <p>
          Start with the two demo scenarios below. The wallet history is inspected
          inside the scanner; the exchange receives only an attested result.
        </p>
      </section>

      <section className="demoSection">
        <div className="sectionHeading">
          <p className="eyebrow">1. Choose a wallet</p>
          <h2>What should the scanner evaluate?</h2>
        </div>
        <div className="scenarioGrid">
          {demoScenarioOrder.map((id) => {
            const option = demoScenarios[id];
            return (
              <button
                className={`scenarioCard ${scenarioId === id ? "selected" : ""} ${option.result === "FAIL" ? "risk" : ""}`}
                key={id}
                onClick={() => selectScenario(id)}
                type="button"
              >
                <span className="scenarioIcon">{option.result === "PASS" ? "✓" : "!"}</span>
                <span className="scenarioCopy">
                  <small>{option.eyebrow}</small>
                  <strong>{option.label}</strong>
                  <span>{option.description}</span>
                </span>
              </button>
            );
          })}
        </div>
      </section>

      <section className="demoSection">
        <div className="sectionHeading">
          <p className="eyebrow">2. Preview private scan</p>
          <h2>{scenario.walletLabel} outgoing transfers</h2>
          <p>These rows explain the scenario. They stay inside the scanner and are never sent to the exchange.</p>
        </div>
        <div className="scanPreview">
          <div className="walletNode">
            <span>Private wallet</span>
            <strong>{scenario.walletLabel}</strong>
          </div>
          <div className="transferList">
            {scenario.outgoingTransfers.map((transfer) => (
              <div className={`transferRow ${transfer.sanctioned ? "sanctioned" : ""}`} key={transfer.label}>
                <span className="transferDirection">→</span>
                <div>
                  <strong>{transfer.label}</strong>
                  <small>{transfer.amount}</small>
                </div>
                <span className={`riskTag ${transfer.sanctioned ? "bad" : "ok"}`}>
                  {transfer.sanctioned ? "Sanctioned match" : "No match"}
                </span>
              </div>
            ))}
          </div>
        </div>
        <button className="primaryButton actionButton" onClick={runGuidedDemo} type="button">
          Run guided screening for {scenario.walletLabel}
        </button>
        <p className="demoDisclaimer">Guided demo preview: this visual path explains the product flow. Use Advanced live mode below for a real provisioned mainnet UFVK.</p>
      </section>

      {demoRan && (
        <section className={`resultHero ${scenario.result === "PASS" ? "pass" : "fail"}`}>
          <div>
            <p className="eyebrow">3. Scanner artifact ready</p>
            <h2>{scenario.result === "PASS" ? "PASS: clean wallet" : "FAIL: sanctioned transfer found"}</h2>
            <p>{scenario.decision}</p>
          </div>
          <div className="resultMetrics">
            <div><span>Outgoing recipients checked</span><strong>{scenario.recipientCount}</strong></div>
            <div><span>Sanctioned matches</span><strong>{scenario.sanctionedHitCount}</strong></div>
            <div><span>Shared wallet records</span><strong>0</strong></div>
          </div>
          <Link className="resultLink" href={`/verifier?scenario=${scenario.id}`}>
            Send artifact to exchange verifier →
          </Link>
          <details className="artifactDetails">
            <summary>Show demo artifact JSON</summary>
            <pre>{JSON.stringify(makeDemoArtifact(scenario), null, 2)}</pre>
          </details>
        </section>
      )}

      <details className="advancedPanel">
        <summary>Advanced live mode: call the deployed Phala scanner</summary>
        <p className="muted">
          This calls the real CVM. It requires a provisioned mainnet UFVK, policy,
          and unexpired deposit intent. The inherited early POC fixtures are testnet-only.
        </p>
        <div className="fixtureToolbar" aria-label="Public fixture loader">
          <button type="button" onClick={() => loadLiveFixture("clean")}>Load Wallet A fixtures</button>
          <button type="button" onClick={() => loadLiveFixture("dirty")}>Load Wallet B fixtures</button>
          <span>Loads only public or read-only demo files from `demo-data`.</span>
        </div>
        {fixtureStatus && <p className="notice warn">{fixtureStatus}</p>}
        <div className="workbench liveGrid">
          <section className="panel">
            <div className="panelHeader">
              <p className="eyebrow">Live step 01</p>
              <h2>Fetch CVM attestation</h2>
            </div>
            <label className="fieldGroup">
              <span>Scanner URL</span>
              <input value={scannerUrl} onChange={(e) => setScannerUrl(e.target.value)} />
            </label>
            <button className="primaryButton" onClick={checkAttestation}>Fetch live attestation</button>
            {attestationStatus && <p className="notice warn">{attestationStatus}</p>}
            {scannerMeasurement && <div className="artifactStatus"><span>Scanner code measurement</span><strong>{scannerMeasurement}</strong></div>}
          </section>
          <section className="panel inputPanel">
            <div className="panelHeader">
              <p className="eyebrow">Live step 02</p>
              <h2>Submit real mainnet inputs</h2>
            </div>
            <label className="fieldGroup"><span>UFVK</span><textarea value={ufvk} onChange={(e) => setUfvk(e.target.value)} /></label>
            <label className="fieldGroup"><span>Policy JSON</span><textarea value={policyJson} onChange={(e) => setPolicyJson(e.target.value)} /></label>
            <label className="fieldGroup"><span>Deposit intent JSON</span><textarea value={intentJson} onChange={(e) => setIntentJson(e.target.value)} /></label>
            <button className="primaryButton" onClick={submitScreen}>Run live private screening</button>
            {error && <pre className="errorBox">{error}</pre>}
          </section>
          <section className="panel artifactPanel">
            <div className="panelHeader">
              <p className="eyebrow">Live step 03</p>
              <h2>Real attested bundle</h2>
            </div>
            <textarea aria-label="Screening bundle JSON" readOnly value={result} placeholder="The real screening bundle will appear here." />
          </section>
        </div>
      </details>
    </main>
  );
}
