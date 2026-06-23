// A purchase = one drop + one fresh ephemeral keypair. The keypair binds this purchase to the
// dispatch blob that will answer it, and to the content it unlocks (this is the e_priv ↔ drop_id ↔
// h_content mapping the design calls out: the dispatch blob carries only K_drop, not the drop_id).

import type { CatalogEntry } from "./api";
import { fromHex, toHex } from "./bytes";
import { generateEphemeralKeypair } from "./seal";

export type Purchase = {
  id: string;
  dropId: number;
  title: string;
  priceZec: string;
  depositAddr: string;
  hContent: string;
  ePub: Uint8Array;
  ePriv: Uint8Array;
  createdAt: number;
};

export async function createPurchase(entry: CatalogEntry, now: number = Date.now()): Promise<Purchase> {
  if (!entry.deposit_addr) {
    throw new Error(`catalog entry for drop ${entry.drop_id} has no deposit_addr`);
  }
  const { ePub, ePriv } = await generateEphemeralKeypair();
  return {
    id: toHex(crypto.getRandomValues(new Uint8Array(8))),
    dropId: entry.drop_id,
    title: entry.title,
    priceZec: entry.price_zec,
    depositAddr: entry.deposit_addr,
    hContent: entry.h_content,
    ePub,
    ePriv,
    createdAt: now
  };
}

// --- Recovery file (lane-B §8.2d) ---
// Holds e_priv → it is a bearer token for this one purchase. The UI must warn about this.

export type RecoveryFile = {
  v: "drop-recovery-1";
  drop_id: number;
  title: string;
  price_zec: string;
  deposit_addr: string;
  h_content: string;
  e_pub: string;
  e_priv: string;
  created_at: number;
};

export function toRecoveryFile(p: Purchase): RecoveryFile {
  return {
    v: "drop-recovery-1",
    drop_id: p.dropId,
    title: p.title,
    price_zec: p.priceZec,
    deposit_addr: p.depositAddr,
    h_content: p.hContent,
    e_pub: toHex(p.ePub),
    e_priv: toHex(p.ePriv),
    created_at: p.createdAt
  };
}

export function fromRecoveryFile(json: string): Purchase {
  const o = JSON.parse(json) as Partial<RecoveryFile>;
  if (o.v !== "drop-recovery-1" || !o.e_priv || !o.e_pub || !o.h_content || o.drop_id === undefined) {
    throw new Error("not a valid drop recovery file");
  }
  return {
    id: toHex(crypto.getRandomValues(new Uint8Array(8))),
    dropId: o.drop_id,
    title: o.title ?? `Drop ${o.drop_id}`,
    priceZec: o.price_zec ?? "",
    depositAddr: o.deposit_addr ?? "",
    hContent: o.h_content,
    ePub: fromHex(o.e_pub),
    ePriv: fromHex(o.e_priv),
    createdAt: o.created_at ?? Date.now()
  };
}
