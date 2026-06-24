import {
  fromHex,
  hexField,
  isObject,
  normalizeHex,
  sha256Hex,
  SmokeError,
  stringField,
  truthyField
} from "./http-smoke-core.mjs";

export async function verifyAttestation(attestation, expectedMeasurement) {
  const pubkeyHash = sha256Hex(fromHex(attestation.provisioning_pubkey_hex));
  const verifier = await loadVerifier();
  const verification = await verifier(attestation.quote_hex);

  if (!verification.ok) {
    throw new SmokeError("attest", `quote verification failed: ${verification.error ?? "unknown error"}`);
  }
  if (!verification.codeMeasurement) {
    throw new SmokeError("attest", "quote verifier did not return a code measurement");
  }
  if (verification.codeMeasurement !== expectedMeasurement) {
    throw new SmokeError("attest", "quote measurement does not match VITE_DROP_EXPECTED_MEASUREMENT_HEX");
  }
  if (!verification.reportData) {
    throw new SmokeError("attest", "quote verifier did not return report_data");
  }
  if (!reportDataBindsPubkey(verification.reportData, pubkeyHash)) {
    throw new SmokeError("attest", "quote report_data does not bind the provisioning public key");
  }

  return verification;
}

async function loadVerifier() {
  const specifier = process.env.VITE_DROP_QVL_MODULE_URL?.trim();
  if (!specifier) {
    throw new SmokeError(
      "verifier setup",
      "VITE_DROP_QVL_MODULE_URL must be set to a Node-importable verifier module exposing verifyQuote/verify"
    );
  }

  let mod;
  try {
    mod = await import(specifier);
  } catch (error) {
    throw new SmokeError(
      "verifier setup",
      `could not import configured VITE_DROP_QVL_MODULE_URL: ${error.message}. Set it to a Node-importable verifier module exposing verifyQuote/verify.`
    );
  }

  const highLevel = verifierFunction(mod) ?? verifierFunction(mod.default);
  if (highLevel) {
    return async (quoteHex) => normalizeVerifierResult(await highLevel(quoteHex));
  }

  throw new SmokeError("verifier setup", "configured VITE_DROP_QVL_MODULE_URL does not expose verifyQuote/verify.");
}

function verifierFunction(value) {
  if (!isObject(value)) {
    return undefined;
  }
  const verifyQuote = Reflect.get(value, "verifyQuote");
  if (typeof verifyQuote === "function") {
    return verifyQuote;
  }
  const verify = Reflect.get(value, "verify");
  return typeof verify === "function" ? verify : undefined;
}

function normalizeVerifierResult(raw) {
  if (!isObject(raw)) {
    return { ok: false, error: "verifier returned a non-object result" };
  }

  const codeMeasurement =
    hexField(raw, "codeMeasurement") ??
    hexField(raw, "code_measurement") ??
    hexField(raw, "mr_td") ??
    hexField(raw, "mrtd") ??
    hexField(raw, "rtmr3") ??
    recursiveHexField(raw, new Set(["codeMeasurement", "code_measurement", "mr_td", "mrtd", "rtmr3"]));
  const reportData =
    hexField(raw, "reportData") ??
    hexField(raw, "report_data") ??
    hexField(raw, "reportdata") ??
    hexField(raw, "user_data") ??
    recursiveHexField(raw, new Set(["reportData", "report_data", "reportdata", "user_data"]));

  const explicitOk = truthyField(raw, "ok") || truthyField(raw, "is_valid") || truthyField(raw, "valid") || okStatus(raw);
  if (!explicitOk) {
    return {
      ok: false,
      error: stringField(raw, "error") ?? stringField(raw, "message") ?? "quote verification failed"
    };
  }
  return { ok: true, codeMeasurement, reportData };
}

function recursiveHexField(value, keys, depth = 0, seen = new Set()) {
  if (depth > 8 || !isObject(value) || seen.has(value)) {
    return undefined;
  }
  seen.add(value);
  for (const key of Reflect.ownKeys(value)) {
    const child = Reflect.get(value, key);
    if (typeof key === "string" && keys.has(key)) {
      const normalized = normalizeRecursiveHexValue(key, child);
      if (normalized) {
        return normalized;
      }
    }
    const nested = recursiveHexField(child, keys, depth + 1, seen);
    if (nested) {
      return nested;
    }
  }
  return undefined;
}

function normalizeRecursiveHexValue(key, value) {
  if (typeof value === "string") {
    return normalizeHex(value);
  }
  if (Array.isArray(value) && key.toLowerCase().includes("rtmr")) {
    return normalizeHex(String(value[3] ?? ""));
  }
  return undefined;
}

function reportDataBindsPubkey(reportDataHex, pubkeyHashHex) {
  return reportDataHex.length >= 64 && reportDataHex.slice(0, 64).toLowerCase() === pubkeyHashHex.toLowerCase();
}

function okStatus(obj) {
  const status = stringField(obj, "status")?.toLowerCase();
  return status === "ok" || status === "valid" || status === "success" || status === "passed" || status === "uptodate";
}
