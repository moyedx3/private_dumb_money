# A1 Interface — Wire Format

## 2. A1이 소유하는 wire format

## 2.1 Payment memo payload

구매자가 Zcash shielded payment memo에 넣는 A1 payload다.

### Raw form

```text
memo[0..8]  = drop_id : u64 big-endian
memo[8..40] = e_pub   : X25519 public key, 32 bytes
length      = 40 bytes
```

Rust helper:

```rust
encode_memo(drop_id: u64, e_pub: &[u8; 32]) -> Vec<u8>
decode_memo(memo: &[u8]) -> Option<(u64, [u8; 32])>
```

### Text fallback form

일부 wallet이 raw binary memo 입력을 어렵게 만들 수 있어 text fallback도 지원한다.

```text
A1B64:<base64url_no_pad(raw40)>
```

Rust helper:

```rust
encode_text_memo(drop_id: u64, e_pub: &[u8; 32]) -> String
```

고정 prefix:

```text
A1B64:
```

### Test vector

입력:

```text
drop_id = 1
e_pub   = 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f
```

text fallback:

```text
A1B64:AAAAAAAAAAEAAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHw
```

### ZIP-321 wrapping 주의

ZIP-321의 `memo=` 파라미터는 “온체인 memo bytes”를 base64url-no-pad로 감싼 값이어야 한다.

```text
raw form:
  on-chain memo bytes = raw40
  ZIP-321 memo=base64url_no_pad(raw40)

text fallback form:
  on-chain memo bytes = utf8("A1B64:" + base64url_no_pad(raw40))
  ZIP-321 memo=base64url_no_pad(on-chain memo bytes)
```

B 쪽 기본값 권장:

```text
1. 데모 wallet/Zashi가 raw 40B를 안정적으로 싣는 것이 확인되면 raw form 기본
2. raw memo가 깨지거나 wallet UI가 text만 받으면 A1B64 text fallback 기본
```

---

## 2.2 Dispatch blob

A1은 결제 검증 후 `K_drop`을 buyer의 `e_pub`로 sealed-box 암호화한다.

```text
blob = crypto_box_seal(K_drop, e_pub)
     = ek_pub(32 bytes) || ciphertext+MAC(48 bytes)
length = 80 bytes
```

구현:

```rust
wrap_k_drop(k_drop: &[u8; 32], e_pub: &[u8; 32]) -> Result<Vec<u8>>
```

상수:

```rust
EPHEMERAL_PUBLIC_KEY_LEN = 32
DISPATCH_BLOB_LEN = 80
```

Buyer open 방식:

```js
const k_drop = sodium.crypto_box_seal_open(blob, e_pub, e_priv)
```

A1 Rust `dryoc`와 B JS `libsodium-wrappers`는 Curve25519 sealed box 기준으로 호환되어야 한다.

---

## 2.3 Dispatch bucket key

A1이 dispatch blob을 저장할 때 사용하는 opaque key다.

```text
bucket_key = blake2b256(ek_pub || txid) as lowercase hex
```

여기서:

```text
ek_pub = dispatch blob[0..32]
txid   = lightwalletd txid bytes [u8; 32]
```

구현:

```rust
blob_key(ek_pub_prefix: &[u8], txid: &[u8; 32]) -> String
```

속성:

```text
- bucket_key에는 drop_id가 직접 들어가지 않음
- buyer identifier도 들어가지 않음
- sealed box ek_pub가 랜덤이므로 사전 계산하기 어려움
```

중요한 협업 포인트:

```text
Buyer는 일반적으로 bucket_key를 미리 모른다.
따라서 production B flow에는 dispatch list/recent endpoint가 필요하다.
```

---
