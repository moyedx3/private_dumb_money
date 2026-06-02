//! 인코딩 단위 테스트 (A3).
//!
//! - extract_taddr: P2PKH/P2SH scriptPubKey에서 t-addr 복원
//! - encode_orchard_recipient: orchard::Address → unified `u1...` bech32m
//!
//! 이 변환들이 잘못되면 sanctioned 매칭이 조용히 실패하므로(다른 인코딩끼리 비교) 핵심.

use std::io::Cursor;

use orchard::keys::{FullViewingKey, Scope, SpendingKey};
use zcash_keys::address::Address;
use zcash_protocol::consensus::Network;
use zcash_transparent::address::{Script, TransparentAddress};

use zcash_scanner_rs::{encode_orchard_recipient, extract_taddr};

// CompactSize 길이 프리픽스를 붙여 raw script bytes → Script로 변환.
// 우리 테스트 길이(25, 23, 기타 작은 수)는 단일 바이트로 들어감.
fn script_from_raw(raw: &[u8]) -> Script {
    assert!(raw.len() < 253, "len < 253 — single-byte CompactSize");
    let mut wire = Vec::with_capacity(raw.len() + 1);
    wire.push(raw.len() as u8);
    wire.extend_from_slice(raw);
    Script::read(Cursor::new(wire)).expect("Script::read")
}

fn p2pkh_script(hash160: &[u8; 20]) -> Script {
    let mut raw = Vec::with_capacity(25);
    raw.push(0x76);
    raw.push(0xa9);
    raw.push(0x14);
    raw.extend_from_slice(hash160);
    raw.push(0x88);
    raw.push(0xac);
    script_from_raw(&raw)
}

fn p2sh_script(hash160: &[u8; 20]) -> Script {
    let mut raw = Vec::with_capacity(23);
    raw.push(0xa9);
    raw.push(0x14);
    raw.extend_from_slice(hash160);
    raw.push(0x87);
    script_from_raw(&raw)
}

#[test]
fn extract_taddr_p2pkh_복원() {
    let hash: [u8; 20] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10, 0x11, 0x12, 0x13, 0x14,
    ];
    let script = p2pkh_script(&hash);
    let taddr = extract_taddr(&script).expect("P2PKH 추출");
    match taddr {
        TransparentAddress::PublicKeyHash(h) => assert_eq!(h, hash),
        other => panic!("expected PublicKeyHash, got {:?}", other),
    }
}

#[test]
fn extract_taddr_p2sh_복원() {
    let hash: [u8; 20] = [
        0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
        0x99, 0xaa, 0xbb, 0xcc, 0xdd,
    ];
    let script = p2sh_script(&hash);
    let taddr = extract_taddr(&script).expect("P2SH 추출");
    match taddr {
        TransparentAddress::ScriptHash(h) => assert_eq!(h, hash),
        other => panic!("expected ScriptHash, got {:?}", other),
    }
}

#[test]
fn extract_taddr_비표준_script는_무시() {
    // 빈 script
    assert!(extract_taddr(&script_from_raw(&[])).is_none());
    // 임의 길이/패턴 — OP_RETURN data carrier 예시 0x6a + payload
    assert!(extract_taddr(&script_from_raw(&[0x6a, 0x04, 0xde, 0xad, 0xbe, 0xef])).is_none());
    // P2PKH 길이는 맞지만 첫 바이트 틀린 경우
    let mut bad = vec![0x77u8, 0xa9, 0x14];
    bad.extend_from_slice(&[0u8; 20]);
    bad.extend_from_slice(&[0x88, 0xac]);
    assert!(extract_taddr(&script_from_raw(&bad)).is_none());
    // P2SH 길이는 맞지만 마지막 바이트 틀린 경우
    let mut bad2 = vec![0xa9u8, 0x14];
    bad2.extend_from_slice(&[0u8; 20]);
    bad2.push(0x88);
    assert!(extract_taddr(&script_from_raw(&bad2)).is_none());
}

#[test]
fn extract_taddr_p2pkh_zcash_mainnet_인코딩이_t1로_시작() {
    // P2PKH t-addr는 mainnet에서 t1 prefix.
    let hash: [u8; 20] = [0u8; 20];
    let taddr = extract_taddr(&p2pkh_script(&hash)).unwrap();
    let encoded = Address::Transparent(taddr).encode(&Network::MainNetwork);
    assert!(
        encoded.starts_with("t1"),
        "P2PKH 인코딩은 t1로 시작해야 함: {}",
        encoded
    );
}

#[test]
fn extract_taddr_p2sh_zcash_mainnet_인코딩이_t3로_시작() {
    // P2SH t-addr는 mainnet에서 t3 prefix.
    let hash: [u8; 20] = [0xffu8; 20];
    let taddr = extract_taddr(&p2sh_script(&hash)).unwrap();
    let encoded = Address::Transparent(taddr).encode(&Network::MainNetwork);
    assert!(
        encoded.starts_with("t3"),
        "P2SH 인코딩은 t3로 시작해야 함: {}",
        encoded
    );
}

// --- Orchard 인코딩 ---

fn deterministic_orchard_address() -> orchard::Address {
    // 결정적 시드 — 테스트마다 같은 주소가 나옴.
    let sk = SpendingKey::from_bytes([7; 32]).expect("valid SK");
    let fvk = FullViewingKey::from(&sk);
    fvk.address_at(0u32, Scope::External)
}

#[test]
fn orchard_인코딩은_mainnet에서_u1로_시작한다() {
    let addr = deterministic_orchard_address();
    let encoded = encode_orchard_recipient(addr, &Network::MainNetwork);
    assert!(
        encoded.starts_with("u1"),
        "mainnet orchard UA는 u1로 시작해야 함: {}",
        encoded
    );
    // bech32m은 raw 43바이트보다 충분히 김 (Padding + checksum).
    assert!(encoded.len() > 60, "u1 주소 길이가 너무 짧음: {}", encoded);
}

#[test]
fn orchard_인코딩은_testnet에서_utest1로_시작한다() {
    let addr = deterministic_orchard_address();
    let encoded = encode_orchard_recipient(addr, &Network::TestNetwork);
    assert!(
        encoded.starts_with("utest1"),
        "testnet orchard UA는 utest1로 시작해야 함: {}",
        encoded
    );
}

#[test]
fn orchard_인코딩_roundtrip은_같은_주소를_돌려준다() {
    let original = deterministic_orchard_address();
    let encoded = encode_orchard_recipient(original, &Network::MainNetwork);

    let decoded = Address::decode(&Network::MainNetwork, &encoded)
        .expect("decode unified u1...");
    let ua = match decoded {
        Address::Unified(ua) => ua,
        other => panic!("expected Unified, got {:?}", other),
    };
    let orchard_back = ua.orchard().expect("UA에 orchard receiver가 들어 있어야 함");

    // raw 43바이트 비교가 결정적인 동등 기준 (PartialEq가 있어도 안전).
    assert_eq!(
        orchard_back.to_raw_address_bytes(),
        original.to_raw_address_bytes()
    );
}

#[test]
fn orchard_인코딩은_결정적이다() {
    let a = encode_orchard_recipient(deterministic_orchard_address(), &Network::MainNetwork);
    let b = encode_orchard_recipient(deterministic_orchard_address(), &Network::MainNetwork);
    assert_eq!(a, b, "같은 입력은 같은 인코딩을 내야 한다");
}
