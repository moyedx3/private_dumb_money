//! `/provision` — open the creator's sealed `K_drop` payload (interface I5, the secret-IN core).
//!
//! The creator seals `{drop_id, price_zat, k_drop, creator_ufvk, h_content}` to the enclave's
//! provisioning public key using libsodium `crypto_box_seal` (Lane C uses libsodium.js — the
//! wire format is identical). Only the measured enclave, holding the KMS-derived secret key,
//! can open it; the Phala operator only ever sees ciphertext.
//!
//! **C4 (rebuild continuity).** The enclave's provisioning keypair is KMS-derived from the code
//! *measurement*. Rebuilding the image changes the measurement → changes the keypair → a creator
//! who provisioned to the OLD build can no longer reach the new one. Operational rule: after any
//! code change + redeploy, creators must re-`POST /provision`. Re-provisioning is idempotent per
//! `drop_id` (the catalog overwrites), so re-sending is safe. Creators detect a measurement change
//! via the `mr_td` published alongside the image (Task 8).

use dryoc::classic::crypto_box::crypto_box_seal_open;
use dryoc::constants::CRYPTO_BOX_SEALBYTES;
use dryoc::keypair::StackKeyPair;
use zeroize::Zeroize;

use crate::{DropConfig, ProvisionPayload};

/// dryoc keys (`StackByteArray<32>`) deref to `&[u8]`; copy into a fixed `[u8; 32]`.
fn key_bytes(k: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(k);
    out
}

/// Reject transparent Zcash addresses (mainnet `t1`/`t3`, testnet `tm`/`t2`). A transparent
/// recipient has no memo field, so A1 would never receive the buyer's `drop_id‖e_pub` and the
/// unlock path silently breaks (lane-B §8 trap 1). Shielded addresses (`z…`/`u…`) carry the memo.
fn is_shielded_addr(addr: &str) -> bool {
    !addr.is_empty() && !addr.starts_with('t')
}

/// Open the creator's sealed I5 payload with the enclave keypair → `(drop_id, DropConfig)`.
/// `sealed` is a libsodium `crypto_box_seal` blob. Fails for anyone without the enclave's
/// secret key — that's the secret-IN guarantee.
pub fn open_provision(sealed: &[u8], kp: &StackKeyPair) -> anyhow::Result<(u64, DropConfig)> {
    if sealed.len() < CRYPTO_BOX_SEALBYTES {
        anyhow::bail!("sealed payload too short ({} bytes)", sealed.len());
    }
    let pk = key_bytes(&kp.public_key);
    let mut sk = key_bytes(&kp.secret_key);

    let mut plain = vec![0u8; sealed.len() - CRYPTO_BOX_SEALBYTES];
    let opened = crypto_box_seal_open(&mut plain, sealed, &pk, &sk);
    sk.zeroize();
    opened.map_err(|e| anyhow::anyhow!("seal_open failed: {e}"))?;

    let mut p: ProvisionPayload = serde_json::from_slice(&plain)?;
    plain.zeroize(); // the decrypted JSON carried k_drop

    let mut raw = hex::decode(&p.k_drop)?;
    p.k_drop.zeroize();
    let k_drop_res: Result<[u8; 32], _> = raw.as_slice().try_into();
    raw.zeroize();
    let k_drop = k_drop_res.map_err(|_| anyhow::anyhow!("k_drop is not 32 bytes"))?;

    if !is_shielded_addr(&p.deposit_addr) {
        anyhow::bail!(
            "deposit_addr must be a shielded address (transparent t-addr drops the memo)"
        );
    }

    Ok((
        p.drop_id,
        DropConfig {
            price_zat: p.price_zat,
            k_drop,
            creator_ufvk: p.creator_ufvk,
            h_content: p.h_content,
            deposit_addr: p.deposit_addr,
        },
    ))
}

/// Creator-side seal (interface I5): libsodium `crypto_box_seal` of `msg` to the enclave
/// pubkey. Lane C does this in libsodium.js; provided here for Rust clients + tests.
pub fn seal_to_enclave(msg: &[u8], enclave_pubkey: &[u8]) -> Vec<u8> {
    let pk = key_bytes(enclave_pubkey);
    let mut sealed = vec![0u8; msg.len() + CRYPTO_BOX_SEALBYTES];
    dryoc::classic::crypto_box::crypto_box_seal(&mut sealed, msg, &pk).expect("seal");
    sealed
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
            k_drop: hex::encode([0xAB; 32]),
            creator_ufvk: "uview1demo".into(),
            h_content: "abc123".into(),
            deposit_addr: "u1demo".into(),
        };
        let sealed = creator_seal(&serde_json::to_vec(&payload).unwrap(), &kp);

        // enclave opens it → (drop_id, DropConfig)
        let (drop_id, cfg) = open_provision(&sealed, &kp).unwrap();
        assert_eq!(drop_id, 1);
        assert_eq!(cfg.price_zat, 1_000_000);
        assert_eq!(cfg.k_drop, [0xAB; 32]);
        assert_eq!(cfg.creator_ufvk, "uview1demo");
        assert_eq!(cfg.h_content, "abc123");
        assert_eq!(cfg.deposit_addr, "u1demo");

        // operator (wrong/missing secret key) cannot open it
        let wrong = crate::attest::provisioning_keypair_from_seed(&[8u8; 32]);
        assert!(open_provision(&sealed, &wrong).is_err());
    }

    #[test]
    fn provision_rejects_transparent_deposit_addr() {
        // A transparent t-addr has no memo field, so A1 would never receive drop_id‖e_pub
        // → unlock silently breaks. Provision must reject it.
        let kp = crate::attest::provisioning_keypair_from_seed(&[7u8; 32]);
        let payload = crate::ProvisionPayload {
            drop_id: 1,
            price_zat: 500,
            k_drop: hex::encode([2u8; 32]),
            creator_ufvk: "uview1x".into(),
            h_content: "h1".into(),
            deposit_addr: "t1ExampleTransparentAddress".into(),
        };
        let sealed = creator_seal(&serde_json::to_vec(&payload).unwrap(), &kp);
        assert!(open_provision(&sealed, &kp).is_err());
    }

    #[test]
    fn provision_accepts_shielded_deposit_addr() {
        let kp = crate::attest::provisioning_keypair_from_seed(&[7u8; 32]);
        let payload = crate::ProvisionPayload {
            drop_id: 1,
            price_zat: 500,
            k_drop: hex::encode([2u8; 32]),
            creator_ufvk: "uview1x".into(),
            h_content: "h1".into(),
            deposit_addr: "u1shieldedunifiedaddress".into(),
        };
        let sealed = creator_seal(&serde_json::to_vec(&payload).unwrap(), &kp);
        let (_, cfg) = open_provision(&sealed, &kp).unwrap();
        assert_eq!(cfg.deposit_addr, "u1shieldedunifiedaddress");
    }
}
