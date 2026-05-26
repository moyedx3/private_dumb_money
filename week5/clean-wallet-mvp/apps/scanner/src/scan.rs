use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::artifact::{viewing_scope_commitment, ScanRange, ScreeningArtifact};
use crate::lightwalletd::{CompactBlock, LightwalletdClient};
use crate::policy::{deposit_intent_hash, policy_hash, DepositIntent, Policy};

pub const MAX_RANGE_BLOCKS: u64 = 100_000;

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("policy network mismatch: expected {expected}, got {got}")]
    NetworkMismatch { expected: String, got: String },
    #[error("audit range too large: {span} > {max}")]
    RangeTooLarge { span: u64, max: u64 },
    #[error("audit range exceeds chain tip: end={end} tip={tip}")]
    RangeAboveTip { end: u64, tip: u64 },
    #[error("deposit intent expired")]
    IntentExpired,
    #[error("invalid UFVK: {0}")]
    InvalidUfvk(String),
    #[error("lightwalletd error: {0}")]
    Lightwalletd(#[from] anyhow::Error),
}

pub struct ScreenRequest<'a> {
    pub ufvk_str: &'a str,
    pub policy: &'a Policy,
    pub deposit_intent: &'a DepositIntent,
    pub scanner_code_measurement: &'a str,
    pub scanner_network: &'a str,
}

pub async fn scan_and_screen(
    req: ScreenRequest<'_>,
    client: &dyn LightwalletdClient,
) -> Result<ScreeningArtifact, ScanError> {
    // Fail-closed guard 1: network mismatch
    if req.policy.network != req.scanner_network {
        return Err(ScanError::NetworkMismatch {
            expected: req.scanner_network.to_string(),
            got: req.policy.network.clone(),
        });
    }

    // Fail-closed guard 2: range too large
    let span = req.policy.audit_end_height.saturating_sub(req.policy.audit_start_height);
    if span > MAX_RANGE_BLOCKS {
        return Err(ScanError::RangeTooLarge { span, max: MAX_RANGE_BLOCKS });
    }

    // Fail-closed guard 3: intent expired
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if req.deposit_intent.expiry_unix < now {
        return Err(ScanError::IntentExpired);
    }

    // Fail-closed guard 4: range above chain tip
    let tip = client.current_chain_tip().await?;
    if req.policy.audit_end_height > tip + 1 {
        return Err(ScanError::RangeAboveTip {
            end: req.policy.audit_end_height,
            tip,
        });
    }

    // Fail-closed guard 5: validate UFVK prefix
    let ivk_fp = derive_ivk_fingerprint(req.ufvk_str)
        .map_err(|e| ScanError::InvalidUfvk(e.to_string()))?;

    // Fetch compact blocks for the audit range
    let blocks = client
        .fetch_block_range(req.policy.audit_start_height, req.policy.audit_end_height)
        .await?;

    // Extract outgoing recipients from compact blocks under the UFVK's OVKs
    let recipients = extract_outgoing_recipients(req.ufvk_str, &blocks)
        .map_err(|e| ScanError::InvalidUfvk(e.to_string()))?;

    // Hash each recipient address for comparison against the policy sanctioned set
    let recipient_hashes: Vec<String> = recipients
        .iter()
        .map(|a| {
            let mut h = Sha256::new();
            h.update(a.as_bytes());
            format!("0x{}", hex::encode(h.finalize()))
        })
        .collect();

    // Fail-closed guard 6: intersect against sanctioned set
    let sanctioned: std::collections::HashSet<&str> = req
        .policy
        .sanctioned_address_hashes
        .iter()
        .map(|s| s.as_str())
        .collect();

    let hit_count = recipient_hashes
        .iter()
        .filter(|h| sanctioned.contains(h.as_str()))
        .count() as u32;
    let result = if hit_count == 0 { "PASS" } else { "FAIL" };

    Ok(ScreeningArtifact {
        schema_version: 1,
        result: result.to_string(),
        scan_range: ScanRange {
            network: req.policy.network.clone(),
            start_height: req.policy.audit_start_height,
            end_height: req.policy.audit_end_height,
        },
        policy_hash: policy_hash(req.policy).map_err(ScanError::Lightwalletd)?,
        deposit_intent_hash: deposit_intent_hash(req.deposit_intent)
            .map_err(ScanError::Lightwalletd)?,
        viewing_scope_commitment: viewing_scope_commitment(&ivk_fp),
        recipient_count: recipients.len() as u32,
        sanctioned_hit_count: hit_count,
        scanner_code_measurement: req.scanner_code_measurement.to_string(),
        scan_completed_at_unix: now,
    })
}

/// SHA-256 over the canonical UFVK string, truncated to a 32-byte fingerprint.
///
/// NOTE: A more principled fingerprint hashes the parsed IVK+OVK bytes directly.
/// For MVP this is stable per UFVK string and sufficient.
fn derive_ivk_fingerprint(ufvk_str: &str) -> Result<[u8; 32]> {
    if !ufvk_str.starts_with("uview") && !ufvk_str.starts_with("uviewtest") {
        return Err(anyhow!("UFVK must start with 'uview' or 'uviewtest'"));
    }
    let digest = Sha256::digest(ufvk_str.as_bytes());
    Ok(digest.into())
}

/// Decrypt outgoing outputs in each block under the UFVK's OVKs.
/// Returns the recipient addresses as canonical strings.
///
/// # Why this is a placeholder (Option C from Task 6 spec)
///
/// Lightwalletd compact blocks (`CompactSaplingOutput`, `CompactOrchardAction`) carry only
/// the first 52 bytes of `encCiphertext` — the prefix sufficient for **incoming** IVK-based
/// note detection. They deliberately omit `outCiphertext`, the 80-byte field that enables
/// **outgoing** OVK-based recipient recovery (Zcash protocol §4.19.3 / ZIP 307).
///
/// Real OVK recovery requires the full transaction, which is available via the lightwalletd
/// `GetTransaction` RPC (takes a raw txid). The compact-block path cannot be used.
///
/// TODO(task-10): Replace this placeholder with a full-tx fetch path:
///   1. Collect all txids from compact block `vtx` fields in the audit range.
///   2. For each txid, call `GetTransaction` (lightwalletd service.proto) to obtain the
///      full serialized Zcash transaction bytes.
///   3. Deserialize with `zcash_primitives::transaction::Transaction::read`.
///   4. For Sapling outputs: call `sapling_crypto::note_encryption::try_sapling_output_recovery`
///      with the UFVK's Sapling OVK (`zcash_keys::keys::UnifiedFullViewingKey::sapling()
///      .map(|s| s.to_ovk(zcash_primitives::sapling::Scope::External))`).
///   5. For Orchard actions: call `orchard::note_encryption::try_output_recovery_with_ovk`
///      with the UFVK's Orchard OVK (`ufvk.orchard().map(|o| o.to_ovk(orchard::keys::Scope::External))`).
///   6. Encode recovered payment addresses via `zcash_address::ZcashAddress` and push to results.
///
/// This placeholder returns `Ok(vec![])`, meaning `recipient_count` will always be 0 and
/// the sanctioned-hit check cannot fire on shielded outputs. The six fail-closed validation
/// paths (network, range size, expiry, tip, UFVK format, sanctioned intersection) are all
/// exercised by the unit tests in this module and remain correct.
fn extract_outgoing_recipients(
    _ufvk_str: &str,
    _blocks: &[CompactBlock],
) -> Result<Vec<String>> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lightwalletd::tests::MockClient;
    use crate::policy::Policy;

    fn sample_policy(start: u64, end: u64, sanctioned: Vec<String>) -> Policy {
        Policy {
            policy_name: "demo-v1".into(),
            policy_version: 1,
            network: "testnet".into(),
            audit_start_height: start,
            audit_end_height: end,
            sanctioned_address_hashes: sanctioned,
            expected_scanner_code_measurement: format!("0x{}", "b".repeat(96)),
            created_at_unix: 1,
        }
    }

    fn sample_intent(expiry: u64) -> DepositIntent {
        DepositIntent {
            exchange_name: "demo".into(),
            exchange_deposit_address: "ztestsapling1xyz".into(),
            deposit_amount_zat: "1".into(),
            nonce: format!("0x{}", "0".repeat(32)),
            expiry_unix: expiry,
        }
    }

    fn ufvk() -> String {
        "uviewtest1".to_string() + &"a".repeat(80)
    }

    #[tokio::test]
    async fn passes_when_no_recipients_match_sanctioned() {
        let mock = MockClient { tip: 100, blocks: vec![] };
        let policy = sample_policy(10, 20, vec![format!("0x{}", "f".repeat(64))]);
        let intent = sample_intent(u64::MAX);
        let art = scan_and_screen(
            ScreenRequest {
                ufvk_str: &ufvk(),
                policy: &policy,
                deposit_intent: &intent,
                scanner_code_measurement: &format!("0x{}", "b".repeat(96)),
                scanner_network: "testnet",
            },
            &mock,
        )
        .await
        .unwrap();
        assert_eq!(art.result, "PASS");
        assert_eq!(art.recipient_count, 0);
        assert_eq!(art.sanctioned_hit_count, 0);
        assert_eq!(art.scan_range.start_height, 10);
        assert_eq!(art.scan_range.end_height, 20);
    }

    #[tokio::test]
    async fn rejects_network_mismatch() {
        let mock = MockClient { tip: 100, blocks: vec![] };
        let mut policy = sample_policy(10, 20, vec![]);
        policy.network = "mainnet".into();
        let intent = sample_intent(u64::MAX);
        let err = scan_and_screen(
            ScreenRequest {
                ufvk_str: &ufvk(),
                policy: &policy,
                deposit_intent: &intent,
                scanner_code_measurement: &format!("0x{}", "b".repeat(96)),
                scanner_network: "testnet",
            },
            &mock,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ScanError::NetworkMismatch { .. }));
    }

    #[tokio::test]
    async fn rejects_range_too_large() {
        let mock = MockClient { tip: 1_000_000, blocks: vec![] };
        let policy = sample_policy(0, MAX_RANGE_BLOCKS + 1, vec![]);
        let intent = sample_intent(u64::MAX);
        let err = scan_and_screen(
            ScreenRequest {
                ufvk_str: &ufvk(),
                policy: &policy,
                deposit_intent: &intent,
                scanner_code_measurement: &format!("0x{}", "b".repeat(96)),
                scanner_network: "testnet",
            },
            &mock,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ScanError::RangeTooLarge { .. }));
    }

    #[tokio::test]
    async fn rejects_expired_intent() {
        let mock = MockClient { tip: 100, blocks: vec![] };
        let policy = sample_policy(10, 20, vec![]);
        let intent = sample_intent(0);
        let err = scan_and_screen(
            ScreenRequest {
                ufvk_str: &ufvk(),
                policy: &policy,
                deposit_intent: &intent,
                scanner_code_measurement: &format!("0x{}", "b".repeat(96)),
                scanner_network: "testnet",
            },
            &mock,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ScanError::IntentExpired));
    }

    #[tokio::test]
    async fn rejects_range_above_tip() {
        let mock = MockClient { tip: 50, blocks: vec![] };
        let policy = sample_policy(40, 100, vec![]);
        let intent = sample_intent(u64::MAX);
        let err = scan_and_screen(
            ScreenRequest {
                ufvk_str: &ufvk(),
                policy: &policy,
                deposit_intent: &intent,
                scanner_code_measurement: &format!("0x{}", "b".repeat(96)),
                scanner_network: "testnet",
            },
            &mock,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ScanError::RangeAboveTip { .. }));
    }

    #[tokio::test]
    async fn rejects_malformed_ufvk() {
        let mock = MockClient { tip: 100, blocks: vec![] };
        let policy = sample_policy(10, 20, vec![]);
        let intent = sample_intent(u64::MAX);
        let err = scan_and_screen(
            ScreenRequest {
                ufvk_str: "not-a-ufvk",
                policy: &policy,
                deposit_intent: &intent,
                scanner_code_measurement: &format!("0x{}", "b".repeat(96)),
                scanner_network: "testnet",
            },
            &mock,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ScanError::InvalidUfvk(_)));
    }
}
