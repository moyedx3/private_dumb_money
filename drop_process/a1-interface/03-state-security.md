# A1 Interface — State and Security

## 6. Encrypted scan state interface

## 6.1 ScanState

```rust
pub trait ScanState {
    fn last_scanned_height(&self) -> Option<u64>;
    fn set_last_scanned_height(&mut self, height: u64);
    fn has_seen_txid(&self, txid: &[u8; 32]) -> bool;
    fn mark_seen_txid(&mut self, txid: [u8; 32]) -> bool;
    fn seen_txids_len(&self) -> usize;
}
```

현재 구현:

```text
MemoryScanState
EncryptedFileScanState<C: StateCipher>
```

## 6.2 StateCipher

```rust
pub trait StateCipher {
    fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>>;
    fn open(&self, ciphertext: &[u8]) -> Result<Vec<u8>>;
}
```

개발 구현:

```text
SecretboxStateCipher
```

주의:

```text
SecretboxStateCipher::from_hex_key / A1_STATE_KEY_HEX는 개발 smoke용이다.
운영에서는 host-visible env key를 쓰면 안 된다.
```

운영 구현 요구:

```rust
struct EnclaveStateCipher;

impl StateCipher for EnclaveStateCipher {
    fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        // enclave/TEE sealing key로 암호화
    }

    fn open(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        // enclave/TEE sealing key로 복호화
    }
}
```

## 6.3 EncryptedFileScanState

```rust
EncryptedFileScanState::load_or_default(path, cipher)
state.save()
```

파일에는 암호문만 저장된다.

평문 state 의미:

```text
last_scanned_height
seen_txids
```

---
