# A1 API Endpoint Vectors

## 목적

creator가 결제 대상 drop/catalog 정보를 등록하고, 구매자가 결제 후 생성된 dispatch blob을 조회할 수 있는 API service vector를 추가했다. 현재 구현은 HTTP 서버를 바로 띄우는 것이 아니라, REST/gRPC/enclave-RPC endpoint가 호출할 수 있는 Rust service layer다. 이렇게 둔 이유는 HTTP 프레임워크 의존성을 추가하지 않고도 enclave 경계와 테스트 가능한 business vector를 먼저 고정하기 위해서다.

## 추가된 endpoint vector

| Endpoint | Handler | 목적 |
| --- | --- | --- |
| `POST /api/creators/{creator_id}/drops` | `ApiVectors::register_creator_drop` | creator drop 등록. scanner가 나중에 `drop_id`로 가격, `K_drop`, creator UFVK를 찾을 수 있게 catalog vector에 저장한다. |
| `GET /api/buyers/dispatch/{bucket_key}` | `ApiVectors::lookup_dispatch` | 구매자가 가진 `bucket_key`로 sealed dispatch blob을 조회한다. |

## 추가/변경 파일

| 파일 | 내용 |
| --- | --- |
| `indexer/src/api.rs` | 신규. API route constants, endpoint vector, creator 등록 request/response, dispatch lookup response, 인메모리 `ApiVectors` 구현. |
| `indexer/src/lib.rs` | `api` 모듈 export 추가. |

## 현재 데이터 흐름

```text
Creator/Admin
  -> POST /api/creators/{creator_id}/drops
  -> ApiVectors::register_creator_drop
  -> CreatorDropRecord 저장
  -> Catalog::lookup(drop_id) 가능

Buyer payment detected by scanner
  -> Engine::on_note(...)
  -> Bucket::put(bucket_key, dispatch_blob)
  -> ApiVectors dispatch vector 저장

Buyer
  -> GET /api/buyers/dispatch/{bucket_key}
  -> ApiVectors::lookup_dispatch
  -> sealed dispatch blob 반환
```

## 등록 요청 구조

현재 Rust service request:

```rust
RegisterCreatorDropRequest {
    creator_id: String,
    creator_ufvk: String,
    price_zat: u64,
    k_drop: [u8; 32],
}
```

응답:

```rust
RegisterCreatorDropResponse {
    drop_id: u64,
    creator_id: String,
}
```

## 구매자 조회 구조

```rust
lookup_dispatch(bucket_key) -> Option<DispatchLookupResponse>
```

응답에는 다음이 포함된다.

```rust
DispatchLookupResponse {
    bucket_key: String,
    bytes: Vec<u8>, // sealed dispatch blob, 현재 80 bytes
}
```

## 보안상 중요한 점

현재 `ApiVectors`는 개발/테스트용 in-memory adapter다. 운영에서는 plaintext `creator_ufvk`와 `k_drop`이 host API 메모리/DB에 남으면 안 된다.

운영 권장 구조:

```text
Host API
  - HTTP routing/auth/rate limit만 담당
  - 가능하면 encrypted_payload_for_enclave만 받음

Enclave API handler
  - attested public key로 받은 payload 복호화
  - creator UFVK / K_drop 검증
  - catalog record를 enclave sealing key로 암호화
  - host DB에는 encrypted catalog blob만 저장
```

즉 현재 API vector는 다음 단계의 HTTP/enclave adapter가 호출할 내부 contract이며, 최종 운영에서는 `ApiVectors` 대신 encrypted DB-backed catalog/bucket 구현으로 교체해야 한다.

## 테스트로 고정된 동작

- endpoint vector에 creator 등록 route와 buyer dispatch 조회 route가 존재한다.
- creator 등록 시 `Catalog::lookup(drop_id)`가 가능해진다.
- invalid registration(`price_zat = 0`)은 거부된다.
- scanner engine이 dispatch를 `Bucket::put`하면 buyer lookup으로 같은 blob을 조회할 수 있다.
- 없는 bucket key는 `None`을 반환한다.

## 아직 남은 작업

1. 실제 HTTP endpoint adapter 추가.
2. attestation 기반 encrypted ingress 추가.
3. encrypted catalog DB store 추가.
4. buyer가 `bucket_key`를 얻는 방식 확정: txid 기반 조회, payment receipt 기반 조회, 또는 client-side deterministic derivation 중 선택 필요.
5. dispatch blob에 대한 access control/rate limit 정책 추가.
