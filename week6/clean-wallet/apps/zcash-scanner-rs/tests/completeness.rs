//! 블록 구간 완전성 검증 단위 테스트 (A2).
//!
//! 네트워크/lightwalletd 없이 CompletenessChecker만 사용해 4가지 케이스 검증:
//! 1) 정상 — 연속 height + 정상 prev_hash chain → 통과
//! 2) 중간 height 누락 → height 위반 에러
//! 3) prev_hash chain 끊김 → chain break 에러
//! 4) 끝 부족 (스트림이 일찍 끝남) → finalize에서 길이 불일치 에러
//!
//! CompactBlock은 프로토버프 정의 그대로 (lwd::CompactBlock).

use zcash_scanner_rs::{lwd, CompletenessChecker};

fn block(height: u64, hash: &[u8], prev_hash: &[u8]) -> lwd::CompactBlock {
    lwd::CompactBlock {
        height,
        hash: hash.to_vec(),
        prev_hash: prev_hash.to_vec(),
        time: 0,
        header: vec![],
        vtx: vec![],
        chain_metadata: None,
    }
}

fn block_with_tx(height: u64, hash: &[u8], prev_hash: &[u8], txids: &[&[u8]]) -> lwd::CompactBlock {
    let mut b = block(height, hash, prev_hash);
    b.vtx = txids
        .iter()
        .enumerate()
        .map(|(i, txid)| lwd::CompactTx {
            index: i as u64,
            txid: txid.to_vec(),
            ..Default::default()
        })
        .collect();
    b
}

#[test]
fn case_1_정상_연속_블록은_통과한다() {
    let mut c = CompletenessChecker::new(100, 102);
    c.add_block(&block(100, b"h100", b"h099")).unwrap();
    c.add_block(&block(101, b"h101", b"h100")).unwrap();
    c.add_block(&block(102, b"h102", b"h101")).unwrap();
    let (tx_locs, blocks_seen) = c.finalize().unwrap();
    assert_eq!(blocks_seen, 3);
    assert!(tx_locs.is_empty());
}

#[test]
fn case_1b_정상_블록의_tx도_수집된다() {
    let mut c = CompletenessChecker::new(100, 101);
    c.add_block(&block_with_tx(100, b"h100", b"h099", &[b"tx_a", b"tx_b"]))
        .unwrap();
    c.add_block(&block_with_tx(101, b"h101", b"h100", &[b"tx_c"]))
        .unwrap();
    let (tx_locs, blocks_seen) = c.finalize().unwrap();
    assert_eq!(blocks_seen, 2);
    assert_eq!(tx_locs.len(), 3);
    assert_eq!(tx_locs[0], (100u32, b"tx_a".to_vec()));
    assert_eq!(tx_locs[1], (100u32, b"tx_b".to_vec()));
    assert_eq!(tx_locs[2], (101u32, b"tx_c".to_vec()));
}

#[test]
fn case_2_중간_height_누락은_거부한다() {
    let mut c = CompletenessChecker::new(100, 102);
    c.add_block(&block(100, b"h100", b"h099")).unwrap();
    // height 101을 건너뛰고 102가 옴 → 즉시 에러.
    let err = c
        .add_block(&block(102, b"h102", b"h101"))
        .expect_err("missing height 101 should be rejected");
    let msg = format!("{:#}", err);
    assert!(
        msg.contains("expected height 101"),
        "error should mention expected height 101, got: {}",
        msg
    );
}

#[test]
fn case_3_prev_hash_chain_끊김은_거부한다() {
    let mut c = CompletenessChecker::new(100, 102);
    c.add_block(&block(100, b"h100", b"h099")).unwrap();
    c.add_block(&block(101, b"h101", b"h100")).unwrap();
    // height 102가 height 101의 hash(h101)를 prev_hash로 가져야 하는데 다른 값 → 에러.
    let err = c
        .add_block(&block(102, b"h102", b"forged_prev"))
        .expect_err("prev_hash mismatch should be rejected");
    let msg = format!("{:#}", err);
    assert!(
        msg.contains("prev_hash chain break"),
        "error should mention prev_hash chain break, got: {}",
        msg
    );
    assert!(
        msg.contains("102"),
        "error should mention height 102, got: {}",
        msg
    );
}

#[test]
fn case_4_끝_블록_부족은_finalize에서_거부한다() {
    let mut c = CompletenessChecker::new(100, 103);
    c.add_block(&block(100, b"h100", b"h099")).unwrap();
    c.add_block(&block(101, b"h101", b"h100")).unwrap();
    // 마지막 두 블록(102, 103)이 도착하지 않은 채 finalize → 길이 불일치.
    let err = c
        .finalize()
        .expect_err("short range should be rejected at finalize");
    let msg = format!("{:#}", err);
    assert!(
        msg.contains("got 2 blocks"),
        "error should mention got 2 blocks, got: {}",
        msg
    );
    assert!(
        msg.contains("expected 4"),
        "error should mention expected 4 (100..103 inclusive), got: {}",
        msg
    );
}

#[test]
fn case_4b_빈_스트림도_finalize에서_거부한다() {
    let c = CompletenessChecker::new(100, 100);
    // 한 블록도 안 들어왔는데 finalize.
    let err = c.finalize().expect_err("empty stream should be rejected");
    let msg = format!("{:#}", err);
    assert!(msg.contains("got 0 blocks"), "got: {}", msg);
    assert!(msg.contains("expected 1"), "got: {}", msg);
}

#[test]
fn case_5_역순_또는_중복_블록도_거부한다() {
    let mut c = CompletenessChecker::new(100, 102);
    c.add_block(&block(100, b"h100", b"h099")).unwrap();
    // 같은 블록 다시 보냄 → height 위반.
    let err = c
        .add_block(&block(100, b"h100", b"h099"))
        .expect_err("duplicate block should be rejected");
    assert!(format!("{:#}", err).contains("expected height 101"));
}

#[test]
fn case_6_단일_블록_정상() {
    let mut c = CompletenessChecker::new(500_000, 500_000);
    c.add_block(&block(500_000, b"hX", b"hPrev")).unwrap();
    let (_, blocks) = c.finalize().unwrap();
    assert_eq!(blocks, 1);
}

// --- C1: start_anchor + PoW verification ---

#[test]
fn start_anchor_가_일치하면_통과() {
    let mut c = CompletenessChecker::new(100, 101).with_start_anchor(b"h099".to_vec());
    c.add_block(&block(100, b"h100", b"h099")).unwrap();
    c.add_block(&block(101, b"h101", b"h100")).unwrap();
    let (_, blocks) = c.finalize().unwrap();
    assert_eq!(blocks, 2);
}

#[test]
fn start_anchor_가_다르면_거부() {
    let mut c = CompletenessChecker::new(100, 101).with_start_anchor(b"expected_anchor".to_vec());
    let err = c
        .add_block(&block(100, b"h100", b"different_anchor"))
        .expect_err("anchor 가 다르면 첫 블록에서 throw");
    let msg = format!("{:#}", err);
    assert!(msg.contains("anchor"), "msg: {}", msg);
}

#[test]
fn pow_verifier_가_켜져있는데_header_가_비어있으면_거부() {
    let mut c = CompletenessChecker::new(100, 100).with_pow_verifier();
    // block.header = vec![] (default — 비어 있음).
    let err = c
        .add_block(&block(100, b"h100", b"h099"))
        .expect_err("header 없으면 PoW 검증 불가 → throw");
    let msg = format!("{:#}", err);
    assert!(msg.contains("header") || msg.contains("PoW"), "msg: {}", msg);
}
