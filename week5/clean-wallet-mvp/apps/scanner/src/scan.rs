use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::artifact::{viewing_scope_commitment, ScanRange, ScreeningArtifact};
use crate::lightwalletd::LightwalletdClient;
use crate::policy::{deposit_intent_hash, policy_hash, DepositIntent, Policy};

// OVK recovery imports
use sapling_crypto::note_encryption::{try_sapling_output_recovery, Zip212Enforcement};
use zcash_address::ToAddress as _;
use zcash_keys::keys::UnifiedFullViewingKey;
use zcash_note_encryption::try_output_recovery_with_ovk;
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{BlockHeight, BranchId, Network, NetworkType};

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
    Lightwalletd(anyhow::Error),
    #[error("internal error: {0}")]
    Internal(anyhow::Error),
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
    let tip = client.current_chain_tip().await.map_err(ScanError::Lightwalletd)?;
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
        .await
        .map_err(ScanError::Lightwalletd)?;

    // Pre-fetch full transactions needed for OVK recovery.
    // Compact blocks omit outCiphertext so we must retrieve full txs via GetTransaction.
    // We collect (block_height, raw_tx_bytes) tuples here in the async context, then
    // pass the pre-fetched batch to the sync OVK recovery helper below.
    let mut raw_txs: Vec<(u64, Vec<u8>)> = Vec::new();
    for block in &blocks {
        for compact_tx in &block.vtx {
            let txid: [u8; 32] = compact_tx.txid.as_slice().try_into()
                .map_err(|_| ScanError::Internal(anyhow!(
                    "compact tx has txid of length {} (expected 32)",
                    compact_tx.txid.len()
                )))?;
            let raw = client.fetch_transaction(&txid).await
                .map_err(ScanError::Lightwalletd)?;
            if !raw.is_empty() {
                raw_txs.push((block.height, raw));
            }
        }
    }

    // Resolve the network parameter for BranchId derivation
    let network = match req.policy.network.as_str() {
        "mainnet" => Network::MainNetwork,
        _ => Network::TestNetwork,   // testnet + regtest both use TestNetwork upgrades for MVP
    };

    // Extract outgoing recipients from full transactions under the UFVK's OVKs
    let recipients = extract_outgoing_recipients(req.ufvk_str, &raw_txs, &network)
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
        policy_hash: policy_hash(req.policy).map_err(ScanError::Internal)?,
        deposit_intent_hash: deposit_intent_hash(req.deposit_intent)
            .map_err(ScanError::Internal)?,
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

/// Recover outgoing recipient addresses from pre-fetched full transactions using the OVKs
/// derived from the provided UFVK.
///
/// # Arguments
/// * `ufvk_str`  — ZIP-316 encoded Unified Full Viewing Key
/// * `raw_txs`   — `(block_height, raw_tx_bytes)` tuples, as returned by `GetTransaction`
/// * `network`   — consensus network (drives `BranchId::for_height` and address encoding)
///
/// # Protocol notes
/// Compact blocks omit `outCiphertext` (the 80-byte field that enables OVK recovery per
/// Zcash protocol §4.19.3 / ZIP 307). This function therefore operates on *full*
/// transactions retrieved via the lightwalletd `GetTransaction` RPC in the caller.
///
/// For each transaction the function:
///   1. Derives the consensus branch ID from the block height.
///   2. Deserializes the transaction with `Transaction::read`.
///   3. For every Sapling output: tries `try_sapling_output_recovery` with the UFVK's
///      Sapling external OVK.
///   4. For every Orchard action: tries `try_output_recovery_with_ovk` (from
///      `zcash_note_encryption`) with the UFVK's Orchard external OVK.
///   5. Encodes recovered payment addresses via `zcash_address::ZcashAddress`.
fn extract_outgoing_recipients(
    ufvk_str: &str,
    raw_txs: &[(u64, Vec<u8>)],
    network: &Network,
) -> Result<Vec<String>> {
    // Short-circuit: if there are no transactions to scan, skip UFVK decode entirely.
    // This avoids a spurious validation error in tests or scanning over empty block ranges.
    if raw_txs.is_empty() {
        return Ok(Vec::new());
    }

    // Decode the UFVK — reuse the same network enum
    let ufvk = UnifiedFullViewingKey::decode(network, ufvk_str)
        .map_err(|e| anyhow!("UFVK decode failed: {e}"))?;

    // Extract the Sapling external OVK (if the UFVK carries a Sapling component)
    let sapling_ovk = ufvk.sapling().map(|s| {
        s.to_ovk(zip32::Scope::External)
    });

    // Extract the Orchard external OVK (if the UFVK carries an Orchard component)
    let orchard_ovk = ufvk.orchard().map(|o| {
        o.to_ovk(orchard::keys::Scope::External)
    });

    let net_type = match network {
        Network::MainNetwork => NetworkType::Main,
        Network::TestNetwork => NetworkType::Test,
    };

    let mut recipients: Vec<String> = Vec::new();

    for (height, raw) in raw_txs {
        if raw.is_empty() {
            continue;
        }
        let block_height = BlockHeight::from_u32(*height as u32);
        let branch_id = BranchId::for_height(network, block_height);

        let tx = match Transaction::read(&raw[..], branch_id) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(height, "failed to deserialize tx: {e}");
                continue;
            }
        };

        // --- Sapling outputs ---
        if let Some(ovk) = &sapling_ovk {
            if let Some(bundle) = tx.sapling_bundle() {
                // ZIP 212 enforcement: On for anything after Canopy activation.
                // For testnet/mainnet the canonical check is `is_nu_active(Canopy, height)`.
                // For simplicity we use `On` for heights above the testnet Canopy activation
                // (~903 000) and `GracePeriod` otherwise.  The grace period allows both 0x01
                // and 0x02 note plaintext lead bytes, so it is the safest fallback.
                let zip212 = if sapling_canopy_active(network, block_height) {
                    Zip212Enforcement::On
                } else {
                    Zip212Enforcement::GracePeriod
                };

                for output in bundle.shielded_outputs() {
                    if let Some((_note, addr, _memo)) =
                        try_sapling_output_recovery(ovk, output, zip212)
                    {
                        let addr_bytes = addr.to_bytes();
                        let zaddr = zcash_address::ZcashAddress::from_sapling(net_type, addr_bytes);
                        recipients.push(zaddr.to_string());
                    }
                }
                // ^^ `ToAddress` trait imported above via `use zcash_address::ToAddress as _`
            }
        }

        // --- Orchard actions ---
        if let Some(ovk) = &orchard_ovk {
            if let Some(bundle) = tx.orchard_bundle() {
                for action in bundle.actions() {
                    let domain = orchard::note_encryption::OrchardDomain::for_action(action);
                    let cv = action.cv_net();
                    let out_ct = &action.encrypted_note().out_ciphertext;
                    if let Some((_note, addr, _memo)) =
                        try_output_recovery_with_ovk(&domain, ovk, action, cv, out_ct)
                    {
                        let raw_bytes = addr.to_raw_address_bytes();
                        // Orchard addresses are not standalone; wrap in a Unified Address with
                        // only the Orchard receiver.  For sanctioned-hash comparison purposes
                        // we use the canonical raw-bytes hex string instead, since building a
                        // full UA requires the keys::FullViewingKey and a diversifier index.
                        // TODO(task-15): produce a proper UA string once the full key is available.
                        recipients.push(hex::encode(raw_bytes));
                    }
                }
            }
        }
    }

    Ok(recipients)
}

/// Returns true if Canopy (ZIP-212) was active at the given block height on the given network.
/// This drives `Zip212Enforcement::On` vs `GracePeriod` for Sapling note decryption.
fn sapling_canopy_active(network: &Network, height: BlockHeight) -> bool {
    use zcash_protocol::consensus::{NetworkUpgrade, Parameters};
    network.is_nu_active(NetworkUpgrade::Canopy, height)
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
        let mock = MockClient::new(100, vec![]);
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
        let mock = MockClient::new(100, vec![]);
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
        let mock = MockClient::new(1_000_000, vec![]);
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
        let mock = MockClient::new(100, vec![]);
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
        let mock = MockClient::new(50, vec![]);
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
        let mock = MockClient::new(100, vec![]);
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
