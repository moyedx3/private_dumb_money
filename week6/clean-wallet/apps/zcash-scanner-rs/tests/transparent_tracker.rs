//! C2 transparent-only 송금 감지 단위 테스트.
//!
//! OurTransparentTracker 의 핵심 동작:
//! 1) add_vouts: 우리 t-addr 로 가는 vout 만 우리 UTXO 로 저장.
//! 2) consume_vins: vin 의 prevout 이 우리 UTXO 면 true 반환 + UTXO 제거.
//! 3) 흐름: tx1 의 우리 vout 을 tx2 가 spend → consume_vins == true → outgoing 신호.

use std::collections::HashSet;

use zcash_scanner_rs::{OurTransparentTracker, OutPoint};
use zcash_transparent::address::TransparentAddress;

fn t_pkh(byte: u8) -> TransparentAddress {
    TransparentAddress::PublicKeyHash([byte; 20])
}

fn t_sh(byte: u8) -> TransparentAddress {
    TransparentAddress::ScriptHash([byte; 20])
}

#[test]
fn tracker_초기상태에는_우리_UTXO_가_없다() {
    let our: HashSet<TransparentAddress> = [t_pkh(0x01)].into_iter().collect();
    let mut t = OurTransparentTracker::new(our);
    let vin = OutPoint {
        txid: vec![0xaa; 32],
        n: 0,
    };
    // 우리 UTXO 가 없으므로 어떤 vin 도 우리 spend 가 아님.
    assert_eq!(t.consume_vins([vin]), false);
}

#[test]
fn add_vouts_는_우리_taddr_만_저장한다() {
    let our: HashSet<TransparentAddress> = [t_pkh(0x01), t_pkh(0x02)].into_iter().collect();
    let our_clone = our.clone();
    let mut t = OurTransparentTracker::new(our);

    let our_addr_1 = t_pkh(0x01);
    let foreign_addr = t_pkh(0xff);
    let txid: Vec<u8> = vec![0xaa; 32];

    // (0, our_addr_1, 100), (1, foreign_addr, 999), (2, our_addr_1, 50)
    let vouts: Vec<(u32, &TransparentAddress, u64)> = vec![
        (0u32, &our_addr_1, 100u64),
        (1u32, &foreign_addr, 999u64),
        (2u32, &our_addr_1, 50u64),
    ];
    t.add_vouts(&txid, vouts.iter().map(|(n, a, v)| (*n, *a, *v)));

    // 우리 vout 두 개 추적, foreign 은 무시 — consume 으로 확인.
    let vin_ours_0 = OutPoint {
        txid: txid.clone(),
        n: 0,
    };
    let vin_ours_2 = OutPoint {
        txid: txid.clone(),
        n: 2,
    };
    let vin_foreign = OutPoint {
        txid: txid.clone(),
        n: 1,
    };
    // foreign 은 ours 가 아니므로 consume_vins 가 false
    assert_eq!(t.consume_vins([vin_foreign.clone()]), false);
    // 우리 vout 0 consume → true
    assert_eq!(t.consume_vins([vin_ours_0.clone()]), true);
    // 다시 consume → 이미 spent → false
    assert_eq!(t.consume_vins([vin_ours_0]), false);
    // 우리 vout 2 는 아직 살아 있음 → true
    assert_eq!(t.consume_vins([vin_ours_2]), true);

    // 확인 — 우리 주소 개수는 보존
    let _ = our_clone;
}

#[test]
fn 흐름_tx1_우리_vout_을_tx2_가_spend() {
    let our_addr = t_pkh(0x42);
    let foreign = t_pkh(0xee);
    let our_set: HashSet<TransparentAddress> = [our_addr.clone()].into_iter().collect();
    let mut t = OurTransparentTracker::new(our_set);

    // tx1: vout[0] 우리 주소로 1000.
    let tx1_id: Vec<u8> = vec![0x11; 32];
    let vouts_tx1: Vec<(u32, &TransparentAddress, u64)> = vec![(0u32, &our_addr, 1000u64)];
    t.add_vouts(&tx1_id, vouts_tx1.iter().map(|(n, a, v)| (*n, *a, *v)));

    // tx2: vin[0] = (tx1_id, 0) → 우리 UTXO spend.
    let outgoing = t.consume_vins([OutPoint {
        txid: tx1_id.clone(),
        n: 0,
    }]);
    assert_eq!(outgoing, true, "tx2 는 우리 UTXO 를 spend → outgoing");

    // tx2 의 vout[0] 은 foreign — 호출자가 이걸 외부 수취인으로 기록할 것.
    assert!(!t.is_ours(&foreign));
    assert!(t.is_ours(&our_addr));
}

#[test]
fn consume_vins_여러개_중_하나만_우리_라도_true() {
    let our_addr = t_pkh(0x01);
    let foreign = t_pkh(0xff);
    let our_set: HashSet<TransparentAddress> = [our_addr.clone()].into_iter().collect();
    let mut t = OurTransparentTracker::new(our_set);

    let our_tx: Vec<u8> = vec![0x11; 32];
    let other_tx: Vec<u8> = vec![0x22; 32];

    let vouts: Vec<(u32, &TransparentAddress, u64)> = vec![(0u32, &our_addr, 500u64)];
    t.add_vouts(&our_tx, vouts.iter().map(|(n, a, v)| (*n, *a, *v)));
    let _ = foreign;

    // tx vins: 한쪽은 모르는 prevout, 한쪽은 우리 prevout.
    let vins = vec![
        OutPoint {
            txid: other_tx,
            n: 7,
        },
        OutPoint {
            txid: our_tx,
            n: 0,
        },
    ];
    assert_eq!(t.consume_vins(vins), true);
}

#[test]
fn p2sh_도_같이_추적() {
    let our_sh = t_sh(0xab);
    let our_set: HashSet<TransparentAddress> = [our_sh.clone()].into_iter().collect();
    let mut t = OurTransparentTracker::new(our_set);

    let tx: Vec<u8> = vec![0x33; 32];
    let vouts: Vec<(u32, &TransparentAddress, u64)> = vec![(0u32, &our_sh, 12345u64)];
    t.add_vouts(&tx, vouts.iter().map(|(n, a, v)| (*n, *a, *v)));

    assert_eq!(
        t.consume_vins([OutPoint {
            txid: tx,
            n: 0
        }]),
        true,
        "P2SH 도 추적 대상"
    );
}

#[test]
fn 우리_주소_없으면_addr_count_는_0() {
    let t = OurTransparentTracker::new(HashSet::new());
    assert_eq!(t.our_addr_count(), 0);
}

#[test]
fn 우리_주소가_여러개면_count_가_그만큼() {
    let our: HashSet<TransparentAddress> =
        [t_pkh(0x01), t_pkh(0x02), t_sh(0x03)].into_iter().collect();
    let t = OurTransparentTracker::new(our);
    assert_eq!(t.our_addr_count(), 3);
}
