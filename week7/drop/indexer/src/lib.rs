pub mod api;
pub mod detect;
pub mod dispatch;
pub mod engine;
pub mod lightwalletd;
pub mod memo;
pub mod scan_loop;
pub mod state;
pub mod zecscope_adapter;

/// Drop configuration owned by Lane A2/catalog provisioning.
///
/// A1 treats this as a mockable boundary: the payment engine only needs the
/// expected price, the content key to dispatch, and the creator viewing key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropConfig {
    pub price_zat: u64,
    pub k_drop: [u8; 32],
    pub creator_ufvk: String,
    pub deposit_addr: String,
}

/// Catalog boundary owned by Lane A2.
///
/// Implementations may be in-memory for tests/demo or backed by the TEE
/// provisioning path later.
pub trait Catalog: Send + Sync {
    fn lookup(&self, drop_id: u64) -> Option<DropConfig>;
}

/// Dispatch blob storage boundary owned by Lane D.
#[async_trait::async_trait]
pub trait Bucket: Send + Sync {
    async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()>;
}
