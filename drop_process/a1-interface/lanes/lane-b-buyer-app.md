# A1 ↔ Lane B Buyer App Interface

## A1이 Lane B에 제공할 것

- public catalog 항목
  - `drop_id`
  - `creator_id`
  - `title`
  - `price_zat` / `price_zec`
  - `deposit_addr`
  - `h_content`
  - memo 생성 규칙
- 결제 memo format
  - `A1B64:<base64url_no_pad(drop_id(8B) || buyer_e_pub(32B))>`
- dispatch 조회 interface
  - 현재 구현: `bucket_key` 기반 lookup boundary
  - 추가 필요: buyer가 알기 쉬운 `recent/list/by-tx` 조회 API

## Lane B가 해야 할 것

```text
catalog 조회
→ buyer e_pub 생성
→ A1 memo 생성
→ wallet 결제 요청 구성
→ 결제 후 dispatch blob 조회
→ buyer secret key로 K_drop 복호화
→ content blob 복호화
```

## 현재 blocking gap

- public catalog endpoint가 아직 실제 HTTP로 노출되지 않음
- `deposit_addr`가 public catalog에 포함되어야 함
- buyer는 보통 `bucket_key`를 사전에 모르므로 dispatch recent/list 계열 endpoint가 필요함
