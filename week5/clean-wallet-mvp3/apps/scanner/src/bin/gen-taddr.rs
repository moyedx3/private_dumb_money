//! Generate a fresh mainnet ZEC transparent (t1...) address from a hex seed.
//! Run: cargo run -p clean-wallet-scanner --bin gen-taddr -- <hex-seed>
use zcash_keys::encoding::AddressCodec;
use zcash_keys::keys::UnifiedSpendingKey;
use zcash_protocol::consensus::Network;
use zcash_transparent::keys::IncomingViewingKey;
use zip32::AccountId;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed_hex = args.get(1).expect("usage: gen-taddr <hex-seed>");
    let seed = hex::decode(seed_hex).expect("hex seed");
    let usk = UnifiedSpendingKey::from_seed(&Network::MainNetwork, &seed, AccountId::ZERO)
        .expect("USK from seed");
    let ufvk = usk.to_unified_full_viewing_key();
    let t_fvk = ufvk
        .transparent()
        .expect("UFVK has a transparent component");
    let external = t_fvk
        .derive_external_ivk()
        .expect("derive external transparent IVK");
    let (t_addr, _idx) = external.default_address();
    println!("{}", t_addr.encode(&Network::MainNetwork));
}
