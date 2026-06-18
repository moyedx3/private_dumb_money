//! HTTP server (axum) — wires A2's routes: /health, /attest (I6), /provision (I5),
//! /catalog (I3-a), /bucket/:key. Lane A1's scan loop is mounted in main.rs once it lands.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use dryoc::keypair::StackKeyPair;
use serde::Deserialize;

use crate::attest::{build_attest_response, provisioning_keypair_from_seed, AttestResponse};
use crate::bucket::FsBucket;
use crate::catalog::CatalogStore;
use crate::dstack::Dstack;
use crate::provision::open_provision;
use crate::{Bucket, CatalogEntry};

pub struct Inner {
    pub ds: Dstack,
    pub kp: StackKeyPair,
    pub catalog: CatalogStore,
    pub bucket: FsBucket,
}

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<Inner>,
}

impl AppState {
    pub fn new(ds: Dstack, provisioning_seed: [u8; 32], catalog: CatalogStore, bucket: FsBucket) -> Self {
        Self {
            inner: Arc::new(Inner {
                ds,
                kp: provisioning_keypair_from_seed(&provisioning_seed),
                catalog,
                bucket,
            }),
        }
    }
}

pub fn router(st: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/attest", get(attest_h))
        .route("/provision", post(provision_h))
        .route("/catalog", get(catalog_h))
        .route("/bucket/:key", get(bucket_get_h).put(bucket_put_h))
        .with_state(st)
}

async fn attest_h(State(s): State<AppState>) -> Result<Json<AttestResponse>, StatusCode> {
    build_attest_response(&s.inner.ds, &s.inner.kp)
        .await
        .map(Json)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}

#[derive(Deserialize)]
struct ProvisionQuery {
    title: Option<String>,
}

async fn provision_h(State(s): State<AppState>, Query(q): Query<ProvisionQuery>, body: Bytes) -> StatusCode {
    match open_provision(&body, &s.inner.kp) {
        Ok((drop_id, cfg)) => {
            s.inner.catalog.insert(drop_id, cfg, q.title.unwrap_or_else(|| "untitled".into()));
            StatusCode::OK
        }
        Err(_) => StatusCode::BAD_REQUEST,
    }
}

async fn catalog_h(State(s): State<AppState>) -> Json<Vec<CatalogEntry>> {
    Json(s.inner.catalog.public_entries())
}

async fn bucket_get_h(State(s): State<AppState>, Path(key): Path<String>) -> Result<Vec<u8>, StatusCode> {
    match s.inner.bucket.get(&key).await {
        Ok(Some(b)) => Ok(b),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn bucket_put_h(State(s): State<AppState>, Path(key): Path<String>, body: Bytes) -> StatusCode {
    match s.inner.bucket.put(&key, &body).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provision::seal_to_enclave;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_state(seed: [u8; 32]) -> AppState {
        let dir = std::env::temp_dir().join(format!("drop-srv-test-{}", seed[0]));
        let _ = std::fs::remove_dir_all(&dir);
        AppState::new(
            crate::dstack::Dstack::new("/nonexistent.sock"), // /attest not exercised in this test
            seed,
            crate::catalog::CatalogStore::default(),
            crate::bucket::FsBucket::new(&dir).unwrap(),
        )
    }

    #[tokio::test]
    async fn provision_over_http_then_catalog_lists_it() {
        let app = router(test_state([7u8; 32]));

        // creator side: seal a drop to the enclave's provisioning pubkey (seed 7)
        let kp = crate::attest::provisioning_keypair_from_seed(&[7u8; 32]);
        let payload = crate::ProvisionPayload {
            drop_id: 1,
            price_zat: 500,
            k_drop_hex: hex::encode([2u8; 32]),
            creator_ufvk: "uview1secret".into(),
            h_content: "h1".into(),
        };
        let sealed = seal_to_enclave(&serde_json::to_vec(&payload).unwrap(), &kp.public_key);

        let res = app
            .clone()
            .oneshot(Request::post("/provision?title=Cat").body(Body::from(sealed)).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), 200);

        let res2 = app
            .oneshot(Request::get("/catalog").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res2.status(), 200);
        let body = axum::body::to_bytes(res2.into_body(), usize::MAX).await.unwrap();
        let s = String::from_utf8(body.to_vec()).unwrap();
        assert!(s.contains("\"drop_id\":1"));
        assert!(s.contains("Cat"));
        assert!(!s.contains("uview1secret")); // secrets stay internal
    }

    #[tokio::test]
    #[ignore]
    async fn live_attest_route_returns_quote() {
        let sock = std::env::var("DSTACK_SOCKET").expect("set DSTACK_SOCKET");
        let dir = std::env::temp_dir().join("drop-srv-live");
        let _ = std::fs::remove_dir_all(&dir);
        let st = AppState::new(
            crate::dstack::Dstack::new(sock),
            [9u8; 32],
            crate::catalog::CatalogStore::default(),
            crate::bucket::FsBucket::new(&dir).unwrap(),
        );
        let res = router(st)
            .oneshot(Request::get("/attest").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let s = String::from_utf8(body.to_vec()).unwrap();
        assert!(s.contains("quote_hex"));
        assert!(s.contains("provisioning_pubkey_hex"));
    }
}
