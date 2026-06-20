import { describe, expect, it } from "vitest";
import { sha256Hex } from "./bytes";
import { reportDataBindsPubkey, verifyAttestationOrThrow } from "./attestation";
import type { AttestResponse, QuoteVerification } from "./attestation";

const pubkey = new Uint8Array(32).fill(7);
const pubkeyHex = Array.from(pubkey, (b) => b.toString(16).padStart(2, "0")).join("");
const attestation: AttestResponse = {
  quote_hex: "abcd",
  provisioning_pubkey_hex: pubkeyHex
};
const measurement = "f".repeat(64);

describe("attestation checks", () => {
  it("checks report_data prefix against sha256(pubkey)", async () => {
    const hash = await sha256Hex(pubkey);
    expect(reportDataBindsPubkey(hash + "0".repeat(64), hash)).toBe(true);
    expect(reportDataBindsPubkey("a".repeat(64) + "0".repeat(64), hash)).toBe(false);
  });

  it("returns the enclave public key only when all checks pass", async () => {
    const hash = await sha256Hex(pubkey);
    const verifier = async (): Promise<QuoteVerification> => ({
      ok: true,
      codeMeasurement: measurement,
      reportData: hash + "0".repeat(64)
    });
    await expect(verifyAttestationOrThrow(attestation, measurement, verifier)).resolves.toEqual(pubkey);
  });

  it("fails closed on quote, measurement, and pubkey binding failures", async () => {
    await expect(
      verifyAttestationOrThrow(attestation, measurement, async () => ({ ok: false, error: "bad quote" }))
    ).rejects.toThrow(/quote verification failed/);

    await expect(
      verifyAttestationOrThrow(attestation, measurement, async () => ({
        ok: true,
        codeMeasurement: "0".repeat(64),
        reportData: "0".repeat(128)
      }))
    ).rejects.toThrow(/measurement/);

    await expect(
      verifyAttestationOrThrow(attestation, measurement, async () => ({
        ok: true,
        codeMeasurement: measurement,
        reportData: "0".repeat(128)
      }))
    ).rejects.toThrow(/report_data/);
  });
});
