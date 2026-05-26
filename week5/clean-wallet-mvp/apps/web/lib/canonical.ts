import canonicalize from "canonicalize";
import { createHash } from "node:crypto";

export function canonicalJson(value: unknown): string {
  const out = canonicalize(value);
  if (out === undefined) throw new Error("canonicalize returned undefined");
  return out;
}

export function sha256Hex(bytes: string | Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}
