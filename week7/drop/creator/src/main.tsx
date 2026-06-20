import React, { useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import { fetchAttestation, postProvision, uploadContentBlob } from "./api";
import { verifyAttestationOrThrow } from "./attestation";
import { utf8Bytes } from "./bytes";
import { encryptContent } from "./content";
import { priceZecToZatNumber } from "./price";
import { buildProvisionPayload, sealProvisionPayload } from "./provision";
import "./styles.css";

type StepState = "idle" | "running" | "done" | "error";

type Steps = {
  encrypt: StepState;
  attest: StepState;
  provision: StepState;
};

const defaultIndexer = import.meta.env.VITE_DROP_INDEXER_URL ?? "http://localhost:8080";
const defaultMeasurement = import.meta.env.VITE_DROP_EXPECTED_MEASUREMENT_HEX ?? "";

function App() {
  const [indexerUrl, setIndexerUrl] = useState(defaultIndexer);
  const [expectedMeasurement, setExpectedMeasurement] = useState(defaultMeasurement);
  const [title, setTitle] = useState("Private demo drop");
  const [dropId, setDropId] = useState("1");
  const [priceZec, setPriceZec] = useState("0.01");
  const [creatorUfvk, setCreatorUfvk] = useState("");
  const [textContent, setTextContent] = useState("Hello from a locally encrypted drop.");
  const [file, setFile] = useState<File | null>(null);
  const [steps, setSteps] = useState<Steps>({ encrypt: "idle", attest: "idle", provision: "idle" });
  const [message, setMessage] = useState("");
  const [result, setResult] = useState<{ hContent: string; sealedBytes: number } | null>(null);

  const canSubmit = useMemo(
    () =>
      indexerUrl.trim() &&
      expectedMeasurement.trim() &&
      title.trim() &&
      dropId.trim() &&
      priceZec.trim() &&
      creatorUfvk.trim() &&
      (file || textContent.trim()),
    [creatorUfvk, dropId, expectedMeasurement, file, indexerUrl, priceZec, textContent, title]
  );

  async function submit() {
    setMessage("");
    setResult(null);
    setSteps({ encrypt: "running", attest: "idle", provision: "idle" });
    try {
      const plaintext = file ? new Uint8Array(await file.arrayBuffer()) : utf8Bytes(textContent);
      const encrypted = await encryptContent(plaintext);
      await uploadContentBlob(indexerUrl, encrypted.hContent, encrypted.blob);
      setSteps({ encrypt: "done", attest: "running", provision: "idle" });

      const attestation = await fetchAttestation(indexerUrl);
      const enclavePubkey = await verifyAttestationOrThrow(attestation, expectedMeasurement);
      setSteps({ encrypt: "done", attest: "done", provision: "running" });

      const priceZat = priceZecToZatNumber(priceZec);
      const parsedDropId = Number(dropId);
      const payload = buildProvisionPayload({
        dropId: parsedDropId,
        priceZat,
        kDrop: encrypted.kDrop,
        creatorUfvk,
        hContent: encrypted.hContent
      });
      const sealed = await sealProvisionPayload(payload, enclavePubkey);
      await postProvision(indexerUrl, title, sealed);

      setSteps({ encrypt: "done", attest: "done", provision: "done" });
      setResult({ hContent: encrypted.hContent, sealedBytes: sealed.length });
      setMessage("Drop provisioned. The catalog can now expose the public entry without secrets.");
    } catch (error) {
      setSteps((prev) => {
        const firstRunning = Object.entries(prev).find(([, state]) => state === "running")?.[0] as keyof Steps | undefined;
        return firstRunning ? { ...prev, [firstRunning]: "error" } : prev;
      });
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <main className="shell">
      <section className="toolbar">
        <div>
          <p className="eyebrow">Lane C</p>
          <h1>Creator Drop Provisioning</h1>
        </div>
        <button className="primary" disabled={!canSubmit} onClick={submit}>
          Encrypt + Provision
        </button>
      </section>

      <section className="grid">
        <div className="panel">
          <h2>Indexer Trust</h2>
          <label>
            Indexer URL
            <input value={indexerUrl} onChange={(e) => setIndexerUrl(e.target.value)} placeholder="http://localhost:8080" />
          </label>
          <label>
            Expected measurement hex
            <input
              value={expectedMeasurement}
              onChange={(e) => setExpectedMeasurement(e.target.value)}
              placeholder="Pinned MrTD or RTMR3 hex"
            />
          </label>
          <p className="note">
            Provisioning is blocked unless the quote verifies, the measurement matches, and report_data binds the
            provisioning public key.
          </p>
        </div>

        <div className="panel">
          <h2>Drop Metadata</h2>
          <div className="pair">
            <label>
              Drop ID
              <input value={dropId} onChange={(e) => setDropId(e.target.value)} inputMode="numeric" />
            </label>
            <label>
              Price ZEC
              <input value={priceZec} onChange={(e) => setPriceZec(e.target.value)} inputMode="decimal" />
            </label>
          </div>
          <label>
            Title
            <input value={title} onChange={(e) => setTitle(e.target.value)} />
          </label>
          <label>
            Creator UFVK
            <textarea value={creatorUfvk} onChange={(e) => setCreatorUfvk(e.target.value)} rows={3} />
          </label>
        </div>

        <div className="panel wide">
          <h2>Content</h2>
          <div className="pair">
            <label>
              File
              <input type="file" onChange={(e) => setFile(e.target.files?.[0] ?? null)} />
            </label>
            <label>
              Text fallback
              <textarea
                value={textContent}
                onChange={(e) => setTextContent(e.target.value)}
                rows={4}
                disabled={Boolean(file)}
              />
            </label>
          </div>
        </div>

        <div className="panel wide">
          <h2>Status</h2>
          <div className="steps">
            <Step label="Encrypt + upload I4 blob" state={steps.encrypt} />
            <Step label="Verify TDX attestation" state={steps.attest} />
            <Step label="Seal I5 payload + provision" state={steps.provision} />
          </div>
          {message ? <p className={result ? "success" : "error"}>{message}</p> : null}
          {result ? (
            <dl className="result">
              <div>
                <dt>h_content</dt>
                <dd>{result.hContent}</dd>
              </div>
              <div>
                <dt>sealed bytes</dt>
                <dd>{result.sealedBytes}</dd>
              </div>
            </dl>
          ) : null}
        </div>
      </section>
    </main>
  );
}

function Step({ label, state }: { label: string; state: StepState }) {
  return (
    <div className={`step ${state}`}>
      <span />
      {label}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
