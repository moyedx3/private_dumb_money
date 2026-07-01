# A1 제공 내용 및 최종 지원 범위

## 1. A1의 역할

A1은 Zcash shielded 결제를 감지하고, 결제가 확인되면 구매자가 콘텐츠 복호화 키를 받을 수 있도록 dispatch blob을 생성하는 결제 엔진이다.

전체 흐름은 다음과 같다.

```text
creator drop 등록
→ 구매자 shielded payment 전송
→ A1 scanner가 lightwalletd로 체인 조회
→ UFVK/IVK로 incoming note 확인
→ memo 파싱
→ 결제 금액 검증
→ 구매자 e_pub으로 K_drop 암호화
→ dispatch blob 생성
→ bucket에 저장
→ 구매자가 dispatch blob 조회
```

---

## 2. 현재 A1에서 하고 있는 내용

### 2.1 체인 스캔

A1은 lightwalletd를 통해 실제 Zcash 체인 데이터를 조회한다.

현재 가능한 동작:

- 특정 블록 범위 조회
- compact block 조회
- full transaction 조회
- UFVK 기반 incoming note 탐지
- shielded memo 디코딩
- 이미 처리한 transaction 중복 방지

---

### 2.2 결제 memo 파싱

구매자는 결제 memo에 A1이 해석할 수 있는 정보를 넣는다.

A1 memo에는 다음 정보가 포함된다.

```text
drop_id + buyer e_pub
```

현재 지원하는 text memo 형식은 다음과 같다.

```text
A1B64:<base64url_no_pad(drop_id(8 bytes) || e_pub(32 bytes))>
```

예시:

```text
A1B64:AAAAAAAAAAEAAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHw
```

A1은 이 memo를 통해 다음을 식별한다.

- 어떤 drop에 대한 결제인지
- 구매자의 임시 공개키 `e_pub`이 무엇인지
- 해당 구매자에게 어떤 dispatch blob을 만들어야 하는지

---

### 2.3 결제 검증

A1은 감지한 incoming note에 대해 다음을 검증한다.

- memo가 A1 형식인지
- `drop_id`가 등록된 drop인지
- 입금액이 `price_zat` 이상인지
- 이미 처리한 transaction이 아닌지

검증에 성공한 결제만 dispatch 생성 대상으로 처리한다.

---

### 2.4 Dispatch blob 생성

결제가 확인되면 A1은 drop의 콘텐츠 키인 `K_drop`을 구매자의 `e_pub`으로 암호화한다.

```text
dispatch_blob = crypto_box_seal(K_drop, buyer_e_pub)
```

결과:

- dispatch blob 크기: 80 bytes
- 구매자만 자신의 private key로 복호화 가능
- 서버 운영자는 원본 `K_drop`을 직접 볼 수 없어야 함

---

### 2.5 Bucket key 생성

A1은 dispatch blob을 저장하기 위한 key를 생성한다.

```text
bucket_key = blake2b256(ek_pub || txid)
```

이 key는 bucket에서 dispatch blob을 조회하는 식별자로 사용된다.

---

### 2.6 Scan state 저장

A1은 scanner 상태를 저장한다.

저장 대상:

- 마지막으로 스캔한 블록 높이
- 이미 처리한 txid 목록

현재 구현에는 암호화된 파일 저장 방식이 포함되어 있다.

목표:

- 중복 dispatch 방지
- scanner 재시작 후 이어서 스캔 가능
- TEE 환경에서는 enclave sealing key로 보호

---

## 3. A1이 다른 파트에 제공해야 하는 인터페이스

### 3.1 Lane B / 구매자 앱에 제공

A1은 구매자 앱이 결제를 만들고 결과를 조회할 수 있도록 다음 정보를 제공해야 한다.

| 항목 | 설명 |
| --- | --- |
| 결제 주소 | creator/drop에 연결된 shielded address |
| 결제 금액 | `price_zat` 또는 ZEC 단위 가격 |
| memo 생성 규칙 | `A1B64:` 기반 memo format |
| dispatch 조회 방법 | bucket key 또는 dispatch 조회 endpoint |
| dispatch blob | 구매자가 복호화할 암호화된 `K_drop` |

구매자 앱 입장에서는 최종적으로 다음 흐름이 가능해야 한다.

```text
catalog 조회
→ 결제 주소/금액/memo 생성
→ wallet으로 shielded payment 전송
→ 결제 확인 대기
→ dispatch blob 조회
→ K_drop 복호화
→ 콘텐츠 복호화
```

---

### 3.2 Lane A2 / Enclave에 제공 또는 연결

A1은 최종적으로 민감한 정보를 enclave 내부에서만 처리해야 한다.

A2와 연결되어야 하는 항목:

| 항목 | 설명 |
| --- | --- |
| UFVK/IVK | 체인 스캔용 view key. 외부 노출 금지 |
| K_drop | 콘텐츠 복호화 키. 외부 노출 금지 |
| scan state | 중복 처리 방지 상태. 암호화 저장 필요 |
| dispatch 생성 | enclave 내부에서 수행되어야 함 |
| creator 등록 | sealed storage에 저장되어야 함 |

최종 목표는 운영자도 다음 값을 직접 볼 수 없게 하는 것이다.

- creator UFVK/IVK
- `K_drop`
- raw scan state
- sealed DB plaintext

---

### 3.3 Lane C / Creator, Content 파트에 제공

Creator/content 파트는 A1에 drop 정보를 등록할 수 있어야 한다.

필요한 등록 정보:

| 항목 | 설명 |
| --- | --- |
| `creator_id` | creator 식별자 |
| `drop_id` | 판매 대상 drop 식별자 |
| `price_zat` | 결제 기준 금액 |
| `K_drop` | 콘텐츠 복호화 키 |
| `deposit_addr` | 구매자가 송금할 shielded address |
| `h_content` | 콘텐츠 blob 또는 manifest 식별자 |

A1은 이 정보를 기반으로 결제를 검증하고 dispatch blob을 만든다.

---

### 3.4 Lane D / Bucket, Storage 파트에 제공

A1은 dispatch blob을 bucket에 저장한다.

필요한 bucket 동작:

```text
put(bucket_key, dispatch_blob)
get(bucket_key) -> dispatch_blob
```

최종적으로는 다음 두 종류의 저장물이 분리되어야 한다.

| 종류 | 설명 |
| --- | --- |
| content blob | 암호화된 실제 콘텐츠 |
| dispatch blob | 구매자별로 암호화된 `K_drop` |

---

## 4. 최종적으로 A1이 지원할 기능

A1의 최종 지원 목표는 다음과 같다.

### 4.1 Creator 등록

```text
creator/drop 등록 API
→ UFVK/IVK, deposit address, price, K_drop, content hash 저장
→ enclave 내부 sealed DB에 보관
```

---

### 4.2 Public catalog 제공

구매자 앱이 결제 정보를 만들 수 있도록 공개 catalog를 제공한다.

예상 제공 정보:

```text
drop_id
creator_id
title
price_zat
price_zec
deposit_addr
h_content
memo format
```

단, `UFVK`, `K_drop` 같은 민감 정보는 catalog에 포함하지 않는다.

---

### 4.3 Shielded 결제 감지

```text
lightwalletd polling
→ 최신 블록 또는 지정 range 조회
→ UFVK/IVK로 incoming note 확인
→ memo 파싱
→ 결제 검증
```

---

### 4.4 Dispatch 생성 및 저장

```text
결제 확인
→ buyer e_pub 추출
→ K_drop sealed-box 암호화
→ dispatch_blob 생성
→ bucket 저장
```

---

### 4.5 Buyer dispatch 조회

구매자는 결제 후 자신의 dispatch blob을 조회할 수 있어야 한다.

지원해야 할 방식:

```text
GET /dispatch/{bucket_key}
```

또는 Lane B 요구에 따라 다음 방식도 필요할 수 있다.

```text
GET /dispatch/recent?buyer_hint=...
GET /dispatch/by-tx/{txid}
GET /dispatch/list
```

현재는 `bucket_key` 기반 조회 boundary가 구현되어 있고, 실제 HTTP endpoint와 buyer-friendly 조회 방식은 추가 작업 대상이다.

---

### 4.6 Enclave 기반 보안 처리

최종 운영 구조에서는 다음 처리가 enclave 내부에서만 이루어져야 한다.

```text
UFVK/IVK 보관
K_drop 보관
creator/drop 등록값 저장
scan state 복호화
memo 검증
key dispatch 생성
```

외부에는 암호화된 상태와 결과 blob만 노출한다.

---

## 5. 현재 완료된 부분과 남은 부분

### 완료 또는 동작 가능

- lightwalletd 연결
- 실제 체인 블록 조회
- UFVK 기반 shielded note 탐지
- memo 디코딩
- 결제 금액 검증
- dispatch blob 생성
- bucket boundary 저장
- encrypted scan state 저장
- tx 중복 처리 방지
- API service vector 초안

### 남은 작업

- 실제 HTTP API 서버 연결
- public catalog endpoint 추가
- creator 등록 endpoint 완성
- buyer dispatch 조회 endpoint 완성
- dispatch recent/list API 추가
- 실제 bucket backend 연결
- enclave sealing key 적용
- sealed creator/drop DB 구현
- confirmation/reorg 정책 추가
- 운영 배포용 polling loop 구성

---

## 6. 한 줄 요약

A1은 최종적으로 **Zcash shielded 결제를 감지하고, memo를 파싱해 결제를 검증한 뒤, 구매자만 복호화할 수 있는 콘텐츠 키 dispatch blob을 생성·저장·조회 가능하게 만드는 결제 레이어**를 제공한다.
