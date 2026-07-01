//! A1-owned Zcash memo payload codec.
//!
//! Frozen format: `drop_id` (u64 big-endian, 8 bytes) followed by the buyer's
//! X25519 ephemeral public key (`e_pub`, 32 bytes).  The raw payload is exactly
//! 40 bytes and may appear at the front of a padded ZIP-302 memo field.
//!
//! Some wallets only expose a human-text memo field. For those wallets A1 also
//! accepts a text fallback: `A1B64:<base64url_no_pad(raw_40_bytes)>`.

/// Length of the A1-owned memo payload: `drop_id(8) || e_pub(32)`.
pub const DROP_MEMO_LEN: usize = 40;

/// Text memo fallback prefix for wallets that cannot write arbitrary raw memo bytes.
pub const TEXT_MEMO_PREFIX: &str = "A1B64:";

/// Drop memo = drop_id (u64 BE, 8 bytes) || e_pub (X25519, 32 bytes) = 40 bytes.
pub fn encode_memo(drop_id: u64, e_pub: &[u8; 32]) -> Vec<u8> {
    let mut memo = Vec::with_capacity(DROP_MEMO_LEN);
    memo.extend_from_slice(&drop_id.to_be_bytes());
    memo.extend_from_slice(e_pub);
    memo
}

/// Encode an A1 text memo fallback for wallets that only accept UTF-8 memo text.
///
/// The returned string can be pasted into a normal wallet memo field:
/// `A1B64:<base64url_no_pad(drop_id(8) || e_pub(32))>`.
pub fn encode_text_memo(drop_id: u64, e_pub: &[u8; 32]) -> String {
    format!(
        "{TEXT_MEMO_PREFIX}{}",
        base64url_no_pad_encode(&encode_memo(drop_id, e_pub))
    )
}

/// Decode either:
/// - native raw A1 memo bytes: `drop_id(8) || e_pub(32)`, or
/// - text fallback: `A1B64:<base64url_no_pad(raw_40_bytes)>`.
///
/// Trailing ZIP-302 zero padding is ignored for both forms.
pub fn decode_memo(memo: &[u8]) -> Option<(u64, [u8; 32])> {
    if let Some(decoded) = decode_text_memo(memo) {
        return Some(decoded);
    }

    decode_raw_memo(memo)
}

fn decode_raw_memo(memo: &[u8]) -> Option<(u64, [u8; 32])> {
    if memo.len() < DROP_MEMO_LEN {
        return None;
    }

    let drop_id = u64::from_be_bytes(memo[0..8].try_into().ok()?);
    let e_pub = memo[8..DROP_MEMO_LEN].try_into().ok()?;
    Some((drop_id, e_pub))
}

fn decode_text_memo(memo: &[u8]) -> Option<(u64, [u8; 32])> {
    let text = std::str::from_utf8(memo)
        .ok()?
        .trim_end_matches('\0')
        .trim();
    let encoded = text.strip_prefix(TEXT_MEMO_PREFIX)?.trim();
    let raw = base64url_no_pad_decode(encoded)?;
    if raw.len() != DROP_MEMO_LEN {
        return None;
    }
    decode_raw_memo(&raw)
}

fn base64url_no_pad_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);

        out.push(ALPHABET[(b0 >> 2) as usize] as char);
        out.push(ALPHABET[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(b2 & 0b0011_1111) as usize] as char);
        }
    }
    out
}

fn base64url_no_pad_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 4 == 1 {
        return None;
    }

    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0u8;
    let mut saw_padding = false;

    for b in s.bytes() {
        if b == b'=' {
            saw_padding = true;
            continue;
        }
        if saw_padding {
            return None;
        }

        let value = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        } as u32;

        acc = (acc << 6) | value;
        bits += 6;
        while bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
            acc &= (1 << bits) - 1;
        }
    }

    if bits > 0 && acc != 0 {
        return None;
    }

    Some(out)
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

    #[test]
    fn text_memo_roundtrips_for_wallet_memo_fields() {
        let e_pub: [u8; 32] = core::array::from_fn(|i| i as u8);
        let text = encode_text_memo(1, &e_pub);

        assert_eq!(
            text,
            "A1B64:AAAAAAAAAAEAAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHw"
        );

        let (drop_id, got) = decode_memo(text.as_bytes()).unwrap();
        assert_eq!(drop_id, 1);
        assert_eq!(got, e_pub);
    }

    #[test]
    fn text_memo_decoder_ignores_wallet_padding() {
        let e_pub = [5u8; 32];
        let mut text = encode_text_memo(7, &e_pub).into_bytes();
        text.extend_from_slice(&[0u8; 16]);

        let (drop_id, got) = decode_memo(&text).unwrap();
        assert_eq!(drop_id, 7);
        assert_eq!(got, e_pub);
    }

    #[test]
    fn text_memo_decoder_rejects_bad_base64_or_wrong_length() {
        assert!(decode_memo(b"A1B64:not valid").is_none());
        assert!(decode_memo(b"A1B64:AA").is_none());
    }
}
