//! Spike #2 (drop spec): prove that a server holding ONLY an Incoming Viewing Key
//! can detect an incoming shielded payment and recover its 512-byte MEMO.
//!
//! This is the drop spec's [C1]+[C2]:
//!   - [C2] INCOMING decryption with the IVK (`try_sapling_note_decryption` /
//!     orchard `try_note_decryption`), NOT clean-wallet's OVK *outgoing* recovery.
//!   - [C2] KEEP the memo (the screener at scan.rs:259/278 discards it as `_memo`).
//!   - [C1] the memo lives in bytes 52..564 of the FULL `enc_ciphertext`, which is
//!     absent from compact blocks — so we fetch the full tx via GetTransaction.
//!
//! Modes:
//!   cargo run -p clean-wallet-scanner --bin ivk-incoming-probe -- <UFVK> txid  <txid-hex>   [lwd-url]
//!   cargo run -p clean-wallet-scanner --bin ivk-incoming-probe -- <UFVK> range <start> <end> [lwd-url]
//!
//! `txid` mode is the fast crux check: paste the txid Zashi showed you. `range` mode
//! mirrors the real indexer: enumerate txids from compact blocks, then full-fetch +
//! decrypt each. Network (mainnet/testnet) is inferred from the UFVK prefix.

use anyhow::{anyhow, Context, Result};
use clean_wallet_scanner::lightwalletd::{GrpcClient, LightwalletdClient};

use sapling_crypto::note_encryption::{
    try_sapling_note_decryption, PreparedIncomingViewingKey as SaplingPreparedIvk,
    Zip212Enforcement,
};
use zcash_keys::keys::UnifiedFullViewingKey;
use zcash_note_encryption::try_note_decryption;
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{
    BlockHeight, BranchId, Network, NetworkUpgrade, Parameters,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage:");
        eprintln!("  {} <UFVK> txid  <txid-hex>   [lightwalletd-url]", args[0]);
        eprintln!("  {} <UFVK> range <start> <end> [lightwalletd-url]", args[0]);
        std::process::exit(2);
    }
    let ufvk_str = args[1].trim().to_string();
    let mode = args[2].as_str();

    // Infer network from the UFVK prefix (ZIP-316).
    let network = if ufvk_str.starts_with("uviewtest") {
        Network::TestNetwork
    } else if ufvk_str.starts_with("uview") {
        Network::MainNetwork
    } else {
        return Err(anyhow!(
            "UFVK must start with 'uview' (mainnet) or 'uviewtest' (testnet)"
        ));
    };

    // Build the work list and pick the lightwalletd URL.
    let (work, url): (Vec<([u8; 32], u64)>, String) = match mode {
        "txid" => {
            let txid_hex = args.get(3).context("txid mode needs <txid-hex>")?;
            let url = args.get(4).cloned().unwrap_or_else(|| default_url(&network));
            // Display txids are big-endian; lightwalletd's internal hash is little-endian.
            let mut le = hex::decode(txid_hex.trim()).context("txid is not valid hex")?;
            le.reverse();
            let txid: [u8; 32] = le
                .as_slice()
                .try_into()
                .map_err(|_| anyhow!("txid must be 32 bytes"))?;
            let client = GrpcClient::new(url.clone(), None);
            // Height is unknown in txid mode; use the chain tip for branch-id / ZIP-212.
            let tip = client
                .current_chain_tip()
                .await
                .context("GetLatestBlock failed")?;
            (vec![(txid, tip)], url)
        }
        "range" => {
            let start: u64 = args.get(3).context("range needs <start>")?.parse().context("start")?;
            let end: u64 = args.get(4).context("range needs <end>")?.parse().context("end")?;
            let url = args.get(5).cloned().unwrap_or_else(|| default_url(&network));
            let client = GrpcClient::new(url.clone(), None);
            let blocks = client
                .fetch_block_range(start, end)
                .await
                .context("GetBlockRange failed")?;
            let mut work = Vec::new();
            for b in &blocks {
                for ctx in &b.vtx {
                    let txid: [u8; 32] = ctx
                        .txid
                        .as_slice()
                        .try_into()
                        .map_err(|_| anyhow!("compact tx has non-32-byte txid"))?;
                    work.push((txid, b.height));
                }
            }
            println!(
                "range {start}..={end}: {} block(s), {} tx(s) to inspect",
                blocks.len(),
                work.len()
            );
            (work, url)
        }
        other => return Err(anyhow!("mode must be 'txid' or 'range', got '{other}'")),
    };

    println!("network = {network:?}");
    println!("lightwalletd = {url}");

    // Decode the UFVK and prepare the INCOMING viewing keys (external scope).
    let ufvk = UnifiedFullViewingKey::decode(&network, &ufvk_str)
        .map_err(|e| anyhow!("UFVK decode failed: {e}"))?;

    let sapling_ivk = ufvk
        .sapling()
        .map(|s| SaplingPreparedIvk::new(&s.to_ivk(zip32::Scope::External)));
    let orchard_ivk = ufvk.orchard().map(|o| {
        orchard::keys::PreparedIncomingViewingKey::new(&o.to_ivk(orchard::keys::Scope::External))
    });
    if sapling_ivk.is_none() && orchard_ivk.is_none() {
        return Err(anyhow!("UFVK carries neither a Sapling nor an Orchard key"));
    }

    let client = GrpcClient::new(url, None);
    let mut found = 0usize;

    for (txid_le, height) in &work {
        let raw = client
            .fetch_transaction(txid_le)
            .await
            .context("GetTransaction failed")?;
        if raw.is_empty() {
            continue;
        }
        let bh = BlockHeight::from_u32(*height as u32);
        let branch_id = BranchId::for_height(&network, bh);
        let tx = match Transaction::read(&raw[..], branch_id) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("  (skip) failed to deserialize tx: {e}");
                continue;
            }
        };
        let txid_be = {
            let mut b = *txid_le;
            b.reverse();
            hex::encode(b)
        };

        // --- Sapling: INCOMING note decryption with the IVK ---
        if let Some(pivk) = &sapling_ivk {
            if let Some(bundle) = tx.sapling_bundle() {
                let zip212 = if network.is_nu_active(NetworkUpgrade::Canopy, bh) {
                    Zip212Enforcement::On
                } else {
                    Zip212Enforcement::GracePeriod
                };
                for output in bundle.shielded_outputs() {
                    if let Some((note, addr, memo)) =
                        try_sapling_note_decryption(pivk, output, zip212)
                    {
                        found += 1;
                        show_hit("sapling", &txid_be, note.value().inner(), &addr.to_bytes(), memo.as_slice());
                    }
                }
            }
        }

        // --- Orchard: INCOMING note decryption with the IVK ---
        if let Some(pivk) = &orchard_ivk {
            if let Some(bundle) = tx.orchard_bundle() {
                for action in bundle.actions() {
                    let domain = orchard::note_encryption::OrchardDomain::for_action(action);
                    if let Some((note, addr, memo)) = try_note_decryption(&domain, pivk, action) {
                        found += 1;
                        show_hit("orchard", &txid_be, note.value().inner(), &addr.to_raw_address_bytes(), &memo);
                    }
                }
            }
        }
    }

    println!("\n=== {found} incoming note(s) decrypted with the IVK ===");
    if found == 0 {
        eprintln!(
            "No incoming notes for this IVK in the inspected tx(s). Check: right UFVK? \
             right network? right txid/range? note actually addressed to this key?"
        );
        std::process::exit(1);
    }
    println!("SPIKE #2 PASS: IVK alone recovered the payment value + 512-byte memo from the full tx.");
    Ok(())
}

fn default_url(network: &Network) -> String {
    match network {
        Network::MainNetwork => "https://zec.rocks:443".to_string(),
        Network::TestNetwork => "https://testnet.zec.rocks:443".to_string(),
    }
}

/// Print a decrypted incoming note. `memo` is the raw memo field (≤512 bytes).
fn show_hit(pool: &str, txid_be: &str, value_zat: u64, addr_raw: &[u8], memo: &[u8]) {
    // Trim trailing zero padding; 0xF6-lead means "no memo" (ZIP-302).
    let trimmed: &[u8] = match memo.iter().rposition(|&b| b != 0) {
        Some(i) => &memo[..=i],
        None => &[],
    };
    let no_memo = trimmed.is_empty() || memo.first() == Some(&0xF6);
    println!("  [{pool}] txid = {txid_be}");
    println!(
        "         value     = {value_zat} zatoshi ({:.8} ZEC)",
        value_zat as f64 / 1e8
    );
    println!("         recipient = {} (raw)", hex::encode(addr_raw));
    if no_memo {
        println!("         memo      = <none>");
    } else {
        println!("         memo.text = {:?}", String::from_utf8_lossy(trimmed));
        println!("         memo.hex  = {}", hex::encode(trimmed));
    }
}
