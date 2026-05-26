//! Integration tests against the regtest docker-compose. Marked `#[ignore]`
//! so they don't run on `cargo test` by default; run with:
//!
//!   ./apps/scanner/tests/regtest_setup.sh
//!   cargo test -p clean-wallet-scanner --test regtest_scan -- --ignored --nocapture

use clean_wallet_scanner::lightwalletd::GrpcClient;
use clean_wallet_scanner::policy::{DepositIntent, Policy};
use clean_wallet_scanner::scan::{scan_and_screen, ScanError, ScreenRequest};
use std::fs;

fn load(name: &str) -> String {
    fs::read_to_string(format!("apps/scanner/tests/.regtest-state/{name}"))
        .unwrap()
        .trim()
        .to_string()
}

fn read_range() -> (u64, u64) {
    let raw = load("range.json");
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    (v["start"].as_u64().unwrap(), v["end"].as_u64().unwrap())
}

fn read_sanctioned_hash() -> String {
    let raw = load("sanctioned.json");
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    v["recipient_hash_hex"].as_str().unwrap().to_string()
}

fn make_policy(start: u64, end: u64, sanctioned: Vec<String>) -> Policy {
    Policy {
        policy_name: "regtest".into(),
        policy_version: 1,
        network: "testnet".into(),
        audit_start_height: start,
        audit_end_height: end,
        sanctioned_address_hashes: sanctioned,
        expected_scanner_code_measurement: format!("0x{}", "b".repeat(96)),
        created_at_unix: 1,
    }
}

fn make_intent() -> DepositIntent {
    DepositIntent {
        exchange_name: "regtest".into(),
        exchange_deposit_address: "ztestsapling1regtest".into(),
        deposit_amount_zat: "1".into(),
        nonce: format!("0x{}", "0".repeat(32)),
        expiry_unix: u64::MAX,
    }
}

fn client() -> GrpcClient {
    // lightwalletd is exposed without TLS in the test compose file
    GrpcClient::new("http://localhost:9067", None)
}

/// Scan wallet A (clean wallet — all recipients are benign).
/// Expects: result = PASS, sanctioned_hit_count = 0, recipient_count >= 1.
#[tokio::test]
#[ignore]
async fn pass_path_wallet_a_clean() {
    let (s, e) = read_range();
    let policy = make_policy(s, e, vec![read_sanctioned_hash()]);
    let intent = make_intent();
    let ufvk = load("wallet-a.ufvk");
    let art = scan_and_screen(
        ScreenRequest {
            ufvk_str: &ufvk,
            policy: &policy,
            deposit_intent: &intent,
            scanner_code_measurement: &format!("0x{}", "b".repeat(96)),
            scanner_network: "testnet",
        },
        &client(),
    )
    .await
    .unwrap();
    assert_eq!(art.result, "PASS");
    assert_eq!(art.sanctioned_hit_count, 0);
    assert!(art.recipient_count > 0, "wallet A should have outgoing recipients");
}

/// Scan wallet B (sent to the sanctioned address).
/// Expects: result = FAIL, sanctioned_hit_count >= 1.
#[tokio::test]
#[ignore]
async fn fail_path_wallet_b_has_sanctioned() {
    let (s, e) = read_range();
    let policy = make_policy(s, e, vec![read_sanctioned_hash()]);
    let intent = make_intent();
    let ufvk = load("wallet-b.ufvk");
    let art = scan_and_screen(
        ScreenRequest {
            ufvk_str: &ufvk,
            policy: &policy,
            deposit_intent: &intent,
            scanner_code_measurement: &format!("0x{}", "b".repeat(96)),
            scanner_network: "testnet",
        },
        &client(),
    )
    .await
    .unwrap();
    assert_eq!(art.result, "FAIL");
    assert!(art.sanctioned_hit_count >= 1);
}

/// Requesting a range far above the regtest tip must fail-closed.
/// Expects: ScanError::RangeAboveTip.
#[tokio::test]
#[ignore]
async fn fail_closed_on_range_above_tip() {
    let policy = make_policy(1_000_000_000, 1_000_000_001, vec![]);
    let intent = make_intent();
    let ufvk = load("wallet-a.ufvk");
    let err = scan_and_screen(
        ScreenRequest {
            ufvk_str: &ufvk,
            policy: &policy,
            deposit_intent: &intent,
            scanner_code_measurement: &format!("0x{}", "b".repeat(96)),
            scanner_network: "testnet",
        },
        &client(),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ScanError::RangeAboveTip { .. }));
}

/// After stopping lightwalletd the scan must fail-closed with a Lightwalletd error.
///
/// Run manually by stopping the container before this test:
///   docker compose -f apps/scanner/docker-compose.test.yml stop lightwalletd
/// Then restart:
///   docker compose -f apps/scanner/docker-compose.test.yml start lightwalletd
#[tokio::test]
#[ignore]
async fn fail_closed_on_lightwalletd_disconnect() {
    let (s, e) = read_range();
    let policy = make_policy(s, e, vec![]);
    let intent = make_intent();
    let ufvk = load("wallet-a.ufvk");
    let result = scan_and_screen(
        ScreenRequest {
            ufvk_str: &ufvk,
            policy: &policy,
            deposit_intent: &intent,
            scanner_code_measurement: &format!("0x{}", "b".repeat(96)),
            scanner_network: "testnet",
        },
        &client(),
    )
    .await;
    assert!(result.is_err(), "disconnected lightwalletd must return an error");
}
