//! Mint the demo "dirty" wallet (Wallet B): a deterministic BIP39 24-word
//! mnemonic plus its mainnet UFVK, derived exactly as Zashi does — BIP39 seed
//! (empty passphrase) -> UnifiedSpendingKey::from_seed at account 0.
//!
//! Restore the printed MNEMONIC into a fresh Zashi wallet to spend; screen the
//! printed UFVK. THROWAWAY ONLY: the default entropy is public, so never hold
//! more than sub-cent funds in this wallet.
//!
//! Run: cargo run -p clean-wallet-scanner --bin gen-dirty-wallet -- [hex-entropy]
use bip0039::Mnemonic;
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
use zcash_protocol::consensus::Network;
use zip32::AccountId;

fn main() {
    let net = Network::MainNetwork;
    // 32 bytes of entropy => 24-word mnemonic. Fixed default = reproducible demo.
    let entropy_hex = std::env::args().nth(1).unwrap_or_else(|| {
        "badc0ffee0ddf00dbadc0ffee0ddf00dbadc0ffee0ddf00dbadc0ffee0ddf00d".to_string()
    });
    let entropy = hex::decode(entropy_hex.trim()).expect("hex entropy (16 or 32 bytes)");

    let mnemonic = <Mnemonic>::from_entropy(entropy).expect("valid entropy length");
    let seed = mnemonic.to_seed(""); // 64-byte BIP39 seed, empty passphrase (Zashi default)
    let usk = UnifiedSpendingKey::from_seed(&net, &seed, AccountId::ZERO).expect("USK from seed");
    let ufvk = usk.to_unified_full_viewing_key();
    let (ua, _) = ufvk
        .default_address(UnifiedAddressRequest::ALLOW_ALL)
        .expect("default unified address");

    println!("MNEMONIC\t{}", mnemonic.phrase());
    println!("UFVK\t{}", ufvk.encode(&net));
    println!("DEFAULT_UA\t{}", ua.encode(&net));
}
