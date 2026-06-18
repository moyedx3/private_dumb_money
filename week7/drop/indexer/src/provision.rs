//! `/provision` — open the creator's sealed `K_drop` payload (interface I5, the secret-IN core).
//!
//! The creator seals `{drop_id, price_zat, k_drop, creator_ufvk, h_content}` to the enclave's
//! provisioning public key using libsodium `crypto_box_seal` (Lane C uses libsodium.js — the
//! wire format is identical). Only the measured enclave, holding the KMS-derived secret key,
//! can open it; the Phala operator only ever sees ciphertext.

use dryoc::classic::crypto_box::crypto_box_seal_open;
use dryoc::constants::CRYPTO_BOX_SEALBYTES;
use dryoc::keypair::StackKeyPair;

use crate::{DropConfig, ProvisionPayload};

/// dryoc keys (`StackByteArray<32>`) deref to `&[u8]`; copy into a fixed `[u8; 32]`.
fn key_bytes(k: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(k);
    out
}

/// Open the creator's sealed I5 payload with the enclave keypair → `(drop_id, DropConfig)`.
/// `sealed` is a libsodium `crypto_box_seal` blob. Fails for anyone without the enclave's
/// secret key — that's the secret-IN guarantee.
pub fn open_provision(sealed: &[u8], kp: &StackKeyPair) -> anyhow::Result<(u64, DropConfig)> {
    if sealed.len() < CRYPTO_BOX_SEALBYTES {
        anyhow::bail!("sealed payload too short ({} bytes)", sealed.len());
    }
    let pk = key_bytes(&kp.public_key);
    let sk = key_bytes(&kp.secret_key);

    let mut plain = vec![0u8; sealed.len() - CRYPTO_BOX_SEALBYTES];
    crypto_box_seal_open(&mut plain, sealed, &pk, &sk)
        .map_err(|e| anyhow::anyhow!("seal_open failed: {e}"))?;

    let p: ProvisionPayload = serde_json::from_slice(&plain)?;
    let k_drop: [u8; 32] = hex::decode(&p.k_drop_hex)?
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("k_drop_hex is not 32 bytes"))?;
    Ok((
        p.drop_id,
        DropConfig { price_zat: p.price_zat, k_drop, creator_ufvk: p.creator_ufvk, h_content: p.h_content },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dryoc::classic::crypto_box::crypto_box_seal;

    /// Simulate Lane C: libsodium `crypto_box_seal` of `msg` to the enclave pubkey.
    fn creator_seal(msg: &[u8], kp: &StackKeyPair) -> Vec<u8> {
        let pk = key_bytes(&kp.public_key);
        let mut sealed = vec![0u8; msg.len() + CRYPTO_BOX_SEALBYTES];
        crypto_box_seal(&mut sealed, msg, &pk).unwrap();
        sealed
    }

    #[test]
    fn enclave_opens_what_creator_sealed_operator_cannot() {
        let kp = crate::attest::provisioning_keypair_from_seed(&[7u8; 32]);
        let payload = crate::ProvisionPayload {
            drop_id: 1,
            price_zat: 1_000_000,
            k_drop_hex: hex::encode([0xAB; 32]),
            creator_ufvk: "uview1demo".into(),
            h_content: "abc123".into(),
        };
        let sealed = creator_seal(&serde_json::to_vec(&payload).unwrap(), &kp);

        // enclave opens it → (drop_id, DropConfig)
        let (drop_id, cfg) = open_provision(&sealed, &kp).unwrap();
        assert_eq!(drop_id, 1);
        assert_eq!(cfg.price_zat, 1_000_000);
        assert_eq!(cfg.k_drop, [0xAB; 32]);
        assert_eq!(cfg.creator_ufvk, "uview1demo");
        assert_eq!(cfg.h_content, "abc123");

        // operator (wrong/missing secret key) cannot open it
        let wrong = crate::attest::provisioning_keypair_from_seed(&[8u8; 32]);
        assert!(open_provision(&sealed, &wrong).is_err());
    }
}
