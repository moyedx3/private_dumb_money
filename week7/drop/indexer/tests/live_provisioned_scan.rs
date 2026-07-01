//! Ignored live integration test: A2 `/provision` + A1 live scan + A2 dispatch routes.
//!
//! This test intentionally loads local `.env` values and must not run in CI by default.
//! It proves that an already-mined payment visible to `A1_UFVK` can flow through the
//! provisioned A2 catalog into A1's scanner/engine and become an HTTP-visible dispatch blob.
//!
//! Required local `.env` or environment:
//! - `A1_UFVK`: creator UFVK that can view the payment transaction.
//! - `A1_SCAN_START`: block height containing the payment.
//!
//! Optional:
//! - `A1_SCAN_END`: defaults to `A1_SCAN_START`.
//! - `A1_DEMO_DROP_ID`: defaults to `1`.
//! - `A1_DEMO_PRICE_ZAT`: defaults to `10000`.
//! - `A1_DEMO_K_DROP_HEX`: defaults to `0x09` repeated 32 times.
//! - `A1_LIVE_DEPOSIT_ADDR`: defaults to a syntactically shielded demo string; the scanner
//!   uses the UFVK, not this display address, for this backfilled-payment smoke.
//! - `LIGHTWALLETD_URL`, `LIGHTWALLETD_BACKUP_URL`.

use std::collections::HashMap;
use std::{env, fs};

use anyhow::{Context, Result};
use axum::body::Body;
use axum::http::Request;
use drop_indexer::attest::provisioning_keypair_from_seed;
use drop_indexer::bucket::FsBucket;
use drop_indexer::catalog::CatalogStore;
use drop_indexer::dstack::Dstack;
use drop_indexer::lightwalletd::GrpcClient;
use drop_indexer::provision::seal_to_enclave;
use drop_indexer::scan_loop::{run_catalog_loop, scan_catalog_once, RuntimeScanConfig};
use drop_indexer::server::{router, AppState};
use drop_indexer::ProvisionPayload;
use tower::ServiceExt;

fn load_dotenv() {
    let contents = [".env", "../.env"]
        .into_iter()
        .find_map(|path| fs::read_to_string(path).ok());
    let Some(contents) = contents else { return };
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || env::var_os(key).is_some() {
            continue;
        }
        let value = value.trim().trim_matches('"').trim_matches('\'');
        env::set_var(key, value);
    }
}

fn required_env(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("missing {name}"))
}

fn env_u64(name: &str) -> Result<Option<u64>> {
    match env::var(name) {
        Ok(v) if !v.trim().is_empty() => {
            Ok(Some(v.parse().with_context(|| format!("invalid {name}"))?))
        }
        _ => Ok(None),
    }
}

fn env_hex_32(name: &str) -> Result<Option<[u8; 32]>> {
    let Ok(value) = env::var(name) else {
        return Ok(None);
    };
    let value = value.trim().strip_prefix("0x").unwrap_or(value.trim());
    if value.is_empty() {
        return Ok(None);
    }
    let bytes = hex::decode(value).with_context(|| format!("decode {name}"))?;
    Ok(Some(bytes.as_slice().try_into().map_err(|_| {
        anyhow::anyhow!("{name} must be 32 bytes / 64 hex chars")
    })?))
}

#[tokio::test]
#[ignore = "requires local UFVK and an already-mined memo-bearing payment block"]
async fn provisioned_catalog_live_scan_publishes_dispatch_route() -> Result<()> {
    load_dotenv();

    let ufvk = required_env("A1_UFVK")?;
    let start = required_env("A1_SCAN_START")?
        .parse::<u64>()
        .context("A1_SCAN_START must be a u64")?;
    let end = env_u64("A1_SCAN_END")?.unwrap_or(start);
    let drop_id = env_u64("A1_DEMO_DROP_ID")?.unwrap_or(1);
    let price_zat = env_u64("A1_DEMO_PRICE_ZAT")?.unwrap_or(10_000);
    let k_drop = env_hex_32("A1_DEMO_K_DROP_HEX")?.unwrap_or([9u8; 32]);
    let deposit_addr =
        env::var("A1_LIVE_DEPOSIT_ADDR").unwrap_or_else(|_| "u1liveprovisionedtest".into());
    let endpoint = env::var("LIGHTWALLETD_URL").unwrap_or_else(|_| "https://zec.rocks:443".into());
    let backup = env::var("LIGHTWALLETD_BACKUP_URL").ok();

    let tmp = env::temp_dir().join(format!("drop-live-provisioned-scan-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let catalog = CatalogStore::default();
    let content = FsBucket::new(tmp.join("content"))?;
    let dispatch = FsBucket::new(tmp.join("dispatch"))?;
    let state = AppState::new(
        Dstack::new("/nonexistent.sock"),
        [7u8; 32],
        catalog.clone(),
        content,
        dispatch.clone(),
    );
    let app = router(state);

    let kp = provisioning_keypair_from_seed(&[7u8; 32]);
    let payload = ProvisionPayload {
        drop_id,
        price_zat,
        k_drop: hex::encode(k_drop),
        creator_ufvk: ufvk,
        h_content: "abcdef".into(),
        deposit_addr,
    };
    let sealed = seal_to_enclave(&serde_json::to_vec(&payload)?, &kp.public_key);
    let res = app
        .clone()
        .oneshot(
            Request::post("/provision?title=Live")
                .body(Body::from(sealed))
                .unwrap(),
        )
        .await?;
    assert_eq!(res.status(), 200, "provision should accept sealed payload");

    let client = GrpcClient::new(endpoint, backup);
    let mut states = HashMap::new();
    let summaries = scan_catalog_once(
        &client,
        catalog,
        dispatch,
        &mut states,
        &RuntimeScanConfig {
            poll_interval: std::time::Duration::from_secs(1),
            batch_size: end.saturating_sub(start).saturating_add(1).max(1),
            start_height: Some(start),
        },
    )
    .await?;
    assert!(
        summaries
            .iter()
            .any(|summary| !summary.dispatches.is_empty()),
        "live scan should publish at least one dispatch"
    );

    let res = app
        .clone()
        .oneshot(Request::get("/dispatch").body(Body::empty()).unwrap())
        .await?;
    assert_eq!(res.status(), 200);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await?;
    let keys: Vec<String> = serde_json::from_slice(&body)?;
    assert!(
        !keys.is_empty(),
        "dispatch route should list published keys"
    );

    let key = &keys[0];
    let res = app
        .oneshot(
            Request::get(format!("/dispatch/{key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(res.status(), 200);
    let blob = axum::body::to_bytes(res.into_body(), usize::MAX).await?;
    assert_eq!(
        blob.len(),
        80,
        "dispatch blob must be libsodium sealed-box K_drop"
    );

    eprintln!("live provisioned scan ok: dispatch_key={key} dispatch_len=80");
    Ok(())
}

#[tokio::test]
#[ignore = "requires local UFVK and an already-mined memo-bearing payment block"]
async fn automatic_background_loop_publishes_dispatch_after_provision() -> Result<()> {
    load_dotenv();

    let ufvk = required_env("A1_UFVK")?;
    let start = required_env("A1_SCAN_START")?
        .parse::<u64>()
        .context("A1_SCAN_START must be a u64")?;
    let end = env_u64("A1_SCAN_END")?.unwrap_or(start);
    let drop_id = env_u64("A1_DEMO_DROP_ID")?.unwrap_or(1);
    let price_zat = env_u64("A1_DEMO_PRICE_ZAT")?.unwrap_or(10_000);
    let k_drop = env_hex_32("A1_DEMO_K_DROP_HEX")?.unwrap_or([9u8; 32]);
    let deposit_addr =
        env::var("A1_LIVE_DEPOSIT_ADDR").unwrap_or_else(|_| "u1liveprovisionedtest".into());
    let endpoint = env::var("LIGHTWALLETD_URL").unwrap_or_else(|_| "https://zec.rocks:443".into());
    let backup = env::var("LIGHTWALLETD_BACKUP_URL").ok();

    let tmp = env::temp_dir().join(format!(
        "drop-live-auto-provisioned-scan-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&tmp);
    let catalog = CatalogStore::default();
    let content = FsBucket::new(tmp.join("content"))?;
    let dispatch = FsBucket::new(tmp.join("dispatch"))?;
    let state = AppState::new(
        Dstack::new("/nonexistent.sock"),
        [7u8; 32],
        catalog.clone(),
        content,
        dispatch.clone(),
    );
    let app = router(state);

    let client = GrpcClient::new(endpoint, backup);
    let loop_handle = tokio::spawn(run_catalog_loop(
        client,
        catalog,
        dispatch,
        RuntimeScanConfig {
            poll_interval: std::time::Duration::from_millis(500),
            batch_size: end.saturating_sub(start).saturating_add(1).max(1),
            start_height: Some(start),
        },
    ));

    let kp = provisioning_keypair_from_seed(&[7u8; 32]);
    let payload = ProvisionPayload {
        drop_id,
        price_zat,
        k_drop: hex::encode(k_drop),
        creator_ufvk: ufvk,
        h_content: "abcdef".into(),
        deposit_addr,
    };
    let sealed = seal_to_enclave(&serde_json::to_vec(&payload)?, &kp.public_key);
    let res = app
        .clone()
        .oneshot(
            Request::post("/provision?title=Live")
                .body(Body::from(sealed))
                .unwrap(),
        )
        .await?;
    assert_eq!(res.status(), 200, "provision should accept sealed payload");

    let mut keys = Vec::<String>::new();
    for _ in 0..20 {
        let res = app
            .clone()
            .oneshot(Request::get("/dispatch").body(Body::empty()).unwrap())
            .await?;
        assert_eq!(res.status(), 200);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await?;
        keys = serde_json::from_slice(&body)?;
        if !keys.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    loop_handle.abort();

    assert!(
        !keys.is_empty(),
        "automatic scanner loop should publish a dispatch after provision"
    );
    let key = &keys[0];
    let res = app
        .oneshot(
            Request::get(format!("/dispatch/{key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(res.status(), 200);
    let blob = axum::body::to_bytes(res.into_body(), usize::MAX).await?;
    assert_eq!(blob.len(), 80);

    eprintln!("live automatic scan ok: dispatch_key={key} dispatch_len=80");
    Ok(())
}
