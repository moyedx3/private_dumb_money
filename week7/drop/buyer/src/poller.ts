// Dispatch polling + unlock. The bucket key tells us nothing (it is blake2b(ek_pub||txid)), so we
// pull every new dispatch blob and trial-open it against each pending purchase's keypair. The one
// that opens yields K_drop; the purchase it opened for tells us which content to fetch and decrypt.

import type { DropApi } from "./api";
import { sha256Hex } from "./bytes";
import { decryptContent } from "./content";
import type { Purchase } from "./purchase";
import { trySealOpen } from "./seal";

export type UnlockResult = {
  purchase: Purchase;
  kDrop: Uint8Array;
  content: Uint8Array;
};

export class DispatchPoller {
  private readonly tried = new Set<string>();

  constructor(private readonly api: DropApi) {}

  /**
   * One poll pass. Fetches new dispatch blobs and trial-opens them against `pending`.
   * Returns the unlocks discovered this pass (usually 0 or 1).
   *
   * Note: a key is marked "tried" after being attempted against all currently-pending purchases.
   * A purchase added *after* a key was seen won't re-match that key — fine for the demo (a payment
   * always precedes its dispatch), revisit if concurrent late-joining purchases are needed.
   */
  async poll(pending: Purchase[]): Promise<UnlockResult[]> {
    if (pending.length === 0) return [];

    const keys = await this.api.listDispatch();
    const unlocked: UnlockResult[] = [];

    for (const key of keys) {
      if (this.tried.has(key)) continue;

      let blob: Uint8Array;
      try {
        blob = await this.api.getDispatch(key);
      } catch {
        continue; // transient fetch error — retry next pass (don't mark tried)
      }

      for (const purchase of pending) {
        const kDrop = trySealOpen(blob, purchase.ePub, purchase.ePriv);
        if (!kDrop) continue; // not ours — skip

        const content = await this.api.getContent(purchase.hContent);
        const got = await sha256Hex(content);
        if (got !== purchase.hContent) {
          throw new Error(`content hash mismatch for drop ${purchase.dropId} — expected ${purchase.hContent}, got ${got}`);
        }
        const plaintext = await decryptContent(content, kDrop);
        unlocked.push({ purchase, kDrop, content: plaintext });
        break; // this blob is consumed by the matching purchase
      }

      this.tried.add(key);
    }

    return unlocked;
  }
}
