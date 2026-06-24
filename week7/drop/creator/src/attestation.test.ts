import { afterEach, describe, expect, it, vi } from "vitest";
import { sha256Hex } from "./bytes";
import { reportDataBindsPubkey, verifyAttestationOrThrow, verifyQuoteWithBrowserQvl } from "./attestation";
import type { AttestResponse, QuoteVerification } from "./attestation";

const pubkey = new Uint8Array(32).fill(7);
const pubkeyHex = Array.from(pubkey, (b) => b.toString(16).padStart(2, "0")).join("");
const attestation: AttestResponse = {
  quote_hex: "abcd",
  provisioning_pubkey_hex: pubkeyHex
};
const measurement = "f".repeat(64);

describe("attestation checks", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

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

  it("accepts verifier result aliases", async () => {
    const hash = await sha256Hex(pubkey);
    const reportData = hash + "0".repeat(64);
    const cases: readonly Readonly<Record<string, unknown>>[] = [
      { ok: true, codeMeasurement: measurement, reportData },
      { is_valid: true, rtmr3: `0x${measurement}`, report_data: `0x${reportData}` },
      { valid: true, mrtd: measurement, user_data: reportData },
      { status: "ok", codeMeasurement: measurement, report_data: reportData }
    ];

    for (const raw of cases) {
      vi.stubGlobal("window", {
        dropQuoteVerifier: {
          verifyQuote: () => raw
        }
      });

      await expect(
        verifyAttestationOrThrow(attestation, `0x${measurement}`, () => verifyQuoteWithBrowserQvl(attestation))
      ).resolves.toEqual(pubkey);
    }
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

  it("rejects missing report_data", async () => {
    const verifier = async (): Promise<QuoteVerification> => ({
      ok: true,
      codeMeasurement: measurement
    });

    await expect(verifyAttestationOrThrow(attestation, measurement, verifier)).rejects.toThrow(/report_data/);
  });

  it("rejects missing normalized report_data", async () => {
    vi.stubGlobal("window", {
      dropQuoteVerifier: {
        verifyQuote: () => ({ ok: true, rtmr3: measurement })
      }
    });

    await expect(
      verifyAttestationOrThrow(attestation, measurement, () => verifyQuoteWithBrowserQvl(attestation))
    ).rejects.toThrow(/report_data/);
  });

  it("rejects missing normalized measurement", async () => {
    const hash = await sha256Hex(pubkey);
    vi.stubGlobal("window", {
      dropQuoteVerifier: {
        verifyQuote: () => ({ ok: true, report_data: hash + "0".repeat(64) })
      }
    });

    await expect(
      verifyAttestationOrThrow(attestation, measurement, () => verifyQuoteWithBrowserQvl(attestation))
    ).rejects.toThrow(/code measurement/);
  });

  it("rejects short expected measurement hex", async () => {
    const hash = await sha256Hex(pubkey);
    const verifier = async (): Promise<QuoteVerification> => ({
      ok: true,
      codeMeasurement: measurement,
      reportData: hash + "0".repeat(64)
    });

    await expect(verifyAttestationOrThrow(attestation, "f".repeat(63), verifier)).rejects.toThrow(/at least 64 hex/);
  });

  it("rejects verifier modules without a callable verifier", async () => {
    vi.stubGlobal("window", {
      dropQuoteVerifier: {}
    });

    await expect(
      verifyAttestationOrThrow(attestation, measurement, () => verifyQuoteWithBrowserQvl(attestation))
    ).rejects.toThrow(/verifyQuote\/verify/);
  });

  it("fails closed with setup guidance when no explicit verifier is configured", async () => {
    vi.stubGlobal("window", {});
    vi.stubEnv("VITE_DROP_QVL_MODULE_URL", "");

    await expect(
      verifyAttestationOrThrow(attestation, measurement, () => verifyQuoteWithBrowserQvl(attestation))
    ).rejects.toThrow(/VITE_DROP_QVL_MODULE_URL or window\.dropQuoteVerifier/);
    await expect(
      verifyAttestationOrThrow(attestation, measurement, () => verifyQuoteWithBrowserQvl(attestation))
    ).rejects.not.toThrow(/@phala\/dcap-qvl-web/);
  });
});
