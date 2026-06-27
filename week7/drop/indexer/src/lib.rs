//! drop-indexer — Lane A2 enclave platform plus Lane A1 payment scanner.
//!
//! A2 owns the trust boundary and HTTP/runtime surfaces: `/attest`, `/provision`,
//! the catalog, content bucket, dispatch bucket, and buyer/creator routes.
//! A1 owns payment detection: UFVK/lightwalletd scanning, memo decoding, payment
//! validation, and dispatch blob production. The shared `Catalog` and `Bucket`
//! traits are the seam between those roles.

pub mod api;
pub mod attest;
pub mod bucket;
pub mod catalog;
pub mod detect;
pub mod dispatch;
pub mod dstack;
pub mod engine;
pub mod lightwalletd;
pub mod memo;
pub mod provision;
pub mod scan_loop;
pub mod server;
pub mod state;
pub mod zecscope_adapter;

/// Internal drop config (interface I3-b). A2 stores it after provisioning;
/// A1 reads it through [`Catalog`] while scanning payments.
#[derive(Clone, Debug, PartialEq, Eq)]
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

/// Hash-addressed blob storage. A1 writes dispatch blobs; B reads/lists
/// dispatch blobs; C writes content blobs.
#[async_trait::async_trait]
pub trait Bucket: Send + Sync {
    async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()>;
    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>>;
    async fn list(&self) -> anyhow::Result<Vec<String>>;
}

/// Interface I5 — what the creator seals to the enclave. `k_drop` is 32 bytes,
/// hex-encoded because this demo uses JSON for the "CBOR/JSON" payload allowed
/// by interfaces.md.
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
