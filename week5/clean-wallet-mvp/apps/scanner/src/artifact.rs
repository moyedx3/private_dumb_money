use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::canonical::{canonicalize, sha256_hex};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanRange {
    pub network: String,
    #[serde(rename = "startHeight")] pub start_height: u64,
    #[serde(rename = "endHeight")] pub end_height: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScreeningArtifact {
    #[serde(rename = "schemaVersion")] pub schema_version: u32,
    pub result: String,
    #[serde(rename = "scanRange")] pub scan_range: ScanRange,
    #[serde(rename = "policyHash")] pub policy_hash: String,
    #[serde(rename = "depositIntentHash")] pub deposit_intent_hash: String,
    #[serde(rename = "viewingScopeCommitment")] pub viewing_scope_commitment: String,
    #[serde(rename = "recipientCount")] pub recipient_count: u32,
    #[serde(rename = "sanctionedHitCount")] pub sanctioned_hit_count: u32,
    #[serde(rename = "scannerCodeMeasurement")] pub scanner_code_measurement: String,
    #[serde(rename = "scanCompletedAtUnix")] pub scan_completed_at_unix: u64,
}

pub fn artifact_hash(a: &ScreeningArtifact) -> Result<String> {
    let bytes = canonicalize(&serde_json::to_value(a)?)?;
    Ok(sha256_hex(&bytes))
}

pub fn artifact_hash_bytes(a: &ScreeningArtifact) -> Result<[u8; 32]> {
    use sha2::{Digest, Sha256};
    let bytes = canonicalize(&serde_json::to_value(a)?)?;
    let digest = Sha256::digest(&bytes);
    Ok(digest.into())
}

/// `sha256(domainTag || ivk_fingerprint_bytes)` where `domainTag = b"clean-wallet-vsc-v1"`.
/// `ivk_fingerprint_bytes` should be a stable 32-byte digest derived from the UFVK's
/// incoming-viewing-key components. Computed in `scan.rs` (Task 6) using `zcash_client_backend`.
pub fn viewing_scope_commitment(ivk_fingerprint: &[u8; 32]) -> String {
    use sha2::{Digest, Sha256};
    const DOMAIN_TAG: &[u8] = b"clean-wallet-vsc-v1";
    let mut h = Sha256::new();
    h.update(DOMAIN_TAG);
    h.update(ivk_fingerprint);
    format!("0x{}", hex::encode(h.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ScreeningArtifact {
        ScreeningArtifact {
            schema_version: 1,
            result: "PASS".into(),
            scan_range: ScanRange { network: "testnet".into(), start_height: 1, end_height: 2 },
            policy_hash: format!("0x{}", "0".repeat(64)),
            deposit_intent_hash: format!("0x{}", "1".repeat(64)),
            viewing_scope_commitment: format!("0x{}", "2".repeat(64)),
            recipient_count: 3,
            sanctioned_hit_count: 0,
            scanner_code_measurement: format!("0x{}", "b".repeat(96)),
            scan_completed_at_unix: 99,
        }
    }

    #[test]
    fn artifact_hash_is_stable() {
        assert_eq!(artifact_hash(&sample()).unwrap(), artifact_hash(&sample()).unwrap());
    }

    #[test]
    fn artifact_hash_bytes_is_32_bytes() {
        assert_eq!(artifact_hash_bytes(&sample()).unwrap().len(), 32);
    }

    #[test]
    fn artifact_hash_changes_on_result_flip() {
        let mut a = sample();
        let baseline = artifact_hash(&a).unwrap();
        a.result = "FAIL".into();
        a.sanctioned_hit_count = 1;
        assert_ne!(artifact_hash(&a).unwrap(), baseline);
    }

    #[test]
    fn viewing_scope_commitment_changes_with_input() {
        let zeros = [0u8; 32];
        let ones = [1u8; 32];
        assert_ne!(viewing_scope_commitment(&zeros), viewing_scope_commitment(&ones));
    }

    #[test]
    fn viewing_scope_commitment_is_deterministic() {
        let fp = [7u8; 32];
        assert_eq!(viewing_scope_commitment(&fp), viewing_scope_commitment(&fp));
    }

    #[test]
    fn fixture_artifact_pass_hash_matches_typescript() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/schemas/fixtures/artifact.pass.sha256.hex");
        let expected = std::fs::read_to_string(&path).unwrap().trim().to_string();

        let input_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/schemas/fixtures/artifact.pass.input.json");
        let a: ScreeningArtifact =
            serde_json::from_slice(&std::fs::read(&input_path).unwrap()).unwrap();
        assert_eq!(artifact_hash(&a).unwrap(), expected);
    }
}
