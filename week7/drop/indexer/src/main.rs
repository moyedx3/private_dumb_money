//! drop-indexer server binary. Reads config from env, derives the provisioning seed from the
//! dstack KMS (stable per measurement), and serves the A2 routes.

use drop_indexer::bucket::FsBucket;
use drop_indexer::catalog::CatalogStore;
use drop_indexer::dstack::Dstack;
use drop_indexer::server::{router, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let sock = std::env::var("DSTACK_SOCKET").unwrap_or_else(|_| "/var/run/dstack.sock".into());
    let bucket_dir = std::env::var("BUCKET_DIR").unwrap_or_else(|_| "/data/bucket".into());
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);

    let ds = Dstack::new(sock);
    // KMS-derived seed, stable per measurement (changes on rebuild — see C4 / Task 9).
    let seed = ds.get_key("drop/provisioning").await?;
    let state = AppState::new(ds, seed, CatalogStore::default(), FsBucket::new(&bucket_dir)?);
    // Integration point: Lane A1's scan_loop::run_loop is spawned here once A1 lands,
    // sharing this state's CatalogStore + bucket.

    let app = router(state);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    eprintln!("drop-indexer listening on :{port}");
    axum::serve(listener, app).await?;
    Ok(())
}
