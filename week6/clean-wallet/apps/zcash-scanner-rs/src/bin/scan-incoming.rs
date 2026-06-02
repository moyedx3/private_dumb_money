//! scan-incoming — UFVK 로 *받은* (incoming) 노트를 출력한다.
//!
//! 본체 스캐너는 outgoing 수취인 (제재 매칭용) 만 노출. faucet 입금이 진짜 들어왔는지
//! 확인하려면 incoming 도 봐야 하는데 그건 본체 artifact 에 안 들어감 (스크리닝 목적 분리).
//! 이 디버그 도구는 같은 사이드카 흐름으로 sapling/orchard incoming 노트를 표시.
//!
//! transparent 입금은 incoming 표시 안 함 — `check-taddr` 가 더 정확함 (UTXO + tx history).
//!
//! 사용:
//!   cargo run --release --bin scan-incoming -- \
//!     --lwd-url https://testnet.zec.rocks:443 \
//!     --network test \
//!     --start 4031000 --end 4032000 \
//!     --ufvk uviewtest1...

use std::collections::HashMap;
use std::io::Cursor;

use anyhow::{anyhow, bail, Context, Result};
use zcash_client_backend::{decrypt_transaction, TransferType};
use zcash_keys::address::Address;
use zcash_keys::keys::UnifiedFullViewingKey;
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{BlockHeight, BranchId, Network};
use zip32::AccountId;

use zcash_scanner_rs::{encode_orchard_recipient, lwd, CompletenessChecker};

struct Args {
    lwd_url: String,
    network: Network,
    ufvk: String,
    start_height: u32,
    end_height: u32,
}

fn parse_args() -> Result<Args> {
    let argv: Vec<String> = std::env::args().collect();
    let mut lwd_url: Option<String> = None;
    let mut network_str: Option<String> = None;
    let mut ufvk: Option<String> = None;
    let mut start: Option<u32> = None;
    let mut end: Option<u32> = None;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--lwd-url" => {
                lwd_url = Some(argv[i + 1].clone());
                i += 2;
            }
            "--network" => {
                network_str = Some(argv[i + 1].clone());
                i += 2;
            }
            "--ufvk" => {
                ufvk = Some(argv[i + 1].clone());
                i += 2;
            }
            "--start" => {
                start = Some(argv[i + 1].parse().context("--start")?);
                i += 2;
            }
            "--end" => {
                end = Some(argv[i + 1].parse().context("--end")?);
                i += 2;
            }
            "--help" | "-h" => {
                eprintln!(
                    "scan-incoming — UFVK 로 받은 sapling/orchard 노트 출력 (faucet 입금 확인용)
사용:
  scan-incoming --lwd-url <url> --network <main|test> --ufvk <uviewtest...> --start N --end M"
                );
                std::process::exit(0);
            }
            other => bail!("unknown arg: {}", other),
        }
    }
    let network = match network_str.as_deref() {
        Some("main") | Some("mainnet") => Network::MainNetwork,
        Some("test") | Some("testnet") | None => Network::TestNetwork,
        Some(other) => bail!("invalid --network: {} (main|mainnet|test|testnet)", other),
    };
    Ok(Args {
        lwd_url: lwd_url.ok_or_else(|| anyhow!("--lwd-url required"))?,
        network,
        ufvk: ufvk.ok_or_else(|| anyhow!("--ufvk required"))?,
        start_height: start.ok_or_else(|| anyhow!("--start required"))?,
        end_height: end.ok_or_else(|| anyhow!("--end required"))?,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args()?;
    eprintln!(
        "[scan-incoming] connecting to {} range=[{}..{}]",
        args.lwd_url, args.start_height, args.end_height
    );

    // 명시적 TLS config — HTTPS 면 자동 활성화 안 됨.
    use tonic::transport::{Channel, ClientTlsConfig};
    let is_https = args.lwd_url.starts_with("https://");
    let endpoint = Channel::from_shared(args.lwd_url.clone()).context("invalid lwd URL")?;
    let endpoint = if is_https {
        endpoint
            .tls_config(ClientTlsConfig::new().with_webpki_roots())
            .context("tls config")?
    } else {
        endpoint
    };
    let channel = endpoint.connect().await.context("connect lightwalletd")?;
    let mut client = lwd::compact_tx_streamer_client::CompactTxStreamerClient::new(channel);

    // tip + UFVK 파싱
    let latest = client
        .get_latest_block(lwd::ChainSpec {})
        .await
        .context("GetLatestBlock")?
        .into_inner();
    eprintln!("[scan-incoming] tip height={}", latest.height);
    if (args.end_height as u64) > latest.height {
        bail!(
            "end_height({}) > tip({}) — chain too short",
            args.end_height,
            latest.height
        );
    }
    let chain_tip = Some(BlockHeight::from_u32(latest.height as u32));

    let ufvk = UnifiedFullViewingKey::decode(&args.network, &args.ufvk)
        .map_err(|e| anyhow!("UFVK decode failed: {}", e))?;
    let mut ufvks: HashMap<u32, UnifiedFullViewingKey> = HashMap::new();
    ufvks.insert(AccountId::ZERO.into(), ufvk);

    // 블록 스트림 + 완전성 체크
    let range_req = lwd::BlockRange {
        start: Some(lwd::BlockId {
            height: args.start_height as u64,
            hash: vec![],
        }),
        end: Some(lwd::BlockId {
            height: args.end_height as u64,
            hash: vec![],
        }),
        pool_types: vec![],
    };
    let mut block_stream = client
        .get_block_range(range_req)
        .await
        .context("GetBlockRange")?
        .into_inner();

    let mut checker = CompletenessChecker::new(args.start_height, args.end_height);
    while let Some(cb) = block_stream.message().await.context("block stream")? {
        checker.add_block(&cb)?;
    }
    let (tx_locations, blocks_seen) = checker.finalize()?;
    eprintln!(
        "[scan-incoming] verified {} blocks · {} txs",
        blocks_seen,
        tx_locations.len()
    );

    // 각 tx fetch + OVK/IVK trial decryption → incoming 노트 출력
    println!("# scan-incoming results");
    println!(
        "# range [{}..{}], {} blocks scanned",
        args.start_height, args.end_height, blocks_seen
    );
    println!();

    let mut total_incoming = 0u32;
    for (height, txid) in &tx_locations {
        let raw = client
            .get_transaction(lwd::TxFilter {
                block: None,
                index: 0,
                hash: txid.clone(),
            })
            .await
            .with_context(|| format!("GetTransaction at {}", height))?
            .into_inner();

        let branch_id = BranchId::for_height(&args.network, BlockHeight::from_u32(*height));
        let tx = match Transaction::read(Cursor::new(&raw.data), branch_id) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("[scan-incoming] parse tx@{} fail: {}", height, e);
                continue;
            }
        };

        let decrypted = decrypt_transaction(
            &args.network,
            Some(BlockHeight::from_u32(*height)),
            chain_tip,
            &tx,
            &ufvks,
        );

        // (a) sapling incoming
        for out in decrypted.sapling_outputs() {
            if !matches!(out.transfer_type(), TransferType::Incoming) {
                continue;
            }
            let value: u64 = out.note_value().into();
            let recipient = out.note().recipient();
            println!(
                "INCOMING sapling  height={}  txid={}  to={}  value_zat={}",
                height,
                hex::encode(txid),
                Address::Sapling(recipient).encode(&args.network),
                value
            );
            total_incoming += 1;
        }

        // (b) orchard incoming
        for out in decrypted.orchard_outputs() {
            if !matches!(out.transfer_type(), TransferType::Incoming) {
                continue;
            }
            let value: u64 = out.note_value().into();
            let recipient = out.note().recipient();
            println!(
                "INCOMING orchard  height={}  txid={}  to={}  value_zat={}",
                height,
                hex::encode(txid),
                encode_orchard_recipient(recipient, &args.network),
                value
            );
            total_incoming += 1;
        }
    }

    println!();
    if total_incoming == 0 {
        println!(
            "(no incoming notes — 이 range 안에 받은 sapling/orchard 노트 없음. transparent 입금이면 check-taddr 사용.)"
        );
    } else {
        println!("total incoming notes: {}", total_incoming);
    }
    Ok(())
}
