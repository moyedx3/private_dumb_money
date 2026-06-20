//! Provisioning keypair + `/attest` payload (interface I6).
//!
//! The enclave derives a stable X25519 keypair from a dstack KMS-derived seed (stable
//! per code measurement), publishes its public key inside the TDX quote's `report_data`,
//! and a creator who verifies the quote encrypts `K_drop` to it (sealed box, interface I5).

use dryoc::keypair::StackKeyPair;

/// X25519 keypair whose secret IS the dstack-derived seed; public derived from it.
/// Deterministic (same seed → same keypair) so a creator who provisioned stays reachable
/// across restarts — but NOT across rebuilds (a new measurement → new seed; see C4 / Task 9).
///
/// Uses `from_secret_key` (NOT `from_seed`) on purpose: the seed IS the X25519 secret, so the
/// published pubkey and seal_open share one key. `from_seed` would hash the seed and change the
/// pubkey — silently unreachable for creators who already provisioned. Don't "fix" it to `from_seed`.
pub fn provisioning_keypair_from_seed(seed: &[u8; 32]) -> StackKeyPair {
    StackKeyPair::from_secret_key((*seed).into())
}

use sha2::{Digest, Sha256};

/// report_data binding: sha256(provisioning pubkey). The creator checks the quote's
/// report_data equals this before encrypting K_drop to the pubkey (interface I6 → I5).
pub fn report_data_for_pubkey(pubkey: &[u8]) -> [u8; 32] {
    Sha256::digest(pubkey).into()
}

/// Interface I6 response: a fresh quote binding sha256(pubkey) + the pubkey itself.
#[derive(serde::Serialize)]
pub struct AttestResponse {
    pub quote_hex: String,
    pub provisioning_pubkey_hex: String,
}

/// Build the `/attest` payload: a quote whose report_data commits to the provisioning
/// pubkey, plus the pubkey, so the creator can verify the quote then encrypt K_drop to it.
pub async fn build_attest_response(
    ds: &crate::dstack::Dstack,
    kp: &StackKeyPair,
) -> anyhow::Result<AttestResponse> {
    let pk = &kp.public_key[..];
    Ok(AttestResponse {
        quote_hex: ds.get_quote(&report_data_for_pubkey(pk)).await?,
        provisioning_pubkey_hex: hex::encode(pk),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_from_seed_is_deterministic() {
        let a = provisioning_keypair_from_seed(&[3u8; 32]);
        let b = provisioning_keypair_from_seed(&[3u8; 32]);
        let c = provisioning_keypair_from_seed(&[4u8; 32]);
        assert_eq!(a.public_key, b.public_key); // same seed → same pubkey (creator stays reachable)
        assert_ne!(a.public_key, c.public_key); // different seed → different pubkey (actually derived)
    }

    #[test]
    fn report_data_is_sha256_of_pubkey() {
        use sha2::{Digest, Sha256};
        let kp = provisioning_keypair_from_seed(&[5u8; 32]);
        let pk = &kp.public_key[..];
        let rd = report_data_for_pubkey(pk);
        // The creator trusts the pubkey only because the quote's report_data commits to it.
        assert_eq!(rd.to_vec(), Sha256::digest(pk).to_vec());
        assert_eq!(rd.len(), 32);
    }

    #[tokio::test]
    #[ignore]
    async fn live_attest_response_binds_pubkey() {
        let sock = std::env::var("DSTACK_SOCKET").expect("set DSTACK_SOCKET");
        let ds = crate::dstack::Dstack::new(sock);
        let kp = provisioning_keypair_from_seed(&[5u8; 32]);
        let resp = build_attest_response(&ds, &kp).await.unwrap();
        assert!(!resp.quote_hex.is_empty());
        assert_eq!(resp.provisioning_pubkey_hex.len(), 64); // 32 bytes hex
    }
}
