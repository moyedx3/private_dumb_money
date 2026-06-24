# 인터페이스 — 서로 주고받는 데이터 모양 (팀 합의 문서)

> **이게 4명이 코드를 붙이는 "접점" 전부.** 첫날 여기 숫자/이름만 확정하면, 그담부턴 각자 상대편을 가짜(mock)로 대체하고 따로 개발해도 마지막에 딱 맞물린다.
>
> **오너: (네 이름).** 아래는 스파이크 #1·#2·#3 돌리면서 이미 정해진 구체값 **초안**이다. 숫자/이름 보고 확정하거나 바꿔라. 바꾸면 이 문서만 고치면 됨 — 각 레인 스펙은 "정확한 값은 interfaces.md 참조"라고만 적혀 있다.
>
> 스파이크 3개 다 통과해서 **6개 전부 지금 확정 가능** (막힌 거 없음).

---

## I1. 메모 (구매자 → 서버) — A1 소유

구매자가 결제에 실어 보내는 쪽지. A1은 아래 **두 형식 모두** 디코드한다.

### I1-a. Raw 형식

체인 위 Zcash 메모 필드에 40바이트 날 바이트(raw)로 들어간다.

```
memo[0..8]   = drop_id   (u64, big-endian)      // 어떤 드롭을 사는지
memo[8..40]  = e_pub      (X25519 공개키, 32바이트) // 구매자 일회용 공개키
총 40바이트  (Zcash 메모는 512바이트라 여유 충분)
```

### I1-b. 텍스트 폴백 형식

raw binary memo 입력이 어려운 지갑을 위해 UTF-8 텍스트 memo도 허용한다.

```
"A1B64:" + base64url_no_pad(drop_id(8B BE) || e_pub(32B))
```

prefix는 고정값이다.

```
A1B64:
```

### ZIP-321 wrapping

ZIP-321 URI의 `memo=` 파라미터는 **온체인 memo bytes**를 base64url-no-pad로 감싼 값이다.

```
raw형:
  온체인 memo bytes = drop_id(8B BE) || e_pub(32B)
  ZIP-321 memo=     = base64url_no_pad(raw40)

텍스트 폴백형:
  온체인 memo bytes = utf8("A1B64:" + base64url_no_pad(raw40))
  ZIP-321 memo=     = base64url_no_pad(utf8("A1B64:..."))
```

### 기본형식

- A1은 raw형과 텍스트 폴백형을 모두 받아야 한다.
- B는 wallet이 raw 40B memo를 byte-identical하게 실을 수 있으면 raw형을 기본으로 쓴다.
- wallet이 raw binary memo를 거부하거나 변형하면 텍스트 폴백형 `A1B64:`를 기본으로 쓴다.
- 데모에서는 wallet 호환성을 위해 `A1B64:`를 우선 사용할 수 있다.

### 교차구현 test vector

B의 encoder와 A1의 decoder가 같은 바이트를 보는지 확인하기 위한 고정 벡터다.

```
drop_id = 1
e_pub   = 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f
raw40   = 0000000000000001000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f
text    = A1B64:AAAAAAAAAAEAAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHw
```

서버 A1은 raw형이면 첫 40바이트를, 텍스트 폴백형이면 `A1B64:` 뒤 base64url payload를 디코드해 `drop_id` / `e_pub`로 쪼갠다.

## I2. dispatch blob (서버 → 구매자) — A1 소유

서버가 콘텐츠 열쇠 `K_drop`(32바이트)을 구매자 `e_pub`로 포장한 꾸러미. **libsodium sealed box** 그대로.

```
blob = crypto_box_seal(K_drop, e_pub)
     = ek_pub(32) || ciphertext+MAC(48)   = 총 80바이트
```

- 서버(A1): `crypto_box_seal(K_drop, e_pub)` → 버킷에 올림.
- 구매자(B): `crypto_box_seal_open(blob, e_pub, e_priv)` → `K_drop` 복원.
- 곡선: Curve25519 (양쪽 동일 — Rust `dryoc` ↔ JS `libsodium-wrappers`).
- **버킷 키**(이 blob이 올라가는 파일 이름): `blake2b256(ek_pub || txid)` 의 hex. → 구매자/드롭 식별자 안 들어감(프라이버시).

## I3. 카탈로그 — C가 등록, A2가 보관/게시, B가 조회

**(a) 공개 카탈로그 엔트리** (구매자 B가 목록에서 봄, JSON):
```json
{ "drop_id": 1, "price_zec": "0.01", "h_content": "<콘텐츠 blob 버킷 키>", "title": "고양이 사진", "deposit_addr": "<shielded address>" }
```

**(b) 내부 드롭 설정** (서버 enclave가 보관, A2가 저장 / A1이 조회 — 절대 공개 안 함):
```
drop_id     : u64
price_zat   : u64            // zatoshi (1 ZEC = 100,000,000 zat)
k_drop      : [u8; 32]       // 콘텐츠 마스터 열쇠
creator_ufvk: String         // 크리에이터 보기전용키(UFVK 문자열, IVK 추출용)
deposit_addr: String         // 구매자가 입금할 shielded address (t1/t3 금지)
```
A1이 쓰는 조회 인터페이스: `Catalog::lookup(drop_id) -> Option<DropConfig{price_zat, k_drop, creator_ufvk, deposit_addr}>`

## I4. 콘텐츠 blob (크리에이터 → 구매자, 버킷 경유) — C 소유

크리에이터가 콘텐츠를 `K_drop`으로 암호화한 것. 서버는 이걸 **절대 못 봄**(그게 핵심).

```
blob = nonce(12) || AES-256-GCM_ciphertext || tag(16)
h_content = sha256(blob) 의 hex    // 버킷 키이자 카탈로그의 h_content
```
- 크리에이터(C): `AES-256-GCM(K_drop, plaintext)` → 버킷에 `h_content` 이름으로 올림.
- 구매자(B): `K_drop` 받은 뒤 같은 방식으로 복호화 → 렌더.

## I5. provisioning — 비밀 넣기 (크리에이터 → 서버) — A2 소유 (스파이크 #3에서 검증된 방식)

크리에이터가 `K_drop` + 보기전용키를 **측정된 enclave만 읽게** 봉인해서 넣는다.

1. C가 `/attest`로 서버 공개키 받고 quote 검증(I6).
2. C가 그 공개키로 sealed box 만들어 전송:
   ```
   payload = { drop_id, price_zat, k_drop(32B), creator_ufvk, deposit_addr, h_content }   // CBOR/JSON
   sealed  = crypto_box_seal(payload, enclave_provisioning_pubkey)
   POST /provision   body: sealed
   ```
3. enclave(A2)가 KMS 파생 개인키로 `crypto_box_seal_open` → 복호화 → 내부 카탈로그(I3-b)에 저장.
- 서버 주인은 `sealed`(암호문)만 봄. (스파이크 #3가 이 "encrypt-to-enclave"를 진짜 하드웨어에서 검증함 — `../spike3/RUNBOOK.md`)

## I6. attestation (서버 → 크리에이터) — A2 소유

서버가 "나 진짜 그 오픈소스 코드 그대로 돈다"를 증명. clean-wallet의 `attest.rs` 형식 재사용.

```
GET /attest  →  { quote_hex, ... }
quote 안:
  report_data[0..32] = sha256(enclave_provisioning_pubkey)   // ← I5에서 쓸 공개키를 quote에 묶음
  Mrtd / Rtmr        = 코드 측정값 (오픈소스 재현빌드 해시와 대조)
```
- 크리에이터(C): quote를 Intel 체인까지 검증(`@phala/dcap-qvl-web` 또는 t16z) + 측정값이 공개 레포 빌드와 일치하는지 확인 → 통과하면 그 공개키로 I5 암호화.

---

## 한눈에 (누가 무엇을 주고받나)

| # | 무엇 | 주는 쪽 → 받는 쪽 | 형식 한 줄 |
|---|---|---|---|
| I1 | 메모 | B → A1 | `drop_id(8) ‖ e_pub(32)` = 40B raw, URI엔 base64url |
| I2 | dispatch blob | A1 → B | sealed box 80B, 키=`blake2b(ek_pub‖txid)` |
| I3 | 카탈로그 | C → A2 → B | 공개 JSON + 내부 DropConfig |
| I4 | 콘텐츠 blob | C → B | `nonce(12) ‖ AESGCM ‖ tag(16)`, 키=sha256 |
| I5 | provisioning | C → A2 | sealed box of `{drop_id,price_zat,k_drop,ufvk,h_content}` |
| I6 | attestation | A2 → C | TDX quote, `report_data=sha256(provisioning pubkey)` |

> 바꾸고 싶은 값 있으면 여기서 고치고 팀에 공지. 특히 확정해야 할 선택지: drop_id 크기(u64면 충분), 카탈로그 저장소(메모리 vs 파일), 버킷 해시(sha256 vs blake2b) — 통일만 하면 됨.
