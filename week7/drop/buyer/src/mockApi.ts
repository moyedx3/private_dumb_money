// In-process stand-in for the indexer + bucket + A1, so the buyer app runs end-to-end with no
// server, no chain, no other lanes. Used by tests and by the app's "demo (mock)" mode.
//
// It holds each drop's K_drop and content blob (which the real system keeps server-side / in the
// enclave). `simulateDispatch` plays A1: given a buyer's e_pub, it seals that drop's K_drop into
// an 80B dispatch blob and publishes it — exactly what a real on-chain payment would trigger.

import type { CatalogEntry, DropApi } from "./api";
import { toHex, utf8Bytes } from "./bytes";
import { encryptContent } from "./content";
import { generateEphemeralKeypair, sealTo, sodiumReady } from "./seal";

type MockDrop = { entry: CatalogEntry; kDrop: Uint8Array };

const DEMO_DEPOSIT_ADDR =
  "u1demoshieldedaddress00000000000000000000000000000000000000000000000000";

export class MockDropApi implements DropApi {
  private readonly drops: MockDrop[] = [];
  private readonly content = new Map<string, Uint8Array>(); // h_content -> blob
  private readonly dispatch = new Map<string, Uint8Array>(); // key -> 80B blob

  /** Two demo drops, ready to browse. */
  static async demo(): Promise<MockDropApi> {
    const api = new MockDropApi();
    await api.addDrop(1, "Cat photo (demo)", "0.01", utf8Bytes("=^..^=  a (pretend) cat photo, unlocked by your payment."));
    await api.addDrop(2, "Secret recipe (demo)", "0.02", utf8Bytes("Recipe: pay shielded ZEC, unlock locally, enjoy."));
    return api;
  }

  async addDrop(dropId: number, title: string, priceZec: string, plaintext: Uint8Array): Promise<void> {
    const enc = await encryptContent(plaintext);
    const entry: CatalogEntry = {
      drop_id: dropId,
      price_zec: priceZec,
      h_content: enc.hContent,
      title,
      deposit_addr: DEMO_DEPOSIT_ADDR
    };
    this.drops.push({ entry, kDrop: enc.kDrop });
    this.content.set(enc.hContent, enc.blob);
  }

  /** Play A1: a payment for `dropId` carrying `ePub` landed → publish its sealed dispatch blob. */
  async simulateDispatch(dropId: number, ePub: Uint8Array): Promise<string> {
    await sodiumReady();
    const drop = this.drops.find((d) => d.entry.drop_id === dropId);
    if (!drop) throw new Error(`mock: unknown drop ${dropId}`);
    const blob = sealTo(drop.kDrop, ePub); // libsodium sealed box, 80 bytes
    const key = toHex(crypto.getRandomValues(new Uint8Array(32))); // opaque, like blake2b(ek_pub||txid)
    this.dispatch.set(key, blob);
    return key;
  }

  /** Add a dispatch blob sealed to a stranger — polling must trial-open and skip it. */
  async seedForeignDispatch(): Promise<string> {
    await sodiumReady();
    const stranger = await generateEphemeralKeypair();
    const blob = sealTo(crypto.getRandomValues(new Uint8Array(32)), stranger.ePub);
    const key = toHex(crypto.getRandomValues(new Uint8Array(32)));
    this.dispatch.set(key, blob);
    return key;
  }

  async fetchCatalog(): Promise<CatalogEntry[]> {
    return this.drops.map((d) => d.entry);
  }

  async listDispatch(): Promise<string[]> {
    return [...this.dispatch.keys()];
  }

  async getDispatch(key: string): Promise<Uint8Array> {
    const blob = this.dispatch.get(key);
    if (!blob) throw new Error(`mock dispatch ${key} not found`);
    return blob;
  }

  async getContent(hContent: string): Promise<Uint8Array> {
    const blob = this.content.get(hContent);
    if (!blob) throw new Error(`mock content ${hContent} not found`);
    return blob;
  }
}
