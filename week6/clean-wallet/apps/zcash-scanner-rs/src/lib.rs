//! zcash-scanner-rs 라이브러리.
//!
//! main.rs는 stdin/stdout IPC + 네트워크 호출 같은 “부수효과”를 담당하고,
//! 진짜 검증 로직(블록 완전성, transparent script 추출, Orchard 인코딩)은 여기 둔다.
//! 그래야 lightwalletd 없이 단위 테스트가 가능하다.

use anyhow::{bail, Context, Result};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use zcash_keys::address::{Address, UnifiedAddress};
use zcash_primitives::block::BlockHeader;
use zcash_protocol::consensus::Network;
use zcash_transparent::address::{Script, TransparentAddress};

pub mod lwd {
    tonic::include_proto!("cash.z.wallet.sdk.rpc");
}

/// 블록 구간을 처음부터 끝까지 받으며 height 연속·prev_hash chain·구간 길이를 검증한다.
///
/// 옵션:
/// - `with_start_anchor(hash)`: 첫 블록의 prev_hash 가 이 값과 일치해야 한다. lightwalletd 가
///   *시작점도 위조* 하는 시나리오를 막는 신뢰 체크포인트 (C1).
/// - `with_pow_verifier()`: 각 블록의 raw header 에 대해 Equihash + target 검증 (C1).
///   `CompactBlock.header` 가 비어 있으면 throw — 서버가 헤더를 보내도록 설정돼야 한다.
///
/// lightwalletd 신뢰를 약화시키는 핵심 — 블록을 누락하거나 끼워넣으면 throw.
pub struct CompletenessChecker {
    start_height: u32,
    end_height: u32,
    expected_height: u64,
    prev_hash_opt: Option<Vec<u8>>,
    blocks_seen: u64,
    tx_locations: Vec<(u32, Vec<u8>)>,
    pow_verifier: Option<PowVerifier>,
    start_anchor_hash: Option<Vec<u8>>,
}

impl CompletenessChecker {
    pub fn new(start_height: u32, end_height: u32) -> Self {
        Self {
            start_height,
            end_height,
            expected_height: start_height as u64,
            prev_hash_opt: None,
            blocks_seen: 0,
            tx_locations: vec![],
            pow_verifier: None,
            start_anchor_hash: None,
        }
    }

    /// 시작 앵커 hash 를 지정한다. 첫 블록의 prev_hash 가 이 값과 일치해야 통과.
    pub fn with_start_anchor(mut self, anchor_hash: Vec<u8>) -> Self {
        self.start_anchor_hash = Some(anchor_hash);
        self
    }

    /// 기본 PoW 검증기(Equihash 200/9)를 활성화한다.
    pub fn with_pow_verifier(mut self) -> Self {
        self.pow_verifier = Some(PowVerifier::new());
        self
    }

    /// 다음 블록을 받아 검증을 수행한다.
    /// 1) cb.height == expected_height (구간 끊김·중복·역순 검출)
    /// 2) 이전 블록을 본 적이 있으면 cb.prev_hash == prev.hash (chain 끊김 검출)
    ///    + 첫 블록일 때 start_anchor 가 설정돼 있으면 cb.prev_hash == anchor
    /// 3) pow_verifier 가 설정돼 있으면 cb.header 의 Equihash + target 검증.
    ///    추가로 verifier 가 계산한 hash 가 cb.hash 와 같은지 cross-check.
    /// 통과 시 그 블록의 (height, txid)를 모은다.
    pub fn add_block(&mut self, cb: &lwd::CompactBlock) -> Result<()> {
        if cb.height != self.expected_height {
            bail!(
                "completeness violation: expected height {}, got {}",
                self.expected_height,
                cb.height
            );
        }
        if let Some(prev) = &self.prev_hash_opt {
            if cb.prev_hash != *prev {
                bail!(
                    "completeness violation: prev_hash chain break at height {}",
                    cb.height
                );
            }
        } else if let Some(anchor) = &self.start_anchor_hash {
            // 첫 블록: start_anchor 와 prev_hash 가 일치해야 함.
            if cb.prev_hash != *anchor {
                bail!(
                    "start anchor mismatch: first block prev_hash != expected anchor (height {})",
                    cb.height
                );
            }
        }

        // PoW 검증 (옵션). header 가 비어 있으면 verifier 가 켜져 있는 한 거부.
        if let Some(verifier) = &self.pow_verifier {
            if cb.header.is_empty() {
                bail!(
                    "PoW verification enabled but CompactBlock at height {} has no header — lightwalletd 가 헤더를 보내도록 설정돼야 한다",
                    cb.height
                );
            }
            let verified = verifier
                .verify_header(&cb.header)
                .with_context(|| format!("PoW verify at height {}", cb.height))?;
            // verifier 가 계산한 해시가 lightwalletd 가 알려준 cb.hash 와 일치해야 한다.
            if verified.hash.as_slice() != cb.hash.as_slice() {
                bail!(
                    "PoW header hash mismatch at height {}: header sha256d={}, but cb.hash={}",
                    cb.height,
                    hex::encode(verified.hash),
                    hex::encode(&cb.hash)
                );
            }
        }

        self.prev_hash_opt = Some(cb.hash.clone());
        for ctx in &cb.vtx {
            self.tx_locations.push((cb.height as u32, ctx.txid.clone()));
        }
        self.expected_height += 1;
        self.blocks_seen += 1;
        Ok(())
    }

    /// 모든 블록을 add_block으로 넣은 뒤 호출 — 구간 총 길이가 맞는지 확인하고
    /// 수집한 (height, txid)를 돌려준다.
    pub fn finalize(self) -> Result<(Vec<(u32, Vec<u8>)>, u64)> {
        let expected_blocks = (self.end_height as u64) - (self.start_height as u64) + 1;
        if self.blocks_seen != expected_blocks {
            bail!(
                "completeness violation: got {} blocks, expected {} for [{}..{}]",
                self.blocks_seen,
                expected_blocks,
                self.start_height,
                self.end_height
            );
        }
        Ok((self.tx_locations, self.blocks_seen))
    }
}

/// 표준 P2PKH `0x76 0xa9 0x14 [20바이트] 0x88 0xac` (25바이트) 또는
/// P2SH `0xa9 0x14 [20바이트] 0x87` (23바이트) scriptPubKey에서 t-addr를 뽑는다.
/// 비표준 스크립트는 무시 (수취인을 우리가 모른다고 보고 스크리닝 매칭에서 제외).
pub fn extract_taddr(script: &Script) -> Option<TransparentAddress> {
    // Script(Code(Vec<u8>)) — Vec까지 한 단계 더 들어간다.
    let bytes = &script.0.0;
    // P2PKH
    if bytes.len() == 25
        && bytes[0] == 0x76
        && bytes[1] == 0xa9
        && bytes[2] == 0x14
        && bytes[23] == 0x88
        && bytes[24] == 0xac
    {
        let mut h = [0u8; 20];
        h.copy_from_slice(&bytes[3..23]);
        return Some(TransparentAddress::PublicKeyHash(h));
    }
    // P2SH
    if bytes.len() == 23 && bytes[0] == 0xa9 && bytes[1] == 0x14 && bytes[22] == 0x87 {
        let mut h = [0u8; 20];
        h.copy_from_slice(&bytes[2..22]);
        return Some(TransparentAddress::ScriptHash(h));
    }
    None
}

// ===================================================================
//  PoW 헤더 체인 검증 (C1) — lightwalletd 신뢰 모델의 마지막 큰 갭 보강.
// ===================================================================

/// Zcash mainnet/testnet 의 Equihash 파라미터 (NU 활성 이후 동일).
/// Regtest 는 (96, 5) 또는 작은 값을 쓰지만 우리는 mainnet/testnet 만 다룬다.
pub const EQUIHASH_N: u32 = 200;
pub const EQUIHASH_K: u32 = 9;

/// 검증 통과한 헤더에서 우리 체인 검증에 필요한 두 값만 뽑아둔다.
#[derive(Debug, Clone)]
pub struct VerifiedHeader {
    /// 이 블록의 SHA-256d 해시 (raw bytes, LE).
    pub hash: [u8; 32],
    /// 이 블록의 prev_block 필드 (raw bytes, LE).
    pub prev_hash: [u8; 32],
}

/// nBits 컴팩트 표현 → 256-bit 타깃 (LE 바이트).
///
/// target = mantissa * 256^(exp - 3) 을 LE 32바이트로 펼친다.
/// Bitcoin/Zcash 규약: mantissa = nBits 의 하위 23비트(부호 비트 제외),
/// exponent = 상위 8비트.
pub fn target_from_bits(bits: u32) -> [u8; 32] {
    let exp = (bits >> 24) as i32;
    let mant = bits & 0x007f_ffff;
    let mut target = [0u8; 32];
    if exp <= 3 {
        // 매우 작은 타깃 — mantissa 를 오른쪽으로 시프트.
        let m = mant >> (8 * (3 - exp) as u32);
        target[0] = (m & 0xff) as u8;
        target[1] = ((m >> 8) & 0xff) as u8;
        target[2] = ((m >> 16) & 0xff) as u8;
    } else {
        let shift = (exp - 3) as usize;
        // shift + 3 > 32 면 오버플로우 (defensive)
        if shift + 3 > 32 {
            // 무효한 타깃: 0 → 어떤 해시도 < 0 일 수 없으므로 항상 실패하도록 둔다.
            return [0u8; 32];
        }
        target[shift] = (mant & 0xff) as u8;
        target[shift + 1] = ((mant >> 8) & 0xff) as u8;
        target[shift + 2] = ((mant >> 16) & 0xff) as u8;
    }
    target
}

/// LE 32바이트로 표현된 256-bit 정수 두 개를 정수 비교.
fn le_bytes_lt(a: &[u8; 32], b: &[u8; 32]) -> bool {
    for i in (0..32).rev() {
        if a[i] < b[i] {
            return true;
        }
        if a[i] > b[i] {
            return false;
        }
    }
    false
}

/// Zcash PoW 헤더 검증기.
///
/// 두 가지를 한다:
/// 1) Equihash(200, 9) 솔루션이 정말 그 헤더에 대해 valid 한지.
/// 2) `sha256d(header_bytes) < target(bits)` 인지.
///
/// 두 항목 모두 통과해야 그 헤더가 *실제로 PoW 작업을 통과한 블록* 임이 확인된다.
/// 즉, lightwalletd 가 "진짜로 보이는" 위조 블록을 만들어 줘도 PoW 가 안 맞으면 잡힌다.
pub struct PowVerifier {
    n: u32,
    k: u32,
}

impl Default for PowVerifier {
    fn default() -> Self {
        Self {
            n: EQUIHASH_N,
            k: EQUIHASH_K,
        }
    }
}

impl PowVerifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// 임의의 Equihash 파라미터로 생성. 테스트용 — mainnet/testnet 은 default 사용.
    pub fn with_params(n: u32, k: u32) -> Self {
        Self { n, k }
    }

    /// 시리얼라이즈된 Zcash 블록 헤더 바이트를 받아 검증.
    pub fn verify_header(&self, header_bytes: &[u8]) -> Result<VerifiedHeader> {
        let header =
            BlockHeader::read(Cursor::new(header_bytes)).context("parse block header")?;

        // 1) Equihash 솔루션 검증.
        // input = 헤더의 [version..bits] 까지 = 첫 108바이트.
        //   (version 4 + prev 32 + merkle 32 + final_sapling 32 + time 4 + bits 4 = 108)
        // nonce  = 다음 32바이트.
        // solution = 그 다음 (CompactSize 길이 프리픽스 포함) 바이트들 — 우리는 length 프리픽스를
        //            제외한 raw solution 만 넘긴다.
        if header_bytes.len() < 140 {
            bail!("header too short: {} bytes", header_bytes.len());
        }
        let input = &header_bytes[0..108];
        let nonce = &header_bytes[108..140];
        let soln = &header.solution; // BlockHeader 가 length-prefix 를 벗긴 raw bytes
        equihash::is_valid_solution(self.n, self.k, input, nonce, soln)
            .map_err(|e| anyhow::anyhow!("equihash solution invalid: {:?}", e))?;

        // 2) 블록 해시 < 타깃.
        // BlockHeader::hash() 는 sha256d 결과 (raw LE bytes — 표시용으로 reverse 되는 그 값).
        let hash = header.hash().0;
        let target = target_from_bits(header.bits);
        if !le_bytes_lt(&hash, &target) {
            bail!(
                "PoW target violation: hash={} >= target (bits=0x{:08x})",
                hex::encode(hash),
                header.bits
            );
        }

        Ok(VerifiedHeader {
            hash,
            prev_hash: header.prev_block.0,
        })
    }
}

// ===================================================================
//  C2 — transparent-only 송금 감지.
// ===================================================================

/// 트랜잭션 출력 위치 — vin 이 가리키는 (prev_txid, vout_index) 와 매칭 가능.
/// txid 는 protocol order (vin.prevout_txid 와 같은 형식).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutPoint {
    pub txid: Vec<u8>,
    pub n: u32,
}

/// 우리가 관측 가능한 transparent UTXO 정보 — 추적용.
#[derive(Debug, Clone)]
pub struct OurUtxo {
    pub addr: TransparentAddress,
    pub value_zat: u64,
}

/// 우리의 transparent UTXO 를 audit window 동안 추적한다 — 외부 자산 없이.
///
/// 알고리즘 (per tx, vin 먼저 vout 다음):
/// 1) 각 vin: prevout 이 our_utxos 안에 있으면 그 tx 는 우리의 outgoing — 호출자가 OutgoingRecord 만든다.
/// 2) 각 vout: 우리 t-addr 로 가는 출력이면 our_utxos 에 추가 (다음 tx 가 spend 할 수 있게).
///
/// 한계: audit window 시작 *이전* 에 생성된 UTXO 는 모른다 — 시작 시점의 our_utxos 가 비어 있으므로
/// 윈도우 안에 들어와서 다시 윈도우 안에서 나가는 transparent-only 자금만 잡힌다.
/// (윈도우 더 깊은 과거에 대한 추적은 GetAddressUtxos API 로 별도 보충 가능 — 후속.)
pub struct OurTransparentTracker {
    our_addrs: HashSet<TransparentAddress>,
    our_utxos: HashMap<OutPoint, OurUtxo>,
}

impl OurTransparentTracker {
    pub fn new(our_addrs: HashSet<TransparentAddress>) -> Self {
        Self {
            our_addrs,
            our_utxos: HashMap::new(),
        }
    }

    pub fn is_ours(&self, addr: &TransparentAddress) -> bool {
        self.our_addrs.contains(addr)
    }

    /// 우리 t-addr 개수 — 디버그/노트 용.
    pub fn our_addr_count(&self) -> usize {
        self.our_addrs.len()
    }

    /// vin 들 중 하나라도 우리 UTXO 를 spend 하면 true 를 돌려주고, 그 UTXO 들을 our_utxos 에서 제거.
    pub fn consume_vins<I>(&mut self, vins: I) -> bool
    where
        I: IntoIterator<Item = OutPoint>,
    {
        let mut any_ours = false;
        for op in vins {
            if self.our_utxos.remove(&op).is_some() {
                any_ours = true;
            }
        }
        any_ours
    }

    /// 한 tx 의 vouts 를 둘러보고 우리 t-addr 로 가는 출력을 our_utxos 에 추가한다.
    /// (txid 는 protocol order — vin.prevout_txid 와 동일 byte order.)
    pub fn add_vouts<'a, I>(&mut self, txid: &[u8], vouts: I)
    where
        I: IntoIterator<Item = (u32, &'a TransparentAddress, u64)>,
    {
        for (n, addr, value) in vouts {
            if self.our_addrs.contains(addr) {
                self.our_utxos.insert(
                    OutPoint {
                        txid: txid.to_vec(),
                        n,
                    },
                    OurUtxo {
                        addr: addr.clone(),
                        value_zat: value,
                    },
                );
            }
        }
    }
}

/// Orchard 수취인을 unified `u1...` bech32m으로 인코딩.
/// from_receivers가 None을 돌려주는 (이론상) 드문 경우엔 raw bytes hex로 폴백.
pub fn encode_orchard_recipient(
    recipient: ::orchard::Address,
    network: &Network,
) -> String {
    match UnifiedAddress::from_receivers(Some(recipient), None, None) {
        Some(ua) => Address::Unified(ua).encode(network),
        None => format!(
            "orchard-raw:{}",
            hex::encode(recipient.to_raw_address_bytes())
        ),
    }
}
