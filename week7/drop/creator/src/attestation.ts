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
  const verification = await verifier(attestation);
  if (!verification.ok) {
    throw new Error(`quote verification failed: ${verification.error ?? "unknown error"}`);
  }
  if (!verification.codeMeasurement) {
    throw new Error("quote verifier did not return a code measurement");
  }
  if (verification.codeMeasurement.toLowerCase() !== expectedMeasurementHex.trim().toLowerCase()) {
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
    const globalVerifier = typeof window !== "undefined" ? window.dropQuoteVerifier : undefined;
    const mod =
      globalVerifier ??
      (await (new Function("specifier", "return import(specifier)") as (
        specifier: string
      ) => Promise<Record<string, unknown>>)(
        import.meta.env.VITE_DROP_QVL_MODULE_URL ?? "@phala/dcap-qvl-web"
      ));
    const verifierModule = mod as Record<string, unknown>;
    const candidates = [
      verifierModule.verifyQuote,
      verifierModule.verify,
      (verifierModule.default as Record<string, unknown> | undefined)?.verifyQuote,
      (verifierModule.default as Record<string, unknown> | undefined)?.verify
    ].filter((fn): fn is (...args: unknown[]) => Promise<unknown> | unknown => typeof fn === "function");
    if (candidates.length === 0) {
      return { ok: false, error: "@phala/dcap-qvl-web did not expose verifyQuote/verify" };
    }
    const raw = await candidates[0](attestation.quote_hex);
    return normalizeQvlResult(raw);
  } catch (error) {
    return {
      ok: false,
      error:
        error instanceof Error
          ? `${error.message}. Provide @phala/dcap-qvl-web through VITE_DROP_QVL_MODULE_URL or window.dropQuoteVerifier.`
          : String(error)
    };
  }
}

function normalizeQvlResult(raw: unknown): QuoteVerification {
  if (!raw || typeof raw !== "object") {
    return { ok: false, error: "verifier returned a non-object result" };
  }
  const obj = raw as Record<string, unknown>;
  const ok = obj.ok === true || obj.is_valid === true || obj.valid === true || obj.status === "ok";
  const codeMeasurement =
    stringField(obj, "codeMeasurement") ??
    stringField(obj, "code_measurement") ??
    stringField(obj, "mr_td") ??
    stringField(obj, "mrtd") ??
    stringField(obj, "rtmr3");
  const reportData =
    stringField(obj, "reportData") ??
    stringField(obj, "report_data") ??
    stringField(obj, "reportdata") ??
    stringField(obj, "user_data");
  return {
    ok,
    codeMeasurement: codeMeasurement?.replace(/^0x/, ""),
    reportData: reportData?.replace(/^0x/, ""),
    error: ok ? undefined : stringField(obj, "error") ?? stringField(obj, "message") ?? "quote verification failed"
  };
}

function stringField(obj: Record<string, unknown>, key: string): string | undefined {
  const value = obj[key];
  return typeof value === "string" ? value : undefined;
}
