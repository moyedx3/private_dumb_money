//! Encrypted scanner cursor/replay state.
//!
//! The host may persist the encrypted bytes, but plaintext state and the state
//! encryption key are intended to exist only inside the enclave/TEE boundary.
//! `SecretboxStateCipher::from_hex_key` is a development adapter; production
//! should provide a `StateCipher` implementation backed by the TEE sealing key.

use anyhow::{anyhow, Context, Result};
use dryoc::dryocsecretbox::{DryocSecretBox, Key, Nonce, VecBox};
use dryoc::types::{Bytes, NewByteArray};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const PLAIN_MAGIC: &[u8; 8] = b"A1STATE1";
const ENC_MAGIC: &[u8; 8] = b"A1STENC1";
const NO_HEIGHT: u64 = u64::MAX;

/// Cursor and replay-guard state used by scanner loops.
pub trait ScanState {
    fn last_scanned_height(&self) -> Option<u64>;
    fn set_last_scanned_height(&mut self, height: u64);
    fn has_seen_txid(&self, txid: &[u8; 32]) -> bool;
    /// Returns true when the txid was newly inserted.
    fn mark_seen_txid(&mut self, txid: [u8; 32]) -> bool;
    fn seen_txids_len(&self) -> usize;
}

/// In-memory implementation for tests and non-persistent development runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryScanState {
    last_scanned_height: Option<u64>,
    seen_txids: HashSet<[u8; 32]>,
}

impl MemoryScanState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_parts(last_scanned_height: Option<u64>, seen_txids: HashSet<[u8; 32]>) -> Self {
        Self {
            last_scanned_height,
            seen_txids,
        }
    }

    pub fn seen_txids(&self) -> &HashSet<[u8; 32]> {
        &self.seen_txids
    }
}

impl ScanState for MemoryScanState {
    fn last_scanned_height(&self) -> Option<u64> {
        self.last_scanned_height
    }

    fn set_last_scanned_height(&mut self, height: u64) {
        self.last_scanned_height = Some(height);
    }

    fn has_seen_txid(&self, txid: &[u8; 32]) -> bool {
        self.seen_txids.contains(txid)
    }

    fn mark_seen_txid(&mut self, txid: [u8; 32]) -> bool {
        self.seen_txids.insert(txid)
    }

    fn seen_txids_len(&self) -> usize {
        self.seen_txids.len()
    }
}

/// Authenticated encryption boundary for scanner state.
pub trait StateCipher {
    fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>>;
    fn open(&self, ciphertext: &[u8]) -> Result<Vec<u8>>;
}

/// Development secretbox cipher.
///
/// This uses XSalsa20-Poly1305 via dryoc/libsodium-compatible secretbox. Do not
/// source this key from host-visible env in production; production should use a
/// TEE sealing-key adapter implementing [`StateCipher`].
#[derive(Clone)]
pub struct SecretboxStateCipher {
    key: [u8; 32],
}

impl SecretboxStateCipher {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    pub fn from_hex_key(hex_key: &str) -> Result<Self> {
        let bytes = hex::decode(hex_key.trim().strip_prefix("0x").unwrap_or(hex_key.trim()))
            .context("decode state key hex")?;
        let key: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| anyhow!("state key must be 32 bytes / 64 hex chars"))?;
        Ok(Self::new(key))
    }
}

impl StateCipher for SecretboxStateCipher {
    fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let key = Key::from(self.key);
        let nonce = Nonce::gen();
        let boxed = DryocSecretBox::encrypt_to_vecbox(plaintext, &nonce, &key);

        let mut out = Vec::with_capacity(ENC_MAGIC.len() + nonce.len() + boxed.to_vec().len());
        out.extend_from_slice(ENC_MAGIC);
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&boxed.into_vec());
        Ok(out)
    }

    fn open(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() < ENC_MAGIC.len() + 24 + 16 {
            return Err(anyhow!("encrypted state is too short"));
        }
        if &ciphertext[..ENC_MAGIC.len()] != ENC_MAGIC {
            return Err(anyhow!("encrypted state magic mismatch"));
        }

        let nonce_start = ENC_MAGIC.len();
        let box_start = nonce_start + 24;
        let nonce = Nonce::try_from(&ciphertext[nonce_start..box_start])
            .map_err(|_| anyhow!("invalid encrypted state nonce"))?;
        let key = Key::from(self.key);
        let boxed = VecBox::from_bytes(&ciphertext[box_start..])
            .map_err(|e| anyhow!("invalid encrypted state box: {e}"))?;
        boxed
            .decrypt_to_vec(&nonce, &key)
            .map_err(|e| anyhow!("decrypt encrypted state: {e}"))
    }
}

/// Encrypted state persisted in a host-visible file.
pub struct EncryptedFileScanState<C: StateCipher> {
    path: PathBuf,
    cipher: C,
    inner: MemoryScanState,
}

impl<C: StateCipher> EncryptedFileScanState<C> {
    pub fn load_or_default(path: impl Into<PathBuf>, cipher: C) -> Result<Self> {
        let path = path.into();
        let inner = if path.exists() {
            let ciphertext = fs::read(&path)
                .with_context(|| format!("read encrypted state {}", path.display()))?;
            let plaintext = cipher
                .open(&ciphertext)
                .with_context(|| format!("decrypt encrypted state {}", path.display()))?;
            deserialize_state(&plaintext)
                .with_context(|| format!("parse encrypted state {}", path.display()))?
        } else {
            MemoryScanState::new()
        };

        Ok(Self {
            path,
            cipher,
            inner,
        })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create state dir {}", parent.display()))?;
            }
        }

        let plaintext = serialize_state(&self.inner)?;
        let ciphertext = self.cipher.seal(&plaintext)?;
        let tmp_path = tmp_path_for(&self.path);

        fs::write(&tmp_path, ciphertext)
            .with_context(|| format!("write temp encrypted state {}", tmp_path.display()))?;
        fs::rename(&tmp_path, &self.path).with_context(|| {
            format!(
                "atomically replace encrypted state {} with {}",
                self.path.display(),
                tmp_path.display()
            )
        })?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn inner(&self) -> &MemoryScanState {
        &self.inner
    }
}

impl<C: StateCipher> ScanState for EncryptedFileScanState<C> {
    fn last_scanned_height(&self) -> Option<u64> {
        self.inner.last_scanned_height()
    }

    fn set_last_scanned_height(&mut self, height: u64) {
        self.inner.set_last_scanned_height(height);
    }

    fn has_seen_txid(&self, txid: &[u8; 32]) -> bool {
        self.inner.has_seen_txid(txid)
    }

    fn mark_seen_txid(&mut self, txid: [u8; 32]) -> bool {
        self.inner.mark_seen_txid(txid)
    }

    fn seen_txids_len(&self) -> usize {
        self.inner.seen_txids_len()
    }
}

pub fn serialize_state(state: &MemoryScanState) -> Result<Vec<u8>> {
    let count: u32 = state
        .seen_txids
        .len()
        .try_into()
        .map_err(|_| anyhow!("too many seen txids to serialize"))?;
    let mut txids = state.seen_txids.iter().copied().collect::<Vec<_>>();
    txids.sort_unstable();

    let mut out = Vec::with_capacity(PLAIN_MAGIC.len() + 8 + 4 + txids.len() * 32);
    out.extend_from_slice(PLAIN_MAGIC);
    out.extend_from_slice(&state.last_scanned_height.unwrap_or(NO_HEIGHT).to_be_bytes());
    out.extend_from_slice(&count.to_be_bytes());
    for txid in txids {
        out.extend_from_slice(&txid);
    }
    Ok(out)
}

pub fn deserialize_state(bytes: &[u8]) -> Result<MemoryScanState> {
    if bytes.len() < PLAIN_MAGIC.len() + 8 + 4 {
        return Err(anyhow!("state plaintext too short"));
    }
    if &bytes[..PLAIN_MAGIC.len()] != PLAIN_MAGIC {
        return Err(anyhow!("state plaintext magic mismatch"));
    }

    let mut pos = PLAIN_MAGIC.len();
    let last = u64::from_be_bytes(bytes[pos..pos + 8].try_into()?);
    pos += 8;
    let count = u32::from_be_bytes(bytes[pos..pos + 4].try_into()?) as usize;
    pos += 4;

    let expected = pos + count * 32;
    if bytes.len() != expected {
        return Err(anyhow!(
            "state plaintext length mismatch: expected {expected}, got {}",
            bytes.len()
        ));
    }

    let mut seen_txids = HashSet::with_capacity(count);
    for chunk in bytes[pos..].chunks_exact(32) {
        seen_txids.insert(chunk.try_into()?);
    }

    Ok(MemoryScanState::from_parts(
        (last != NO_HEIGHT).then_some(last),
        seen_txids,
    ))
}

fn tmp_path_for(path: &Path) -> PathBuf {
    let mut tmp = path.to_path_buf();
    let ext = path
        .extension()
        .map(|e| format!("{}.tmp", e.to_string_lossy()))
        .unwrap_or_else(|| "tmp".to_string());
    tmp.set_extension(ext);
    tmp
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample_state() -> MemoryScanState {
        let mut state = MemoryScanState::new();
        state.set_last_scanned_height(3387683);
        state.mark_seen_txid([1u8; 32]);
        state.mark_seen_txid([2u8; 32]);
        state
    }

    #[test]
    fn memory_state_serializes_roundtrip() {
        let state = sample_state();
        let encoded = serialize_state(&state).unwrap();
        let decoded = deserialize_state(&encoded).unwrap();

        assert_eq!(decoded.last_scanned_height(), Some(3387683));
        assert!(decoded.has_seen_txid(&[1u8; 32]));
        assert!(decoded.has_seen_txid(&[2u8; 32]));
        assert_eq!(decoded.seen_txids_len(), 2);
    }

    #[test]
    fn secretbox_cipher_hides_plaintext_and_authenticates() {
        let cipher = SecretboxStateCipher::new([7u8; 32]);
        let plaintext = serialize_state(&sample_state()).unwrap();
        let ciphertext = cipher.seal(&plaintext).unwrap();

        assert_ne!(ciphertext, plaintext);
        assert!(!ciphertext
            .windows(PLAIN_MAGIC.len())
            .any(|w| w == PLAIN_MAGIC));
        assert_eq!(cipher.open(&ciphertext).unwrap(), plaintext);

        let wrong_cipher = SecretboxStateCipher::new([8u8; 32]);
        assert!(wrong_cipher.open(&ciphertext).is_err());
    }

    #[test]
    fn encrypted_file_state_saves_only_ciphertext() {
        let mut path = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("a1-state-{unique}.enc"));

        let cipher = SecretboxStateCipher::new([3u8; 32]);
        let mut state = EncryptedFileScanState::load_or_default(&path, cipher.clone()).unwrap();
        state.set_last_scanned_height(123);
        state.mark_seen_txid([9u8; 32]);
        state.save().unwrap();

        let on_disk = fs::read(&path).unwrap();
        assert!(!on_disk.windows(PLAIN_MAGIC.len()).any(|w| w == PLAIN_MAGIC));
        assert!(!on_disk.windows(32).any(|w| w == [9u8; 32]));

        let loaded = EncryptedFileScanState::load_or_default(&path, cipher).unwrap();
        assert_eq!(loaded.last_scanned_height(), Some(123));
        assert!(loaded.has_seen_txid(&[9u8; 32]));

        let _ = fs::remove_file(path);
    }
}
