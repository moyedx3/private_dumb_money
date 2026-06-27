//! A1-owned dispatch blob utilities.
//!
//! The blob format is libsodium-compatible sealed box output:
//! `ek_pub(32) || ciphertext+MAC(48)` for a 32-byte `K_drop`, totaling 80 bytes.
//! Bucket keys are opaque `blake2b-256(ek_pub || txid)` hex strings.

use anyhow::{anyhow, Result};
use blake2::{digest::consts::U32, Blake2b, Digest};
use dryoc::dryocbox::{DryocBox, PublicKey};

pub const K_DROP_LEN: usize = 32;
pub const SEALED_BOX_OVERHEAD: usize = 48;
pub const DISPATCH_BLOB_LEN: usize = K_DROP_LEN + SEALED_BOX_OVERHEAD;
pub const EPHEMERAL_PUBLIC_KEY_LEN: usize = 32;

/// Wrap `K_drop` for the buyer's X25519 public key using a libsodium-compatible sealed box.
///
/// Output layout: `ek_pub(32) || ciphertext+MAC(48)` = 80 bytes for a 32-byte `K_drop`.
pub fn wrap_k_drop(
    k_drop: &[u8; K_DROP_LEN],
    e_pub: &[u8; EPHEMERAL_PUBLIC_KEY_LEN],
) -> Result<Vec<u8>> {
    let recipient_public_key: PublicKey = (*e_pub).into();
    let sealed = DryocBox::seal_to_vecbox(k_drop, &recipient_public_key)
        .map_err(|e| anyhow!("seal K_drop: {e:?}"))?;
    let blob = sealed.to_vec();
    if blob.len() != DISPATCH_BLOB_LEN {
        return Err(anyhow!(
            "sealed dispatch blob length mismatch: got {}, expected {DISPATCH_BLOB_LEN}",
            blob.len()
        ));
    }
    Ok(blob)
}

/// Opaque bucket key: `blake2b-256(ek_pub || txid)`.
///
/// `ek_pub_prefix` should be the first 32 bytes of the sealed dispatch blob. The
/// function accepts a slice to keep call sites simple (`blob_key(&blob[..32], txid)`).
pub fn blob_key(ek_pub_prefix: &[u8], txid: &[u8; 32]) -> String {
    let mut h = Blake2b::<U32>::new();
    h.update(ek_pub_prefix);
    h.update(txid);
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dryoc::dryocbox::VecBox;
    use dryoc::keypair::StackKeyPair;
    use dryoc::types::ByteArray;

    #[test]
    fn buyer_can_open_dispatch_blob() {
        let buyer = StackKeyPair::gen();
        let e_pub = *buyer.public_key.as_array();
        let k_drop = [42u8; K_DROP_LEN];

        let blob = wrap_k_drop(&k_drop, &e_pub).unwrap();
        assert_eq!(blob.len(), DISPATCH_BLOB_LEN);

        let sealed = VecBox::from_sealed_bytes(&blob).unwrap();
        let opened = sealed.unseal_to_vec(&buyer).unwrap();
        assert_eq!(opened.as_slice(), &k_drop);
    }

    #[test]
    fn blob_key_is_blake2b_256_hex() {
        let ek_pub = [1u8; EPHEMERAL_PUBLIC_KEY_LEN];
        let txid = [2u8; 32];

        let key = blob_key(&ek_pub, &txid);

        assert_eq!(key.len(), 64);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(key, blob_key(&ek_pub, &txid));
        assert_ne!(key, blob_key(&[3u8; EPHEMERAL_PUBLIC_KEY_LEN], &txid));
    }
}
