# Lane A1 — Payment-Flow Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the drop indexer's payment-flow core — watch a creator's shielded address via IVK, detect a buyer's payment, recover `(drop_id, e_pub)` from the memo, and publish an ECIES-wrapped `K_drop` dispatch blob to the bucket.

**Architecture:** A new `drop-indexer` Rust crate that reuses clean-wallet's proven lightwalletd gRPC client and the branch-tolerant IVK detector from `ivk-incoming-probe`. The engine consumes two **mockable trait boundaries it does not own** — `Catalog` (drop config: price, `K_drop`, creator UFVK — owned by Lane A2) and `Bucket` (blob storage — owned by Lane D) — so A1 builds and tests fully in isolation. The two formats A1 **does** own (memo layout, dispatch-blob layout) are frozen here and must match the team's `interfaces.md`.

**Tech Stack:** Rust, `tokio`, `tonic` (gRPC), `zcash_primitives`/`orchard`/`sapling-crypto`/`zcash_keys` (note decryption), `dryoc` 0.7.x (libsodium-compatible sealed box, interops with Lane B's `libsodium.js`), `anyhow`, `thiserror`.

---

## Current implementation status (2026-06-21)

Overall plan completion: **about 55–60%**. The chain-query, UFVK detection, memo codec, dispatch wrapping, replay guard, and payment engine foundations are working; scan-loop and live smoke remain pending.

| Plan task | Current state | Evidence / files |
| --- | --- | --- |
| Task 0 — scaffold + lightwalletd | **Implemented** | `indexer/Cargo.toml`, `indexer/build.rs`, `indexer/proto/*`, `indexer/src/lightwalletd.rs`, `indexer/src/bin/check-lightwalletd.rs`, `indexer/src/lib.rs`; live `check-lightwalletd` can fetch tip/ranges/raw tx bytes. `DropConfig`, `Catalog`, `Bucket` boundaries are now in `lib.rs`. |
| Task 1 — memo codec | **Implemented and unit-tested** | `indexer/src/memo.rs`; `encode_memo` / `decode_memo` cover raw 40-byte `drop_id || e_pub`, `A1B64:<base64url(raw40)>` wallet-text fallback, wrong-length reject, and trailing ZIP-302 zero padding. Added ignored live integration test `indexer/tests/live_chain_memo.rs` for an already-mined memo tx. |
| Task 2 — branch-tolerant tx reader | **Implemented and unit-tested** | `indexer/src/detect.rs`; tests cover NU5 branch-id patch and txid byte-order helpers. |
| Task 3 — IVK incoming detector | **Implemented enough for live UFVK detection; golden fixture still missing** | `detect_incoming` handles Sapling/Orchard and now scans both external + internal scopes. `probe-ufvk` live run found Orchard note at height `3363067`, value `74999`, memo `<none>`. Pending: hermetic spike fixture test. |
| Extra — zecscope compact scan smoke | **Implemented** | `indexer/src/zecscope_adapter.rs`, `indexer/src/bin/zecscope-scan.rs`; live run found the same Orchard candidate from compact blocks. This is a fast candidate-detection helper, not a memo path. |
| Task 4 — sealed-box dispatch | **Implemented and unit-tested** | `indexer/src/dispatch.rs`; `wrap_k_drop` returns 80-byte libsodium sealed box, buyer-open test passes, `blob_key` returns deterministic blake2b-256 hex. |
| Task 5 — replay guard | **Implemented and unit-tested** | `indexer/src/engine.rs`; `SeenTxids::first_time` rejects duplicate txids. Demo scope is in-memory; production persistence remains open. |
| Task 6 — payment engine | **Implemented and unit-tested** | `indexer/src/engine.rs`; `Engine::on_note` does replay check, catalog lookup, underpay skip, sealed-box wrap, opaque key derivation, and `Bucket::put`. Tests cover valid payment, underpay, duplicate txid, and unknown drop. |
| Task 7 — scan loop | **Not started** | `indexer/src/scan_loop.rs` missing. |
| Task 8 — live smoke binary | **Not started** | `indexer/src/bin/scan-live.rs` missing. |

### Current runnable commands

Connectivity / raw lightwalletd check:

```bash
A1_SCAN_START=3363060 A1_SCAN_END=3363067 \
  cargo run --manifest-path indexer/Cargo.toml --bin check-lightwalletd
```

Fast compact-block UFVK candidate scan:

```bash
A1_UFVK=<creator_ufvk> A1_SCAN_START=3363060 A1_SCAN_END=3363067 \
  cargo run --manifest-path indexer/Cargo.toml --bin zecscope-scan
```

Full transaction decrypt and memo display:

```bash
A1_UFVK=<creator_ufvk> A1_SCAN_START=3363067 A1_SCAN_END=3363067 \
  cargo run --manifest-path indexer/Cargo.toml --bin probe-ufvk
```

Ignored live integration test for an already-mined memo-bearing shielded tx:

```bash
A1_UFVK=<creator_ufvk> \
A1_TXID_HEX=<display_txid_hex> \
A1_TX_HEIGHT=<height> \
A1_EXPECTED_DROP_ID=<drop_id> \
A1_EXPECTED_E_PUB_HEX=<64_hex_chars> \
cargo test --manifest-path indexer/Cargo.toml --test live_chain_memo -- --ignored --nocapture
```

Latest verification performed:

```bash
cargo fmt --manifest-path indexer/Cargo.toml
cargo check --manifest-path indexer/Cargo.toml
cargo test --manifest-path indexer/Cargo.toml memo
cargo test --manifest-path indexer/Cargo.toml dispatch
cargo test --manifest-path indexer/Cargo.toml engine
cargo test --manifest-path indexer/Cargo.toml --test live_chain_memo
cargo test --manifest-path indexer/Cargo.toml detect::tests
cargo test --manifest-path indexer/Cargo.toml zecscope_adapter
cargo test --manifest-path indexer/Cargo.toml
```

---

## Frozen interfaces this lane owns (put these in `interfaces.md`)

- **Memo (40 bytes raw, in the Zcash memo field):** `drop_id` (u64, big-endian, 8 bytes) ‖ `e_pub` (X25519 public key, 32 bytes). Lane B base64url-encodes these 40 bytes into the ZIP-321 `memo=` param; Zashi puts the raw 40 bytes on-chain; A1 reads them back.
- **Dispatch blob (libsodium sealed box):** `crypto_box_seal(K_drop, e_pub)` → `ek_pub (32) ‖ ciphertext+MAC (48)` = 80 bytes. Buyer opens with `crypto_box_seal_open` using `(e_pub, e_priv)`. Curve25519 both sides.
- **Bucket key for a dispatch blob:** `blake2b-256(ek_pub ‖ txid)` hex — opaque, carries no buyer/drop identifier (spec §5 "dispatch blob unlinkable").

## Boundaries this lane mocks (owned by others)

```rust
// Owned by Lane A2 (catalog/provisioning). A1 mocks it.
pub struct DropConfig { pub price_zat: u64, pub k_drop: [u8; 32], pub creator_ufvk: String }
pub trait Catalog: Send + Sync { fn lookup(&self, drop_id: u64) -> Option<DropConfig>; }

// Owned by Lane D (bucket). A1 mocks it.
#[async_trait::async_trait]
pub trait Bucket: Send + Sync { async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()>; }
```

## File structure

- Create `week7/drop/indexer/Cargo.toml` — the `drop-indexer` crate.
- Create `week7/drop/indexer/src/lib.rs` — module wiring + the two traits above.
- Create `week7/drop/indexer/src/memo.rs` — memo encode/decode (A1-owned format).
- Create `week7/drop/indexer/src/detect.rs` — IVK incoming detection + branch-tolerant tx read (productionized probe).
- Create `week7/drop/indexer/src/dispatch.rs` — sealed-box wrap + bucket key.
- Create `week7/drop/indexer/src/engine.rs` — PaymentNote → checks → wrap → publish.
- Create `week7/drop/indexer/src/scan_loop.rs` — lightwalletd poll loop (compact detect → full-tx fetch).
- Copy `week5/clean-wallet-mvp/apps/scanner/src/lightwalletd.rs` → `indexer/src/lightwalletd.rs` and `proto/` → `indexer/proto/` (self-contained; keep the `GrpcClient` + `LightwalletdClient` trait).

---

### Task 0: Scaffold the crate and copy the proven lightwalletd client

**Files:**
- Create: `week7/drop/indexer/Cargo.toml`, `week7/drop/indexer/build.rs`, `week7/drop/indexer/src/lib.rs`
- Copy: `week5/clean-wallet-mvp/apps/scanner/src/lightwalletd.rs` → `indexer/src/lightwalletd.rs`; `.../apps/scanner/proto/*` → `indexer/proto/`

- [ ] **Step 1: Write `Cargo.toml`** (crate/decrypt/lightwalletd/zecscope/dispatch deps exist)

```toml
[package]
name = "drop-indexer"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
async-trait = "0.1"
thiserror = "1"
tokio = { version = "1", features = ["full"] }
tonic = { version = "0.11", features = ["tls", "tls-webpki-roots"] }
prost = "0.12"
orchard = "0.13"
sapling-crypto = { version = "0.7", default-features = false }
zcash_note_encryption = "0.4"
zcash_keys = { version = "0.13", default-features = false, features = ["sapling", "orchard"] }
zcash_primitives = "0.27"
zcash_protocol = "0.8"
zip32 = "0.2"
hex = "0.4"
dryoc = "0.7" # 0.5.x conflicts with current Rust slice as_array; 0.8 conflicts with Zcash sha2 pin
blake2 = "0.10"
tracing = "0.1"

[build-dependencies]
tonic-build = "0.11"
```

- [x] **Step 2: Copy `build.rs` and proto** (identical to the scanner's; compiles `service.proto` + `compact_formats.proto`)
- [x] **Step 3: Write `src/lib.rs`** with the module declarations + the two mock-boundary traits from above (`DropConfig`, `Catalog`, `Bucket`).
- [x] **Step 4: Run `cargo build -p drop-indexer`** — PASS via `cargo check --manifest-path indexer/Cargo.toml` / `cargo test --manifest-path indexer/Cargo.toml`.
- [x] **Step 5: Commit** — scaffold/lightwalletd work was committed earlier as `Feat: scaffold drop-indexer lightwalletd client` / `Feat: add live lightwalletd check tool`.

---

### Task 1: Memo codec (A1-owned format)

**Files:** Create `week7/drop/indexer/src/memo.rs`; Test: same file `#[cfg(test)]`.

- [x] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn memo_roundtrips() {
        let e_pub = [7u8; 32];
        let raw = encode_memo(0xDEAD_BEEF, &e_pub);
        assert_eq!(raw.len(), 40);
        let (drop_id, got) = decode_memo(&raw).unwrap();
        assert_eq!(drop_id, 0xDEAD_BEEF);
        assert_eq!(got, e_pub);
    }
    #[test]
    fn decode_rejects_wrong_len() { assert!(decode_memo(&[0u8; 39]).is_none()); }
}
```

- [ ] **Step 2: Run `cargo test -p drop-indexer memo`** — fail-first run skipped; implemented directly in this session.
- [x] **Step 3: Implement**

```rust
/// Drop memo = drop_id (u64 BE, 8 bytes) || e_pub (X25519, 32 bytes) = 40 bytes.
pub fn encode_memo(drop_id: u64, e_pub: &[u8; 32]) -> Vec<u8> {
    let mut m = Vec::with_capacity(40);
    m.extend_from_slice(&drop_id.to_be_bytes());
    m.extend_from_slice(e_pub);
    m
}

/// Decode the leading 40 bytes of a Zcash memo. Trailing zero padding is ignored.
pub fn decode_memo(memo: &[u8]) -> Option<(u64, [u8; 32])> {
    if memo.len() < 40 { return None; }
    let drop_id = u64::from_be_bytes(memo[0..8].try_into().ok()?);
    let e_pub: [u8; 32] = memo[8..40].try_into().ok()?;
    Some((drop_id, e_pub))
}
```

- [x] **Step 4: Run `cargo test -p drop-indexer memo`** — PASS via `cargo test --manifest-path indexer/Cargo.toml memo`. Covers raw 40B memo and `A1B64:<base64url(raw40)>` text fallback. Also added `cargo test --manifest-path indexer/Cargo.toml --test live_chain_memo` (ignored by default) for existing chain memo fixtures.
- [x] **Step 5: Commit** — committed as `Feat: add UFVK memo scanner`.

#### Wallet text memo fallback

Some wallets expose only a UTF-8 memo field and cannot write arbitrary raw memo bytes. A1 therefore also accepts this text form while keeping the raw I1 format as canonical:

```text
A1B64:<base64url_no_pad(drop_id(8B BE) || e_pub(32B))>
```

Current test value for `drop_id=1` and `e_pub=000102...1f`:

```text
A1B64:AAAAAAAAAAEAAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHw
```

---

### Task 2: Branch-tolerant transaction reader

**Files:** Create `week7/drop/indexer/src/detect.rs` (start it here); Test: same file.

- [x] **Step 1: Write the failing test** (a v5 tx with an unknown branch id must still parse)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patches_unknown_branch_on_v5() {
        // Minimal v5 header: header=0x80000005, versiongroupid, branchid=0x5437f330 (unknown).
        let mut raw = vec![0x05,0x00,0x00,0x80, 0x0a,0x27,0xa7,0x26, 0x30,0xf3,0x37,0x54];
        raw.extend_from_slice(&[0u8; 8]); // lock_time + expiry (enough to not panic on slice)
        // We only assert the branch-id bytes get rewritten to NU5 before the (failing) parse.
        let patched = patch_v5_branch_to_nu5(&raw);
        assert_eq!(&patched[8..12], &0xC2D6_D0B4u32.to_le_bytes());
    }
}
```

- [x] **Step 2: Run `cargo test -p drop-indexer detect::tests::patches`** — covered with manifest-path test workflow.
- [x] **Step 3: Implement** (the helper proven live in `ivk-incoming-probe`)

```rust
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{BlockHeight, BranchId, Network};

/// A v5 tx embeds its consensus branch id at bytes [8..12]. A branch newer than this
/// librustzcash build fails the parse though the layout is identical and the branch id
/// is irrelevant to note decryption. Rewrite it to NU5. (See spec.md changelog C6.)
pub fn patch_v5_branch_to_nu5(raw: &[u8]) -> Vec<u8> {
    let mut p = raw.to_vec();
    if p.len() > 12 && p[0..4] == [0x05, 0x00, 0x00, 0x80] {
        p[8..12].copy_from_slice(&0xC2D6_D0B4u32.to_le_bytes());
    }
    p
}

pub fn read_tx_lenient(raw: &[u8], network: &Network, height: BlockHeight) -> anyhow::Result<Transaction> {
    let branch = BranchId::for_height(network, height);
    match Transaction::read(raw, branch) {
        Ok(t) => Ok(t),
        Err(_) => Ok(Transaction::read(&patch_v5_branch_to_nu5(raw)[..], BranchId::Nu5)?),
    }
}
```

- [x] **Step 4: Run `cargo test -p drop-indexer detect::tests::patches`** — PASS via `cargo test --manifest-path indexer/Cargo.toml detect::tests`.
- [x] **Step 5: Commit** — committed as `Feat: add UFVK memo scanner`.

---

### Task 3: IVK incoming detector (Orchard + Sapling, keep the memo)

**Files:** Modify `week7/drop/indexer/src/detect.rs`; Test: same file.

- [ ] **Step 1: Write the failing test** — use the real spike #2 mainnet tx as a golden fixture. Save its raw bytes to `indexer/tests/fixtures/spike12_tx.bin` (fetch once via `phala`/`lightwalletd` or `xxd`), and the UFVK to a const. Assert the detector recovers value `10000` and memo starting `spike12|`.

```rust
#[tokio::test]
async fn detects_real_mainnet_payment_and_memo() {
    let raw = include_bytes!("../tests/fixtures/spike12_tx.bin");
    let ufvk = include_str!("../tests/fixtures/spike12_ufvk.txt").trim();
    let notes = detect_incoming(ufvk, raw, &Network::MainNetwork, 9_999_999).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].value_zat, 10_000);
    assert!(notes[0].memo.starts_with(b"spike12|"));
}
```

- [ ] **Step 2: Run `cargo test -p drop-indexer detect::tests::detects_real`** — Expected: FAIL.
- [x] **Step 3: Implement** (port the proven probe logic; `IncomingNote { value_zat, memo: Vec<u8> }`). Current implementation also records pool and scans both external/internal scopes.

```rust
use sapling_crypto::note_encryption::{try_sapling_note_decryption, PreparedIncomingViewingKey as SaplingPivk, Zip212Enforcement};
use zcash_keys::keys::UnifiedFullViewingKey;
use zcash_note_encryption::try_note_decryption;
use zcash_protocol::consensus::{NetworkUpgrade, Parameters};

pub struct IncomingNote { pub value_zat: u64, pub memo: Vec<u8> }

pub fn detect_incoming(ufvk_str: &str, raw_tx: &[u8], network: &Network, height: u32) -> anyhow::Result<Vec<IncomingNote>> {
    let ufvk = UnifiedFullViewingKey::decode(network, ufvk_str).map_err(|e| anyhow::anyhow!("ufvk: {e}"))?;
    let s_ivk = ufvk.sapling().map(|s| SaplingPivk::new(&s.to_ivk(zip32::Scope::External)));
    let o_ivk = ufvk.orchard().map(|o| orchard::keys::PreparedIncomingViewingKey::new(&o.to_ivk(orchard::keys::Scope::External)));
    let bh = BlockHeight::from_u32(height);
    let tx = read_tx_lenient(raw_tx, network, bh)?;
    let mut out = Vec::new();
    if let (Some(pivk), Some(b)) = (&s_ivk, tx.sapling_bundle()) {
        let z = if network.is_nu_active(NetworkUpgrade::Canopy, bh) { Zip212Enforcement::On } else { Zip212Enforcement::GracePeriod };
        for o in b.shielded_outputs() {
            if let Some((note, _addr, memo)) = try_sapling_note_decryption(pivk, o, z) {
                out.push(IncomingNote { value_zat: note.value().inner(), memo: memo.as_slice().to_vec() });
            }
        }
    }
    if let (Some(pivk), Some(b)) = (&o_ivk, tx.orchard_bundle()) {
        for a in b.actions() {
            let d = orchard::note_encryption::OrchardDomain::for_action(a);
            if let Some((note, _addr, memo)) = try_note_decryption(&d, pivk, a) {
                out.push(IncomingNote { value_zat: note.value().inner(), memo: memo.to_vec() });
            }
        }
    }
    Ok(out)
}
```

- [ ] **Step 4: Run the test** — pending golden fixture. Live verification instead found an Orchard note at height `3363067`, value `74999`, memo `<none>`.
- [x] **Step 5: Commit** — UFVK detector/probe implementation committed as `Feat: add UFVK memo scanner`; golden fixture follow-up remains pending.

---

### Extra Task 3a: zecscope-scanner compact-block smoke (implemented)

This task was added during investigation to cross-check UFVK detection with the public `zecscope-scanner` API from docs.rs. It is not the final memo path, but it is useful for fast candidate detection before full transaction fetch/decrypt.

**Files:**
- `indexer/src/zecscope_adapter.rs`
- `indexer/src/bin/zecscope-scan.rs`

- [x] Convert generated lightwalletd compact protobuf structs into `zecscope_scanner::CompactBlock` / `CompactTx` JSON-friendly types.
- [x] Add `zecscope-scan` binary using `Scanner::new(Network)` + `ScanRequest`.
- [x] Run live compact scan over `3363060..=3363067`; result: `zecscope.matches=1`, Orchard incoming candidate, `amount_zat=74999`.
- [x] Unit-test the adapter byte-to-hex conversion.
- [ ] Normalize zecscope txid output to display byte order or document it clearly in CLI output; current zecscope txid is protocol-order and can differ from explorer/display txid.

---

### Task 4: Sealed-box dispatch wrap + bucket key

**Files:** Create `week7/drop/indexer/src/dispatch.rs`; Test: same file.

- [x] **Step 1: Write the failing test** (TEE seals → buyer opens with e_priv)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use dryoc::sealedbox::SealedBox;
    use dryoc::keypair::KeyPair;
    #[test]
    fn buyer_can_open_dispatch_blob() {
        let buyer = KeyPair::gen();                       // (e_priv, e_pub)
        let e_pub: [u8;32] = buyer.public_key.as_array().clone();
        let k_drop = [42u8; 32];
        let blob = wrap_k_drop(&k_drop, &e_pub).unwrap();
        assert_eq!(blob.len(), 80);                       // 32 ek_pub + 48 ct+MAC
        let opened = SealedBox::unseal_to_vec(&blob, &buyer).unwrap();
        assert_eq!(opened.as_slice(), &k_drop);
    }
}
```

- [ ] **Step 2: Run `cargo test -p drop-indexer dispatch`** — fail-first run skipped; implemented directly in this session.
- [x] **Step 3: Implement**. Current implementation uses `dryoc::dryocbox` (`dryoc 0.7.x`) rather than the older plan snippet API because `dryoc 0.5.x` no longer compiles on the current Rust toolchain.

```rust
use anyhow::{anyhow, Result};
use blake2::{digest::consts::U32, Blake2b, Digest};
use dryoc::dryocbox::{DryocBox, PublicKey};

/// libsodium sealed box: ek_pub(32) || ciphertext+MAC(48). Buyer opens with e_priv.
pub fn wrap_k_drop(k_drop: &[u8; 32], e_pub: &[u8; 32]) -> Result<Vec<u8>> {
    let pk: PublicKey = (*e_pub).into();
    let sealed = DryocBox::seal_to_vecbox(k_drop, &pk)
        .map_err(|e| anyhow!("seal K_drop: {e:?}"))?;
    Ok(sealed.to_vec())
}

/// Opaque bucket key — no buyer/drop identifier (spec §5).
pub fn blob_key(ek_pub_prefix: &[u8], txid: &[u8; 32]) -> String {
    let mut h = Blake2b::<U32>::new();
    h.update(ek_pub_prefix);
    h.update(txid);
    hex::encode(h.finalize())
}
```

- [x] **Step 4: Run the test** — PASS via `cargo test --manifest-path indexer/Cargo.toml dispatch` and full `cargo test --manifest-path indexer/Cargo.toml`.
- [x] **Step 5: Commit** — committed as `Feat: add payment dispatch engine`.

---

### Task 5: Replay guard (nullifier set)

**Files:** Modify `week7/drop/indexer/src/engine.rs` (create it); Test: same file.

- [x] **Step 1: Write the replay guard test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_duplicate_txid() {
        let mut seen = SeenTxids::default();
        let id = [1u8; 32];
        assert!(seen.first_time(&id));   // first → process
        assert!(!seen.first_time(&id));  // duplicate → skip
    }
}
```

- [ ] **Step 2: Run `cargo test -p drop-indexer engine::tests::rejects_dup`** — fail-first run skipped; implemented directly in this session.
- [x] **Step 3: Implement** (in-memory for the demo; note in code that production persists this)

```rust
use std::collections::HashSet;
#[derive(Default)]
pub struct SeenTxids(HashSet<[u8; 32]>);
impl SeenTxids {
    /// Returns true the first time a txid is seen; false on replay. Demo-scope in-memory;
    /// production must persist (a restart must not re-dispatch). Tracked: Open-Q replay window.
    pub fn first_time(&mut self, txid: &[u8; 32]) -> bool { self.0.insert(*txid) }
}
```

- [x] **Step 4: Run the test** — PASS via `cargo test --manifest-path indexer/Cargo.toml engine` and full `cargo test --manifest-path indexer/Cargo.toml`.
- [x] **Step 5: Commit** — committed as `Feat: add payment dispatch engine`.

---

### Task 6: Engine — note → checks → wrap → publish

**Files:** Modify `week7/drop/indexer/src/engine.rs`; Test: same file (mock `Catalog` + `Bucket`).

- [x] **Step 1: Write the test** — a valid payment publishes exactly one blob; an underpayment publishes none.

```rust
#[tokio::test]
async fn valid_payment_publishes_one_blob_underpay_none() {
    let cat = MockCatalog { price: 10_000, k_drop: [9u8;32] };
    let bucket = MockBucket::default();
    let mut eng = Engine::new(cat, bucket.clone());
    let e_pub = dryoc::keypair::KeyPair::gen().public_key.as_array().clone();

    // pays price → 1 blob
    eng.on_note(&Note{ drop_id:1, e_pub, value_zat:10_000, txid:[1u8;32] }).await.unwrap();
    assert_eq!(bucket.count(), 1);
    // underpay → still 1 (no new blob)
    eng.on_note(&Note{ drop_id:1, e_pub, value_zat:9_999, txid:[2u8;32] }).await.unwrap();
    assert_eq!(bucket.count(), 1);
}
```

- [ ] **Step 2: Run `cargo test -p drop-indexer engine::tests::valid_payment`** — fail-first run skipped; implemented directly in this session.
- [x] **Step 3: Implement**

```rust
use crate::{Catalog, Bucket};
use crate::dispatch::{wrap_k_drop, blob_key};

pub struct Note { pub drop_id: u64, pub e_pub: [u8;32], pub value_zat: u64, pub txid: [u8;32] }

pub struct Engine<C: Catalog, B: Bucket> { cat: C, bucket: B, seen: SeenTxids }
impl<C: Catalog, B: Bucket> Engine<C, B> {
    pub fn new(cat: C, bucket: B) -> Self { Self { cat, bucket, seen: SeenTxids::default() } }

    /// Process one detected incoming note. Publishes a dispatch blob iff the payment is
    /// fresh, the drop exists, and value >= price. Idempotent on txid.
    pub async fn on_note(&mut self, n: &Note) -> anyhow::Result<()> {
        if !self.seen.first_time(&n.txid) { return Ok(()); }
        let Some(cfg) = self.cat.lookup(n.drop_id) else { return Ok(()); };
        if n.value_zat < cfg.price_zat { tracing::warn!(n.drop_id, "underpaid"); return Ok(()); }
        let blob = wrap_k_drop(&cfg.k_drop, &n.e_pub)?;
        let key = blob_key(&blob[..32], &n.txid);
        self.bucket.put(&key, &blob).await
    }
}
```

- [x] **Step 4: Run the test** — PASS via `cargo test --manifest-path indexer/Cargo.toml engine` and full `cargo test --manifest-path indexer/Cargo.toml`.
- [x] **Step 5: Commit** — committed as `Feat: add payment dispatch engine`.

---

### Task 7: Scan loop — lightwalletd poll → notes → engine

**Files:** Create `week7/drop/indexer/src/scan_loop.rs`; Test: same file (mock `LightwalletdClient`, reuse the scanner's mock pattern).

- [ ] **Step 1: Write the failing test** — given a compact block referencing one txid and a canned full tx (the spike12 fixture), the loop drives the engine to publish one blob.

```rust
#[tokio::test]
async fn loop_detects_and_dispatches() {
    let client = MockClient::with_tx([0xAB;32], include_bytes!("../tests/fixtures/spike12_tx.bin").to_vec());
    let cat = MockCatalog { price: 10_000, k_drop: [9u8;32] };
    let bucket = MockBucket::default();
    let ufvk = include_str!("../tests/fixtures/spike12_ufvk.txt").trim().to_string();
    scan_once(&client, &ufvk, &Network::MainNetwork, 0, 0, &mut Engine::new(cat, bucket.clone())).await.unwrap();
    assert_eq!(bucket.count(), 1);
}
```

- [ ] **Step 2: Run `cargo test -p drop-indexer scan_loop`** — Expected: FAIL.
- [ ] **Step 3: Implement** — `scan_once`: `fetch_block_range(start,end)` → for each `vtx.txid` → `fetch_transaction` → `detect_incoming` → for each note `decode_memo` → `engine.on_note`. (Then `run_loop` wraps `scan_once` from a cursor to tip on an interval.)

```rust
pub async fn scan_once<C: LightwalletdClient, K: Catalog, B: Bucket>(
    client: &C, ufvk: &str, net: &Network, start: u64, end: u64, eng: &mut Engine<K, B>,
) -> anyhow::Result<()> {
    for block in client.fetch_block_range(start, end).await? {
        for ctx in &block.vtx {
            let txid: [u8;32] = ctx.txid.as_slice().try_into()?;
            let raw = client.fetch_transaction(&txid).await?;
            if raw.is_empty() { continue; }
            for note in detect_incoming(ufvk, &raw, net, block.height as u32)? {
                if let Some((drop_id, e_pub)) = decode_memo(&note.memo) {
                    eng.on_note(&Note { drop_id, e_pub, value_zat: note.value_zat, txid }).await?;
                }
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run the test** — Expected: PASS (full chain: compact → full tx → IVK detect → memo → wrap → publish).
- [ ] **Step 5: Commit** — `git commit -m "feat(drop-indexer): scan loop wiring (end-to-end with mocks)"`

---

### Task 8: Live smoke binary

**Files:** Create `week7/drop/indexer/src/bin/scan-live.rs`.

- [ ] **Step 1: Write a `main`** that takes `<creator-ufvk> <start> <end>`, builds `GrpcClient::new("https://zec.rocks:443", None)`, an in-memory `Catalog` with one demo drop, and a logging `Bucket` that prints `put(key, len)`, then calls `scan_once`. (No new test; this is the manual end-to-end against the real spike12 payment.)
- [ ] **Step 2: Run** `cargo run -p drop-indexer --bin scan-live -- "$(cat /tmp/spike12_ufvk.txt)" <h> <h>` over the block holding the spike12 tx — Expected: prints one `put(...)` line.
- [ ] **Step 3: Commit** — `git commit -m "feat(drop-indexer): live smoke binary"`

---

## Self-review

- **Spec coverage:** memo (§4.3) ✓ Task 1; IVK incoming + full-tx + branch tolerance (§3.1/§3.2/C6) ✓ Tasks 2–3; ECIES dispatch blob (§4.3) ✓ Task 4; amount check + replay (§7.3) ✓ Tasks 5–6; scan loop (§4.3) ✓ Task 7. **Not in this lane (correctly):** secret-IN provisioning, attestation, catalog persistence, bucket impl → Lanes A2/D (mocked here).
- **Type consistency:** `IncomingNote{value_zat,memo}` (Task 3) → `Note{drop_id,e_pub,value_zat,txid}` (Task 6) via `decode_memo` (Task 1); `DropConfig{price_zat,k_drop,creator_ufvk}` consistent Task 0/6.
- **Interface risk:** memo + dispatch-blob formats here MUST equal `interfaces.md`. Confirm with Lane B (buyer must `crypto_box_seal_open` the exact 80-byte blob and base64url the exact 40-byte memo) at the Day-1 kickoff before Task 4/1 land.
- **Open dependency:** the golden fixture (`spike12_tx.bin`) — capture the raw tx of `ae11a454…` once and commit it so Task 3/7 tests are hermetic.

## Execution handoff

**Plan saved to `week7/drop/plan-a1-payment-flow.md`. Two execution options:**

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — execute tasks in this session with checkpoints.

Which approach?
