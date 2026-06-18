# Lane A2 — Enclave Platform Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the drop indexer's enclave platform — the attestation endpoint (`/attest`, interface I6), the secret-IN provisioning endpoint (`/provision`, I5), the catalog store (I3), a stub bucket, the HTTP server, and the reproducible Docker → Phala deploy — so a creator can verify the enclave and seal `K_drop` into it, and Lane A1's engine reads the resulting drop config.

**Architecture:** A2 owns the `attest` / `provision` / `catalog` / `bucket` / `server` modules of the shared `drop-indexer` crate (A1 owns `scan` / `dispatch`). The trust core reuses spike #3's *verified* mechanism: the enclave derives a stable X25519 keypair from the dstack KMS, publishes its public key inside the TDX quote's `report_data`, and a creator who verifies the quote encrypts `K_drop` to that key via a libsodium sealed box. No new cryptographic invention — it's the spike-#3 "encrypt-to-enclave" flow turned into an HTTP endpoint.

**Tech Stack:** Rust, `axum` (HTTP), `tokio`, `dryoc` (libsodium-compatible sealed box, interops with Lane C's `libsodium.js`), `sha2`, `serde`/`serde_json`, raw `UnixStream` for the dstack socket (same pattern as clean-wallet `attest.rs`). Deploy: Docker + `phala` CLI.

---

## Where this fits (interfaces)

A2 SERVES three things and OWNS the trust boundary:
- **I6 `GET /attest`** → TDX quote with `report_data[0..32] = sha256(provisioning_pubkey)` + the pubkey. (Creator C verifies this.)
- **I5 `POST /provision`** ← creator's `crypto_box_seal({drop_id, price_zat, k_drop, creator_ufvk, h_content}, provisioning_pubkey)`. A2 `seal_open`s it and stores the drop.
- **I3 catalog** → internal `DropConfig` (read by A1 via the `Catalog` trait) + public catalog JSON (browsed by B).
- Plus a **stub bucket** (PUT/GET/LIST) that A1 (dispatch blobs), C (content blobs), and B (polling) all use.

**Reference (already in the repo):** `week5/clean-wallet-mvp/apps/scanner/src/attest.rs` (dstack `/GetQuote` + `/Info` over UDS, `report_data` packing), `.../src/server.rs` (axum routes + error mapping), `week5/clean-wallet-mvp/scripts/deploy-cvm.sh`, and **`week7/drop/spike3/RUNBOOK.md`** (the secret-IN flow you are productionizing — read it first).

## Shared crate contract (coordinate with A1 at the kickoff)

`drop-indexer/src/lib.rs` defines the boundary types. A1 *mocks* these; A2 provides the *real* impls:

```rust
pub struct DropConfig { pub price_zat: u64, pub k_drop: [u8; 32], pub creator_ufvk: String }
pub trait Catalog: Send + Sync { fn lookup(&self, drop_id: u64) -> Option<DropConfig>; }

#[async_trait::async_trait]
pub trait Bucket: Send + Sync {
    async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()>;
    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>>;   // B polls with this
    async fn list(&self) -> anyhow::Result<Vec<String>>;                 // B lists new dispatch blobs
}
```
> Note: A1's plan declared `Bucket` with only `put` (it only writes). A2 OWNS the bucket, so it extends the trait with `get`/`list`. Land this extension at the kickoff so A1's mock implements all three (its `get`/`list` can be trivial).

## File structure

- `drop-indexer/Cargo.toml` — add A2 deps (axum, dryoc, serde_json, sha2).
- `drop-indexer/src/dstack.rs` — UDS client: `get_quote(report_data)`, `get_key(path)`, `info_mrtd()`.
- `drop-indexer/src/attest.rs` — provisioning keypair + `/attest` payload builder.
- `drop-indexer/src/provision.rs` — `seal_open` the I5 payload → `DropConfig`.
- `drop-indexer/src/catalog.rs` — in-memory store; `Catalog` impl; public catalog JSON.
- `drop-indexer/src/bucket.rs` — filesystem-backed `Bucket` impl.
- `drop-indexer/src/server.rs` — axum routes wiring everything (+ A1's scan loop).
- `drop-indexer/Dockerfile`, reuse `deploy-cvm.sh`.

---

### Task 0: Crate + shared types (skip if A1 already scaffolded)

**Files:** Create/extend `drop-indexer/Cargo.toml`, `drop-indexer/src/lib.rs`.

- [ ] **Step 1: Ensure the crate exists.** If A1 hasn't run their Task 0, create `drop-indexer/` per A1's plan (Cargo.toml + lib.rs + copied `lightwalletd.rs`). Then add A2 deps to `Cargo.toml`:

```toml
axum = "0.7"
dryoc = "0.5"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
tower-http = { version = "0.5", features = ["cors"] }
```

- [ ] **Step 2: Add shared types to `lib.rs`** (the `Bucket` trait with all three methods, `DropConfig`, `Catalog`, and the provisioning payload + public entry):

```rust
pub mod dstack; pub mod attest; pub mod provision; pub mod catalog; pub mod bucket; pub mod server;

#[derive(Clone)]
pub struct DropConfig { pub price_zat: u64, pub k_drop: [u8; 32], pub creator_ufvk: String, pub h_content: String }

pub trait Catalog: Send + Sync { fn lookup(&self, drop_id: u64) -> Option<DropConfig>; }

#[async_trait::async_trait]
pub trait Bucket: Send + Sync {
    async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()>;
    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>>;
    async fn list(&self) -> anyhow::Result<Vec<String>>;
}

/// What the creator seals to the enclave (interface I5). `k_drop` is hex (32 bytes).
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ProvisionPayload {
    pub drop_id: u64, pub price_zat: u64, pub k_drop_hex: String,
    pub creator_ufvk: String, pub h_content: String,
}

/// Public catalog entry the buyer browses (interface I3-a). No secrets.
#[derive(serde::Serialize, Clone)]
pub struct CatalogEntry { pub drop_id: u64, pub price_zat: u64, pub h_content: String, pub title: String }
```

- [ ] **Step 3: Run `cargo build -p drop-indexer`** — Expected: PASS.
- [ ] **Step 4: Commit** — `git commit -m "feat(drop-indexer): A2 shared types + Bucket get/list"`

---

### Task 1: dstack UDS client — get_quote (port from attest.rs)

**Files:** Create `drop-indexer/src/dstack.rs`; Test: same file.

- [ ] **Step 1: Write the failing test** (mock-shape: packing 32 bytes into the 64-byte report_data)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_data_is_zero_padded_to_64() {
        let rd = [9u8; 32];
        let padded = pad_report_data(&rd);
        assert_eq!(padded.len(), 64);
        assert_eq!(&padded[..32], &rd);
        assert_eq!(&padded[32..], &[0u8; 32]);
    }
}
```

- [ ] **Step 2: Run `cargo test -p drop-indexer dstack::tests::report_data`** — Expected: FAIL.
- [ ] **Step 3: Implement** the UDS client (reuse `attest.rs`'s `post_uds_json` verbatim; add `get_quote` + `info_mrtd`):

```rust
use anyhow::{anyhow, Result};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream as StdUnixStream;

pub struct Dstack { pub socket: String }

pub fn pad_report_data(rd: &[u8; 32]) -> [u8; 64] { let mut p = [0u8; 64]; p[..32].copy_from_slice(rd); p }

impl Dstack {
    pub fn new(socket: impl Into<String>) -> Self { Self { socket: socket.into() } }

    pub async fn get_quote(&self, report_data: &[u8; 32]) -> Result<String> {
        let body = serde_json::json!({ "report_data": hex::encode(pad_report_data(report_data)) });
        let resp = post_uds_json(&self.socket, "/GetQuote", &body).await?;
        resp.get("quote").and_then(|v| v.as_str()).map(|s| s.to_string())
            .ok_or_else(|| anyhow!("dstack /GetQuote: missing 'quote'"))
    }

    pub async fn info_mrtd(&self) -> Result<String> {
        let resp = post_uds_json(&self.socket, "/Info", &serde_json::json!({})).await?;
        let tcb = resp.get("tcb_info").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("missing tcb_info"))?;
        let tcb: serde_json::Value = serde_json::from_str(tcb)?;   // dstack quirk: it's a JSON string
        tcb.get("mrtd").and_then(|v| v.as_str()).map(|s| s.to_string())
            .ok_or_else(|| anyhow!("missing mrtd"))
    }
}

async fn post_uds_json(socket: &str, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
    let socket = socket.to_string(); let path = path.to_string(); let body = body.to_string();
    tokio::task::spawn_blocking(move || {
        let mut s = StdUnixStream::connect(&socket)?;
        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len());
        s.write_all(req.as_bytes())?;
        let mut buf = String::new(); s.read_to_string(&mut buf)?;
        let json_start = buf.find("\r\n\r\n").ok_or_else(|| anyhow!("no body"))? + 4;
        Ok(serde_json::from_str(&buf[json_start..])?)
    }).await?
}
```

- [ ] **Step 4: Run `cargo test -p drop-indexer dstack::tests::report_data`** — Expected: PASS.
- [ ] **Step 5: Live check (optional, against the simulator).** Start it: `phala simulator start`. Add an `#[ignore]` test that calls `Dstack::new(sim_sock).get_quote(&[0u8;32])` and asserts the quote hex is non-empty. Run with `cargo test -p drop-indexer -- --ignored dstack`.
- [ ] **Step 6: Commit** — `git commit -m "feat(drop-indexer): dstack UDS client (get_quote, info_mrtd)"`

---

### Task 2: Provisioning keypair — stable, KMS-derived (the spike-#3 key, productionized)

**Files:** Modify `drop-indexer/src/dstack.rs` (add `get_key`); Create `drop-indexer/src/attest.rs`.

- [ ] **Step 1: Discover the dstack key endpoint** (do this once against the simulator — the exact path/shape is the one thing not in the repo yet). The dstack guest agent derives app-bound keys; in v0.5.x the call is a JSON POST. Probe it:

```bash
phala simulator start
SOCK=$(ls ~/.phala-cloud/simulator/*/dstack.sock | head -1)
# Try the likely endpoint; if 404, inspect appkeys.json (env_crypt_key is the known-present x25519 key)
printf 'POST /GetKey HTTP/1.1\r\nHost: localhost\r\nContent-Length: 31\r\nConnection: close\r\n\r\n{"path":"drop/provisioning"}' | socat - UNIX-CONNECT:$SOCK
cat ~/.phala-cloud/simulator/*/appkeys.json   # fallback reference: "env_crypt_key" (32-byte hex x25519)
```
Record the working endpoint/field. (If `/GetKey` isn't present in the sim, read the `env_crypt_key` from the dstack `/Info`/appkeys path — it is the dstack-managed x25519 secret and is stable per measurement.)

- [ ] **Step 2: Write the failing test** (the keypair must be DETERMINISTIC — same across calls — so a creator who provisioned can still be reached):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keypair_from_seed_is_deterministic() {
        let seed = [3u8; 32];
        let a = provisioning_keypair_from_seed(&seed);
        let b = provisioning_keypair_from_seed(&seed);
        assert_eq!(a.public_key, b.public_key);   // same seed → same pubkey
        assert_eq!(a.public_key.len(), 32);
    }
}
```

- [ ] **Step 3: Run `cargo test -p drop-indexer attest::tests::keypair`** — Expected: FAIL.
- [ ] **Step 4: Implement.** Derive the X25519 keypair from the dstack-provided 32-byte secret (`get_key` result, or `env_crypt_key`). `dstack.rs`: add `get_key`. `attest.rs`: build the keypair:

```rust
// in dstack.rs
impl Dstack {
    /// 32-byte app-bound secret, stable per measurement. Endpoint confirmed in Task 2 Step 1.
    pub async fn get_key(&self, path: &str) -> anyhow::Result<[u8; 32]> {
        let resp = post_uds_json(&self.socket, "/GetKey", &serde_json::json!({ "path": path })).await?;
        let hexk = resp.get("key").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("dstack /GetKey: missing 'key'"))?;
        let raw = hex::decode(hexk)?;
        raw.as_slice().try_into().map_err(|_| anyhow::anyhow!("key not 32 bytes"))
    }
}

// in attest.rs
use dryoc::keypair::StackKeyPair;
pub fn provisioning_keypair_from_seed(seed: &[u8; 32]) -> StackKeyPair {
    // X25519 keypair whose secret is the dstack-derived seed; public derived from it.
    StackKeyPair::from_secret_key((*seed).into())
}
```

- [ ] **Step 5: Run `cargo test -p drop-indexer attest::tests::keypair`** — Expected: PASS.
- [ ] **Step 6: Commit** — `git commit -m "feat(drop-indexer): KMS-derived deterministic provisioning keypair"`

---

### Task 3: `/attest` payload — bind the provisioning pubkey into report_data (I6)

**Files:** Modify `drop-indexer/src/attest.rs`; Test: same file.

- [ ] **Step 1: Write the failing test** (report_data must equal sha256(pubkey) so the creator can trust the key they encrypt to):

```rust
#[tokio::test]
async fn attest_payload_binds_pubkey() {
    let kp = provisioning_keypair_from_seed(&[5u8; 32]);
    let rd = report_data_for_pubkey(&kp.public_key);
    use sha2::{Digest, Sha256};
    assert_eq!(rd.to_vec(), Sha256::digest(kp.public_key.as_slice()).to_vec());
}
```

- [ ] **Step 2: Run `cargo test -p drop-indexer attest::tests::attest_payload`** — Expected: FAIL.
- [ ] **Step 3: Implement**

```rust
use sha2::{Digest, Sha256};
use serde::Serialize;

pub fn report_data_for_pubkey(pubkey: &impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(pubkey.as_ref()).into()
}

#[derive(Serialize)]
pub struct AttestResponse { pub quote_hex: String, pub provisioning_pubkey_hex: String }

/// Builds the I6 payload: a fresh quote binding sha256(pubkey) + the pubkey itself.
pub async fn build_attest_response(ds: &crate::dstack::Dstack, kp: &dryoc::keypair::StackKeyPair) -> anyhow::Result<AttestResponse> {
    let rd = report_data_for_pubkey(&kp.public_key);
    Ok(AttestResponse {
        quote_hex: ds.get_quote(&rd).await?,
        provisioning_pubkey_hex: hex::encode(kp.public_key.as_slice()),
    })
}
```

- [ ] **Step 4: Run the test** — Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(drop-indexer): /attest binds provisioning pubkey into report_data (I6)"`

---

### Task 4: `/provision` — seal_open the secret payload (I5, the secret-IN core)

**Files:** Create `drop-indexer/src/provision.rs`; Test: same file.

- [ ] **Step 1: Write the failing test** (a creator seals a payload to the enclave pubkey; the enclave opens it; an outsider without the secret key cannot):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use dryoc::sealedbox::SealedBox;
    #[test]
    fn enclave_opens_what_creator_sealed() {
        let kp = crate::attest::provisioning_keypair_from_seed(&[7u8; 32]);
        let payload = crate::ProvisionPayload {
            drop_id: 1, price_zat: 1_000_000,
            k_drop_hex: hex::encode([0xAB; 32]), creator_ufvk: "uview1demo".into(),
            h_content: "abc123".into(),
        };
        // creator side: seal JSON to the enclave's PUBLIC key
        let sealed = SealedBox::seal_to_vec(serde_json::to_vec(&payload).unwrap().as_slice(),
                        &kp.public_key).unwrap();
        // enclave side: open with the keypair → DropConfig
        let cfg = open_provision(&sealed, &kp).unwrap();
        assert_eq!(cfg.price_zat, 1_000_000);
        assert_eq!(cfg.k_drop, [0xAB; 32]);
        assert_eq!(cfg.creator_ufvk, "uview1demo");
    }
}
```

- [ ] **Step 2: Run `cargo test -p drop-indexer provision::tests`** — Expected: FAIL.
- [ ] **Step 3: Implement**

```rust
use dryoc::sealedbox::SealedBox;
use dryoc::keypair::StackKeyPair;
use crate::{DropConfig, ProvisionPayload};

/// Open the creator's sealed I5 payload with the enclave keypair → internal DropConfig.
/// The operator, lacking the secret key, cannot open `sealed` (that's the whole point).
pub fn open_provision(sealed: &[u8], kp: &StackKeyPair) -> anyhow::Result<DropConfig> {
    let plain = SealedBox::unseal_to_vec(sealed, kp)
        .map_err(|e| anyhow::anyhow!("seal_open failed: {e:?}"))?;
    let p: ProvisionPayload = serde_json::from_slice(&plain)?;
    let k: [u8; 32] = hex::decode(&p.k_drop_hex)?.as_slice().try_into()
        .map_err(|_| anyhow::anyhow!("k_drop not 32 bytes"))?;
    Ok(DropConfig { price_zat: p.price_zat, k_drop: k, creator_ufvk: p.creator_ufvk, h_content: p.h_content })
}
```

- [ ] **Step 4: Run the test** — Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(drop-indexer): /provision seal_open (secret-IN, I5)"`

---

### Task 5: Catalog store — `Catalog` impl + public JSON (I3)

**Files:** Create `drop-indexer/src/catalog.rs`; Test: same file.

- [ ] **Step 1: Write the failing test** (after provisioning, A1's `lookup` returns the config; the public view hides secrets):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::DropConfig;
    #[test]
    fn lookup_after_insert_and_public_hides_secrets() {
        let store = CatalogStore::default();
        store.insert(1, DropConfig { price_zat: 500, k_drop: [1;32], creator_ufvk: "uview1x".into(), h_content: "h1".into() }, "cat.png".into());
        let cfg = store.lookup(1).unwrap();
        assert_eq!(cfg.price_zat, 500);
        let public = store.public_entries();
        assert_eq!(public.len(), 1);
        assert_eq!(public[0].h_content, "h1");
        // public JSON must not contain the key material
        let json = serde_json::to_string(&public).unwrap();
        assert!(!json.contains("uview1x"));
    }
}
```

- [ ] **Step 2: Run `cargo test -p drop-indexer catalog::tests`** — Expected: FAIL.
- [ ] **Step 3: Implement** (in-memory for the demo; note where production would persist):

```rust
use std::collections::HashMap;
use std::sync::RwLock;
use crate::{Catalog, CatalogEntry, DropConfig};

#[derive(Default)]
pub struct CatalogStore { inner: RwLock<HashMap<u64, (DropConfig, String)>> } // (config, title)

impl CatalogStore {
    pub fn insert(&self, drop_id: u64, cfg: DropConfig, title: String) {
        self.inner.write().unwrap().insert(drop_id, (cfg, title));
    }
    pub fn public_entries(&self) -> Vec<CatalogEntry> {
        self.inner.read().unwrap().iter()
            .map(|(id, (c, t))| CatalogEntry { drop_id: *id, price_zat: c.price_zat, h_content: c.h_content.clone(), title: t.clone() })
            .collect()
    }
}
impl Catalog for CatalogStore {
    fn lookup(&self, drop_id: u64) -> Option<DropConfig> {
        self.inner.read().unwrap().get(&drop_id).map(|(c, _)| c.clone())
    }
}
```

- [ ] **Step 4: Run the test** — Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(drop-indexer): catalog store (Catalog impl + public JSON, I3)"`

---

### Task 6: Bucket — filesystem-backed PUT/GET/LIST

**Files:** Create `drop-indexer/src/bucket.rs`; Test: same file.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn bucket_put_get_list_roundtrip() {
    let dir = std::env::temp_dir().join("drop-bucket-test");
    let _ = std::fs::remove_dir_all(&dir);
    let b = FsBucket::new(&dir).unwrap();
    b.put("k1", b"hello").await.unwrap();
    assert_eq!(b.get("k1").await.unwrap().as_deref(), Some(&b"hello"[..]));
    assert_eq!(b.get("missing").await.unwrap(), None);
    assert_eq!(b.list().await.unwrap(), vec!["k1".to_string()]);
}
```

- [ ] **Step 2: Run `cargo test -p drop-indexer bucket::tests`** — Expected: FAIL.
- [ ] **Step 3: Implement** (keys are hex, so safe as filenames; demo-scope local FS — note S3/Blossom is the production swap):

```rust
use crate::Bucket;
use std::path::PathBuf;

pub struct FsBucket { dir: PathBuf }
impl FsBucket {
    pub fn new(dir: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let dir = dir.into(); std::fs::create_dir_all(&dir)?; Ok(Self { dir })
    }
}
#[async_trait::async_trait]
impl Bucket for FsBucket {
    async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()> {
        tokio::fs::write(self.dir.join(key), bytes).await?; Ok(())
    }
    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        match tokio::fs::read(self.dir.join(key)).await {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    async fn list(&self) -> anyhow::Result<Vec<String>> {
        let mut out = Vec::new();
        let mut rd = tokio::fs::read_dir(&self.dir).await?;
        while let Some(e) = rd.next_entry().await? { out.push(e.file_name().to_string_lossy().into_owned()); }
        Ok(out)
    }
}
```

- [ ] **Step 4: Run the test** — Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(drop-indexer): filesystem bucket (put/get/list)"`

---

### Task 7: Server wiring — axum routes + mount A1's scan loop

**Files:** Create `drop-indexer/src/server.rs`; `drop-indexer/src/main.rs`; Test: `server.rs`.

- [ ] **Step 1: Write the failing test** (provision over HTTP, then the public catalog shows the drop):

```rust
#[tokio::test]
async fn provision_then_public_catalog_lists_it() {
    let st = AppState::new_for_test([7u8; 32]);                // fixed provisioning seed
    let app = router(st.clone());
    // creator seals to the enclave pubkey and POSTs it
    let kp = crate::attest::provisioning_keypair_from_seed(&[7u8; 32]);
    let payload = crate::ProvisionPayload { drop_id: 1, price_zat: 500, k_drop_hex: hex::encode([2u8;32]), creator_ufvk: "uview1x".into(), h_content: "h1".into() };
    let sealed = dryoc::sealedbox::SealedBox::seal_to_vec(serde_json::to_vec(&payload).unwrap().as_slice(), &kp.public_key).unwrap();
    let res = request(&app, "POST", "/provision", Some((sealed, "title=Cat"))).await;
    assert_eq!(res.status, 200);
    let cat = request(&app, "GET", "/catalog", None).await;
    assert!(cat.body.contains("\"drop_id\":1"));
    assert!(!cat.body.contains("uview1x"));                    // secrets stay internal
}
```
(Use a tiny `request` helper over `tower::ServiceExt::oneshot`; `title` comes as a query/header alongside the sealed body.)

- [ ] **Step 2: Run `cargo test -p drop-indexer server::tests::provision_then`** — Expected: FAIL.
- [ ] **Step 3: Implement** the routes + state. `AppState` holds the `Dstack`, the provisioning `StackKeyPair`, the `CatalogStore`, and the `FsBucket`. Routes:
  - `GET /health` → `"ok"`
  - `GET /attest` → `build_attest_response(...)` (I6)
  - `POST /provision` → `open_provision(body, &kp)` → `catalog.insert(...)` (I5)
  - `GET /catalog` → `catalog.public_entries()` (I3-a)
  - `GET /bucket/:key` → `bucket.get` ; `PUT /bucket/:key` → `bucket.put` ; `GET /bucket` → `bucket.list`

```rust
use axum::{Router, routing::{get, post, put}, extract::{State, Path}, http::StatusCode, Json};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState { pub inner: Arc<Inner> }
pub struct Inner { pub ds: crate::dstack::Dstack, pub kp: dryoc::keypair::StackKeyPair,
                   pub catalog: crate::catalog::CatalogStore, pub bucket: crate::bucket::FsBucket }

pub fn router(st: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/attest", get(attest_h))
        .route("/provision", post(provision_h))
        .route("/catalog", get(catalog_h))
        .route("/bucket/:key", get(bucket_get_h).put(bucket_put_h))
        .with_state(st)
}

async fn attest_h(State(s): State<AppState>) -> Result<Json<crate::attest::AttestResponse>, StatusCode> {
    crate::attest::build_attest_response(&s.inner.ds, &s.inner.kp).await
        .map(Json).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}
async fn provision_h(State(s): State<AppState>, body: axum::body::Bytes) -> StatusCode {
    match crate::provision::open_provision(&body, &s.inner.kp) {
        Ok(cfg) => { let id = /* read drop_id from the reopened payload */ cfg_drop_id(&cfg); s.inner.catalog.insert(id, cfg, "untitled".into()); StatusCode::OK }
        Err(_) => StatusCode::BAD_REQUEST,
    }
}
async fn catalog_h(State(s): State<AppState>) -> Json<Vec<crate::CatalogEntry>> { Json(s.inner.catalog.public_entries()) }
```
> Note: `open_provision` currently drops `drop_id` (it's not in `DropConfig`). Fix by returning `(u64, DropConfig)` from `open_provision` so `provision_h` knows the id. Update Task 4's signature to `-> anyhow::Result<(u64, DropConfig)>` and its test accordingly. (Caught in self-review — see below.)

- [ ] **Step 4: Run the test** — Expected: PASS.
- [ ] **Step 5: Wire `main.rs`** — read `DSTACK_SOCKET`, `BUCKET_DIR`, `PORT` from env; derive the provisioning seed via `Dstack::get_key("drop/provisioning")`; build `AppState`; `axum::serve`. Also spawn A1's `scan_loop::run_loop` with `&catalog` + `&bucket` (the real impls).
- [ ] **Step 6: Commit** — `git commit -m "feat(drop-indexer): axum server wiring (attest/provision/catalog/bucket)"`

---

### Task 8: Reproducible Docker image + Phala deploy

**Files:** Create `drop-indexer/Dockerfile`; reuse `week5/clean-wallet-mvp/scripts/deploy-cvm.sh` (copy + adjust paths).

- [ ] **Step 1: Write the Dockerfile** — multi-stage, **reproducible**: pin the base image by digest, `cargo build --locked --release`, copy the binary into a minimal runtime. Mount the dstack socket at `/var/run/dstack.sock`.
- [ ] **Step 2: Build twice, compare hashes** — `docker build ... -t drop-indexer:t1` then again `-t drop-indexer:t2`; `docker inspect --format='{{.Id}}'` on both must match. If not, hunt non-determinism (timestamps, build args). Expected: identical image id.
- [ ] **Step 3: docker-compose with the env mapping** (the spike #3 gotcha — sealed envs must be mapped through `environment:`):

```yaml
services:
  indexer:
    image: ${IMAGE}
    restart: unless-stopped
    ports: ["8080:8080"]
    environment:
      DSTACK_SOCKET: "/var/run/dstack.sock"
      BUCKET_DIR: "/data/bucket"
      PORT: "8080"
    volumes:
      - /var/run/dstack.sock:/var/run/dstack.sock
```

- [ ] **Step 4: Deploy** — `phala deploy --name drop-indexer --compose <compose> --instance-type tdx.medium --wait` (follow `week7/drop/spike3/RUNBOOK.md` — you proved this exact flow there).
- [ ] **Step 5: Verify the real quote** — `curl https://<cvm>/attest` → `phala cvms attestation --cvm-id drop-indexer` → verify at proof.t16z.com (Check 1 PASSES on real Phala). Publish the `mr_td` to the public repo README so creators (Lane C) have the measurement to compare against.
- [ ] **Step 6: Commit** — `git commit -m "feat(drop-indexer): reproducible Docker image + Phala deploy"`

---

### Task 9: Re-provisioning / measurement-change continuity (C4)

**Files:** Modify `drop-indexer/src/server.rs`, `drop-indexer/src/provision.rs`.

- [ ] **Step 1: Write the failing test** — provisioning the same `drop_id` twice overwrites (idempotent), so a creator can re-provision to a rebuilt enclave without a duplicate:

```rust
#[tokio::test]
async fn reprovision_same_drop_overwrites() {
    let st = AppState::new_for_test([7u8;32]);
    // provision drop 1 at price 500, then re-provision at 700 → catalog shows 700, one entry
    // (helper seals+posts as in Task 7)
    provision_drop(&st, 1, 500).await; provision_drop(&st, 1, 700).await;
    assert_eq!(st.inner.catalog.public_entries().len(), 1);
    assert_eq!(st.inner.catalog.lookup(1).unwrap().price_zat, 700);
}
```

- [ ] **Step 2: Run it** — Expected: FAIL (or PASS if `insert` already overwrites — then assert it stays one entry and add the doc note).
- [ ] **Step 3: Implement / document.** Ensure `insert` overwrites by `drop_id`. Add a module doc comment to `provision.rs` capturing the **C4 reality** (see `spec.md` changelog C6's sibling C4): *"The provisioning keypair is KMS-derived from the code measurement. Rebuilding the image changes the measurement → changes the keypair → a creator who provisioned to the OLD build can no longer reach this one. Operational rule: when you redeploy with code changes, creators must re-provision (re-`POST /provision`). The public `mr_td` published in Task 8 is how they detect the change."*
- [ ] **Step 4: Run the test** — Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(drop-indexer): idempotent re-provisioning + C4 doc"`

---

## Self-review

**1. Spec coverage** (against `team/lane-A2-enclave-platform.md` + `interfaces.md`):
- I6 `/attest` + report_data binding ✓ Tasks 1,3 — I5 `/provision` secret-IN ✓ Task 4 — I3 catalog (internal + public) ✓ Task 5 — bucket (A1/B/C dependency) ✓ Task 6 — server ✓ Task 7 — reproducible build + Phala deploy ✓ Task 8 — C4 continuity ✓ Task 9. KMS-derived stable key ✓ Task 2.
- **Not in this lane (correctly):** payment detection + dispatch (A1), buyer/creator UIs (B/C). A1's `scan_loop`/`Engine` are *consumed* in Task 7, not built here.

**2. Placeholder scan:** one real gap caught — Task 4's `open_provision` dropped `drop_id`, but Task 7 needs it. **Fix:** change `open_provision` to return `(u64, DropConfig)` and update Task 4's test + Task 7's handler. (Noted inline in Task 7 Step 3.) Implementers: apply that signature from the start.

**3. Type consistency:** `Dstack` (Task 1) → used in `build_attest_response` (Task 3) + `main` (Task 7) ✓. `StackKeyPair` from `provisioning_keypair_from_seed` (Task 2) → `build_attest_response` (3) + `open_provision` (4) ✓. `DropConfig` (Task 0) ← `open_provision` (4) → `CatalogStore` (5) → A1's `Catalog::lookup` ✓. `Bucket` get/list (Task 0) ← `FsBucket` (6) ✓.

**Dependency note:** Tasks 0–6 build + test fully offline (no Phala). Task 1 Step 5 and Task 8 need the dstack simulator / real Phala. Task 7 mounts A1's modules — if A1 isn't ready, stub `scan_loop::run_loop` as a no-op to keep the server testable, and wire it for real at integration.

## Execution handoff

**Plan saved to `week7/drop/plan-a2-enclave-platform.md`. Two execution options:**

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — execute tasks in this session with checkpoints.

Which approach? (Tasks 0–6 are pure-Rust + offline, so they're the fast first batch; 7–9 bring in the simulator/Phala.)
