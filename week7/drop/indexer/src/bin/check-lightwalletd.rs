use anyhow::{anyhow, Context, Result};
use drop_indexer::lightwalletd::{GrpcClient, LightwalletdClient};
use std::{env, fs};

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

fn txid_from_hex_le(hex_txid: &str) -> Result<[u8; 32]> {
    let s = hex_txid
        .trim()
        .strip_prefix("0x")
        .unwrap_or(hex_txid.trim());
    if s.len() != 64 {
        return Err(anyhow!("A1_TXID_HEX must be 64 hex chars"));
    }
    let mut bytes = [0u8; 32];
    for i in 0..32 {
        bytes[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)?;
    }
    bytes.reverse(); // display txid is big-endian; lightwalletd CompactTx hash uses little-endian bytes.
    Ok(bytes)
}

#[tokio::main]
async fn main() -> Result<()> {
    load_dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let endpoint = env::var("LIGHTWALLETD_URL").unwrap_or_else(|_| "https://zec.rocks:443".into());
    let backup = env::var("LIGHTWALLETD_BACKUP_URL")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let client = GrpcClient::new(endpoint.clone(), backup.clone());

    println!("lightwalletd.primary={endpoint}");
    if let Some(backup) = backup {
        println!("lightwalletd.backup={backup}");
    }

    let tip = client
        .current_chain_tip()
        .await
        .context("get latest block")?;
    println!("chain.tip={tip}");

    let start = env_u64("A1_SCAN_START")?.unwrap_or(tip);
    let end = env_u64("A1_SCAN_END")?.unwrap_or(start);
    let blocks = client
        .fetch_block_range(start, end)
        .await
        .with_context(|| format!("get block range {start}..{end}"))?;
    println!("blocks.fetched={}", blocks.len());
    for block in blocks.iter().take(5) {
        println!(
            "block.height={} compact_txs={}",
            block.height,
            block.vtx.len()
        );
    }

    let fetch_first_tx = env::var("A1_FETCH_FIRST_TX")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(true);
    if fetch_first_tx {
        if let Some(tx) = blocks
            .iter()
            .flat_map(|b| b.vtx.iter())
            .find(|tx| tx.txid.len() == 32)
        {
            let txid: [u8; 32] = tx.txid.as_slice().try_into()?;
            let raw = client
                .fetch_transaction(&txid)
                .await
                .context("get first compact transaction")?;
            println!("first_compact_tx.raw_bytes={}", raw.len());
        } else {
            println!("first_compact_tx.raw_bytes=skipped_no_32_byte_txid");
        }
    }

    if let Ok(txid) = env::var("A1_TXID_HEX") {
        if !txid.trim().is_empty() {
            let txid_le = txid_from_hex_le(&txid)?;
            let raw = client
                .fetch_transaction(&txid_le)
                .await
                .context("get transaction")?;
            println!("tx.raw_bytes={}", raw.len());
        }
    }

    Ok(())
}
