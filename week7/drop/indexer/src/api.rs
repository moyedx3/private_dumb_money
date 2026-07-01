//! API-facing vectors for creator provisioning and buyer dispatch lookup.
//!
//! This module intentionally avoids choosing an HTTP framework. The structs and
//! handlers are the service layer that an outer REST/gRPC/enclave-RPC endpoint
//! can call. In production, plaintext creator secrets must enter through an
//! attested enclave ingress; this in-memory store is a development/test adapter.

use crate::{Bucket, Catalog, DropConfig};
use anyhow::{anyhow, Result};
use std::sync::{Arc, Mutex};

pub const CREATOR_DROP_REGISTER_ENDPOINT: &str = "POST /api/creators/{creator_id}/drops";
pub const PUBLIC_CATALOG_ENDPOINT: &str = "GET /api/catalog";
pub const BUYER_DISPATCH_LIST_ENDPOINT: &str = "GET /api/buyers/dispatch";
pub const BUYER_DISPATCH_LOOKUP_ENDPOINT: &str = "GET /api/buyers/dispatch/{bucket_key}";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiEndpoint {
    pub method: &'static str,
    pub path: &'static str,
    pub purpose: &'static str,
}

/// The current external API vector. These are route contracts, not a bound HTTP
/// implementation.
pub fn endpoint_vector() -> Vec<ApiEndpoint> {
    vec![
        ApiEndpoint {
            method: "POST",
            path: "/api/creators/{creator_id}/drops",
            purpose: "Register a creator drop/catalog record inside the enclave boundary.",
        },
        ApiEndpoint {
            method: "GET",
            path: "/api/catalog",
            purpose: "Return public buyer catalog entries, including shielded deposit addresses but no secrets.",
        },
        ApiEndpoint {
            method: "GET",
            path: "/api/buyers/dispatch",
            purpose: "List dispatch blob keys only, so buyers can trial-open dispatches without scanning content blobs.",
        },
        ApiEndpoint {
            method: "GET",
            path: "/api/buyers/dispatch/{bucket_key}",
            purpose: "Return one sealed dispatch blob for a key discovered through the dispatch list.",
        },
    ]
}

#[derive(Clone, PartialEq, Eq)]
pub struct RegisterCreatorDropRequest {
    pub creator_id: String,
    pub creator_ufvk: String,
    pub deposit_addr: String,
    pub price_zat: u64,
    pub k_drop: [u8; 32],
    pub h_content: String,
}

// Hand-written Debug so a stray {:?}/trace on this request cannot leak the content master
// key (k_drop) or the creator viewing key (creator_ufvk) into host-visible logs. Mirrors the
// DropConfig redaction (Task 2). ApiVectors is a dev/test adapter, but the leak class is closed.
impl std::fmt::Debug for RegisterCreatorDropRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisterCreatorDropRequest")
            .field("creator_id", &self.creator_id)
            .field("creator_ufvk", &"[redacted]")
            .field("deposit_addr", &self.deposit_addr)
            .field("price_zat", &self.price_zat)
            .field("k_drop", &"[redacted]")
            .field("h_content", &self.h_content)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterCreatorDropResponse {
    pub drop_id: u64,
    pub creator_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatorDropRecord {
    pub drop_id: u64,
    pub creator_id: String,
    pub config: DropConfig,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicCatalogEntry {
    pub drop_id: u64,
    pub creator_id: String,
    pub price_zat: u64,
    pub h_content: String,
    pub deposit_addr: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatchBlobRecord {
    pub bucket_key: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatchLookupResponse {
    pub bucket_key: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatchListResponse {
    /// Dispatch blob keys only. Content blob keys are intentionally excluded so
    /// buyer polling never downloads large encrypted content while looking for
    /// its 80-byte dispatch blob.
    pub bucket_keys: Vec<String>,
}

#[derive(Debug)]
struct ApiVectorsInner {
    next_drop_id: u64,
    drops: Vec<CreatorDropRecord>,
    dispatches: Vec<DispatchBlobRecord>,
}

/// In-memory API vector store.
///
/// It implements both existing A1 boundaries:
/// - [`Catalog`] for scanner price/key lookup.
/// - [`Bucket`] for dispatch publication.
///
/// The buyer lookup endpoint reads the same dispatch vector written by
/// `Bucket::put`.
#[derive(Clone, Debug)]
pub struct ApiVectors {
    inner: Arc<Mutex<ApiVectorsInner>>,
}

impl Default for ApiVectors {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiVectors {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ApiVectorsInner {
                next_drop_id: 1,
                drops: vec![],
                dispatches: vec![],
            })),
        }
    }

    /// Endpoint handler for `POST /api/creators/{creator_id}/drops`.
    pub fn register_creator_drop(
        &self,
        req: RegisterCreatorDropRequest,
    ) -> Result<RegisterCreatorDropResponse> {
        validate_registration(&req)?;

        let mut inner = self.inner.lock().unwrap();
        let drop_id = inner.next_drop_id;
        inner.next_drop_id = inner
            .next_drop_id
            .checked_add(1)
            .ok_or_else(|| anyhow!("drop id counter overflow"))?;

        inner.drops.push(CreatorDropRecord {
            drop_id,
            creator_id: req.creator_id.clone(),
            config: DropConfig {
                price_zat: req.price_zat,
                k_drop: req.k_drop,
                creator_ufvk: req.creator_ufvk,
                h_content: req.h_content,
                deposit_addr: req.deposit_addr,
            },
            active: true,
        });

        Ok(RegisterCreatorDropResponse {
            drop_id,
            creator_id: req.creator_id,
        })
    }

    /// Endpoint handler for `GET /api/buyers/dispatch`.
    ///
    /// This is the buyer discovery surface requested by Lane B: buyers fetch
    /// dispatch keys only, then `GET` individual 80-byte dispatch blobs and
    /// trial-open them locally with their `e_priv`.
    pub fn list_dispatch_keys(&self) -> DispatchListResponse {
        let inner = self.inner.lock().unwrap();
        DispatchListResponse {
            bucket_keys: inner
                .dispatches
                .iter()
                .map(|record| record.bucket_key.clone())
                .collect(),
        }
    }

    /// Endpoint handler for `GET /api/buyers/dispatch/{bucket_key}`.
    pub fn lookup_dispatch(&self, bucket_key: &str) -> Option<DispatchLookupResponse> {
        let inner = self.inner.lock().unwrap();
        inner
            .dispatches
            .iter()
            .find(|record| record.bucket_key == bucket_key)
            .map(|record| DispatchLookupResponse {
                bucket_key: record.bucket_key.clone(),
                bytes: record.bytes.clone(),
            })
    }

    pub fn creator_drops(&self) -> Vec<CreatorDropRecord> {
        self.inner.lock().unwrap().drops.clone()
    }

    /// Endpoint handler for `GET /api/catalog`.
    ///
    /// Public catalog entries contain buyer payment data only. Secrets such as
    /// `creator_ufvk` and `k_drop` remain inside the catalog boundary.
    pub fn list_public_catalog(&self) -> Vec<PublicCatalogEntry> {
        self.inner
            .lock()
            .unwrap()
            .drops
            .iter()
            .filter(|record| record.active)
            .map(|record| PublicCatalogEntry {
                drop_id: record.drop_id,
                creator_id: record.creator_id.clone(),
                price_zat: record.config.price_zat,
                h_content: record.config.h_content.clone(),
                deposit_addr: record.config.deposit_addr.clone(),
            })
            .collect()
    }

    pub fn dispatch_blobs(&self) -> Vec<DispatchBlobRecord> {
        self.inner.lock().unwrap().dispatches.clone()
    }
}

impl Catalog for ApiVectors {
    fn lookup(&self, drop_id: u64) -> Option<DropConfig> {
        self.inner
            .lock()
            .unwrap()
            .drops
            .iter()
            .find(|record| record.drop_id == drop_id && record.active)
            .map(|record| record.config.clone())
    }
}

#[async_trait::async_trait]
impl Bucket for ApiVectors {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(existing) = inner
            .dispatches
            .iter_mut()
            .find(|record| record.bucket_key == key)
        {
            existing.bytes = bytes.to_vec();
        } else {
            inner.dispatches.push(DispatchBlobRecord {
                bucket_key: key.to_string(),
                bytes: bytes.to_vec(),
            });
        }
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.lookup_dispatch(key).map(|record| record.bytes))
    }

    async fn list(&self) -> Result<Vec<String>> {
        Ok(self.list_dispatch_keys().bucket_keys)
    }
}

fn validate_registration(req: &RegisterCreatorDropRequest) -> Result<()> {
    if req.creator_id.trim().is_empty() {
        return Err(anyhow!("creator_id is required"));
    }
    if req.creator_ufvk.trim().is_empty() {
        return Err(anyhow!("creator_ufvk is required"));
    }
    validate_deposit_addr(&req.deposit_addr)?;
    if req.price_zat == 0 {
        return Err(anyhow!("price_zat must be greater than zero"));
    }
    if req.h_content.trim().is_empty() {
        return Err(anyhow!("h_content is required"));
    }
    Ok(())
}

fn validate_deposit_addr(addr: &str) -> Result<()> {
    let addr = addr.trim();
    if addr.is_empty() {
        return Err(anyhow!("deposit_addr is required"));
    }
    if addr.starts_with("t1") || addr.starts_with("t3") {
        return Err(anyhow!(
            "deposit_addr must be shielded; transparent t-addresses cannot carry shielded memos"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::DISPATCH_BLOB_LEN;
    use crate::engine::{Engine, Note};
    use dryoc::keypair::StackKeyPair;
    use dryoc::types::ByteArray;

    fn registration() -> RegisterCreatorDropRequest {
        RegisterCreatorDropRequest {
            creator_id: "creator-1".to_string(),
            creator_ufvk: "uview1test".to_string(),
            deposit_addr: "u1testshieldeddeposit".to_string(),
            price_zat: 10_000,
            k_drop: [9u8; 32],
            h_content: "abc123".to_string(),
        }
    }

    #[test]
    fn endpoint_vector_lists_creator_and_buyer_routes() {
        let endpoints = endpoint_vector();
        assert_eq!(endpoints.len(), 4);
        assert!(endpoints
            .iter()
            .any(|e| e.method == "POST" && e.path == "/api/creators/{creator_id}/drops"));
        assert!(endpoints
            .iter()
            .any(|e| e.method == "GET" && e.path == "/api/catalog"));
        assert!(endpoints
            .iter()
            .any(|e| e.method == "GET" && e.path == "/api/buyers/dispatch"));
        assert!(endpoints
            .iter()
            .any(|e| e.method == "GET" && e.path == "/api/buyers/dispatch/{bucket_key}"));
    }

    #[test]
    fn creator_registration_populates_catalog_lookup() {
        let api = ApiVectors::new();
        let response = api.register_creator_drop(registration()).unwrap();

        assert_eq!(response.drop_id, 1);
        assert_eq!(response.creator_id, "creator-1");
        let config = api.lookup(response.drop_id).unwrap();
        assert_eq!(config.price_zat, 10_000);
        assert_eq!(config.k_drop, [9u8; 32]);
        assert_eq!(config.deposit_addr, "u1testshieldeddeposit");
        assert_eq!(api.creator_drops().len(), 1);
        assert_eq!(
            api.list_public_catalog(),
            vec![PublicCatalogEntry {
                drop_id: 1,
                creator_id: "creator-1".to_string(),
                price_zat: 10_000,
                h_content: "abc123".to_string(),
                deposit_addr: "u1testshieldeddeposit".to_string(),
            }]
        );
    }

    #[test]
    fn rejects_invalid_creator_registration() {
        let api = ApiVectors::new();
        let mut req = registration();
        req.price_zat = 0;
        assert!(api.register_creator_drop(req).is_err());
    }

    #[test]
    fn rejects_transparent_deposit_address_for_memo_payments() {
        let api = ApiVectors::new();
        for transparent in ["t1abc", "t3abc"] {
            let mut req = registration();
            req.deposit_addr = transparent.to_string();
            let err = api.register_creator_drop(req).unwrap_err().to_string();
            assert!(err.contains("shielded"));
        }
    }

    #[tokio::test]
    async fn engine_dispatch_can_be_looked_up_by_buyer_bucket_key() {
        let api = ApiVectors::new();
        let drop = api.register_creator_drop(registration()).unwrap();
        let buyer = StackKeyPair::gen();
        let e_pub = *buyer.public_key.as_array();

        let mut engine = Engine::new(api.clone(), api.clone(), "uview1test");
        let dispatch = engine
            .on_note(&Note {
                drop_id: drop.drop_id,
                e_pub,
                value_zat: 10_000,
                txid: [7u8; 32],
            })
            .await
            .unwrap()
            .unwrap();

        let found = api.lookup_dispatch(&dispatch.bucket_key).unwrap();
        assert_eq!(found.bucket_key, dispatch.bucket_key);
        assert_eq!(found.bytes.len(), DISPATCH_BLOB_LEN);
        assert_eq!(
            api.list_dispatch_keys(),
            DispatchListResponse {
                bucket_keys: vec![dispatch.bucket_key],
            }
        );
        assert_eq!(api.dispatch_blobs().len(), 1);
    }

    #[tokio::test]
    async fn dispatch_list_exposes_keys_without_blob_bytes_or_content() {
        let api = ApiVectors::new();

        api.put("deadbeef", &[1u8; DISPATCH_BLOB_LEN])
            .await
            .unwrap();
        api.put("cafebabe", &[2u8; DISPATCH_BLOB_LEN])
            .await
            .unwrap();

        let list = api.list_dispatch_keys();

        assert_eq!(
            list,
            DispatchListResponse {
                bucket_keys: vec!["deadbeef".to_string(), "cafebabe".to_string()],
            }
        );
        // The public discovery response contains keys only; clients fetch and
        // trial-open individual dispatch blobs through lookup_dispatch.
        assert_eq!(list.bucket_keys.len(), api.dispatch_blobs().len());
        assert!(api.lookup_dispatch("deadbeef").is_some());
    }

    #[test]
    fn missing_dispatch_returns_none() {
        let api = ApiVectors::new();
        assert!(api.lookup_dispatch("missing").is_none());
    }

    #[test]
    fn register_request_debug_redacts_secrets() {
        let req = RegisterCreatorDropRequest {
            creator_id: "creator-1".into(),
            creator_ufvk: "uview1secret".into(),
            deposit_addr: "u1demo".into(),
            price_zat: 500,
            k_drop: [0xAB; 32],
            h_content: "h1".into(),
        };
        let s = format!("{req:?}");

        // secrets must NOT appear
        assert!(s.contains("redacted"), "Debug must redact secret fields: {s}");
        assert!(!s.contains("uview1secret"), "creator_ufvk leaked into Debug: {s}");
        // [u8; 32] derived Debug prints DECIMAL ("[171, 171, ...]", 0xAB = 171), never hex.
        assert!(!s.contains("171, 171"), "k_drop bytes leaked into Debug: {s}");

        // non-secret fields stay visible for diagnostics
        assert!(s.contains("creator-1"), "creator_id should remain visible: {s}");
        assert!(s.contains("u1demo"), "deposit_addr should remain visible: {s}");
    }
}
