//! drop-indexer server binary. Reads config from env, derives the provisioning seed from the
//! dstack KMS (stable per measurement), and serves the A2 routes.

use drop_indexer::bucket::FsBucket;
use drop_indexer::catalog::CatalogStore;
use drop_indexer::dstack::Dstack;
use drop_indexer::lightwalletd::GrpcClient;
use drop_indexer::scan_loop::{run_catalog_loop, RuntimeScanConfig};
use drop_indexer::server::{router, AppState};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let sock = std::env::var("DSTACK_SOCKET").unwrap_or_else(|_| "/var/run/dstack.sock".into());
    let bucket_root = std::path::PathBuf::from(
        std::env::var("BUCKET_DIR").unwrap_or_else(|_| "/data/bucket".into()),
    );
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let ds = Dstack::new(sock);
    // KMS-derived seed, stable per measurement (changes on rebuild — see C4 / Task 9).
    let seed = ds.get_key("drop/provisioning").await?;
    let content = FsBucket::new(bucket_root.join("content"))?;
    let dispatch = FsBucket::new(bucket_root.join("dispatch"))?;
    let catalog = CatalogStore::default();
    maybe_spawn_a1_scanner(catalog.clone(), dispatch.clone());
    let state = AppState::new(ds, seed, catalog, content, dispatch);

    let app = router(state);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    eprintln!("drop-indexer listening on :{port}");
    axum::serve(listener, app).await?;
    Ok(())
}

fn maybe_spawn_a1_scanner(catalog: CatalogStore, dispatch: FsBucket) {
    if !env_bool("A1_SCAN_ENABLE") {
        return;
    }

    let primary =
        std::env::var("LIGHTWALLETD_URL").unwrap_or_else(|_| "https://zec.rocks:443".to_string());
    let backup = std::env::var("LIGHTWALLETD_BACKUP_URL").ok();
    let poll_interval = env_u64("A1_SCAN_POLL_SECS")
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(30));
    let batch_size = env_u64("A1_SCAN_BATCH_BLOCKS").unwrap_or(10).max(1);
    let start_height = env_u64("A1_SCAN_START");

    let cfg = RuntimeScanConfig {
        poll_interval,
        batch_size,
        start_height,
    };
    let client = GrpcClient::new(primary, backup);

    tokio::spawn(async move {
        if let Err(err) = run_catalog_loop(client, catalog, dispatch, cfg).await {
            eprintln!("A1 scanner stopped: {err:?}");
        }
    });
}

fn env_bool(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
    )
}

fn env_u64(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}
