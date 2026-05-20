# Orchard 기반 익명 구독 시스템 — 프라이버시 모델 및 한계

* **상태**: 아이디어/프로토콜 레벨 설계.
* **목표**:

  1. 서버가 사용자의 실제 wallet identity를 알지 못하게 한다.
  2. 결제자와 콘텐츠 접근자를 직접 연결하기 어렵게 만든다.
  3. 서버는 “누구냐” 대신 “유효한 구독자냐”만 검증한다.
  4. TEE 없이 Orchard + zk membership proof 기반으로 구성한다.

---

# 1. 핵심 아이디어

기존 구독 시스템:

```text id="sykw1p"
결제
→ 서버 DB 등록
→ 사용자 인증
→ 콘텐츠 제공
```

문제:

```text id="l5v4eq"
서버가:
- 누가 결제했는지
- 누가 콘텐츠를 보는지
- 어떤 활동을 했는지
모두 알 수 있음
```

---

# 제안 구조

```text id="jq3x5h"
Orchard shielded payment
→ blind credential 발급
→ zk membership proof
→ 콘텐츠 접근
```

즉:

```text id="jlwm01"
identity-based access
```

가 아니라:

```text id="jlwm02"
membership/capability-based access
```

구조.

---

# 2. High-level Architecture

```text id="jlwm03"
  ┌────────────┐
  │  CREATOR   │
  │            │
  │ 월별 콘텐츠 │
  │ 암호화      │
  └─────┬──────┘
        │ encrypted content
        ▼
 ┌─────────────────┐
 │  PUBLIC BUCKET  │
 │ encrypted blobs │
 └─────────────────┘

  ┌────────────┐
  │ SUBSCRIBER │
  │   wallet   │
  │            │
  │ Orchard    │
  │ payment    │
  │ blind cred │
  │ zk proof   │
  └─────┬──────┘
        │
        ▼
 ┌────────────────────┐
 │ SUBSCRIPTION SERVER│
 │                    │
 │ membership root    │
 │ proof verification │
 └────────────────────┘
```

---

# 3. 핵심 구성 요소

| 컴포넌트                  | 역할                            |
| --------------------- | ----------------------------- |
| Orchard payment       | shielded subscription payment |
| Blind credential      | 결제자와 credential linkage 감소    |
| Membership root       | 유효 구독 commitment 집합           |
| zk proof              | “나는 valid member다” 증명         |
| Public bucket         | encrypted content 저장          |
| Threshold key release | 월별 콘텐츠 키 분산 제공                |

---

# 4. Credential Model

사용자는 로컬에서 credential 생성:

```text id="jlwm04"
credential = {
  creator,
  tier,
  epoch,
  secret
}
```

commitment 생성:

```text id="jlwm05"
C = Commit(credential)
```

중요:

```text id="jlwm06"
credential 생성 자체는
권한이 아님
```

실제 권한은:

```text id="jlwm07"
C ∈ valid_members_root
```

일 때만 발생.

즉:

```text id="jlwm08"
issuer-controlled membership inclusion
```

이 핵심.

---

# 5. Blind Credential Issuance

문제:

```text id="jlwm09"
서버가 commitment를 직접 보면:
결제자 ↔ credential
매핑 가능
```

해결:

```text id="jlwm10"
blind credential issuance
```

사용자는:

```text id="jlwm11"
Blind(C)
```

를 제출.

서버는:

```text id="jlwm12"
Sign(Blind(C))
```

만 수행.

사용자는 나중에 unblind.

결과:

```text id="jlwm13"
서버는 실제 credential commitment를 모름
```

---

# 6. Membership Root

서버는:

```text id="jlwm14"
valid_members_root
```

만 유지.

예:

```text id="jlwm15"
현재 유효한 premium 구독자 commitment 집합
```

실제 identity 저장 안 함.

---

# 7. zk Membership Proof

사용자 콘텐츠 요청 시:

```text id="jlwm16"
prove(
  C ∈ valid_members_root
)
```

생성.

서버는:

```text id="jlwm17"
verify(proof, root)
```

만 수행.

즉 서버는:

```text id="jlwm18"
“누구인가?”
```

를 모르고:

```text id="jlwm19"
“유효한 member인가?”
```

만 확인.

---

# 8. 월별 콘텐츠 키

Creator는 월별 키 생성:

```text id="jlwm20"
K_Alice_2026_05
```

콘텐츠 암호화:

```text id="jlwm21"
video.mp4
→ AES-GCM(K_Alice_2026_05)
```

구독자는 proof 제출 후:

* threshold key share
* encrypted key bundle

등으로 월 키 획득.

---

# 9. 서버가 아는 것

```text id="jlwm22"
- 어떤 콘텐츠 요청이 있었는지
- proof가 valid한지
- 어떤 epoch가 활성인지
```

---

# 10. 서버가 모르게 하고 싶은 것

```text id="jlwm23"
- 실제 사용자 identity
- wallet address
- payment tx
- 결제자 = 콘텐츠 사용자
- 어떤 commitment가 누구 것인지
```

---

# 11. 현재 구조의 한계

완전 anonymous는 아님.

위험 요소:

```text id="jlwm24"
- timing correlation
- IP correlation
- issuance timing
- witness retrieval leakage
- network metadata
```

즉 서버가:

```text id="jlwm25"
“높은 확률로 동일 사용자”
```

정도 추론 가능성 존재.

---

# 12. 완화 방법

## Batch Inclusion

```text id="jlwm26"
100명씩 root 업데이트
```

→ timing correlation 감소.

---

## Relay / Tor / Nym

```text id="jlwm27"
직접 연결 제거
```

→ network linkage 감소.

---

## Public Bucket + Trial Decrypt

```text id="jlwm28"
누가 어떤 key blob을 받았는지
```

모르게.

---

## PIR Witness Retrieval

```text id="jlwm29"
“내 witness 주세요”
```

요청 leakage 감소.

---

## Nullifier Design

```text id="jlwm30"
반복 사용 fingerprint 최소화
```

---

# 13. 보장되는 것

| 속성                     | 상태    |
| ---------------------- | ----- |
| payment privacy        | 강함    |
| wallet address privacy | 강함    |
| 콘텐츠 암호화                | 강함    |
| zk proof anonymity     | 강함    |
| 결제와 접근 분리              | 부분 가능 |
| 서버 DB 기반 tracking 제거   | 가능    |

---

# 14. 보장되지 않는 것

| 속성                          | 이유                    |
| --------------------------- | --------------------- |
| 완전 network anonymity        | Tor/mixnet 필요         |
| timing unlinkability        | batch 없으면 어려움         |
| DRM                         | 재배포 가능                |
| 완전 issuance unlinkability   | blind issuance 품질에 의존 |
| traffic analysis resistance | 별도 시스템 필요             |

---

# 15. 핵심 철학

기존 Web2:

```text id="jlwm31"
서버가 사용자 상태를 소유
```

이 구조:

```text id="jlwm32"
사용자 지갑이 상태를 소유
서버는 proof만 검증
```

즉:

```text id="jlwm33"
identity-centric system
```

에서:

```text id="jlwm34"
proof/capability-centric system
```

으로 이동하는 구조.
