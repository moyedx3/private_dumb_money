//! Provisioning keypair + `/attest` payload (interface I6).
//!
//! The enclave derives a stable X25519 keypair from a dstack KMS-derived seed (stable
//! per code measurement), publishes its public key inside the TDX quote's `report_data`,
//! and a creator who verifies the quote encrypts `K_drop` to it (sealed box, interface I5).

use dryoc::keypair::StackKeyPair;

/// X25519 keypair whose secret IS the dstack-derived seed; public derived from it.
/// Deterministic (same seed → same keypair) so a creator who provisioned stays reachable
/// across restarts — but NOT across rebuilds (a new measurement → new seed; see C4 / Task 9).
pub fn provisioning_keypair_from_seed(seed: &[u8; 32]) -> StackKeyPair {
    StackKeyPair::from_secret_key((*seed).into())
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
}
