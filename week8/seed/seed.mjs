// Pre-seed the demo indexer with drops: encrypt each image with a fresh K_drop, upload the
// content blob, then provision the drop (seal K_drop + creator UFVK to the indexer's dev-seed
// pubkey — same crypto path the creator app uses, minus the TEE attestation).
//
// No TEE: the indexer runs headless with A2_DEV_PROVISIONING_SEED_HEX, so we derive its
// provisioning pubkey here (X25519 base-point mult of the seed — matches Rust dryoc) and
// crypto_box_seal to it. A1's real mainnet scanner then watches each drop's deposit_addr.
//
// Run:  SEED_HEX=<32B hex> INDEXER_URL=http://localhost:8080 node seed.mjs [drops.json]

import { readFileSync } from "node:fs";
import { createRequire } from "node:module";

// libsodium-wrappers ships a broken ESM entry for Node; load its CommonJS build instead.
const sodium = createRequire(import.meta.url)("libsodium-wrappers");

const INDEXER = process.env.INDEXER_URL || "http://localhost:8080";
const SEED_HEX = process.env.SEED_HEX;
const DROPS_FILE = process.argv[2] || process.env.DROPS_FILE || "./drops.json";

if (!SEED_HEX || SEED_HEX.length !== 64) {
  console.error("set SEED_HEX to the indexer's A2_DEV_PROVISIONING_SEED_HEX (64 hex chars)");
  process.exit(1);
}

const hex = (b) => Buffer.from(b).toString("hex");
const fromHex = (h) => new Uint8Array(Buffer.from(h, "hex"));

async function sha256Hex(bytes) {
  return hex(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)));
}

// Content blob (interface I4): nonce(12) || AES-256-GCM ciphertext||tag(16). Byte-identical to
// what the buyer's content.ts decrypts.
async function aesGcmEncrypt(plaintext, kDrop) {
  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const key = await crypto.subtle.importKey("raw", kDrop, "AES-GCM", false, ["encrypt"]);
  const ct = new Uint8Array(
    await crypto.subtle.encrypt({ name: "AES-GCM", iv: nonce, tagLength: 128 }, key, plaintext)
  );
  const blob = new Uint8Array(nonce.length + ct.length);
  blob.set(nonce, 0);
  blob.set(ct, nonce.length);
  return blob;
}

await sodium.ready;
const enclavePub = sodium.crypto_scalarmult_base(fromHex(SEED_HEX));
console.log(`indexer   : ${INDEXER}`);
console.log(`enclavePub: ${hex(enclavePub)}\n`);

const drops = JSON.parse(readFileSync(DROPS_FILE, "utf8"));
let ok = 0;
for (const d of drops) {
  try {
    const img = new Uint8Array(readFileSync(d.image)); // relative to cwd (run from week8/seed)
    const kDrop = crypto.getRandomValues(new Uint8Array(32));
    const content = await aesGcmEncrypt(img, kDrop);
    const hContent = await sha256Hex(content);

    let r = await fetch(`${INDEXER}/bucket/${hContent}`, { method: "PUT", body: content });
    if (!r.ok) throw new Error(`content PUT ${r.status}`);

    const price_zat = Math.round(parseFloat(d.price_zec) * 1e8);
    const payload = new TextEncoder().encode(
      JSON.stringify({
        drop_id: d.drop_id,
        price_zat,
        k_drop: hex(kDrop),
        creator_ufvk: d.creator_ufvk,
        h_content: hContent,
        deposit_addr: d.deposit_addr
      })
    );
    const sealed = sodium.crypto_box_seal(payload, enclavePub);
    r = await fetch(`${INDEXER}/provision?title=${encodeURIComponent(d.title)}`, {
      method: "POST",
      body: sealed
    });
    if (!r.ok) throw new Error(`provision ${r.status}`);
    console.log(`✅ drop ${d.drop_id} "${d.title}" — ${d.price_zec} ZEC → ${d.deposit_addr.slice(0, 16)}…  (h_content ${hContent.slice(0, 12)}…)`);
    ok++;
  } catch (e) {
    console.error(`❌ drop ${d.drop_id} "${d.title}": ${e.message}`);
  }
}
console.log(`\n${ok}/${drops.length} provisioned. Check: ${INDEXER}/catalog`);
