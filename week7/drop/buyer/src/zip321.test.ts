import { describe, expect, it } from "vitest";
import { onchainMemoBytes } from "./memo";
import { sodiumReady } from "./seal";
import { buildPaymentUri, isShieldedAddress } from "./zip321";

describe("ZIP-321 URI", () => {
  it("builds zcash: URI with amount and base64url memo", async () => {
    await sodiumReady();
    const ePub = crypto.getRandomValues(new Uint8Array(32));
    const uri = buildPaymentUri({
      depositAddr: "u1shieldeddemo",
      priceZec: "0.01",
      onchainMemo: onchainMemoBytes("raw", 1, ePub)
    });
    expect(uri.startsWith("zcash:u1shieldeddemo?amount=0.01&memo=")).toBe(true);
  });

  it("rejects transparent addresses (t1/t3) — memo would be dropped", async () => {
    await sodiumReady();
    const ePub = crypto.getRandomValues(new Uint8Array(32));
    expect(isShieldedAddress("t1abc")).toBe(false);
    expect(isShieldedAddress("t3abc")).toBe(false);
    expect(isShieldedAddress("u1abc")).toBe(true);
    expect(isShieldedAddress("zs1abc")).toBe(true);
    expect(() =>
      buildPaymentUri({
        depositAddr: "t1transparent",
        priceZec: "0.01",
        onchainMemo: onchainMemoBytes("raw", 1, ePub)
      })
    ).toThrow(/transparent/);
  });
});
