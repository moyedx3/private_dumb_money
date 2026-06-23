// Opt-in local persistence of the active purchase so a tab close / refresh doesn't forfeit it
// (lane-B §8 trap 2). Stores the recovery payload (which includes e_priv — a bearer token for this
// one purchase) in localStorage with a 24h TTL. Guarded so it no-ops outside a browser (tests/node).
//
// Security: this is the same exposure as the downloadable recovery file — e_priv sits on the
// device. It is opt-in and the UI warns about it. Cleared on unlock / cancel.

import type { Purchase } from "./purchase";
import { fromRecoveryFile, toRecoveryFile } from "./purchase";

const KEY = "drop-buyer-active-purchase";
const TTL_MS = 24 * 60 * 60 * 1000;

function storage(): Storage | null {
  try {
    return typeof localStorage !== "undefined" ? localStorage : null;
  } catch {
    return null;
  }
}

export function savePurchase(p: Purchase, now: number = Date.now()): void {
  const s = storage();
  if (!s) return;
  s.setItem(KEY, JSON.stringify({ savedAt: now, recovery: toRecoveryFile(p) }));
}

export function loadPurchase(now: number = Date.now()): Purchase | null {
  const s = storage();
  if (!s) return null;
  const raw = s.getItem(KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as { savedAt?: number; recovery?: unknown };
    if (typeof parsed.savedAt !== "number" || now - parsed.savedAt > TTL_MS) {
      s.removeItem(KEY);
      return null;
    }
    return fromRecoveryFile(JSON.stringify(parsed.recovery));
  } catch {
    s.removeItem(KEY);
    return null;
  }
}

export function clearPurchase(): void {
  storage()?.removeItem(KEY);
}
