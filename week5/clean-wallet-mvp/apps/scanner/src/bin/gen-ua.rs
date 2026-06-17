//! Spike #2 helper: derive a mainnet receiving address (UA) + its matching UFVK
//! from a hex seed. Pay the UA from Zashi WITH a memo, then scan with the UFVK via
//! `ivk-incoming-probe` to prove IVK-only incoming detection + memo recovery.
//!
//!   cargo run -p clean-wallet-scanner --bin gen-ua -- <hex-seed (>= 32 bytes)>
//!
//! Example seed: 64 hex chars (32 bytes). Keep it secret-ish — it controls the UA.
use anyhow::{anyhow, Result};
use zcash_keys::keys::{ReceiverRequirement, UnifiedAddressRequest, UnifiedSpendingKey};
use zcash_protocol::consensus::Network;
use zip32::AccountId;

fn main() -> Result<()> {
    let seed_hex = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("usage: gen-ua <hex-seed (>= 32 bytes)>"))?;
    let seed = hex::decode(seed_hex.trim()).map_err(|e| anyhow!("seed is not valid hex: {e}"))?;
    if seed.len() < 32 {
        return Err(anyhow!("seed must be >= 32 bytes ({} hex chars); got {}", 64, seed.len()));
    }

    let usk = UnifiedSpendingKey::from_seed(&Network::MainNetwork, &seed, AccountId::ZERO)
        .map_err(|e| anyhow!("USK from seed: {e}"))?;
    let ufvk = usk.to_unified_full_viewing_key();

    // Orchard-only unified address: shielded + memo-capable, and what Zashi pays to.
    let request = UnifiedAddressRequest::custom(
        ReceiverRequirement::Require, // orchard
        ReceiverRequirement::Omit,    // sapling
        ReceiverRequirement::Omit,    // transparent (p2pkh)
    )
    .map_err(|e| anyhow!("address request: {e:?}"))?;
    let (ua, _idx) = ufvk
        .default_address(request)
        .map_err(|e| anyhow!("default_address: {e}"))?;

    println!("# Spike #2 test vector (mainnet)");
    println!();
    println!("UFVK  (scan with this — ivk-incoming-probe arg 1):");
    println!("{}", ufvk.encode(&Network::MainNetwork));
    println!();
    println!("UA    (pay THIS from Zashi, and attach a memo):");
    println!("{}", ua.encode(&Network::MainNetwork));
    Ok(())
}
