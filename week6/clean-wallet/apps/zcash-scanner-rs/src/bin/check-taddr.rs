//! check-taddr — lightwalletd 에 직접 query 해서 transparent address 의 잔액과 UTXO 를 출력한다.
//!
//! testnet block explorer 들이 부실해서 (대부분 500/down/address lookup 없음) 만든 도구.
//! 우리 mnemonic 에서 derive 된 t-addr 들이 실제로 ZEC 를 받았는지 직접 확인 가능.
//!
//! 사용:
//!   cargo run --release --bin check-taddr -- \
//!       --lwd-url https://lightwalletd.testnet.electriccoin.co:9067 \
//!       --addr tmFj... --addr tmAb... \
//!       [--start-height 0]
//!
//! 출력:
//!   - lightwalletd tip height
//!   - 각 주소의 현재 잔액
//!   - 각 주소의 UTXO 목록 (height, txid, value)

use anyhow::{anyhow, bail, Context, Result};
use zcash_scanner_rs::lwd;

struct Args {
    lwd_url: String,
    addrs: Vec<String>,
    start_height: u64,
}

fn parse_args() -> Result<Args> {
    let argv: Vec<String> = std::env::args().collect();
    let mut lwd_url: Option<String> = None;
    let mut addrs: Vec<String> = vec![];
    let mut start_height: u64 = 0;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--lwd-url" => {
                lwd_url = Some(
                    argv.get(i + 1)
                        .ok_or_else(|| anyhow!("--lwd-url 뒤에 URL"))?
                        .clone(),
                );
                i += 2;
            }
            "--addr" => {
                addrs.push(
                    argv.get(i + 1)
                        .ok_or_else(|| anyhow!("--addr 뒤에 t-address"))?
                        .clone(),
                );
                i += 2;
            }
            "--start-height" => {
                start_height = argv
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--start-height 뒤에 숫자"))?
                    .parse()
                    .context("invalid start-height")?;
                i += 2;
            }
            "--help" | "-h" => {
                eprintln!(
                    "check-taddr — lightwalletd 직접 query
사용법:
  check-taddr --lwd-url <url> --addr <t-address> [--addr <another>] [--start-height N]

예:
  check-taddr --lwd-url https://lightwalletd.testnet.electriccoin.co:9067 \\
              --addr tmFj9eT8sNUMWxSe52EFX27U7DdBLX2ZzKj --addr tmRoXX..."
                );
                std::process::exit(0);
            }
            other => bail!("unknown arg: {}", other),
        }
    }
    let lwd_url = lwd_url.ok_or_else(|| anyhow!("--lwd-url required"))?;
    if addrs.is_empty() {
        bail!("--addr 가 최소 한 개 필요");
    }
    Ok(Args {
        lwd_url,
        addrs,
        start_height,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args().context("argument parsing")?;
    eprintln!("[check-taddr] connecting to {}...", args.lwd_url);

    // 명시적 TLS config — tonic 0.14 의 connect(url) 단축은 자동 TLS 안 함.
    use tonic::transport::{Channel, ClientTlsConfig};
    let is_https = args.lwd_url.starts_with("https://");
    let endpoint = Channel::from_shared(args.lwd_url.clone())
        .context("invalid lightwalletd URL")?;
    let endpoint = if is_https {
        endpoint
            .tls_config(ClientTlsConfig::new().with_webpki_roots())
            .context("configure TLS")?
    } else {
        endpoint
    };
    let channel = endpoint.connect().await.context("connect to lightwalletd")?;
    let mut client = lwd::compact_tx_streamer_client::CompactTxStreamerClient::new(channel);

    // 1) tip
    let latest = client
        .get_latest_block(lwd::ChainSpec {})
        .await
        .context("GetLatestBlock")?
        .into_inner();
    println!("# lightwalletd: {}", args.lwd_url);
    println!("# tip height: {}", latest.height);
    println!();

    for addr in &args.addrs {
        println!("=== {} ===", addr);

        // 2) balance (모든 confirmed UTXO 합산)
        let balance = client
            .get_taddress_balance(lwd::AddressList {
                addresses: vec![addr.clone()],
            })
            .await
            .with_context(|| format!("GetTaddressBalance for {}", addr))?
            .into_inner();
        println!("balance_zat: {}", balance.value_zat);
        println!("balance_zec: {}", (balance.value_zat as f64) / 100_000_000.0);

        // 3) 현재 살아 있는 UTXO 목록 (이미 spend 된 건 빠짐).
        let utxos = client
            .get_address_utxos(lwd::GetAddressUtxosArg {
                addresses: vec![addr.clone()],
                start_height: args.start_height,
                max_entries: 0,
            })
            .await
            .with_context(|| format!("GetAddressUtxos for {}", addr))?
            .into_inner();
        if utxos.address_utxos.is_empty() {
            println!("utxos: (none — 현재 UTXO 없음. 받았지만 이미 보냈을 수도)");
        } else {
            println!("utxos:");
            for u in &utxos.address_utxos {
                println!(
                    "  height={:>8}  txid={}  vout={}  value_zat={}",
                    u.height,
                    hex::encode(&u.txid),
                    u.index,
                    u.value_zat
                );
            }
        }

        // 4) 이 주소가 들어 있는 모든 transaction 의 history (받기·보내기 모두 — UTXO 가
        //    이미 spend 됐어도 잡힘. faucet 받은 사실 자체를 확인하는 데 가장 유용).
        let mut tx_stream = client
            .get_taddress_txids(lwd::TransparentAddressBlockFilter {
                address: addr.clone(),
                range: Some(lwd::BlockRange {
                    start: Some(lwd::BlockId {
                        height: args.start_height,
                        hash: vec![],
                    }),
                    end: Some(lwd::BlockId {
                        height: latest.height,
                        hash: vec![],
                    }),
                    pool_types: vec![],
                }),
            })
            .await
            .with_context(|| format!("GetTaddressTxids for {}", addr))?
            .into_inner();
        let mut tx_count = 0u32;
        println!("history (이 주소가 등장한 모든 tx — receive·send 모두):");
        while let Some(raw_tx) = tx_stream.message().await? {
            // raw_tx.height: tx 가 mined 된 height. raw_tx.data: full tx bytes (대량이라 안 출력).
            // txid 는 tx bytes 에서 직접 못 뽑으니 hash 출력은 생략 — height 만 보여줌.
            // (더 자세한 정보 필요 시 GetTaddressTransactions + 직접 parse 가능.)
            println!(
                "  height={:>8}  tx_data_len={} bytes",
                raw_tx.height,
                raw_tx.data.len()
            );
            tx_count += 1;
            if tx_count >= 50 {
                println!("  ... (50개에서 자름)");
                break;
            }
        }
        if tx_count == 0 {
            println!("  (none — 이 주소로 들어오거나 나간 transparent tx 가 아직 없음)");
        } else {
            println!("  → 총 {} tx 발견. 받기/보내기 둘 다 포함.", tx_count);
        }
        println!();
    }

    eprintln!("[check-taddr] done.");
    Ok(())
}
