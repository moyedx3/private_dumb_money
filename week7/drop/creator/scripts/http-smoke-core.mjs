import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";

export const EVIDENCE_DIR = ".omo/evidence";
export const HEX_PATTERN = /^[0-9a-fA-F]+$/;

export class SmokeError extends Error {
  constructor(stage, message, options) {
    super(`${stage}: ${message}`, options);
    this.name = "SmokeError";
    this.stage = stage;
  }
}

export async function ensureEvidenceDir() {
  await mkdir(EVIDENCE_DIR, { recursive: true });
}

export async function writeJsonEvidence(name, data) {
  await writeFile(`${EVIDENCE_DIR}/${name}`, `${JSON.stringify(data, null, 2)}\n`);
}

export function normalizeBaseUrl(input) {
  try {
    const url = new URL(input);
    if (url.protocol !== "http:" && url.protocol !== "https:") {
      throw new Error("must use http or https");
    }
    if (url.username || url.password) {
      throw new Error(`credential-bearing URLs are not accepted (${safeUrlLabel(input)})`);
    }
    if (url.search || url.hash) {
      throw new Error(`query and fragment are not accepted in VITE_DROP_INDEXER_URL (${safeUrlLabel(input)})`);
    }
    return url.toString().replace(/\/+$/, "");
  } catch (error) {
    throw new SmokeError("setup", `invalid VITE_DROP_INDEXER_URL: ${redactConfiguredSecrets(error.message)}`);
  }
}

export function safeUrlLabel(input) {
  try {
    const url = new URL(input);
    return `${url.protocol}//${url.host}${url.pathname.replace(/\/+$/, "")}`;
  } catch {
    return "<invalid-url>";
  }
}

export function redactConfiguredSecrets(text) {
  const secrets = [
    process.env.VITE_DROP_INDEXER_URL,
    process.env.VITE_DROP_QVL_MODULE_URL,
    process.env.VITE_DROP_PCCS_URL,
    process.env.PCCS_URL
  ].filter((value) => typeof value === "string" && value.length > 0);
  return secrets.reduce((redacted, secret) => redacted.replaceAll(secret, "[redacted-env]"), text);
}

export function normalizeExpectedMeasurement(input) {
  const normalized = normalizeHex(input);
  if (!normalized || normalized.length < 64) {
    throw new SmokeError("setup", "VITE_DROP_EXPECTED_MEASUREMENT_HEX must be at least 64 hex characters");
  }
  return normalized;
}

export function joinUrl(base, path) {
  return `${base.replace(/\/+$/, "")}/${path.replace(/^\/+/, "")}`;
}

export function fromHex(hex) {
  if (hex.length % 2 !== 0 || !HEX_PATTERN.test(hex)) {
    throw new SmokeError("hex", "invalid hex input");
  }
  return Uint8Array.from(hex.match(/.{2}/g) ?? [], (byte) => Number.parseInt(byte, 16));
}

export function sha256Hex(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

export function normalizeHex(input) {
  const normalized = input.trim().replace(/^0x/i, "").toLowerCase();
  return /^[0-9a-f]+$/.test(normalized) ? normalized : undefined;
}

export function hexField(obj, key) {
  const value = stringField(obj, key);
  return value ? normalizeHex(value) : undefined;
}

export function stringField(obj, key) {
  const value = Reflect.get(obj, key);
  return typeof value === "string" ? value : undefined;
}

export function truthyField(obj, key) {
  return Reflect.get(obj, key) === true;
}

export function isObject(value) {
  return typeof value === "object" && value !== null;
}

export function summarizeText(text) {
  const normalized = redactConfiguredSecrets(text).replace(/\s+/g, " ").trim();
  if (!normalized) {
    return "<empty body>";
  }
  return normalized.length > 120 ? `${normalized.slice(0, 117)}...` : normalized;
}
