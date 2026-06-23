import { describe, expect, it } from "vitest";
import { generateEphemeralKeypair, sealTo, sodiumReady, trySealOpen } from "./seal";

describe("dispatch blob (I2) sealed box", () => {
  it("opens a blob sealed to our key; 80 bytes for a 32-byte K_drop", async () => {
    await sodiumReady();
    const { ePub, ePriv } = await generateEphemeralKeypair();
    const kDrop = crypto.getRandomValues(new Uint8Array(32));
    const blob = sealTo(kDrop, ePub);
    expect(blob.length).toBe(80); // ek_pub(32) || ct+MAC(48)
    expect(trySealOpen(blob, ePub, ePriv)).toEqual(kDrop);
  });

  it("returns null for a blob sealed to someone else (trial-open skips it)", async () => {
    await sodiumReady();
    const me = await generateEphemeralKeypair();
    const other = await generateEphemeralKeypair();
    const blob = sealTo(crypto.getRandomValues(new Uint8Array(32)), other.ePub);
    expect(trySealOpen(blob, me.ePub, me.ePriv)).toBeNull();
  });

  it("fresh keypairs each call (no reuse)", async () => {
    await sodiumReady();
    const a = await generateEphemeralKeypair();
    const b = await generateEphemeralKeypair();
    expect(a.ePub).not.toEqual(b.ePub);
  });
});
