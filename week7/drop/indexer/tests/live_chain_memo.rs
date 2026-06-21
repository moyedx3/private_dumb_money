//! Ignored live test for a memo-bearing shielded transaction that already exists on-chain.
//!
//! This is intentionally not a polling test: it fetches one explicit transaction by txid,
//! decrypts it with the supplied UFVK, and validates the A1 memo layout
//! `drop_id(8 BE) || e_pub(32)`.
//!
//! Required environment variables:
//! - `A1_UFVK`: creator UFVK that can view the transaction
//! - `A1_TXID_HEX`: explorer/display txid hex of the existing transaction
//! - `A1_TX_HEIGHT`: mined block height of the transaction
//! - `A1_EXPECTED_DROP_ID`: expected decimal drop id in the memo
//! - `A1_EXPECTED_E_PUB_HEX`: expected 32-byte X25519 public key hex
//!
//! Optional:
//! - `LIGHTWALLETD_URL` (default: `https://zec.rocks:443`)
//! - `LIGHTWALLETD_BACKUP_URL`
//!
//! Run:
//! ```bash
//! A1_UFVK='<creator_ufvk>' \
//! A1_TXID_HEX='<display_txid_hex>' \
//! A1_TX_HEIGHT='<height>' \
//! A1_EXPECTED_DROP_ID='<drop_id>' \
//! A1_EXPECTED_E_PUB_HEX='<64_hex_chars>' \
//! cargo test --manifest-path indexer/Cargo.toml --test live_chain_memo -- --ignored --nocapture
//! ```

use anyhow::{anyhow, Context, Result};
use drop_indexer::detect::{
    detect_incoming, display_memo_bytes, display_txid_to_lightwalletd_bytes,
    infer_network_from_ufvk,
};
use drop_indexer::lightwalletd::{GrpcClient, LightwalletdClient};
use drop_indexer::memo::decode_memo;
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

fn required_env(name: &str) -> Result<String> {
    env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .with_context(|| format!("missing {name}"))
}

fn default_url(network: &Network) -> &'static str {
    match network {
        Network::MainNetwork => "https://zec.rocks:443",
        Network::TestNetwork => "https://testnet.zec.rocks:443",
    }
}

fn expected_epub_from_env() -> Result<[u8; 32]> {
    let hex = required_env("A1_EXPECTED_E_PUB_HEX")?;
    let bytes = hex::decode(hex.trim().strip_prefix("0x").unwrap_or(hex.trim()))
        .context("A1_EXPECTED_E_PUB_HEX must be hex")?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("A1_EXPECTED_E_PUB_HEX must decode to 32 bytes"))
}

#[tokio::test]
#[ignore = "requires existing on-chain memo tx env vars; does not poll"]
async fn decodes_existing_chain_payment_memo() -> Result<()> {
    load_dotenv();

    let ufvk = required_env("A1_UFVK")?;
    let txid_hex = required_env("A1_TXID_HEX")?;
    let height: u32 = required_env("A1_TX_HEIGHT")?
        .parse()
        .context("A1_TX_HEIGHT must be a u32")?;
    let expected_drop_id: u64 = required_env("A1_EXPECTED_DROP_ID")?
        .parse()
        .context("A1_EXPECTED_DROP_ID must be a u64")?;
    let expected_e_pub = expected_epub_from_env()?;

    let network = infer_network_from_ufvk(&ufvk)?;
    let endpoint = env::var("LIGHTWALLETD_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default_url(&network).to_string());
    let backup = env::var("LIGHTWALLETD_BACKUP_URL")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let client = GrpcClient::new(endpoint, backup);

    let txid = display_txid_to_lightwalletd_bytes(&txid_hex)?;
    let raw = client
        .fetch_transaction(&txid)
        .await
        .with_context(|| format!("fetch existing tx {txid_hex}"))?;
    assert!(!raw.is_empty(), "lightwalletd returned empty raw tx bytes");

    let notes = detect_incoming(&ufvk, &raw, &network, height)
        .with_context(|| format!("decrypt existing tx {txid_hex}"))?;
    assert!(
        !notes.is_empty(),
        "UFVK did not decrypt any incoming notes for tx {txid_hex}"
    );

    let mut decoded = Vec::new();
    for note in &notes {
        if let Some(memo) = display_memo_bytes(&note.memo) {
            if let Some((drop_id, e_pub)) = decode_memo(memo) {
                decoded.push((drop_id, e_pub));
            }
        }
    }

    assert!(
        !decoded.is_empty(),
        "no decrypted note had an A1 40-byte drop_id||e_pub memo"
    );
    assert!(
        decoded
            .iter()
            .any(|(drop_id, e_pub)| *drop_id == expected_drop_id && *e_pub == expected_e_pub),
        "decoded memos did not contain expected drop_id/e_pub"
    );

    Ok(())
}
