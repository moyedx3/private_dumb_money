// libsodium sealed-box + base64url helpers.
//
// The dispatch blob (interface I2) is a libsodium `crypto_box_seal(K_drop, e_pub)` (80B). A1
// produces it with Rust `dryoc`; we open it with `libsodium-wrappers` — same Curve25519 sealed
// box, byte-compatible. `crypto_box_seal_open` THROWS on a MAC mismatch (a blob not meant for us)
// — that throw is the "not mine, skip it" signal during trial-open.

import sodium from "libsodium-wrappers";

let readyPromise: Promise<void> | null = null;

/** Resolve once the WASM is initialized. Call before any sodium.* use. */
export async function sodiumReady(): Promise<void> {
  if (!readyPromise) {
    readyPromise = sodium.ready;
  }
  await readyPromise;
}

export type EphemeralKeypair = { ePub: Uint8Array; ePriv: Uint8Array };

/** Fresh X25519 keypair — one per purchase, never reused (reuse links two purchases). */
export async function generateEphemeralKeypair(): Promise<EphemeralKeypair> {
  await sodiumReady();
  const kp = sodium.crypto_box_keypair();
  return { ePub: kp.publicKey, ePriv: kp.privateKey };
}

/**
 * Trial-open a dispatch blob. Returns `K_drop` (32B) if it was sealed to this keypair, else null.
 * Requires `sodiumReady()` to have resolved.
 */
export function trySealOpen(blob: Uint8Array, ePub: Uint8Array, ePriv: Uint8Array): Uint8Array | null {
  try {
    return sodium.crypto_box_seal_open(blob, ePub, ePriv);
  } catch {
    return null; // MAC failure = not our blob
  }
}

/** libsodium sealed box — used by tests/mock to stand in for A1's dryoc seal. */
export function sealTo(message: Uint8Array, recipientPub: Uint8Array): Uint8Array {
  return sodium.crypto_box_seal(message, recipientPub);
}

export function base64urlNoPad(bytes: Uint8Array): string {
  return sodium.to_base64(bytes, sodium.base64_variants.URLSAFE_NO_PADDING);
}

export function fromBase64urlNoPad(text: string): Uint8Array {
  return sodium.from_base64(text, sodium.base64_variants.URLSAFE_NO_PADDING);
}
