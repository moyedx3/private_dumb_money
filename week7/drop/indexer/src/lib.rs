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
#[derive(Clone, PartialEq, Eq)]
pub struct DropConfig {
    pub price_zat: u64,
    pub k_drop: [u8; 32],
    pub creator_ufvk: String,
    pub h_content: String,
    pub deposit_addr: String,
}

// Hand-written Debug so a stray `{:?}`/trace line cannot leak the content master key
// (`k_drop`) or the creator viewing key (`creator_ufvk`, also the N2 ownership secret) into
// host-visible logs. Non-secret fields stay visible for diagnostics.
impl std::fmt::Debug for DropConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DropConfig")
            .field("price_zat", &self.price_zat)
            .field("k_drop", &"[redacted]")
            .field("creator_ufvk", &"[redacted]")
            .field("h_content", &self.h_content)
            .field("deposit_addr", &self.deposit_addr)
            .finish()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropconfig_debug_redacts_secrets() {
        let cfg = DropConfig {
            price_zat: 500,
            k_drop: [0xAB; 32],
            creator_ufvk: "uview1secret".into(),
            h_content: "h1".into(),
            deposit_addr: "u1demo".into(),
        };
        let s = format!("{cfg:?}");

        // secrets must NOT appear
        assert!(s.contains("redacted"), "Debug must redact secret fields: {s}");
        assert!(!s.contains("uview1secret"), "creator_ufvk leaked into Debug: {s}");
        // NOTE: [u8; 32]'s derived Debug prints DECIMAL ("[171, 171, ...]", 0xAB = 171),
        // never hex "ababab" — check the decimal form, or a vulnerable impl passes vacuously.
        assert!(!s.contains("171, 171"), "k_drop bytes leaked into Debug: {s}");

        // non-secret fields stay visible for diagnostics
        assert!(s.contains("500"), "price_zat should remain visible: {s}");
        assert!(s.contains("u1demo"), "deposit_addr should remain visible: {s}");
    }
}
