import React, { useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { fetchAttestation, postProvision, uploadContentBlob } from "./api";
import { verifyAttestationOrThrow } from "./attestation";
import { utf8Bytes } from "./bytes";
import { encryptContent } from "./content";
import { priceZecToZatNumber } from "./price";
import { buildProvisionPayload, parseDropId, sealProvisionPayload } from "./provision";
import "./styles.css";

type StepState = "idle" | "running" | "done" | "error";

type Steps = {
  encrypt: StepState;
  attest: StepState;
  provision: StepState;
};

type StepKey = keyof Steps;

type ValidatedForm = {
  readonly dropId: number;
  readonly priceZat: number;
  readonly expectedMeasurement: string;
};

const defaultIndexer = import.meta.env.VITE_DROP_INDEXER_URL ?? "http://localhost:8080";
const defaultMeasurement = import.meta.env.VITE_DROP_EXPECTED_MEASUREMENT_HEX ?? "";
const measurementHexPattern = /^(?:0x)?[0-9a-fA-F]{64,}$/;
const stepKeys: readonly StepKey[] = ["encrypt", "attest", "provision"];

function App() {
  const [indexerUrl, setIndexerUrl] = useState(defaultIndexer);
  const [expectedMeasurement, setExpectedMeasurement] = useState(defaultMeasurement);
  const [title, setTitle] = useState("Private demo drop");
  const [dropId, setDropId] = useState("1");
  const [priceZec, setPriceZec] = useState("0.01");
  const [creatorUfvk, setCreatorUfvk] = useState("");
  const [depositAddr, setDepositAddr] = useState("");
  const [textContent, setTextContent] = useState("Hello from a locally encrypted drop.");
  const [file, setFile] = useState<File | null>(null);
  const [steps, setSteps] = useState<Steps>({ encrypt: "idle", attest: "idle", provision: "idle" });
  const [message, setMessage] = useState("");
  const [result, setResult] = useState<{ hContent: string; sealedBytes: number } | null>(null);
  const inFlightRef = useRef(false);
  const isRunning = Object.values(steps).some((state) => state === "running");

  const canSubmit = useMemo(
    () =>
      Boolean(
        indexerUrl.trim() &&
          expectedMeasurement.trim() &&
          title.trim() &&
          dropId.trim() &&
          priceZec.trim() &&
          creatorUfvk.trim() &&
          depositAddr.trim() &&
          (file || textContent.trim())
      ),
    [creatorUfvk, depositAddr, dropId, expectedMeasurement, file, indexerUrl, priceZec, textContent, title]
  );

  async function submit() {
    if (inFlightRef.current) {
      return;
    }
    inFlightRef.current = true;
    setMessage("");
    setResult(null);
    setSteps({ encrypt: "idle", attest: "idle", provision: "idle" });
    try {
      const validated = validateForm({ dropId, priceZec, expectedMeasurement });
      setSteps({ encrypt: "running", attest: "idle", provision: "idle" });
      const plaintext = file ? new Uint8Array(await file.arrayBuffer()) : utf8Bytes(textContent);
      const encrypted = await runStage("encrypt/upload", async () => {
        const encryptedContent = await encryptContent(plaintext);
        await uploadContentBlob(indexerUrl, encryptedContent.hContent, encryptedContent.blob);
        return encryptedContent;
      });
      setSteps({ encrypt: "done", attest: "running", provision: "idle" });

      const enclavePubkey = await runStage("attest", async () => {
        const attestation = await fetchAttestation(indexerUrl);
        return verifyAttestationOrThrow(attestation, validated.expectedMeasurement);
      });
      setSteps({ encrypt: "done", attest: "done", provision: "running" });

      const sealed = await runStage("provision", async () => {
        const payload = buildProvisionPayload({
          dropId: validated.dropId,
          priceZat: validated.priceZat,
          kDrop: encrypted.kDrop,
          creatorUfvk,
          hContent: encrypted.hContent,
          depositAddr
        });
        const sealedPayload = await sealProvisionPayload(payload, enclavePubkey);
        await postProvision(indexerUrl, title, sealedPayload);
        return sealedPayload;
      });

      setSteps({ encrypt: "done", attest: "done", provision: "done" });
      setResult({ hContent: encrypted.hContent, sealedBytes: sealed.length });
      setMessage("Drop provisioned. The catalog can now expose the public entry without secrets.");
    } catch (error) {
      setSteps((prev) => {
        const firstRunning = stepKeys.find((step) => prev[step] === "running");
        return firstRunning ? { ...prev, [firstRunning]: "error" } : prev;
      });
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      inFlightRef.current = false;
    }
  }

  return (
    <main className="shell">
      <section className="toolbar">
        <div className="title-block">
          <p className="eyebrow">Lane C</p>
          <h1>Creator Drop Provisioning</h1>
        </div>
        <button className="primary" disabled={!canSubmit || isRunning} onClick={submit}>
          Encrypt + Provision
        </button>
      </section>

      <section className="grid">
        <div className="panel">
          <h2>Indexer Trust</h2>
          <label className="field">
            Indexer URL
            <input value={indexerUrl} onChange={(e) => setIndexerUrl(e.target.value)} placeholder="http://localhost:8080" />
          </label>
          <label className="field">
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
            <label className="field">
              Drop ID
              <input value={dropId} onChange={(e) => setDropId(e.target.value)} inputMode="numeric" />
            </label>
            <label className="field">
              Price ZEC
              <input value={priceZec} onChange={(e) => setPriceZec(e.target.value)} inputMode="decimal" />
            </label>
          </div>
          <label className="field">
            Title
            <input value={title} onChange={(e) => setTitle(e.target.value)} />
          </label>
          <label className="field">
            Creator UFVK
            <textarea value={creatorUfvk} onChange={(e) => setCreatorUfvk(e.target.value)} rows={3} />
          </label>
          <label className="field">
            Shielded deposit address
            <textarea value={depositAddr} onChange={(e) => setDepositAddr(e.target.value)} rows={3} />
          </label>
        </div>

        <div className="panel wide">
          <h2>Content</h2>
          <div className="pair">
            <label className="field">
              File
              <input type="file" onChange={(e) => setFile(e.target.files?.[0] ?? null)} />
            </label>
            <label className="field">
              Text fallback
              <textarea
                value={textContent}
                onChange={(e) => setTextContent(e.target.value)}
                rows={4}
                disabled={Boolean(file)}
              />
            </label>
          </div>
          <p className="note">
            {file
              ? `Using selected file: ${file.name}. Text fallback is ignored while a file is selected.`
              : "No file selected. The text fallback will be encrypted."}
          </p>
        </div>

        <div className="panel wide">
          <h2>Status</h2>
          <div className="steps" aria-label="Provisioning stages">
            <Step label="Encrypt + upload I4 blob" state={steps.encrypt} />
            <Step label="Verify TDX attestation" state={steps.attest} />
            <Step label="Seal I5 payload + provision" state={steps.provision} />
          </div>
          {message ? (
            <p className={`message ${result ? "success" : "error"}`} role="status" aria-live="polite">
              {message}
            </p>
          ) : null}
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

function validateForm(input: { readonly dropId: string; readonly priceZec: string; readonly expectedMeasurement: string }): ValidatedForm {
  try {
    return {
      dropId: parseDropId(input.dropId),
      priceZat: priceZecToZatNumber(input.priceZec),
      expectedMeasurement: parseMeasurementHex(input.expectedMeasurement)
    };
  } catch (error) {
    throw new Error(`validation: ${errorMessage(error)}`, { cause: error });
  }
}

function parseMeasurementHex(input: string): string {
  const trimmed = input.trim();
  if (!measurementHexPattern.test(trimmed)) {
    throw new Error("expected measurement hex must be at least 64 hex characters");
  }
  return trimmed;
}

async function runStage<T>(stage: string, action: () => Promise<T>): Promise<T> {
  try {
    return await action();
  } catch (error) {
    throw new Error(`${stage}: ${errorMessage(error)}`, { cause: error });
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function Step({ label, state }: { label: string; state: StepState }) {
  return (
    <div className={`step ${state}`} aria-label={`${label}: ${state}`}>
      <span aria-hidden="true" />
      {label}
    </div>
  );
}

const rootElement = document.getElementById("root");
if (!rootElement) {
  throw new Error("root element is missing");
}

createRoot(rootElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
