use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;

use clean_wallet_scanner::attest::{Attestor, DstackAttestor};
use clean_wallet_scanner::lightwalletd::GrpcClient;
use clean_wallet_scanner::server::{router, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let primary = env::var("LIGHTWALLETD_PRIMARY")
        .unwrap_or_else(|_| "https://testnet.zec.rocks:443".into());
    let backup = env::var("LIGHTWALLETD_BACKUP").ok();
    let network = env::var("NETWORK").unwrap_or_else(|_| "testnet".into());
    let socket = env::var("DSTACK_SOCKET").unwrap_or_else(|_| "/var/run/dstack.sock".into());

    let attestor = Arc::new(DstackAttestor::new(socket));
    let info = attestor
        .info()
        .await
        .map_err(|e| anyhow::anyhow!("dstack info failed at startup: {e}"))?;
    tracing::info!(measurement = %info.code_measurement, "scanner starting with code measurement");

    let state = AppState {
        client: Arc::new(GrpcClient::new(primary, backup)),
        attestor,
        scanner_code_measurement: info.code_measurement,
        scanner_network: network,
        scan_lock: Arc::new(Mutex::new(())),
    };

    let app = router(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("listening on :8080");
    axum::serve(listener, app).await?;
    Ok(())
}
