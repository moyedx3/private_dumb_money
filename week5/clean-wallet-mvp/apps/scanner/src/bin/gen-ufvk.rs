//! Minimal UFVK generator for demo. Run:
//!   cargo run -p clean-wallet-scanner --bin gen-ufvk -- <hex-seed>
use zcash_keys::keys::UnifiedSpendingKey;
use zcash_protocol::consensus::Network;
use zip32::AccountId;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed_hex = args.get(1).expect("usage: gen-ufvk <hex-seed>");
    let seed = hex::decode(seed_hex).expect("hex seed");
    let usk = UnifiedSpendingKey::from_seed(&Network::MainNetwork, &seed, AccountId::ZERO)
        .expect("USK from seed");
    let ufvk = usk.to_unified_full_viewing_key();
    println!("{}", ufvk.encode(&Network::MainNetwork));
}
