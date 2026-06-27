use anyhow::{anyhow, Context, Result};
use drop_indexer::detect::{infer_network_from_ufvk, lightwalletd_txid_to_display_hex};
use drop_indexer::engine::Engine;
use drop_indexer::lightwalletd::{GrpcClient, LightwalletdClient};
use drop_indexer::scan_loop::{scan_once, scan_once_with_state};
use drop_indexer::state::{EncryptedFileScanState, ScanState, SecretboxStateCipher};
use drop_indexer::{Bucket, Catalog, DropConfig};
use std::{env, fs};
use zcash_protocol::consensus::Network;

#[derive(Clone)]
struct DemoCatalog {
    drop_id: u64,
    price_zat: u64,
    k_drop: [u8; 32],
    creator_ufvk: String,
}

impl Catalog for DemoCatalog {
    fn lookup(&self, drop_id: u64) -> Option<DropConfig> {
        (drop_id == self.drop_id).then(|| DropConfig {
            price_zat: self.price_zat,
            k_drop: self.k_drop,
            creator_ufvk: self.creator_ufvk.clone(),
            h_content: "abc123".to_string(),
            deposit_addr: "u1demoaddress".to_string(),
        })
    }
}

#[derive(Clone, Default)]
struct LoggingBucket;

#[async_trait::async_trait]
impl Bucket for LoggingBucket {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<()> {
        println!("bucket.put key={key} len={}", bytes.len());
        Ok(())
    }

    async fn get(&self, _key: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    async fn list(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
}

fn load_dotenv() {
    let Ok(contents) = fs::read_to_string(".env") else {
        return;
    };
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

fn env_u64(name: &str) -> Result<Option<u64>> {
    match env::var(name) {
        Ok(v) if !v.trim().is_empty() => {
            Ok(Some(v.parse().with_context(|| format!("invalid {name}"))?))
        }
        _ => Ok(None),
    }
}

fn env_string(name: &str) -> Option<String> {
    env::var(name).ok().filter(|s| !s.trim().is_empty())
}

fn env_required(name: &str) -> Result<String> {
    env_string(name).with_context(|| format!("missing {name}"))
}

fn env_hex_32(name: &str) -> Result<Option<[u8; 32]>> {
    let Ok(value) = env::var(name) else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    hex_32(value)
        .with_context(|| format!("invalid {name}"))
        .map(Some)
}

fn hex_32(value: &str) -> Result<[u8; 32]> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(value)?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("expected 32 bytes / 64 hex chars"))
}

fn default_url(network: &Network) -> &'static str {
    match network {
        Network::MainNetwork => "https://zec.rocks:443",
        Network::TestNetwork => "https://testnet.zec.rocks:443",
    }
}

fn usage() -> &'static str {
    "usage: A1_UFVK=<uview...> [A1_SCAN_START=<h>] [A1_SCAN_END=<h>] [A1_DEMO_DROP_ID=1] [A1_DEMO_PRICE_ZAT=10000] [A1_DEMO_K_DROP_HEX=<64hex>] [A1_STATE_FILE=<path> A1_STATE_KEY_HEX=<dev-only-64hex>] cargo run --manifest-path indexer/Cargo.toml --bin scan-live\n       or: cargo run --manifest-path indexer/Cargo.toml --bin scan-live -- <ufvk> [start] [end]"
}

#[tokio::main]
async fn main() -> Result<()> {
    load_dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{}", usage());
        return Ok(());
    }

    let ufvk = args.first().cloned().or_else(|| env::var("A1_UFVK").ok());
    let ufvk = ufvk
        .filter(|v| !v.trim().is_empty())
        .context("missing creator UFVK; set A1_UFVK or pass it as arg")?;
    let network = infer_network_from_ufvk(&ufvk)?;

    let endpoint =
        env_string("LIGHTWALLETD_URL").unwrap_or_else(|| default_url(&network).to_string());
    let backup = env_string("LIGHTWALLETD_BACKUP_URL");
    let client = GrpcClient::new(endpoint.clone(), backup.clone());

    let tip = client
        .current_chain_tip()
        .await
        .context("get latest block")?;
    let explicit_start = match args.get(1) {
        Some(v) => Some(v.parse().context("invalid start height arg")?),
        None => env_u64("A1_SCAN_START")?,
    };
    let explicit_end = match args.get(2) {
        Some(v) => Some(v.parse().context("invalid end height arg")?),
        None => env_u64("A1_SCAN_END")?,
    };

    let demo_drop_id = env_u64("A1_DEMO_DROP_ID")?.unwrap_or(1);
    let demo_price_zat = env_u64("A1_DEMO_PRICE_ZAT")?.unwrap_or(10_000);
    let demo_k_drop = env_hex_32("A1_DEMO_K_DROP_HEX")?.unwrap_or([9u8; 32]);
    let state_file = env_string("A1_STATE_FILE");

    println!("network={network:?}");
    println!("lightwalletd.primary={endpoint}");
    if let Some(backup) = &backup {
        println!("lightwalletd.backup={backup}");
    }
    println!("chain.tip={tip}");
    println!("demo.drop_id={demo_drop_id}");
    println!("demo.price_zat={demo_price_zat}");

    let catalog = DemoCatalog {
        drop_id: demo_drop_id,
        price_zat: demo_price_zat,
        k_drop: demo_k_drop,
        creator_ufvk: ufvk.clone(),
    };
    let mut engine = Engine::new(catalog, LoggingBucket);

    let summary = if let Some(path) = state_file {
        let key = env_required("A1_STATE_KEY_HEX").context(
            "A1_STATE_KEY_HEX is development-only; production should replace this with a TEE sealing-key StateCipher",
        )?;
        let cipher = SecretboxStateCipher::from_hex_key(&key)?;
        let mut state = EncryptedFileScanState::load_or_default(&path, cipher)?;
        let start = explicit_start
            .or_else(|| state.last_scanned_height().map(|h| h.saturating_add(1)))
            .unwrap_or(tip);
        let end = explicit_end.unwrap_or(tip);

        println!("state.file={}", state.path().display());
        println!(
            "state.last_scanned_height={}",
            state
                .last_scanned_height()
                .map(|h| h.to_string())
                .unwrap_or_else(|| "<none>".to_string())
        );
        println!("state.seen_txids={}", state.seen_txids_len());
        println!("scan.range={start}..={end}");

        if start > end {
            println!("summary.no_new_blocks=true");
            return Ok(());
        }

        let summary = scan_once_with_state(
            &client,
            &ufvk,
            &network,
            start,
            end,
            &mut state,
            &mut engine,
        )
        .await?;
        state.save()?;
        println!(
            "state.saved_last_scanned_height={}",
            state
                .last_scanned_height()
                .map(|h| h.to_string())
                .unwrap_or_else(|| "<none>".to_string())
        );
        println!("state.saved_seen_txids={}", state.seen_txids_len());
        summary
    } else {
        let start = explicit_start.unwrap_or(tip);
        let end = explicit_end.unwrap_or(start);
        println!("scan.range={start}..={end}");
        scan_once(&client, &ufvk, &network, start, end, &mut engine).await?
    };

    println!("summary.blocks_fetched={}", summary.blocks_fetched);
    println!("summary.compact_txs={}", summary.compact_txs);
    println!("summary.full_txs_fetched={}", summary.full_txs_fetched);
    println!("summary.incoming_notes={}", summary.incoming_notes);
    println!("summary.notes_without_memo={}", summary.notes_without_memo);
    println!("summary.decoded_memos={}", summary.decoded_memos);
    println!("summary.undecodable_memos={}", summary.undecodable_memos);
    println!("summary.dispatches={}", summary.dispatches.len());

    for dispatch in &summary.dispatches {
        println!(
            "dispatch drop_id={} value_zat={} txid={} bucket_key={}",
            dispatch.drop_id,
            dispatch.value_zat,
            lightwalletd_txid_to_display_hex(&dispatch.txid),
            dispatch.bucket_key
        );
    }

    if summary.dispatches.is_empty() {
        eprintln!(
            "No dispatches produced. Check UFVK, range, memo format, drop_id, and A1_DEMO_PRICE_ZAT."
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_32_byte_hex_with_optional_prefix() {
        let raw = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
        let got = hex_32(raw).unwrap();
        assert_eq!(got[0], 0);
        assert_eq!(got[31], 31);
        assert_eq!(hex_32(&format!("0x{raw}")).unwrap(), got);
    }

    #[test]
    fn rejects_non_32_byte_hex() {
        assert!(hex_32("00").is_err());
    }
}
