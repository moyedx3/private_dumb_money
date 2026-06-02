//! zcash-scanner-rs 바이너리.
//!
//! Node 스캐너가 stdin으로 JSON 요청을 주고, 우리가 stdout으로 JSON 결과를 돌려준다.
//! UFVK + 블록 범위 → lightwalletd에서 full tx 가져와 OVK로 출금 record 복원.
//!
//! 검증 로직(블록 완전성·t-addr 추출·Orchard 인코딩)은 lib.rs로 분리 — 단위 테스트 가능.
//! 여기 main.rs는 IPC + 네트워크 + zcash_client_backend 호출 같은 부수효과만 다룬다.
//!
//! Phase B 강화 (신뢰 모델 보강):
//! - **블록 구간 완전성 검증** — height 연속 + prev_hash chain link.
//!   lightwalletd가 임의로 블록을 누락·치환하면 throw (D6 completeness 보강).
//! - **Transparent vout 처리** — 우리가 보낸 tx의 t-addr 수취인까지 잡는다 (OFAC
//!   SDN의 ZEC 주소 다수가 t-addr이므로 핵심 correctness 항목).
//! - **Orchard 수취인 unified 인코딩** — `u1...` 정식 형태. 미스매치 회피.
//! - **UFVK 메모리 zeroize** — drop 시 비밀이 든 페이지가 0으로 채워진다.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{self, Cursor, Read};

use zcash_client_backend::{decrypt_transaction, TransferType};
use zcash_keys::address::Address;
use zcash_keys::keys::UnifiedFullViewingKey;
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{BlockHeight, BranchId, Network};
use zcash_transparent::address::TransparentAddress;
use zcash_transparent::keys::{IncomingViewingKey, NonHardenedChildIndex};
use zeroize::Zeroizing;

use zcash_scanner_rs::{
    encode_orchard_recipient, extract_taddr, lwd, CompletenessChecker, OurTransparentTracker,
    OutPoint,
};

/// UFVK 의 transparent component 에서 derive 할 BIP44 외/내부 인덱스 개수.
/// 0..N 인덱스를 derive 해서 우리 t-addr 집합을 구성. 일반 지갑에서 이보다 많은 주소를 쓰는 경우는 드물지만
/// 필요하면 ScanRequest 로 조절 가능하게 확장 가능.
const TRANSPARENT_DERIVE_INDICES: u32 = 20;

type AccountId = u32;
const ACCOUNT_ID: AccountId = 0;

#[derive(Deserialize, Debug)]
struct ScanRequest {
    /// "main" | "test"
    network: String,
    /// 예: "https://lwd.zec.pro:443"
    lightwalletd_url: String,
    /// Unified Full Viewing Key 문자열 (uview...). enclave 내부에서만 평문.
    ufvk: String,
    start_height: u32,
    end_height: u32,
    /// 선택: 시작 앵커 — start_height 직전 블록의 hash (hex, LE 32바이트).
    /// 지정하면 첫 블록의 prev_hash 가 이 값과 일치해야 한다 (C1 — 신뢰 체크포인트).
    #[serde(default)]
    start_anchor_hash_hex: Option<String>,
    /// 선택: 각 블록의 PoW(Equihash + difficulty target)를 검증한다 (C1).
    /// 활성화 시 lightwalletd 가 CompactBlock.header 를 보내야 한다 — 안 보내면 throw.
    #[serde(default)]
    verify_pow: bool,
}

#[derive(Serialize, Debug, Clone)]
struct BlockRangeSer {
    start: u32,
    end: u32,
}

#[derive(Serialize, Debug)]
struct OutgoingRecord {
    txid: String,
    block_height: u32,
    /// 정규 인코딩된 수취인 주소 (sapling zs1.., orchard u1.., transparent t1../t3..).
    recipient_address: String,
    amount_zat: String,
    /// "sapling" | "orchard" | "transparent"
    pool: String,
}

#[derive(Serialize, Debug)]
struct ScanResponse {
    ok: bool,
    scanned_range: Option<BlockRangeSer>,
    /// lightwalletd가 알려준 최신 블록 — 연결성·범위 검증에 쓰임.
    lightwalletd_tip: Option<u64>,
    outgoing_records: Vec<OutgoingRecord>,
    error: Option<String>,
    /// 진행 상태·검증 메시지. UFVK·수취인 등 비밀은 들어가지 않는다.
    notes: Vec<String>,
}

#[tokio::main]
async fn main() {
    let response = match run().await {
        Ok(r) => r,
        Err(e) => ScanResponse {
            ok: false,
            scanned_range: None,
            lightwalletd_tip: None,
            outgoing_records: vec![],
            error: Some(format!("{:#}", e)),
            notes: vec![],
        },
    };
    let exit_ok = response.ok;
    let json = serde_json::to_string_pretty(&response).unwrap_or_else(|_| {
        String::from("{\"ok\":false,\"error\":\"failed to serialize response\"}")
    });
    println!("{}", json);
    if !exit_ok {
        std::process::exit(1);
    }
}

async fn run() -> Result<ScanResponse> {
    // 1. stdin에서 JSON 요청 읽기.
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).context("read stdin")?;
    let req: ScanRequest = serde_json::from_str(&input).context("invalid input JSON")?;

    // UFVK는 비밀 — Zeroizing으로 감싸 drop 시 메모리 0으로 채운다.
    let ufvk_secret: Zeroizing<String> = Zeroizing::new(req.ufvk.clone());

    eprintln!(
        "[zcash-scanner-rs] network={} lwd={} range=[{}..{}]",
        req.network, req.lightwalletd_url, req.start_height, req.end_height
    );

    if req.end_height < req.start_height {
        bail!(
            "end_height({}) < start_height({})",
            req.end_height,
            req.start_height
        );
    }

    // 2. 네트워크 + UFVK 파싱.
    let network = match req.network.as_str() {
        "main" | "mainnet" => Network::MainNetwork,
        "test" | "testnet" => Network::TestNetwork,
        other => return Err(anyhow!("invalid network: {} (main|mainnet|test|testnet)", other)),
    };
    let ufvk = UnifiedFullViewingKey::decode(&network, ufvk_secret.as_str())
        .map_err(|e| anyhow!("UFVK decode failed: {}", e))?;

    // C2: UFVK 의 transparent 컴포넌트에서 외/내부 IVK 를 거쳐 t-addr 0..N 을 derive.
    // 이 집합으로 (1) 우리 vout 을 추적해 UTXO 를 모으고, (2) 우리 vin 인지 (= outgoing) 판정한다.
    let our_taddrs: HashSet<TransparentAddress> = {
        let mut s: HashSet<TransparentAddress> = HashSet::new();
        if let Some(apk) = ufvk.transparent() {
            let external = apk
                .derive_external_ivk()
                .map_err(|e| anyhow!("derive external IVK failed: {}", e))?;
            let internal = apk
                .derive_internal_ivk()
                .map_err(|e| anyhow!("derive internal IVK failed: {}", e))?;
            for i in 0..TRANSPARENT_DERIVE_INDICES {
                let idx = NonHardenedChildIndex::from_index(i)
                    .ok_or_else(|| anyhow!("invalid child index {}", i))?;
                if let Ok(addr) = external.derive_address(idx) {
                    s.insert(addr);
                }
                if let Ok(addr) = internal.derive_address(idx) {
                    s.insert(addr);
                }
            }
        }
        s
    };
    eprintln!(
        "[zcash-scanner-rs] derived {} transparent addresses from UFVK",
        our_taddrs.len()
    );

    let mut ufvks: HashMap<AccountId, UnifiedFullViewingKey> = HashMap::new();
    ufvks.insert(ACCOUNT_ID, ufvk);

    // 3. lightwalletd 연결 + tip 확인.
    // HTTPS URL 에서는 *명시적으로* TLS config 를 attach 해야 한다 — tonic 0.14 의
    // `connect(url)` 단축 함수는 scheme 만으로 TLS 를 자동 활성화하지 않아서, 그냥
    // 부르면 plain HTTP/2 로 시도하다 server 가 frame size error 또는 connection reset
    // 으로 끊는다. ClientTlsConfig::new() 가 webpki-roots feature 와 결합해 standard
    // CA bundle 을 사용한다.
    use tonic::transport::{Channel, ClientTlsConfig};
    let is_https = req.lightwalletd_url.starts_with("https://");
    let endpoint = Channel::from_shared(req.lightwalletd_url.clone())
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
    let latest = client
        .get_latest_block(lwd::ChainSpec {})
        .await
        .context("GetLatestBlock")?
        .into_inner();
    let chain_tip = Some(BlockHeight::from_u32(latest.height as u32));
    eprintln!(
        "[zcash-scanner-rs] lightwalletd tip: height={}",
        latest.height
    );
    if (req.end_height as u64) > latest.height {
        bail!(
            "end_height({}) > lightwalletd tip({}) — chain too short",
            req.end_height,
            latest.height
        );
    }

    // 4. [start, end] compact 블록 스트림 → 완전성 검증 + (height, txid) 수집.
    let range_req = lwd::BlockRange {
        start: Some(lwd::BlockId {
            height: req.start_height as u64,
            hash: vec![],
        }),
        end: Some(lwd::BlockId {
            height: req.end_height as u64,
            hash: vec![],
        }),
        pool_types: vec![],
    };
    let mut block_stream = client
        .get_block_range(range_req)
        .await
        .context("GetBlockRange")?
        .into_inner();

    let mut checker = CompletenessChecker::new(req.start_height, req.end_height);
    if let Some(anchor_hex) = &req.start_anchor_hash_hex {
        let bytes = hex::decode(anchor_hex)
            .map_err(|e| anyhow!("invalid start_anchor_hash_hex: {}", e))?;
        if bytes.len() != 32 {
            bail!("start_anchor_hash_hex must be 32 bytes (got {})", bytes.len());
        }
        checker = checker.with_start_anchor(bytes);
    }
    if req.verify_pow {
        checker = checker.with_pow_verifier();
        eprintln!("[zcash-scanner-rs] PoW verification enabled");
    }
    while let Some(cb) = block_stream.message().await.context("block stream")? {
        checker.add_block(&cb)?;
    }
    let (tx_locations, blocks_seen) = checker.finalize()?;
    eprintln!(
        "[zcash-scanner-rs] completeness ok: {} blocks · {} txs",
        blocks_seen,
        tx_locations.len()
    );

    // 5. 각 tx: full 가져와 → parse → decrypt_transaction → 수취인 추출.
    let mut records: Vec<OutgoingRecord> = vec![];
    let mut tracker = OurTransparentTracker::new(our_taddrs);
    for (height, txid) in &tx_locations {
        let raw = client
            .get_transaction(lwd::TxFilter {
                block: None,
                index: 0,
                hash: txid.clone(),
            })
            .await
            .with_context(|| format!("GetTransaction at height {}", height))?
            .into_inner();

        let branch_id = BranchId::for_height(&network, BlockHeight::from_u32(*height));
        let tx = Transaction::read(Cursor::new(&raw.data), branch_id)
            .with_context(|| format!("parse tx at height {}", height))?;

        let decrypted = decrypt_transaction(
            &network,
            Some(BlockHeight::from_u32(*height)),
            chain_tip,
            &tx,
            &ufvks,
        );

        // (a) Sapling outgoing — zs1... 인코딩.
        let mut any_outgoing_shielded = false;
        for output in decrypted.sapling_outputs() {
            if !matches!(output.transfer_type(), TransferType::Outgoing) {
                continue;
            }
            any_outgoing_shielded = true;
            let recipient = output.note().recipient();
            let value: u64 = output.note_value().into();
            records.push(OutgoingRecord {
                txid: hex::encode(txid),
                block_height: *height,
                recipient_address: Address::Sapling(recipient).encode(&network),
                amount_zat: value.to_string(),
                pool: "sapling".to_string(),
            });
        }

        // (b) Orchard outgoing — u1... unified 정식 인코딩.
        for output in decrypted.orchard_outputs() {
            if !matches!(output.transfer_type(), TransferType::Outgoing) {
                continue;
            }
            any_outgoing_shielded = true;
            let recipient = output.note().recipient();
            let value: u64 = output.note_value().into();
            records.push(OutgoingRecord {
                txid: hex::encode(txid),
                block_height: *height,
                recipient_address: encode_orchard_recipient(recipient, &network),
                amount_zat: value.to_string(),
                pool: "orchard".to_string(),
            });
        }

        // (c) Transparent 처리.
        //   c1) shielded outgoing 이 이미 잡힌 tx 면, 같은 tx 의 transparent 출력도 우리가 보낸 수취인.
        //       (P2PKH/P2SH 표준 script 만 추출, 나머지는 무시.)
        //   c2) (C2) shielded outgoing 이 없더라도, 이 tx 의 vin 이 우리 t-addr UTXO 를 spend 하면
        //       transparent-only 송금이다 → 마찬가지로 vout 의 비-우리 t-addr 를 수취인으로 기록.
        //       동시에 vout 의 우리 t-addr 행은 다음 tx 가 spend 할 수 있게 tracker 에 저장.
        let mut treat_as_outgoing = any_outgoing_shielded;

        if let Some(bundle) = tx.transparent_bundle() {
            // vins 먼저 — tracker 가 우리 UTXO spend 를 감지.
            let vin_outpoints = bundle.vin.iter().map(|vin| {
                // OutPoint::hash() 는 prevout 해시(LE). vout idx 는 OutPoint::n().
                let prev_op = vin.prevout();
                OutPoint {
                    txid: prev_op.hash().to_vec(),
                    n: prev_op.n(),
                }
            });
            if tracker.consume_vins(vin_outpoints) {
                treat_as_outgoing = true;
            }

            if treat_as_outgoing {
                // 이 tx 는 우리 outgoing — vout 의 *비-우리* t-addr 를 수취인으로 기록.
                for vout in &bundle.vout {
                    if let Some(taddr) = extract_taddr(vout.script_pubkey()) {
                        if tracker.is_ours(&taddr) {
                            continue; // 우리 change/self-pay — 수취인 아님.
                        }
                        let value: u64 = u64::from(vout.value());
                        records.push(OutgoingRecord {
                            txid: hex::encode(txid),
                            block_height: *height,
                            recipient_address: Address::Transparent(taddr).encode(&network),
                            amount_zat: value.to_string(),
                            pool: "transparent".to_string(),
                        });
                    }
                }
            }

            // vout 처리 — 우리 t-addr 로 가는 출력을 tracker 에 추가 (다음 tx 의 vin 매칭용).
            let mut vouts_for_tracker: Vec<(u32, TransparentAddress, u64)> = vec![];
            for (i, vout) in bundle.vout.iter().enumerate() {
                if let Some(taddr) = extract_taddr(vout.script_pubkey()) {
                    if tracker.is_ours(&taddr) {
                        vouts_for_tracker.push((i as u32, taddr, u64::from(vout.value())));
                    }
                }
            }
            tracker.add_vouts(
                txid,
                vouts_for_tracker.iter().map(|(n, a, v)| (*n, a, *v)),
            );
        }
    }

    Ok(ScanResponse {
        ok: true,
        scanned_range: Some(BlockRangeSer {
            start: req.start_height,
            end: req.end_height,
        }),
        lightwalletd_tip: Some(latest.height),
        outgoing_records: records,
        error: None,
        notes: vec![format!(
            "verified {} blocks · {} tx · prev_hash chain ok",
            blocks_seen,
            tx_locations.len()
        )],
    })
}
