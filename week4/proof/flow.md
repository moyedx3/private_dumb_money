# Orchard 기반 익명 구독 시스템 — 간단한 전체 플로우

## 한 줄 요약

Creator는 콘텐츠를 암호화해서 공개 저장소에 올리고,
구독자는 Orchard로 익명 결제 후 “익명 구독권(credential)”을 발급받는다.
이후 서버는 사용자의 identity 대신 “유효한 구독자인지”만 zk proof로 검증한다.

---

# 등장인물

| 역할            | 설명                   |
| ------------- | -------------------- |
| Creator       | 콘텐츠 제작자              |
| Subscriber    | 구독자                  |
| Server        | 구독권 발급 및 proof 검증    |
| Public Bucket | 암호화된 콘텐츠 저장소         |
| Orchard       | shielded payment 레이어 |

---

# 전체 흐름

```text id="flow01"
[Creator]
콘텐츠 암호화
→ Public Bucket 업로드

[Subscriber]
Orchard 결제
→ 익명 credential 발급
→ zk proof 생성
→ 콘텐츠 키 획득
→ 로컬 복호화

[Server]
결제 확인
→ credential 승인
→ membership root 관리
→ proof 검증
```

---

# STEP 1 — Creator

Creator는 먼저 월별 콘텐츠 키 생성:

```text id="flow02"
K_Alice_2026_05
```

그리고 콘텐츠 암호화:

```text id="flow03"
video.mp4
→ AES-GCM(K_Alice_2026_05)
→ video.enc
```

암호화된 파일만 Public Bucket에 업로드.

즉 누구나 다운로드는 가능하지만:

```text id="flow04"
복호화는 불가능
```

상태.

---

# STEP 2 — Subscriber 결제

Subscriber wallet은:

```text id="flow05"
Orchard shielded payment
```

로 구독 결제 수행.

즉:

```text id="flow06"
결제자 privacy 보호
```

---

# STEP 3 — Subscriber가 Credential 생성

Wallet 로컬에서:

```text id="flow07"
credential = {
  creator,
  tier,
  epoch,
  secret
}
```

생성.

그리고:

```text id="flow08"
C = Commit(credential)
```

생성.

---

# STEP 4 — Blind Credential Issuance

Wallet:

```text id="flow09"
Blind(C)
```

생성 후:

```text id="flow10"
POST /issue
```

endpoint로 전송.

서버는:

```text id="flow11"
결제 확인
```

후:

```text id="flow12"
Sign(Blind(C))
```

수행.

Wallet은 unblind하여:

```text id="flow13"
issuer-signed credential
```

획득.

---

# 왜 Blind를 쓰는가?

Blind 처리 덕분에 서버는:

```text id="flow14"
실제 credential commitment를 직접 보지 못함
```

즉:

```text id="flow15"
결제자 ↔ credential
```

연결이 어려워짐.

---

# STEP 5 — Membership Root 생성

서버는 여러 구독자의 commitment를 batch로 모음:

```text id="flow16"
C1
C2
C3
...
```

그리고:

```text id="flow17"
valid_members_root
```

생성.

의미:

```text id="flow18"
“현재 유효한 구독자 집합”
```

---

# 왜 Batch Update를 하는가?

즉시 업데이트 시:

```text id="flow19"
결제 직후 새 leaf 추가
```

↓

```text id="flow20"
“이 commitment는 Bob 것이겠네”
```

추론 가능.

그래서:

```text id="flow21"
여러 명을 모아서
한 번에 root 업데이트
```

수행.

즉:

```text id="flow22"
timing correlation 감소
```

목적.

---

# STEP 6 — Subscriber가 zk Proof 생성

Wallet은:

* issuer signature
* membership witness
* secret

을 이용해:

```text id="flow23"
“나는 현재 valid subscriber다”
```

를 zk proof로 생성.

---

# STEP 7 — Server가 Proof 검증

Server는:

```text id="flow24"
verify(proof, valid_members_root)
```

만 수행.

즉 서버는:

```text id="flow25"
“누구냐?”
```

를 모르고:

```text id="flow26"
“유효한 구독자냐?”
```

만 확인.

---

# STEP 8 — Subscriber가 콘텐츠 복호화

Proof valid면 Subscriber는:

```text id="flow27"
K_Alice_2026_05
```

획득 가능.

그리고 로컬에서:

```text id="flow28"
AES-GCM.decrypt(
  K_Alice_2026_05,
  video.enc
)
```

수행.

↓

```text id="flow29"
원본 콘텐츠 복호화
```

---

# 서버가 아는 것

```text id="flow30"
- 누군가 valid proof 제출
- 어떤 콘텐츠 요청
- proof valid 여부
```

---

# 서버가 모르는 것

```text id="flow31"
- 실제 사용자 identity
- wallet address
- payment tx
- 어떤 credential이 누구 것인지
```

---

# 핵심 철학

기존 Web2:

```text id="flow32"
“너 누구냐?”
```

이 구조:

```text id="flow33"
“유효한 구독자냐?”
```

만 검증.

즉:

```text id="flow34"
identity-based access
```

에서:

```text id="flow35"
proof/capability-based access
```

로 이동하는 구조.
