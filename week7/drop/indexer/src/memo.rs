//! A1-owned Zcash memo payload codec.
//!
//! Frozen format: `drop_id` (u64 big-endian, 8 bytes) followed by the buyer's
//! X25519 ephemeral public key (`e_pub`, 32 bytes).  The raw payload is exactly
//! 40 bytes and may appear at the front of a padded ZIP-302 memo field.

/// Length of the A1-owned memo payload: `drop_id(8) || e_pub(32)`.
pub const DROP_MEMO_LEN: usize = 40;

/// Drop memo = drop_id (u64 BE, 8 bytes) || e_pub (X25519, 32 bytes) = 40 bytes.
pub fn encode_memo(drop_id: u64, e_pub: &[u8; 32]) -> Vec<u8> {
    let mut memo = Vec::with_capacity(DROP_MEMO_LEN);
    memo.extend_from_slice(&drop_id.to_be_bytes());
    memo.extend_from_slice(e_pub);
    memo
}

/// Decode the leading 40 bytes of a Zcash memo. Trailing ZIP-302 zero padding is ignored.
pub fn decode_memo(memo: &[u8]) -> Option<(u64, [u8; 32])> {
    if memo.len() < DROP_MEMO_LEN {
        return None;
    }

    let drop_id = u64::from_be_bytes(memo[0..8].try_into().ok()?);
    let e_pub = memo[8..DROP_MEMO_LEN].try_into().ok()?;
    Some((drop_id, e_pub))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memo_roundtrips() {
        let e_pub = [7u8; 32];
        let raw = encode_memo(0xDEAD_BEEF, &e_pub);
        assert_eq!(raw.len(), DROP_MEMO_LEN);

        let (drop_id, got) = decode_memo(&raw).unwrap();
        assert_eq!(drop_id, 0xDEAD_BEEF);
        assert_eq!(got, e_pub);
    }

    #[test]
    fn decode_rejects_wrong_len() {
        assert!(decode_memo(&[0u8; DROP_MEMO_LEN - 1]).is_none());
    }

    #[test]
    fn decode_ignores_trailing_padding() {
        let e_pub = [9u8; 32];
        let mut raw = encode_memo(42, &e_pub);
        raw.extend_from_slice(&[0u8; 472]);

        let (drop_id, got) = decode_memo(&raw).unwrap();
        assert_eq!(drop_id, 42);
        assert_eq!(got, e_pub);
    }
}
