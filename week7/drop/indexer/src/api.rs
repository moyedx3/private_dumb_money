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
            path: "/api/buyers/dispatch/{bucket_key}",
            purpose: "Return the sealed dispatch blob for a buyer-held bucket key.",
        },
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterCreatorDropRequest {
    pub creator_id: String,
    pub creator_ufvk: String,
    pub price_zat: u64,
    pub k_drop: [u8; 32],
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
pub struct DispatchBlobRecord {
    pub bucket_key: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatchLookupResponse {
    pub bucket_key: String,
    pub bytes: Vec<u8>,
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
            },
            active: true,
        });

        Ok(RegisterCreatorDropResponse {
            drop_id,
            creator_id: req.creator_id,
        })
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
}

fn validate_registration(req: &RegisterCreatorDropRequest) -> Result<()> {
    if req.creator_id.trim().is_empty() {
        return Err(anyhow!("creator_id is required"));
    }
    if req.creator_ufvk.trim().is_empty() {
        return Err(anyhow!("creator_ufvk is required"));
    }
    if req.price_zat == 0 {
        return Err(anyhow!("price_zat must be greater than zero"));
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
            price_zat: 10_000,
            k_drop: [9u8; 32],
        }
    }

    #[test]
    fn endpoint_vector_lists_creator_and_buyer_routes() {
        let endpoints = endpoint_vector();
        assert_eq!(endpoints.len(), 2);
        assert!(endpoints
            .iter()
            .any(|e| e.method == "POST" && e.path == "/api/creators/{creator_id}/drops"));
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
        assert_eq!(api.creator_drops().len(), 1);
    }

    #[test]
    fn rejects_invalid_creator_registration() {
        let api = ApiVectors::new();
        let mut req = registration();
        req.price_zat = 0;
        assert!(api.register_creator_drop(req).is_err());
    }

    #[tokio::test]
    async fn engine_dispatch_can_be_looked_up_by_buyer_bucket_key() {
        let api = ApiVectors::new();
        let drop = api.register_creator_drop(registration()).unwrap();
        let buyer = StackKeyPair::gen();
        let e_pub = *buyer.public_key.as_array();

        let mut engine = Engine::new(api.clone(), api.clone());
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
        assert_eq!(api.dispatch_blobs().len(), 1);
    }

    #[test]
    fn missing_dispatch_returns_none() {
        let api = ApiVectors::new();
        assert!(api.lookup_dispatch("missing").is_none());
    }
}
