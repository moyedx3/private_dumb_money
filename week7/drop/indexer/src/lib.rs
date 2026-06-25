//! drop-indexer — Lane A2 (enclave platform).
//!
//! Owns the trust boundary: `/attest` (interface I6), `/provision` secret-IN (I5),
//! the catalog (I3), the bucket, and the HTTP server. Reuses spike #3's verified
//! "encrypt-to-enclave" flow (see week7/drop/spike3/RUNBOOK.md).
//!
//! These boundary types are shared with Lane A1: A1 *mocks* them; A2 provides the
//! real implementations (catalog.rs, bucket.rs).

pub mod dstack;
pub mod attest;
pub mod provision;
pub mod catalog;
pub mod bucket;
pub mod server;

/// Internal drop config (interface I3-b). Read by A1's engine via the `Catalog` trait.
#[derive(Clone)]
pub struct DropConfig {
    pub price_zat: u64,
    pub k_drop: [u8; 32],
    pub creator_ufvk: String,
    pub h_content: String,
    pub deposit_addr: String,
}

/// A1 looks up a drop's config by id.
pub trait Catalog: Send + Sync {
    fn lookup(&self, drop_id: u64) -> Option<DropConfig>;
}

/// Hash-addressed blob storage. A1 writes dispatch blobs; B reads/lists them; C writes content blobs.
#[async_trait::async_trait]
pub trait Bucket: Send + Sync {
    async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()>;
    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>>;
    async fn list(&self) -> anyhow::Result<Vec<String>>;
}

/// Interface I5 — what the creator seals to the enclave. `k_drop` is 32 bytes, hex-encoded
/// because this demo uses JSON for the "CBOR/JSON" payload allowed by interfaces.md.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ProvisionPayload {
    pub drop_id: u64,
    pub price_zat: u64,
    pub k_drop: String,
    pub creator_ufvk: String,
    pub h_content: String,
    pub deposit_addr: String,
}

/// Interface I3-a — public catalog entry the buyer browses. No secrets.
#[derive(serde::Serialize, Clone)]
pub struct CatalogEntry {
    pub drop_id: u64,
    pub price_zec: String,
    pub h_content: String,
    pub title: String,
    pub deposit_addr: String,
}
