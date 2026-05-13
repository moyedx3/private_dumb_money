# Pay Anyone Legend — 구체적 예시 Walkthrough

> **읽는 사람:** 팀원 / 처음 이 시스템 보는 사람
> **목적:** 한 사용자의 결제 시나리오를 처음~끝까지 따라가며 *"PAL이 실제로 어떻게 동작하는지"* 감 잡기
> **연관:** [`TEAM-WALKTHROUGH.md`](./TEAM-WALKTHROUGH.md)는 서브시스템별 분석, 이 문서는 **하나의 거래를 시간순으로**.

---

## 시나리오

**김민준이 PAL 웹앱에서 ChatGPT Pro를 결제한다.**

- 김민준 Zcash 주소: `t1KMjAsd...` (transparent)
- 결제 금액: **20 USDC 상당의 ZEC** (약 0.12 ZEC)
- 가맹점 USDC 수신: `0x03fBbA1b...` (Base)
- 콘텐츠 URL: `chatgpt.com/premium-link?session=xyz`

---

## 🎬 거래 흐름 — 시간순

### T0 — 자연어 입력
김민준이 입력창에 `"Pay ChatGPT Pro"` 입력.

### T0 + 1초 — Intent 파싱
- PAL 서버가 OpenAI embedding으로 쿼리 벡터화
- Supabase pgvector가 등록된 ChatGPT Pro 서비스와 매칭 (cosine 0.87)
- **LLM 호출 없이** `{amount: 20, currency: USDC, chain: base, bridgeFrom: zcash, receivingAddress: 0x03fBbA1b...}` 즉시 조립
- `bridgeFrom: "zcash"`는 코드에 하드코딩

### T0 + 2초 — swapWallet 파생
- PAL이 NEAR Chain Sig으로 EVM 주소 결정론적 파생
- `(accountId="anyone-pay.near", path="base-1")` → `0xABC1234...`
- ⚠️ **path가 하드코딩**이라 모든 PAL 사용자가 이 주소를 공유

### T0 + 3초 — 1Click에 quote 요청
PAL이 1Click API(`1click.chaindefuser.com/v0/quote`)에 보내는 한 요청에 다음 정보가 같이 담김:
- `originAsset = ZEC`, `destinationAsset = USDC on Base`
- `recipient = 0xABC1234...` (swapWallet)
- `refundTo = t1KMjAsd...` (김민준 ZEC 주소)
- `amount = 20 USDC`, `deadline = T0 + 3분`

> 🚨 **privacy 사건 #1 — API 레벨**
> 1Click(Defuse Labs Limited, Gibraltar)이 한 요청에서 sender ZEC + recipient EVM을 다 봄. **AML 스크리닝 자동 적용.**

### T0 + 3.5초 — 1Click 응답
```
depositAddress: "t1QcXyZ..."  ← 1Click solver 소유 transparent 주소
swapId:         "swap_a7b9x123"
quote.amountIn: "0.123456 ZEC"
deadline:       T0 + 3분
```

### T0 + 4초 — Supabase에 deposit 저장 + QR 표시
- `deposit_tracking` 테이블에 row insert (PK = `t1QcXyZ...`)
- 김민준 화면에 QR (값 = `t1QcXyZ...` raw 주소)
- ⚠️ ZIP-321 URI 아니라서 amount 자동 입력 안 됨 → 김민준이 수동 입력 필요

### T0 + 30초 — 김민준 ZEC 송금
김민준이 Zcash 지갑에서 `0.123456 ZEC`를 `t1QcXyZ...`로 송금.

> 🚨 **privacy 사건 #2 — L1 레벨**
> transparent 주소라서 Zcash 익스플로러에서 sender, recipient, amount 다 공개. shielded 보호 0.

### T0 + 2~3분 — Vercel cron 폴링
Vercel cron이 매 1분마다 1Click API에 `getExecutionStatus("t1QcXyZ...")` 호출.

- T0 + 2분: `PROCESSING` (아직 swap 중)
- T0 + 3분: **`SUCCESS`** ← 이게 트리거

**1Click 내부에서 무슨 일이 일어났냐 (PAL은 모름):**
1. solver가 ZEC 받음
2. NEAR Intents(`intents.near`)에서 atomic settlement
3. Token bridge로 **USDC 20개를 swapWallet `0xABC1234...`에 전달**

> 이 시점 Base 익스플로러로 swapWallet 잔고 조회하면 **20 USDC** 있음.

### T0 + 3분 1초 — x402 분기 진입
cron이 SUCCESS 감지 → `signX402TransactionWithChainSignature()` 호출.

### T0 + 3분 3초~38초 — NEAR MPC 사인 2번
**왜 2번?** EIP-3009 표준 따르려고 분리:

- **사인 #1** (약 15초) — EIP-712 *authorization* 해시
  → "swapWallet이 0x03fBbA1b...한테 USDC 20개 보내는 걸 허가"
  → USDC 컨트랙트가 on-chain ecrecover로 검증할 사인
- **사인 #2** (약 16초) — *EVM 트랜잭션* 해시
  → 위 사인을 calldata에 박은 raw tx
  → Base 노드가 broadcast 시 검증할 사인

PAL이 사이에 ecrecover 사전 검증 → mismatch면 abort (silent failure 방지).

### T0 + 3분 39초~42초 — Base 메인넷 broadcast
viem이 Base RPC에 raw tx 전송 → 약 3초 뒤 블록 포함.
- USDC 20개: `0xABC1234...` → `0x03fBbA1b...`
- tx hash: `0xdef456...`

### T0 + 3분 43초 — Supabase 업데이트
```
signed_payload = "0xdef456..."   ← 이름과 달리 tx hash가 들어감
x402_executed  = true
confirmed      = true
```

### T0 + 4분 — UI가 알아챔 → 콘텐츠 unlock
김민준 브라우저는 1초마다 폴링 중이었음. `signed_payload` 들어온 거 감지하면:
- content page로 redirect
- 가맹점 URL에 GET 요청 + 헤더 `X-PAYMENT: 0xdef456...`
- 가맹점이 (Base 익스플로러로 검증 후) ChatGPT 콘텐츠 반환

✅ **결제 완료. 총 ~4분.**

---

## 💸 자금 흐름 (체인별)

```
[Zcash]   김민준 ZEC ──0.12 ZEC──▶ 1Click solver 주소
                                        │ (1Click 내부 swap, off-chain)
                                        ▼
[Base]    1Click ──20 USDC──▶ swapWallet 0xABC1234... ──20 USDC──▶ 가맹점 0x03fBbA1b...

[NEAR]    자산 0. v1.signer가 사인만 2번 (15초 + 16초)
```

> **핵심:** NEAR는 자산 통로가 아니라 *사인 출장소*. 자산은 Zcash와 Base에만 흐름.

---

## ⏱️ 시간 요약

| 시각 | 일어난 일 |
|---|---|
| T0 | 김민준 자연어 입력 |
| T0 + 4초 | QR 표시 |
| T0 + 30초 | 김민준 ZEC 송금 |
| T0 + 3분 | 1Click SUCCESS 감지 |
| T0 + 3분~3분 38초 | MPC 사인 2회 (총 ~30초) |
| T0 + 3분 42초 | Base 블록 포함 |
| T0 + 4분 | 콘텐츠 unlock |

**총 ~4분.** 그중 절반이 ZEC 송금 + Zcash confirmation 대기, 나머지가 cron polling + MPC 사인 latency.

---

## ⚠️ 이 거래에서 일어난 함정 5가지

1. **Privacy 0** — 1Click이 sender ZEC + recipient EVM 다 봄 + Zcash 익스플로러에서 L1 공개
2. **AML 스크리닝** — sanctioned 주소였으면 1Click이 거절
3. **QR이 ZIP-321 URI 아님** — 김민준이 amount 수동 입력. 잘못 치면 `INCOMPLETE_DEPOSIT` (환불 endpoint 없음)
4. **swapWallet 공유** — 모든 PAL 사용자가 이 주소를 공유. A USDC가 B 결제에 쓰일 가능성
5. **X-PAYMENT가 tx hash** — 표준 x402(EIP-712 서명 기대)와 비호환. PAL 전용 가맹점만 가능

---

## 🌈 우리 팀이 카테고리 E를 진짜로 구현하면?

같은 시나리오(*"Pay ChatGPT Pro"*) 우리 버전:

```
T0       김민준: "Pay ChatGPT Pro"
T0+2초    서버가 HTTP 402 + paymentRequirements 발행
           (scheme: shielded-zcash, payTo: zs1..., challenge nonce)
T0+3초    김민준 지갑이 shielded tx 생성 (memo에 challenge nonce)
T0+30초   Zcash 메인넷 confirm
           L1에서 sender/recipient/amount/memo 모두 🔒 비공개
T0+30초   가맹점이 viewing key로 memo 복호화 → 매칭 확인
T0+31초   콘텐츠 반환 ✅
```

**우리 버전의 우위:**
- 1Click 없음 → privacy 보존, AML linkage 없음
- swapWallet 없음 → per-user 격리 자동
- MPC 없음 → latency 30초 절감
- NEAR 의존 없음

**총 시간 ~30~60초** (PAL의 4분 대비 1/4 이하).

---

## 한 줄 정리

> *"PAL 거래 한 건이 4분 걸리고 그 사이 김민준의 ZEC 송금 정보는 Gibraltar 회사와 Zcash 익스플로러에 다 노출된다. 우리가 진짜 카테고리 E를 구현하면 같은 거래가 30~60초에 끝나고 어떤 외부 관찰자도 보지 못한다."*
