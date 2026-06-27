use anyhow::{anyhow, Context, Result};
use drop_indexer::detect::{
    detect_incoming, display_memo_bytes, display_txid_to_lightwalletd_bytes,
    infer_network_from_ufvk, lightwalletd_txid_to_display_hex,
};
use drop_indexer::lightwalletd::{GrpcClient, LightwalletdClient};
use std::{env, fs};
use zcash_protocol::consensus::Network;

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

fn env_required(name: &str) -> Result<String> {
    env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .with_context(|| format!("missing {name}"))
}

fn env_u64(name: &str) -> Result<Option<u64>> {
    match env::var(name) {
        Ok(v) if !v.trim().is_empty() => {
            Ok(Some(v.parse().with_context(|| format!("invalid {name}"))?))
        }
        _ => Ok(None),
    }
}

fn default_url(network: &Network) -> &'static str {
    match network {
        Network::MainNetwork => "https://zec.rocks:443",
        Network::TestNetwork => "https://testnet.zec.rocks:443",
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    load_dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let ufvk = env_required("A1_UFVK")?;
    let network = infer_network_from_ufvk(&ufvk)?;
    let endpoint = env::var("LIGHTWALLETD_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default_url(&network).to_string());
    let backup = env::var("LIGHTWALLETD_BACKUP_URL")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let client = GrpcClient::new(endpoint.clone(), backup.clone());

    println!("network={network:?}");
    println!("lightwalletd.primary={endpoint}");
    if let Some(backup) = backup {
        println!("lightwalletd.backup={backup}");
    }

    let work: Vec<([u8; 32], u32)> = if let Ok(txid_hex) = env::var("A1_TXID_HEX") {
        if txid_hex.trim().is_empty() {
            vec![]
        } else {
            let txid = display_txid_to_lightwalletd_bytes(&txid_hex)?;
            let height = if let Some(h) = env_u64("A1_TX_HEIGHT")? {
                h
            } else {
                client
                    .current_chain_tip()
                    .await
                    .context("GetLatestBlock for fallback tx height")?
            };
            vec![(
                txid,
                height
                    .try_into()
                    .map_err(|_| anyhow!("height exceeds u32"))?,
            )]
        }
    } else {
        vec![]
    };

    let work = if work.is_empty() {
        let start = env_u64("A1_SCAN_START")?.context("missing A1_SCAN_START or A1_TXID_HEX")?;
        let end = env_u64("A1_SCAN_END")?.unwrap_or(start);
        let blocks = client
            .fetch_block_range(start, end)
            .await
            .with_context(|| format!("GetBlockRange {start}..{end}"))?;
        let mut work = Vec::new();
        for block in &blocks {
            let height: u32 = block
                .height
                .try_into()
                .map_err(|_| anyhow!("block height exceeds u32"))?;
            for tx in &block.vtx {
                if tx.txid.len() != 32 {
                    eprintln!("skip compact tx with non-32-byte txid at height={height}");
                    continue;
                }
                work.push((tx.txid.as_slice().try_into()?, height));
            }
        }
        println!(
            "range={start}..={end} blocks={} txs_to_inspect={}",
            blocks.len(),
            work.len()
        );
        work
    } else {
        work
    };

    if work.is_empty() {
        return Err(anyhow!("no transactions to inspect"));
    }

    let mut found = 0usize;
    for (txid, height) in work {
        let txid_display = lightwalletd_txid_to_display_hex(&txid);
        let raw = client
            .fetch_transaction(&txid)
            .await
            .with_context(|| format!("GetTransaction {txid_display}"))?;
        if raw.is_empty() {
            println!("txid={txid_display} height={height} raw_bytes=0 skipped");
            continue;
        }

        let notes = detect_incoming(&ufvk, &raw, &network, height)
            .with_context(|| format!("IVK trial-decrypt {txid_display}"))?;
        println!(
            "txid={txid_display} height={height} raw_bytes={} incoming_notes={}",
            raw.len(),
            notes.len()
        );
        for note in notes {
            found += 1;
            println!("  pool={}", note.pool.as_str());
            println!("  value_zat={}", note.value_zat);
            println!("  value_zec={:.8}", note.value_zat as f64 / 100_000_000.0);
            match display_memo_bytes(&note.memo) {
                Some(trimmed) => {
                    println!("  memo_utf8={:?}", String::from_utf8_lossy(trimmed));
                    println!("  memo_hex={}", hex::encode(trimmed));
                }
                None => println!("  memo=<none>"),
            }
        }
    }

    println!("incoming_notes.total={found}");
    if found == 0 {
        eprintln!(
            "No incoming notes decrypted. Check UFVK/network/range/txid and make sure the payment is shielded to this account."
        );
    }
    Ok(())
}
