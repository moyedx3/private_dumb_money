# A1 Interface — Engine and Scanner

## 3. A1 내부 business boundary

## 3.1 DropConfig

A1 engine이 결제 검증과 dispatch 생성에 필요한 최소 drop config다.

```rust
pub struct DropConfig {
    pub price_zat: u64,
    pub k_drop: [u8; 32],
    pub creator_ufvk: String,
}
```

의미:

| 필드 | 의미 |
| --- | --- |
| `price_zat` | 필요한 최소 결제 금액, zatoshi 단위 |
| `k_drop` | content/subscription unlock key, 32 bytes |
| `creator_ufvk` | 해당 creator/drop 결제 감지용 UFVK |

운영 주의:

```text
DropConfig 평문은 enclave 내부에서만 존재해야 한다.
host DB에는 encrypted catalog record만 저장하는 것이 목표다.
```

## 3.2 Catalog trait

A1 engine은 DB나 provisioning 구조를 직접 알지 않는다.

```rust
pub trait Catalog: Send + Sync {
    fn lookup(&self, drop_id: u64) -> Option<DropConfig>;
}
```

협업 의미:

```text
A2/enclave/catalog lane은 drop_id -> DropConfig lookup을 제공해야 한다.
```

## 3.3 Bucket trait

A1은 dispatch blob 저장소 구현을 직접 알지 않는다.

```rust
#[async_trait::async_trait]
pub trait Bucket: Send + Sync {
    async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()>;
}
```

협업 의미:

```text
D/storage lane은 Bucket::put(key, dispatch_blob)을 실제 bucket/R2/S3/DB에 연결해야 한다.
```

현재 개발 adapter:

```text
LoggingBucket
ApiVectors as Bucket
```

---

## 4. Scanner interface

## 4.1 Lightwalletd client boundary

```rust
#[async_trait]
pub trait LightwalletdClient: Send + Sync {
    async fn current_chain_tip(&self) -> Result<u64>;
    async fn fetch_block_range(&self, start: u64, end: u64) -> Result<Vec<CompactBlock>>;
    async fn fetch_transaction(&self, txid: &[u8; 32]) -> Result<Vec<u8>>;
}
```

현재 구현:

```text
GrpcClient
MockClient for tests
```

주의:

```text
compact block에는 full memo가 없다.
A1은 compact txid를 보고 반드시 GetTransaction으로 full tx를 다시 가져온다.
```

## 4.2 scan_once

수동/live smoke 또는 명시 범위 scan용.

```rust
pub async fn scan_once<C, K, B>(
    client: &C,
    ufvk: &str,
    network: &Network,
    start: u64,
    end: u64,
    engine: &mut Engine<K, B>,
) -> Result<ScanSummary>
where
    C: LightwalletdClient + ?Sized,
    K: Catalog,
    B: Bucket;
```

동작:

```text
GetBlockRange(start..=end)
→ each compact txid
→ GetTransaction(txid)
→ detect_incoming(ufvk, raw_tx, network, height)
→ decode_memo(raw40 or A1B64)
→ Engine::on_note
```

## 4.3 scan_once_with_state

운영 scanner loop가 사용해야 하는 state-aware scan 함수.

```rust
pub async fn scan_once_with_state<C, K, B, S>(
    client: &C,
    ufvk: &str,
    network: &Network,
    start: u64,
    end: u64,
    state: &mut S,
    engine: &mut Engine<K, B>,
) -> Result<ScanSummary>
where
    C: LightwalletdClient + ?Sized,
    K: Catalog,
    B: Bucket,
    S: ScanState;
```

추가 동작:

```text
- 이미 state에 있는 txid는 full tx fetch 전에 skip
- dispatch가 생성된 txid만 seen_txids에 기록
- range가 오류 없이 끝나면 last_scanned_height 갱신
```

## 4.4 ScanSummary

```rust
pub struct ScanSummary {
    pub blocks_fetched: usize,
    pub compact_txs: usize,
    pub full_txs_fetched: usize,
    pub incoming_notes: usize,
    pub notes_without_memo: usize,
    pub decoded_memos: usize,
    pub undecodable_memos: usize,
    pub dispatches: Vec<PaymentDispatch>,
}
```

운영 모니터링에서 유용한 값:

```text
incoming_notes
decoded_memos
undecodable_memos
dispatches.len()
```

---

## 5. Payment engine interface

## 5.1 Input Note

`detect + memo decode` 이후 engine에 들어가는 결제 후보.

```rust
pub struct Note {
    pub drop_id: u64,
    pub e_pub: [u8; 32],
    pub value_zat: u64,
    pub txid: [u8; 32],
}
```

## 5.2 Output PaymentDispatch

```rust
pub struct PaymentDispatch {
    pub drop_id: u64,
    pub txid: [u8; 32],
    pub value_zat: u64,
    pub bucket_key: String,
}
```

## 5.3 Engine flow

```rust
Engine::on_note(&Note) -> Result<Option<PaymentDispatch>>
```

동작 순서:

```text
1. txid replay guard
2. Catalog::lookup(drop_id)
3. value_zat >= price_zat 확인
4. wrap_k_drop(k_drop, e_pub)
5. bucket_key = blake2b256(ek_pub_of_sealed_box || txid)
6. Bucket::put(bucket_key, dispatch_blob)
7. PaymentDispatch 반환
```

반환이 `None`인 경우:

```text
- duplicate txid
- unknown drop_id
- underpaid note
```

---
