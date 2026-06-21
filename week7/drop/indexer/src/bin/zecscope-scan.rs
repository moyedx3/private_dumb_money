use anyhow::{anyhow, Context, Result};
use drop_indexer::lightwalletd::{GrpcClient, LightwalletdClient};
use drop_indexer::zecscope_adapter::to_zecscope_blocks;
use std::{env, fs};
use zecscope_scanner::{Network, ScanRequest, Scanner};

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

fn network_from_env_or_key(viewing_key: &str) -> Result<Network> {
    match env::var("A1_NETWORK")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" => {
            if viewing_key.trim().starts_with("uviewtest") {
                Ok(Network::TestNetwork)
            } else {
                Ok(Network::MainNetwork)
            }
        }
        "main" | "mainnet" | "mainnetwork" => Ok(Network::MainNetwork),
        "test" | "testnet" | "testnetwork" => Ok(Network::TestNetwork),
        other => Err(anyhow!(
            "unsupported A1_NETWORK={other}; use mainnet or testnet"
        )),
    }
}

fn default_endpoint(network: &Network) -> &'static str {
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

    let viewing_key =
        env::var("A1_UFVK").context("set A1_UFVK to a uview1... Unified Full Viewing Key")?;
    let network = network_from_env_or_key(&viewing_key)?;
    let endpoint =
        env::var("LIGHTWALLETD_URL").unwrap_or_else(|_| default_endpoint(&network).into());
    let backup = env::var("LIGHTWALLETD_BACKUP_URL")
        .ok()
        .filter(|s| !s.trim().is_empty());

    let client = GrpcClient::new(endpoint.clone(), backup.clone());
    let tip = client
        .current_chain_tip()
        .await
        .context("get latest block")?;
    let start = env_u64("A1_SCAN_START")?.unwrap_or(tip);
    let end = env_u64("A1_SCAN_END")?.unwrap_or(start);
    if start > end {
        return Err(anyhow!("A1_SCAN_START must be <= A1_SCAN_END"));
    }

    println!("zecscope_scanner.version=0.1.0");
    println!("network={:?}", network);
    println!("lightwalletd.primary={endpoint}");
    if let Some(backup) = &backup {
        println!("lightwalletd.backup={backup}");
    }
    println!("chain.tip={tip}");
    println!("scan.range={start}..={end}");

    let lightwalletd_blocks = client
        .fetch_block_range(start, end)
        .await
        .with_context(|| format!("get block range {start}..={end}"))?;
    let compact_tx_count: usize = lightwalletd_blocks.iter().map(|b| b.vtx.len()).sum();
    println!("blocks.fetched={}", lightwalletd_blocks.len());
    println!("compact_txs.fetched={compact_tx_count}");

    let compact_blocks = to_zecscope_blocks(&lightwalletd_blocks);
    let scanner = Scanner::new(network);
    let request = ScanRequest {
        viewing_key,
        key_id: env::var("A1_KEY_ID").unwrap_or_else(|_| "a1-creator".into()),
        compact_blocks,
    };

    let txs = scanner
        .scan(&request)
        .context("scan compact blocks with zecscope-scanner")?;
    println!("zecscope.matches={}", txs.len());
    for tx in txs {
        println!(
            "match txid={} height={} pool={} direction={:?} amount_zat={} amount_zec={:.8} memo={}",
            tx.txid,
            tx.height,
            tx.pool,
            tx.direction,
            tx.amount_zat,
            tx.amount_zec(),
            tx.memo.as_deref().unwrap_or("<none>")
        );
    }

    Ok(())
}
