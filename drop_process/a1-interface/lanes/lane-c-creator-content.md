# A1 ↔ Lane C Creator / Content Interface

## Lane C가 A1에 등록해야 할 정보

- `creator_id`
- `drop_id`
- `price_zat`
- `K_drop`
- `deposit_addr`
- `h_content`
- optional public metadata
  - `title`
  - `description`

## A1이 이 정보를 쓰는 방식

```text
incoming memo의 drop_id 확인
→ catalog에서 drop config 조회
→ incoming value >= price_zat 검증
→ K_drop을 buyer_e_pub으로 wrapping
→ dispatch blob 생성
```

## 주의점

- `K_drop`은 public catalog에 절대 포함하면 안 됨
- `UFVK/IVK`도 외부 노출 금지
- public catalog에는 구매자가 결제 생성에 필요한 공개 정보만 포함해야 함
