//! decode-ua — Unified Address 를 분해해 어떤 receiver (orchard/sapling/transparent) 가
//! 들어있는지 출력. multi-receiver UA 인지 orchard-only/sapling-only 인지, 어떤 인코딩인지 진단.
//!
//! 사용:
//!   decode-ua u1rrzu7p70vgyme6...                 (mainnet)
//!   decode-ua utest1zexample...                   (testnet)
//!
//! 출력 예:
//!   network: mainnet
//!   receivers: [orchard]
//!   orchard_raw_hex: 6f1e7d4a...43bytes
//!
//! 같은 receiver 라도 multi-receiver vs orchard-only 면 encoding string 이 다름. 이걸로
//! 사용자가 본 주소가 정확히 어떤 형태인지 알 수 있다.

use anyhow::{anyhow, bail, Result};
use zcash_keys::address::{Address, UnifiedAddress};
use zcash_protocol::consensus::Network;

fn main() -> Result<()> {
    let argv: Vec<String> = std::env::args().collect();
    let addr_str = argv.get(1).ok_or_else(|| {
        anyhow!("usage: decode-ua <unified-address> (u1... or utest1...)")
    })?;

    // Network 추측 — UA prefix 로 판단.
    let (network, net_label) = if addr_str.starts_with("u1") {
        (Network::MainNetwork, "mainnet")
    } else if addr_str.starts_with("utest1") {
        (Network::TestNetwork, "testnet")
    } else {
        bail!(
            "UA prefix 가 u1 또는 utest1 이어야 한다 — 받은: {}",
            &addr_str.chars().take(10).collect::<String>()
        );
    };

    let decoded = Address::decode(&network, addr_str)
        .ok_or_else(|| anyhow!("UA decode 실패 — bech32m 형식 확인"))?;

    let ua: UnifiedAddress = match decoded {
        Address::Unified(ua) => ua,
        Address::Sapling(_) => bail!("이 주소는 sapling-standalone (zs1...) — UA 가 아님"),
        Address::Transparent(_) => bail!("이 주소는 transparent (t1.../tm...) — UA 가 아님"),
        other => bail!("예상치 못한 주소 종류: {:?}", other),
    };

    println!("network: {}", net_label);
    println!("input_length: {} chars", addr_str.len());
    println!();

    let mut receivers: Vec<&str> = vec![];
    if ua.orchard().is_some() {
        receivers.push("orchard");
    }
    if ua.sapling().is_some() {
        receivers.push("sapling");
    }
    if ua.transparent().is_some() {
        receivers.push("transparent");
    }
    println!("receivers: [{}]", receivers.join(", "));
    println!();

    // 각 receiver raw bytes 출력 — 같은 receiver 인지 비교용.
    if let Some(orchard) = ua.orchard() {
        let raw = orchard.to_raw_address_bytes();
        println!("orchard_raw_43bytes: {}", hex::encode(raw));
        // Diversifier (11 bytes) + pk_d (32 bytes).
        println!("  diversifier:  {}", hex::encode(&raw[..11]));
        println!("  pk_d:         {}", hex::encode(&raw[11..]));
    }
    if let Some(sapling) = ua.sapling() {
        let raw = sapling.to_bytes();
        println!("sapling_raw_43bytes: {}", hex::encode(raw));
        println!("  diversifier:  {}", hex::encode(&raw[..11]));
        println!("  pk_d:         {}", hex::encode(&raw[11..]));
    }
    if let Some(t) = ua.transparent() {
        // TransparentAddress 의 Debug 형식 + encode 형식 둘 다.
        let encoded = Address::Transparent(*t).encode(&network);
        println!("transparent: {}  (kind: {:?})", encoded, t);
    }

    println!();
    println!("진단 가이드:");
    if receivers.len() == 1 {
        println!("  → single-receiver UA. 같은 underlying key 를 multi-receiver 로 다시 encode 하면");
        println!("    완전히 다른 u1... string 이 나온다. raw 비교로 같은 receiver 인지 확인 가능.");
    } else {
        println!("  → multi-receiver UA. sender 가 이걸로 보내면 wallet 이 선호 pool 로 자동 라우팅.");
        println!("    그 송금을 우리 scanner 가 추출하면 *그 라우팅된 pool 의 single-receiver UA* 로");
        println!("    normalize 됨. 즉 string 으로 비교하면 입력과 다름. raw bytes 로 매칭해야 일치.");
    }
    Ok(())
}
