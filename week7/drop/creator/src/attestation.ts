import { fromHex, sha256Hex } from "./bytes";

export type AttestResponse = {
  quote_hex: string;
  provisioning_pubkey_hex: string;
};

export type QuoteVerification = {
  ok: boolean;
  codeMeasurement?: string;
  reportData?: string;
  error?: string;
};

export type QuoteVerifier = (attestation: AttestResponse) => Promise<QuoteVerification>;
type VerifierFunction = (quoteHex: string) => Promise<unknown> | unknown;
type VerifierLoadResult =
  | { readonly kind: "loaded"; readonly verify: VerifierFunction }
  | { readonly kind: "failed"; readonly error: string };

declare global {
  interface Window {
    dropQuoteVerifier?: {
      verifyQuote?: (quoteHex: string) => Promise<unknown> | unknown;
      verify?: (quoteHex: string) => Promise<unknown> | unknown;
    };
  }
}

export function reportDataBindsPubkey(reportDataHex: string, pubkeyHashHex: string): boolean {
  return reportDataHex.length >= 64 && reportDataHex.slice(0, 64).toLowerCase() === pubkeyHashHex.toLowerCase();
}

export async function verifyAttestationOrThrow(
  attestation: AttestResponse,
  expectedMeasurementHex: string,
  verifier: QuoteVerifier = verifyQuoteWithBrowserQvl
): Promise<Uint8Array> {
  const pubkey = fromHex(attestation.provisioning_pubkey_hex);
  if (pubkey.length !== 32) {
    throw new Error("attestation provisioning_pubkey_hex must be 32 bytes");
  }
  const expectedMeasurement = normalizeExpectedMeasurement(expectedMeasurementHex);
  const verification = await verifier(attestation);
  if (!verification.ok) {
    throw new Error(`quote verification failed: ${verification.error ?? "unknown error"}`);
  }
  if (!verification.codeMeasurement) {
    throw new Error("quote verifier did not return a code measurement");
  }
  if (verification.codeMeasurement !== expectedMeasurement) {
    throw new Error("quote measurement does not match the pinned expected measurement");
  }
  if (!verification.reportData) {
    throw new Error("quote verifier did not return report_data");
  }
  const pubkeyHash = await sha256Hex(pubkey);
  if (!reportDataBindsPubkey(verification.reportData, pubkeyHash)) {
    throw new Error("quote report_data does not bind the provisioning public key");
  }
  return pubkey;
}

export async function verifyQuoteWithBrowserQvl(attestation: AttestResponse): Promise<QuoteVerification> {
  try {
    const verifier = await loadVerifierFunction();
    if (verifier.kind === "failed") {
      return { ok: false, error: verifier.error };
    }
    const raw = await verifier.verify(attestation.quote_hex);
    return normalizeQvlResult(raw);
  } catch (error) {
    return {
      ok: false,
      error:
        error instanceof Error
          ? `${error.message}. Provide VITE_DROP_QVL_MODULE_URL or window.dropQuoteVerifier.`
          : String(error)
    };
  }
}

function normalizeQvlResult(raw: unknown): QuoteVerification {
  if (!isObject(raw)) {
    return { ok: false, error: "verifier returned a non-object result" };
  }

  const ok = truthyField(raw, "ok") || truthyField(raw, "is_valid") || truthyField(raw, "valid") || okStatus(raw);
  const codeMeasurement =
    hexField(raw, "codeMeasurement") ??
    hexField(raw, "code_measurement") ??
    hexField(raw, "mr_td") ??
    hexField(raw, "mrtd") ??
    hexField(raw, "rtmr3");
  const reportData =
    hexField(raw, "reportData") ??
    hexField(raw, "report_data") ??
    hexField(raw, "reportdata") ??
    hexField(raw, "user_data");

  if (!ok) {
    return { ok: false, error: stringField(raw, "error") ?? stringField(raw, "message") ?? "quote verification failed" };
  }
  if (!codeMeasurement) {
    return { ok: false, error: "quote verifier did not return a code measurement" };
  }
  if (!reportData) {
    return { ok: false, error: "quote verifier did not return report_data" };
  }

  return {
    ok: true,
    codeMeasurement,
    reportData
  };
}

async function loadVerifierFunction(): Promise<VerifierLoadResult> {
  const browserVerifier = typeof window !== "undefined" ? window.dropQuoteVerifier : undefined;
  const mod = browserVerifier ?? (await explicitQvlModule());
  const direct = verifierFunction(mod);
  if (direct) {
    return { kind: "loaded", verify: direct };
  }
  const nested = verifierFunction(objectField(mod, "default"));
  if (nested) {
    return { kind: "loaded", verify: nested };
  }
  return { kind: "failed", error: "quote verifier module did not expose verifyQuote/verify" };
}

function importQvlModule(specifier: string): Promise<unknown> {
  return import(/* @vite-ignore */ specifier);
}

async function explicitQvlModule(): Promise<unknown> {
  const specifier = import.meta.env.VITE_DROP_QVL_MODULE_URL?.trim();
  if (!specifier) {
    throw new Error("quote verifier setup requires VITE_DROP_QVL_MODULE_URL or window.dropQuoteVerifier");
  }
  return importQvlModule(specifier);
}

function verifierFunction(value: unknown): VerifierFunction | undefined {
  return functionField(value, "verifyQuote") ?? functionField(value, "verify");
}

function normalizeExpectedMeasurement(input: string): string {
  const normalized = normalizeHex(input);
  if (!normalized || normalized.length < 64) {
    throw new Error("expected measurement hex must be at least 64 hex characters");
  }
  return normalized;
}

function normalizeHex(input: string): string | undefined {
  const normalized = input.trim().replace(/^0x/i, "").toLowerCase();
  return /^[0-9a-f]+$/.test(normalized) ? normalized : undefined;
}

function hexField(obj: object, key: string): string | undefined {
  const value = stringField(obj, key);
  return value ? normalizeHex(value) : undefined;
}

function stringField(obj: object, key: string): string | undefined {
  const value = Reflect.get(obj, key);
  return typeof value === "string" ? value : undefined;
}

function objectField(obj: unknown, key: string): object | undefined {
  if (!isObject(obj)) {
    return undefined;
  }
  const value = Reflect.get(obj, key);
  return isObject(value) ? value : undefined;
}

function functionField(obj: unknown, key: string): VerifierFunction | undefined {
  if (!isObject(obj)) {
    return undefined;
  }
  const value = Reflect.get(obj, key);
  return typeof value === "function" ? value : undefined;
}

function truthyField(obj: object, key: string): boolean {
  return Reflect.get(obj, key) === true;
}

function okStatus(obj: object): boolean {
  const status = stringField(obj, "status")?.toLowerCase();
  return status === "ok" || status === "valid" || status === "success" || status === "passed";
}

function isObject(value: unknown): value is object {
  return typeof value === "object" && value !== null;
}
