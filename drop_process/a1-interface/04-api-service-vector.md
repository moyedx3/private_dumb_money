# A1 Interface — API Service Vector

## 7. API service vector interface

현재 A1에는 HTTP framework가 없다. 대신 HTTP/enclave adapter가 호출할 Rust service vector가 있다.

## 7.1 Endpoint vector

```rust
endpoint_vector() -> Vec<ApiEndpoint>
```

현재 contract:

```text
POST /api/creators/{creator_id}/drops
GET  /api/buyers/dispatch/{bucket_key}
```

주의:

```text
GET /api/buyers/dispatch/{bucket_key}는 현재 보조 조회용이다.
Lane B production polling에는 dispatch list/recent endpoint가 추가로 필요하다.
```

## 7.2 RegisterCreatorDropRequest

현재 개발용 request:

```rust
pub struct RegisterCreatorDropRequest {
    pub creator_id: String,
    pub creator_ufvk: String,
    pub price_zat: u64,
    pub k_drop: [u8; 32],
}
```

response:

```rust
pub struct RegisterCreatorDropResponse {
    pub drop_id: u64,
    pub creator_id: String,
}
```

운영 주의:

```text
이 request는 plaintext 개발 vector다.
운영에서는 creator_ufvk/k_drop이 host API에 평문으로 노출되면 안 된다.
```

## 7.3 ApiVectors

```rust
pub struct ApiVectors;
```

제공 기능:

```rust
register_creator_drop(req) -> Result<RegisterCreatorDropResponse>
lookup_dispatch(bucket_key) -> Option<DispatchLookupResponse>
creator_drops() -> Vec<CreatorDropRecord>
dispatch_blobs() -> Vec<DispatchBlobRecord>
```

trait 구현:

```rust
impl Catalog for ApiVectors
impl Bucket for ApiVectors
```

즉 테스트/개발에서는 다음 흐름이 가능하다.

```text
ApiVectors::register_creator_drop
→ Engine::new(api.clone(), api.clone())
→ Engine::on_note
→ ApiVectors::lookup_dispatch(bucket_key)
```

## 7.4 현재 API vector의 부족한 점

Lane B integration 기준으로 아직 필요하다.

```rust
PublicCatalogEntry {
    drop_id,
    title,
    price_zec,
    h_content,
    deposit_addr,
}

list_public_catalog() -> Vec<PublicCatalogEntry>
list_dispatch_keys() -> Vec<String>
list_recent_dispatches() -> Vec<DispatchBlobRecord>
```

HTTP contract 후보:

```http
GET /api/catalog
GET /api/dispatch
GET /api/dispatch/{key}
```

---
