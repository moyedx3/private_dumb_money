# A1 ↔ Lane D Storage / Bucket Interface

## A1이 bucket에 저장하는 것

A1은 결제가 확인된 구매자별 dispatch blob을 저장한다.

```text
put(bucket_key, dispatch_blob)
get(bucket_key) -> dispatch_blob
```

## bucket_key

```text
bucket_key = blake2b256(ek_pub || txid)
```

## dispatch_blob

```text
dispatch_blob = crypto_box_seal(K_drop, buyer_e_pub)
```

- 크기: 80 bytes
- 구매자만 복호화 가능

## content blob과의 분리

| 종류 | 담당 | 설명 |
| --- | --- | --- |
| content blob | Lane C/D | 암호화된 콘텐츠 본문 |
| dispatch blob | A1/D | 구매자별로 암호화된 K_drop |

## 현재 gap

- 현재 A1은 bucket boundary와 in-memory/service vector까지 있음
- 실제 bucket backend adapter는 추가 필요
