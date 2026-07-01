use anyhow::{anyhow, Result};
use orchard::note_encryption::OrchardDomain;
use sapling_crypto::note_encryption::{
    try_sapling_note_decryption, PreparedIncomingViewingKey as SaplingPreparedIvk,
    Zip212Enforcement,
};
use zcash_keys::keys::UnifiedFullViewingKey;
use zcash_note_encryption::try_note_decryption;
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{BlockHeight, BranchId, Network, NetworkUpgrade, Parameters};

/// One shielded note decrypted with the creator's incoming viewing key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingNote {
    pub pool: ShieldedPool,
    pub value_zat: u64,
    /// Raw ZIP-302 memo bytes as recovered from the full transaction ciphertext.
    pub memo: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShieldedPool {
    Sapling,
    Orchard,
}

impl ShieldedPool {
    pub fn as_str(self) -> &'static str {
        match self {
            ShieldedPool::Sapling => "sapling",
            ShieldedPool::Orchard => "orchard",
        }
    }
}

/// Infer Zcash network from a ZIP-316 UFVK prefix.
pub fn infer_network_from_ufvk(ufvk: &str) -> Result<Network> {
    if ufvk.starts_with("uviewtest") {
        Ok(Network::TestNetwork)
    } else if ufvk.starts_with("uview") {
        Ok(Network::MainNetwork)
    } else {
        Err(anyhow!(
            "UFVK must start with 'uview' (mainnet) or 'uviewtest' (testnet)"
        ))
    }
}

/// Validate that a provisioned creator UFVK is syntactically decodable and
/// contains at least one shielded viewing key usable by the scanner.
pub fn validate_ufvk(ufvk: &str) -> Result<()> {
    let network = infer_network_from_ufvk(ufvk)?;
    let decoded = UnifiedFullViewingKey::decode(&network, ufvk)
        .map_err(|e| anyhow!("UFVK decode failed: {e}"))?;

    if decoded.sapling().is_none() && decoded.orchard().is_none() {
        return Err(anyhow!("UFVK carries neither a Sapling nor an Orchard key"));
    }

    Ok(())
}

/// Convert explorer/display txid hex (big-endian) to lightwalletd txid bytes.
pub fn display_txid_to_lightwalletd_bytes(txid_hex: &str) -> Result<[u8; 32]> {
    let s = txid_hex
        .trim()
        .strip_prefix("0x")
        .unwrap_or(txid_hex.trim());
    if s.len() != 64 {
        return Err(anyhow!("txid must be 64 hex chars"));
    }
    let mut bytes: [u8; 32] = hex::decode(s)?
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("txid must decode to 32 bytes"))?;
    bytes.reverse();
    Ok(bytes)
}

/// Convert lightwalletd txid bytes to explorer/display txid hex.
pub fn lightwalletd_txid_to_display_hex(txid: &[u8; 32]) -> String {
    let mut bytes = *txid;
    bytes.reverse();
    hex::encode(bytes)
}

/// A v5 tx embeds its consensus branch id at bytes [8..12]. A branch newer than this
/// librustzcash build can fail parse although note decryption does not depend on that
/// branch id. Rewrite to NU5 for trial-decryption fallback. See spec.md C6.
pub fn patch_v5_branch_to_nu5(raw: &[u8]) -> Vec<u8> {
    let mut patched = raw.to_vec();
    if patched.len() > 12 && patched[0..4] == [0x05, 0x00, 0x00, 0x80] {
        patched[8..12].copy_from_slice(&0xC2D6_D0B4u32.to_le_bytes());
    }
    patched
}

/// Parse a Zcash transaction, retrying v5 txs with a NU5 branch-id patch for incoming
/// note decryption compatibility across newer network upgrades.
pub fn read_tx_lenient(raw: &[u8], network: &Network, height: u32) -> Result<Transaction> {
    let block_height = BlockHeight::from_u32(height);
    let branch_id = BranchId::for_height(network, block_height);
    match Transaction::read(raw, branch_id) {
        Ok(tx) => Ok(tx),
        Err(first_err) => {
            let patched = patch_v5_branch_to_nu5(raw);
            if patched == raw {
                return Err(first_err.into());
            }
            Transaction::read(&patched[..], BranchId::Nu5).map_err(Into::into)
        }
    }
}

/// Recover incoming shielded notes and memos addressed to `ufvk_str` from a full raw tx.
///
/// This intentionally uses incoming IVK decryption, not OVK outgoing recovery, and must be
/// fed full transaction bytes from `GetTransaction`; compact blocks do not include the
/// full 512-byte memo ciphertext.
pub fn detect_incoming(
    ufvk_str: &str,
    raw_tx: &[u8],
    network: &Network,
    height: u32,
) -> Result<Vec<IncomingNote>> {
    let ufvk = UnifiedFullViewingKey::decode(network, ufvk_str)
        .map_err(|e| anyhow!("UFVK decode failed: {e}"))?;

    // ZIP-316 UFVKs can derive both external receiving keys and internal/change
    // keys.  `zecscope-scanner` scans both scopes; mirror that behavior here so
    // the full-transaction memo path does not miss notes that are visible only
    // through the internal scope.
    let sapling_ivks = ufvk
        .sapling()
        .map(|s| {
            [zip32::Scope::External, zip32::Scope::Internal]
                .into_iter()
                .map(|scope| SaplingPreparedIvk::new(&s.to_ivk(scope)))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let orchard_ivks = ufvk
        .orchard()
        .map(|o| {
            [
                orchard::keys::Scope::External,
                orchard::keys::Scope::Internal,
            ]
            .into_iter()
            .map(|scope| orchard::keys::PreparedIncomingViewingKey::new(&o.to_ivk(scope)))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if sapling_ivks.is_empty() && orchard_ivks.is_empty() {
        return Err(anyhow!("UFVK carries neither a Sapling nor an Orchard key"));
    }

    let block_height = BlockHeight::from_u32(height);
    let tx = read_tx_lenient(raw_tx, network, height)?;
    let mut notes = Vec::new();

    if let Some(bundle) = tx.sapling_bundle() {
        let zip212 = if network.is_nu_active(NetworkUpgrade::Canopy, block_height) {
            Zip212Enforcement::On
        } else {
            Zip212Enforcement::GracePeriod
        };

        for pivk in &sapling_ivks {
            for output in bundle.shielded_outputs() {
                if let Some((note, _addr, memo)) = try_sapling_note_decryption(pivk, output, zip212)
                {
                    notes.push(IncomingNote {
                        pool: ShieldedPool::Sapling,
                        value_zat: note.value().inner(),
                        memo: memo.as_slice().to_vec(),
                    });
                }
            }
        }
    }

    if let Some(bundle) = tx.orchard_bundle() {
        for pivk in &orchard_ivks {
            for action in bundle.actions() {
                let domain = OrchardDomain::for_action(action);
                if let Some((note, _addr, memo)) = try_note_decryption(&domain, pivk, action) {
                    notes.push(IncomingNote {
                        pool: ShieldedPool::Orchard,
                        value_zat: note.value().inner(),
                        memo: memo.to_vec(),
                    });
                }
            }
        }
    }

    Ok(notes)
}

/// Trim ZIP-302 zero padding for display. A first byte of 0xF6 is "no memo".
pub fn display_memo_bytes(memo: &[u8]) -> Option<&[u8]> {
    if memo.first() == Some(&0xF6) {
        return None;
    }
    match memo.iter().rposition(|&b| b != 0) {
        Some(i) => Some(&memo[..=i]),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_unknown_branch_on_v5() {
        let mut raw = vec![
            0x05, 0x00, 0x00, 0x80, // header = v5 overwintered
            0x0a, 0x27, 0xa7, 0x26, // version group id
            0x30, 0xf3, 0x37, 0x54, // unknown newer branch id
        ];
        raw.extend_from_slice(&[0u8; 8]);
        let patched = patch_v5_branch_to_nu5(&raw);
        assert_eq!(&patched[8..12], &0xC2D6_D0B4u32.to_le_bytes());
    }

    #[test]
    fn display_txid_roundtrips_lightwalletd_order() {
        let display = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
        let le = display_txid_to_lightwalletd_bytes(display).unwrap();
        assert_eq!(lightwalletd_txid_to_display_hex(&le), display);
    }

    #[test]
    fn trims_padding_and_detects_no_memo() {
        let mut memo = vec![b'a', b'b', b'c'];
        memo.extend_from_slice(&[0u8; 10]);
        assert_eq!(display_memo_bytes(&memo), Some(&b"abc"[..]));
        assert_eq!(display_memo_bytes(&[0xF6, 0, 0]), None);
    }
}
