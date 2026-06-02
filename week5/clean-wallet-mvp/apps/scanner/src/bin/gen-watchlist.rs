//! Mint a fresh mainnet WATCHLIST ("sanctioned") address from a hex seed and
//! print the exact sanctioned-set hashes the scanner computes for each pool.
//!
//! The scanner (see `scan.rs::extract_outgoing_recipients`) turns a recovered
//! recipient into a string and hashes `"0x" + hex(sha256(string))`:
//!   - Sapling output  -> string = ZcashAddress::from_sapling(Main, bytes) = "zs1..."
//!   - Orchard action  -> string = hex(addr.to_raw_address_bytes())  (86 hex chars)
//! So a payment to this unified address hits the set whichever pool the wallet
//! uses, as long as BOTH hashes below are in `sanctionedAddressHashes`.
//!
//! Run: cargo run -p clean-wallet-scanner --bin gen-watchlist -- <hex-seed>
use sha2::{Digest, Sha256};
use zcash_address::ToAddress as _;
use zcash_address::ZcashAddress;
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
use zcash_protocol::consensus::{Network, NetworkType};
use zip32::AccountId;

fn sha256_0x(s: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(s);
    format!("0x{}", hex::encode(h.finalize()))
}

fn main() {
    let net = Network::MainNetwork;
    let net_type = NetworkType::Main;
    let seed_hex = std::env::args()
        .nth(1)
        .expect("usage: gen-watchlist <hex-seed>");
    let seed = hex::decode(seed_hex.trim()).expect("hex seed");
    let usk = UnifiedSpendingKey::from_seed(&net, &seed, AccountId::ZERO).expect("USK from seed");
    let ufvk = usk.to_unified_full_viewing_key();

    let (ua, _idx) = ufvk
        .default_address(UnifiedAddressRequest::ALLOW_ALL)
        .expect("default unified address");

    println!("UNIFIED_ADDRESS\t{}", ua.encode(&net));

    if let Some(o) = ua.orchard() {
        let raw_hex = hex::encode(o.to_raw_address_bytes());
        println!("orchard_recipient_string\t{}", raw_hex);
        println!("orchard_hash\t{}", sha256_0x(raw_hex.as_bytes()));
    }
    if let Some(s) = ua.sapling() {
        let zstr = ZcashAddress::from_sapling(net_type, s.to_bytes()).to_string();
        println!("sapling_recipient_string\t{}", zstr);
        println!("sapling_hash\t{}", sha256_0x(zstr.as_bytes()));
    }
}
