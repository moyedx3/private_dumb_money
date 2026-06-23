import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { clearPurchase, loadPurchase, savePurchase } from "./persist";
import { createPurchase } from "./purchase";
import { sodiumReady } from "./seal";
import type { CatalogEntry } from "./api";

// Minimal in-memory localStorage so the browser-only path is testable under the node env.
function installMemoryStorage() {
  const map = new Map<string, string>();
  (globalThis as { localStorage?: Storage }).localStorage = {
    getItem: (k) => map.get(k) ?? null,
    setItem: (k, v) => void map.set(k, String(v)),
    removeItem: (k) => void map.delete(k),
    clear: () => map.clear(),
    key: (i) => [...map.keys()][i] ?? null,
    get length() {
      return map.size;
    }
  } as Storage;
}

const entry: CatalogEntry = {
  drop_id: 1,
  price_zec: "0.01",
  h_content: "abc",
  title: "Cat",
  deposit_addr: "u1shieldeddemo"
};

describe("purchase persistence", () => {
  beforeEach(() => installMemoryStorage());
  afterEach(() => delete (globalThis as { localStorage?: Storage }).localStorage);

  it("saves and reloads a purchase (e_priv survives a reload)", async () => {
    await sodiumReady();
    const p = await createPurchase(entry);
    savePurchase(p);

    const loaded = loadPurchase();
    expect(loaded).not.toBeNull();
    expect(loaded?.dropId).toBe(1);
    expect(loaded?.ePriv).toEqual(p.ePriv);
    expect(loaded?.ePub).toEqual(p.ePub);
  });

  it("drops a purchase past the 24h TTL", async () => {
    await sodiumReady();
    const p = await createPurchase(entry);
    const t0 = 1_000_000;
    savePurchase(p, t0);
    expect(loadPurchase(t0 + 1000)).not.toBeNull();
    expect(loadPurchase(t0 + 25 * 60 * 60 * 1000)).toBeNull(); // expired
  });

  it("clear removes it", async () => {
    await sodiumReady();
    savePurchase(await createPurchase(entry));
    clearPurchase();
    expect(loadPurchase()).toBeNull();
  });

  it("no-ops without localStorage (node default)", () => {
    delete (globalThis as { localStorage?: Storage }).localStorage;
    expect(loadPurchase()).toBeNull();
    expect(() => clearPurchase()).not.toThrow();
  });
});
