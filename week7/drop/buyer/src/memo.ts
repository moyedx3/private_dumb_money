// Memo payload (interface I1). The buyer encodes it; A1 decodes it. Must match A1's
// `indexer/src/memo.rs` byte-for-byte.
//
// Two forms (A1 accepts both):
//   raw   : drop_id(8, u64 big-endian) || e_pub(32)            = 40 bytes
//   text  : "A1B64:" || base64url_nopad(raw 40 bytes)          (for wallets that mangle binary)
//
// The ZIP-321 `memo=` parameter is always base64url(no-pad) of the ON-CHAIN memo bytes:
//   raw  → on-chain memo = the 40 raw bytes
//   text → on-chain memo = the ASCII "A1B64:..." string (UTF-8)
// `zip321.ts` does that outer base64url; here we only produce the on-chain memo bytes.

import { utf8Bytes } from "./bytes";
import { base64urlNoPad, fromBase64urlNoPad } from "./seal";

export const DROP_MEMO_LEN = 40;
/** Frozen text-fallback prefix — must equal A1's `TEXT_MEMO_PREFIX`. */
export const TEXT_MEMO_PREFIX = "A1B64:";

export type MemoForm = "raw" | "text";

export function encodeMemoRaw(dropId: number | bigint, ePub: Uint8Array): Uint8Array {
  if (ePub.length !== 32) {
    throw new Error("e_pub must be 32 bytes");
  }
  const memo = new Uint8Array(DROP_MEMO_LEN);
  new DataView(memo.buffer).setBigUint64(0, BigInt(dropId), false); // false = big-endian
  memo.set(ePub, 8);
  return memo;
}

export function encodeMemoText(dropId: number | bigint, ePub: Uint8Array): string {
  return TEXT_MEMO_PREFIX + base64urlNoPad(encodeMemoRaw(dropId, ePub));
}

/** The bytes that land in the on-chain Zcash memo field for the chosen form. */
export function onchainMemoBytes(form: MemoForm, dropId: number | bigint, ePub: Uint8Array): Uint8Array {
  return form === "raw" ? encodeMemoRaw(dropId, ePub) : utf8Bytes(encodeMemoText(dropId, ePub));
}

/** Decode either form back to (dropId, ePub) — mirrors A1's `decode_memo`, used in round-trip tests. */
export function decodeMemo(memo: Uint8Array): { dropId: bigint; ePub: Uint8Array } | null {
  const text = tryUtf8(memo);
  if (text && text.startsWith(TEXT_MEMO_PREFIX)) {
    const encoded = text.slice(TEXT_MEMO_PREFIX.length).trim();
    try {
      return decodeRaw(fromBase64urlNoPad(encoded));
    } catch {
      return null;
    }
  }
  return decodeRaw(memo);
}

function decodeRaw(memo: Uint8Array): { dropId: bigint; ePub: Uint8Array } | null {
  if (memo.length < DROP_MEMO_LEN) {
    return null;
  }
  const view = new DataView(memo.buffer, memo.byteOffset, memo.byteLength);
  const dropId = view.getBigUint64(0, false);
  const ePub = memo.slice(8, DROP_MEMO_LEN);
  return { dropId, ePub };
}

function tryUtf8(bytes: Uint8Array): string | null {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes).replace(/\0+$/, "");
  } catch {
    return null;
  }
}
