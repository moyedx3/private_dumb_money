//! HTTP server (axum) — wires A2's routes: /health, /attest (I6), /provision (I5),
//! /catalog (I3-a), /dispatch (R-A2-1), /dispatch/:key (R-A2-3), /bucket/:key.
//! Lane A1's scan loop is mounted in main.rs once it lands.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use dryoc::keypair::StackKeyPair;
use serde::Deserialize;
use tower_http::cors::CorsLayer;

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
    pub content: FsBucket,
    pub dispatch: FsBucket,
}

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<Inner>,
}

impl AppState {
    pub fn new(
        ds: Dstack,
        provisioning_seed: [u8; 32],
        catalog: CatalogStore,
        content: FsBucket,
        dispatch: FsBucket,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                ds,
                kp: provisioning_keypair_from_seed(&provisioning_seed),
                catalog,
                content,
                dispatch,
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
        .route("/dispatch", get(dispatch_list_h))
        .route("/dispatch/:key", get(dispatch_get_h))
        .route("/bucket/:key", get(bucket_get_h).put(bucket_put_h))
        .with_state(st)
        // Lane B/C are browser clients calling cross-origin. Permissive for the demo;
        // tighten to the known web origins for production.
        .layer(CorsLayer::permissive())
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
    match s.inner.content.get(&key).await {
        Ok(Some(b)) => Ok(b),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn bucket_put_h(State(s): State<AppState>, Path(key): Path<String>, body: Bytes) -> StatusCode {
    match s.inner.content.put(&key, &body).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn dispatch_list_h(State(s): State<AppState>) -> Result<Json<Vec<String>>, StatusCode> {
    match s.inner.dispatch.list().await {
        Ok(keys) => Ok(Json(keys)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn dispatch_get_h(State(s): State<AppState>, Path(key): Path<String>) -> Result<Vec<u8>, StatusCode> {
    match s.inner.dispatch.get(&key).await {
        Ok(Some(b)) => Ok(b),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
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
        let base = std::env::temp_dir().join(format!("drop-srv-test-{}", seed[0]));
        let _ = std::fs::remove_dir_all(&base);
        AppState::new(
            crate::dstack::Dstack::new("/nonexistent.sock"), // /attest not exercised in this test
            seed,
            crate::catalog::CatalogStore::default(),
            crate::bucket::FsBucket::new(base.join("content")).unwrap(),
            crate::bucket::FsBucket::new(base.join("dispatch")).unwrap(),
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
            k_drop: hex::encode([2u8; 32]),
            creator_ufvk: "uview1secret".into(),
            h_content: "h1".into(),
            deposit_addr: "u1demo".into(),
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
        assert!(s.contains("\"price_zec\":\"0.000005\""));
        assert!(s.contains("Cat"));
        assert!(!s.contains("uview1secret")); // secrets stay internal
        assert!(s.contains("u1demo")); // deposit_addr surfaces in the public catalog (R-A2-2)
    }

    #[tokio::test]
    async fn reprovision_same_drop_overwrites() {
        // After a rebuild (new keypair), a creator re-provisions; the catalog must overwrite
        // by drop_id, not duplicate (C4). Here: same drop_id, updated price.
        let app = router(test_state([7u8; 32]));
        let kp = crate::attest::provisioning_keypair_from_seed(&[7u8; 32]);
        let seal_price = |price: u64| {
            let payload = crate::ProvisionPayload {
                drop_id: 1,
                price_zat: price,
                k_drop: hex::encode([2u8; 32]),
                creator_ufvk: "uview1x".into(),
                h_content: "h1".into(),
                deposit_addr: "u1demo".into(),
            };
            seal_to_enclave(&serde_json::to_vec(&payload).unwrap(), &kp.public_key)
        };
        for price in [500u64, 700] {
            let res = app
                .clone()
                .oneshot(Request::post("/provision?title=Cat").body(Body::from(seal_price(price))).unwrap())
                .await
                .unwrap();
            assert_eq!(res.status(), 200);
        }
        let res = app.oneshot(Request::get("/catalog").body(Body::empty()).unwrap()).await.unwrap();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let s = String::from_utf8(body.to_vec()).unwrap();
        assert!(s.contains("\"price_zec\":\"0.000007\"")); // latest wins
        assert_eq!(s.matches("\"drop_id\":1").count(), 1); // exactly one entry
    }

    #[tokio::test]
    async fn cross_origin_request_gets_cors_header() {
        // Lane B (buyer) and C (creator) are browser apps that call this API cross-origin;
        // without an Access-Control-Allow-Origin header the browser blocks every call.
        let app = router(test_state([1u8; 32]));
        let res = app
            .oneshot(
                Request::get("/health")
                    .header("origin", "https://buyer.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            res.headers().contains_key("access-control-allow-origin"),
            "browser cross-origin calls (Lane B/C) need a CORS header"
        );
    }

    #[tokio::test]
    async fn dispatch_list_returns_only_dispatch_keys_not_content() {
        // A1 writes a dispatch blob; C writes a content blob. /dispatch must list only the
        // dispatch key so the buyer never trial-downloads multi-MB content blobs (R-A2-3).
        let st = test_state([3u8; 32]);
        st.inner.dispatch.put("aa", b"dispatch-blob").await.unwrap();
        st.inner.content.put("bb", b"content-blob").await.unwrap();
        let app = router(st);

        let res = app
            .oneshot(Request::get("/dispatch").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let keys: Vec<String> = serde_json::from_slice(&body).unwrap();
        assert_eq!(keys, vec!["aa".to_string()]); // content key "bb" must NOT appear
    }

    #[tokio::test]
    async fn dispatch_get_serves_blob_content_route_stays_separate() {
        let st = test_state([4u8; 32]);
        st.inner.dispatch.put("ab", b"dblob").await.unwrap();
        st.inner.content.put("cd", b"cblob").await.unwrap();
        let app = router(st);

        let d = app
            .clone()
            .oneshot(Request::get("/dispatch/ab").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(d.status(), 200);
        let db = axum::body::to_bytes(d.into_body(), usize::MAX).await.unwrap();
        assert_eq!(db.as_ref(), b"dblob");

        // a content key is not reachable through the dispatch store
        let miss = app
            .clone()
            .oneshot(Request::get("/dispatch/cd").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(miss.status(), 404);

        // content is still served on /bucket/:key
        let c = app
            .oneshot(Request::get("/bucket/cd").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(c.status(), 200);
        let cb = axum::body::to_bytes(c.into_body(), usize::MAX).await.unwrap();
        assert_eq!(cb.as_ref(), b"cblob");
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
            crate::bucket::FsBucket::new(dir.join("content")).unwrap(),
            crate::bucket::FsBucket::new(dir.join("dispatch")).unwrap(),
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
