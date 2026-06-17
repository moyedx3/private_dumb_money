use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::canonical::{canonicalize, sha256_hex};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    #[serde(rename = "policyName")] pub policy_name: String,
    #[serde(rename = "policyVersion")] pub policy_version: u32,
    pub network: String,
    #[serde(rename = "auditStartHeight")] pub audit_start_height: u64,
    #[serde(rename = "auditEndHeight")] pub audit_end_height: u64,
    #[serde(rename = "sanctionedAddressHashes")] pub sanctioned_address_hashes: Vec<String>,
    #[serde(rename = "expectedScannerCodeMeasurement")] pub expected_scanner_code_measurement: String,
    #[serde(rename = "createdAtUnix")] pub created_at_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepositIntent {
    #[serde(rename = "exchangeName")] pub exchange_name: String,
    #[serde(rename = "exchangeDepositAddress")] pub exchange_deposit_address: String,
    #[serde(rename = "depositAmountZat")] pub deposit_amount_zat: String,
    pub nonce: String,
    #[serde(rename = "expiryUnix")] pub expiry_unix: u64,
}

pub fn policy_hash(p: &Policy) -> Result<String> {
    let bytes = canonicalize(&serde_json::to_value(p)?)?;
    Ok(format!("0x{}", sha256_hex(&bytes)))
}

pub fn deposit_intent_hash(d: &DepositIntent) -> Result<String> {
    let bytes = canonicalize(&serde_json::to_value(d)?)?;
    Ok(format!("0x{}", sha256_hex(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_policy() -> Policy {
        Policy {
            policy_name: "demo-v1".into(),
            policy_version: 1,
            network: "testnet".into(),
            audit_start_height: 2_900_000,
            audit_end_height: 2_950_000,
            sanctioned_address_hashes: vec![format!("0x{}", "a".repeat(64))],
            expected_scanner_code_measurement: format!("0x{}", "b".repeat(96)),
            created_at_unix: 1_716_700_000,
        }
    }

    #[test]
    fn policy_hash_is_stable() {
        let h1 = policy_hash(&sample_policy()).unwrap();
        let h2 = policy_hash(&sample_policy()).unwrap();
        assert_eq!(h1, h2);
        assert!(h1.starts_with("0x") && h1.len() == 66);
    }

    #[test]
    fn policy_hash_changes_when_any_field_changes() {
        let mut p = sample_policy();
        let baseline = policy_hash(&p).unwrap();
        p.policy_version = 2;
        assert_ne!(policy_hash(&p).unwrap(), baseline);
    }

    #[test]
    fn deposit_intent_hash_changes_with_nonce() {
        let d1 = DepositIntent {
            exchange_name: "x".into(),
            exchange_deposit_address: "z".into(),
            deposit_amount_zat: "1".into(),
            nonce: format!("0x{}", "0".repeat(32)),
            expiry_unix: 1,
        };
        let mut d2 = d1.clone();
        d2.nonce = format!("0x{}", "f".repeat(32));
        assert_ne!(deposit_intent_hash(&d1).unwrap(), deposit_intent_hash(&d2).unwrap());
    }
}
