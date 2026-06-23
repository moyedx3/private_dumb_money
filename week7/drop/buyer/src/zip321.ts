// ZIP-321 payment URI builder.
//
//   zcash:<deposit_addr>?amount=<ZEC>&memo=<base64url_nopad(on-chain memo bytes)>
//
// The deposit address MUST be shielded (Sapling/Orchard). A transparent address (t1/t3) silently
// drops the memo, so A1 would never receive (drop_id, e_pub) and the buyer could never unlock
// (lane-B §8 trap 1). We hard-reject transparent addresses here.

import { base64urlNoPad } from "./seal";

/** Reject transparent addresses (t1/t3). Shielded UA (`u1...`) / Sapling (`zs...`) pass. */
export function isShieldedAddress(addr: string): boolean {
  return !/^t[13]/i.test(addr.trim());
}

export function buildPaymentUri(args: {
  depositAddr: string;
  priceZec: string;
  onchainMemo: Uint8Array;
}): string {
  const addr = args.depositAddr.trim();
  if (!addr) {
    throw new Error("missing deposit address (catalog entry has no deposit_addr)");
  }
  if (!isShieldedAddress(addr)) {
    throw new Error("deposit address is transparent (t1/t3); memo would be dropped — a shielded address is required");
  }
  const memoParam = base64urlNoPad(args.onchainMemo);
  const amount = encodeURIComponent(args.priceZec.trim());
  return `zcash:${addr}?amount=${amount}&memo=${memoParam}`;
}
