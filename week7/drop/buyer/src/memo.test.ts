import { describe, expect, it } from "vitest";
import { decodeMemo, encodeMemoRaw, encodeMemoText, onchainMemoBytes } from "./memo";
import { sodiumReady } from "./seal";

describe("memo (I1)", () => {
  it("matches A1's frozen text-memo test vector (cross-impl base64url check)", async () => {
    await sodiumReady();
    const ePub = new Uint8Array(Array.from({ length: 32 }, (_, i) => i)); // [0,1,...,31]
    // == indexer/src/memo.rs `text_memo_roundtrips_for_wallet_memo_fields`
    expect(encodeMemoText(1, ePub)).toBe(
      "A1B64:AAAAAAAAAAEAAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHw"
    );
  });

  it("raw memo is 40B drop_id(8 BE) || e_pub(32) and round-trips", async () => {
    await sodiumReady();
    const ePub = crypto.getRandomValues(new Uint8Array(32));
    const raw = encodeMemoRaw(0xdeadbeefn, ePub);
    expect(raw.length).toBe(40);
    const decoded = decodeMemo(raw);
    expect(decoded?.dropId).toBe(0xdeadbeefn);
    expect(decoded?.ePub).toEqual(ePub);
  });

  it("both on-chain forms decode back to the same (drop_id, e_pub)", async () => {
    await sodiumReady();
    const ePub = crypto.getRandomValues(new Uint8Array(32));
    const raw = onchainMemoBytes("raw", 7, ePub);
    const text = onchainMemoBytes("text", 7, ePub);
    expect(raw.length).toBe(40);
    expect(decodeMemo(raw)?.dropId).toBe(7n);
    expect(decodeMemo(text)?.dropId).toBe(7n);
    expect(decodeMemo(text)?.ePub).toEqual(ePub);
  });
});
