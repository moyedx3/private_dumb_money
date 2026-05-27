# Clean Wallet MVP — 현재 상태 (KR)

> Zcash mainnet 대상, TEE attested scanner 기반 off-ramp screening MVP.
> 본 문서는 코드와 운영의 **단일 진실 (snapshot)**. 작성 기준: 2026-05-27.

---

## 한 줄 요약

**"이 사용자의 UFVK 로 본, 정해진 블록 구간 안에서, 정해진 sanctioned 주소들로 나가는 송금이 있었는가?"** 라는 *좁은* 질문에 PASS/FAIL 답을 내는 시스템. 답이 정직하려면 (a) 코드가 TEE 안에서 그대로 돌았고, (b) 블록체인 데이터가 사전 합의된 lightwalletd 에서 왔고, (c) 답이 정책·요청과 묶여 있어야 함 — 그 3개를 verifier 가 한 번에 확인.

---

## 비유 — 약물 검사 랩

```
사용자 = 검사받는 사람
UFVK = 혈액 sample (한 시야만 보여줌, spending key 아님)
Scanner in TEE = 무균 분석실 (밖에서 안 못 봄, 결과만 봉인 후 내보냄)
Lightwalletd = 혈액은행 (검증된 곳에서만 받기)
Artifact = 봉인된 분석 보고서
Quote = 분석실 자체의 진위 증명서
Verifier = 거래소 컴플라이언스 데스크
```

빠진 한 가지가 있으면 보고서 의미 없음:
- 무균실이 진짜인지 (→ TDX quote)
- 보고서가 그 분석실에서 나왔는지 (→ quote 의 `report_data` 에 artifact hash 봉인)
- 보고서가 *이 사용자, 이 정책, 이 거래소* 의 것인지 (→ artifact 안 policyHash/depositIntentHash)

---

## 아키텍처 다이어그램

```
┌────────────────────┐         ┌─────────────────────────────────────────┐
│ User (Prover)      │         │ Phala Cloud CVM (Intel TDX)             │
│  Next.js /prover   │         │ ┌─────────────────────────────────────┐ │
│                    │ POST    │ │ Untrusted Host                      │ │
│  UFVK + policy +   │────────►│ │  - axum HTTP (/health /attestation  │ │
│  DepositIntent     │ /screen │ │    /screen)                         │ │
│                    │         │ │  - dstack socket bridge             │ │
└────────────────────┘         │ └─────────────────────────────────────┘ │
         ▲                     │ ┌─────────────────────────────────────┐ │
         │ bundle              │ │ Trusted Scanner Code                │ │
         │ (artifact + quote)  │ │  - policy 검증 (network, range, ...) │ │
         │                     │ │  - zcash_client_backend 로          │ │
         │                     │ │    OVK 기반 outgoing 추출           │ │
         │                     │ │  - sanctioned hash 집합 교집합      │ │
         │                     │ │  - artifact JSON 생성               │ │
         │                     │ │  - dstack 으로 TDX quote 요청       │ │
         │                     │ │    (report_data = sha256(artifact)) │ │
         │                     │ └─────────────────────────────────────┘ │
         │                     │              │                          │
         │                     │              ▼                          │
         │                     │      ┌──────────────────┐               │
         │                     │      │ Lightwalletd     │               │
         │                     │      │ (zec.rocks:443)  │               │
         │                     │      │ tonic gRPC + TLS │               │
         │                     │      └──────────────────┘               │
         │                     └─────────────────────────────────────────┘
         │
         ▼
┌────────────────────┐
│ Exchange (Verifier)│  3가지 binding check
│  Next.js /verifier │   ① TDX quote 진위 (Phala API or local DCAP)
│                    │   ② artifactHash == quote.report_data
│  bundle + policy + │   ③ artifact.policyHash == sha256(policy)
│  DepositIntent     │     artifact.depositIntentHash == sha256(intent)
│                    │  → 모두 OK 면 artifact.result 신뢰
└────────────────────┘
```

---

## 무엇을 하는가 (구체적 흐름)

### 1. Prover 측 (사용자)

1. `/attestation` 호출 → Scanner 의 현재 TDX quote 받음 → policy 의 `expectedScannerCodeMeasurement` 와 MRTD 일치 확인 (pre-flight).
2. `/screen` 으로 `{ufvk, policy, depositIntent}` 제출.
3. Scanner 안에서:
   - **fail-closed guards**: network 일치, range 크기, range vs chain tip, intent expiry, UFVK prefix 검증.
   - `auditStartHeight..auditEndHeight` 구간의 compact block 을 lightwalletd 에서 stream.
   - UFVK 의 OVK 로 outgoing note 의 recipient 복호화.
   - 각 recipient 주소를 `sha256(addr_string)` 으로 hash → policy 의 `sanctionedAddressHashes` 와 set intersection.
   - artifact JSON 생성 (`result: PASS|FAIL`, `recipientCount`, `sanctionedHitCount`, `scanRange`, `policyHash`, `depositIntentHash`, ...).
   - `report_data = sha256(canonicalJson(artifact))` 로 TDX quote 요청.
4. Bundle (`{artifact, quote_hex}`) 받아서 verifier 에 전달.

### 2. Verifier 측 (거래소)

1. Quote 검증 — Phala cloud-api 또는 local DCAP (선택).
2. `report_data` 가 `sha256(canonicalJson(artifact))` 와 일치하는지 (artifact 가 *이 quote 안*에서 나왔는지).
3. Artifact 의 `policyHash` 와 `depositIntentHash` 가 verifier 가 들고 있는 policy/intent 의 hash 와 일치하는지.
4. Artifact 의 `scannerCodeMeasurement` 가 policy 의 `expectedScannerCodeMeasurement` 와 일치하는지.
5. 모두 통과면 `artifact.result` (PASS/FAIL) 를 신뢰.

---

## 현재 작동 / 테스트된 부분

### 빌드 & 단위 테스트

| 항목 | 상태 |
|---|---|
| `cargo check -p clean-wallet-scanner --all-targets` | ✅ 통과 (warning 없음) |
| Scanner unit tests (`cargo test -p clean-wallet-scanner --lib`) | ✅ 통과 — policy 검증, artifact 직렬화, server fail-closed 경로 |
| Web unit tests (`pnpm test`, vitest) | ✅ `canonical.test.ts`, `policy.test.ts`, `verify-quote.test.ts` |
| `gen-ufvk` 빌드 + 실행 (mainnet) | ✅ |
| `gen-taddr` 빌드 + 실행 (mainnet, t1...) | ✅ — 본 commit 에서 추가 |

### Live network (live_testnet 테스트는 이제 mainnet 대상)

| 항목 | 상태 |
|---|---|
| `zec.rocks:443` TLS handshake + chain tip fetch | ✅ 확인 (CN=zec.rocks, verify 0) |
| Lightwalletd primary→backup failover (코드 경로) | ✅ unit test 로 mock 검증 |

### 통합 테스트 (#[ignore]'d, 수동 실행)

| 테스트 | 상태 | 이유 |
|---|---|---|
| `regtest_scan` 4개 (PASS / FAIL / network mismatch / lightwalletd disconnect) | 🟡 **#[ignore]** | `regtest_setup.sh` 가 zebrad wallet RPC 미지원으로 wallet provisioning TODO. compose infra (zebrad+lightwalletd) 는 stand up OK. |
| `live_testnet_returns_a_tip` → 이제 mainnet 으로 의미 변경 | 🟡 코드는 testnet 호스트 하드코딩, mainnet 으로 리팩토 필요 |

### TDX / dstack

| 항목 | 상태 |
|---|---|
| Local TDX quote parser fallback | ✅ sim quote 에 대해 binding check 통과 |
| dstack `/Info` 응답 (mrtd nested tcb_info) 파싱 | ✅ |
| Phala 실배포 → 실 quote 검증 | 🔴 **미실행** (아래 TODO) |

---

## 남은 TODO

### 🔴 차단 (live demo 가능해지려면 필수)

| 항목 | 메모 |
|---|---|
| **Phala Cloud 배포** | `./scripts/deploy-cvm.sh` 실행 → CVM URL 확보 → MRTD 캡처. Docker daemon + GHCR login 필요. |
| **policy.demo.json:expectedScannerCodeMeasurement** | 위에서 캡처한 MRTD 로 `0xFILL_IN_AFTER_PHALA_DEPLOY` 치환. |
| **Wallet A / B 셋업 (Zashi mainnet)** | 둘 다 새 seed → UFVK 추출 → `demo-data/ufvk-clean.txt`, `ufvk-dirty.txt` 에 저장. |
| **Wallet A/B funding** | 거래소에서 ~0.01 ZEC 씩 송금. Wallet B 가 shielded 받게 UA 사용 권장 (audit window §2.2 회피). |
| **Wallet B → 데모 sanctioned 송금** | 목적지: `t1Ss8dERcHbR9tQx6rN3tjhzK1vvAz4QgZu` (sanctioned-set.json 에 박혀 있음). z→t 송금으로 OVK 기반 detection 활용. |
| **policy.demo.json:auditStartHeight/EndHeight** | 위 송금 confirm block ±10 으로 셋팅 후 commit. |
| **`live_testnet_returns_a_tip` 테스트** | 함수명/호스트 mainnet 으로 리팩토 (`testnet.zec.rocks` → `zec.rocks`). |

### 🟡 품질 (live demo 후 따라가도 됨, 한계 doc §2 항목들)

| 항목 | 한계 doc 매핑 | 메모 |
|---|---|---|
| Receiver canonicalization (UA decode → 각 receiver 별 hash 집합) | §2.1 / D13 | 현재 raw string sha256. Vizor vs scanner-extracted UA encoding mismatch 위험. |
| Audit window 이전 transparent UTXO load (`GetAddressUtxos`) | §2.2 / F2 | window 이전 받은 UTXO 의 spend 미감지. |
| PoW header chain 검증 (lightwalletd 가 header 비워 보내는 케이스) | §2.3 / F1/F3 | `GetBlock` 별도 fetch or Zebra 직접 운영. |
| Local DCAP verifier (`@phala/dcap-qvl`) 옵션 | §2.7 / F4 | 현재 Phala cloud-api 의존. |
| Canonical JSON (RFC 8785) — `serde_jcs` 이미 있음, 도입 범위 확장 | §2.8 | cross-language verifier 보장. |
| Fail-closed guards 6단 명시화 | §2.5 | 부분 검증만 됨. |

### 🟢 narrative / 문서

| 항목 | 메모 |
|---|---|
| `regtest_setup.sh` 의 wallet provisioning unblock | Zebra wallet RPC 지원 추가되거나, lightwalletd 측에서 wallet 만들기. |
| `apps/scanner/tests/regtest_scan.rs` 의 `#[ignore]` 해제 | 위 unblock 후. |
| 데모 영상 / 스크린샷 (proof.t16z.com 의 "Genuine TDX quote") | runbook Step G. |
| 1pager / 발표 자료 | week5 마무리. |

---

## 인접 작업 (참고)

| 위치 | 누가 | 무엇 |
|---|---|---|
| `week5/clean-wallet-mvp2/` | nogie-dev | Python PoC. fixture scanner + mock attestor + CLI 데모. 같은 narrative 의 *executable spec* 버전. |
| `week5/clean-wallet-limitations.md` | naba4 | 본 시스템의 본질적/구현/운영적 한계 정리 (Korean). 본 README 의 §2.1~2.9 TODO 매핑이 여기에 있음. |
| `week4/clean-wallet/` | (본인) | 원안 idea + technical explanation. |

---

## 참고 문서

- `docs/trust-model.md` — 신뢰 모델, attacker model.
- `docs/task-15-runbook.md` — Phala 배포부터 live demo 까지 step-by-step.
- `docs/demo-script.md` — 발표 시나리오.
- `../clean-wallet-limitations.md` — 한계 정리 (must-read).

---

## 부록 — 용어집

| 용어 | 뜻 |
|---|---|
| UFVK | Unified Full Viewing Key. 한 wallet 의 모든 pool (orchard/sapling/transparent) 에 대한 incoming + outgoing viewing 능력. Spending key 아님. |
| OVK | Outgoing Viewing Key. 본인이 보낸 shielded note 를 디코드할 수 있는 key. UFVK 에 포함. |
| IVK | Incoming Viewing Key. 받은 note 를 디코드할 수 있는 key. |
| Compact block | Lightwalletd 가 전달하는 압축된 block 표현. light wallet 이 시간/대역폭 아끼게 해줌. |
| Diversifier | 같은 wallet 에서 무한히 많은 raw 주소를 만들 수 있게 하는 인덱스. Privacy 의 핵심, 동시에 audit 의 약점 (§1.1). |
| MRTD | TDX 의 *code measurement*. 배포된 image 의 hash 같은 역할. |
| Quote | TDX hardware 가 서명한 attestation 증명서. MRTD, report_data, signature 포함. |
| `report_data` | Quote 안에 사용자가 박을 수 있는 64-byte payload. 우리는 `sha256(artifact)` 을 박음 → artifact 가 *이 quote 안에서* 나왔다는 binding. |
| Artifact | Scanner 가 만드는 결과 JSON. result, scanRange, policyHash, depositIntentHash, scannerCodeMeasurement, ... 포함. |
| dstack | Phala 의 TDX SDK. unix socket 으로 enclave 안에서 quote/key 요청 가능. |
| Canonical JSON | 같은 JSON 객체 → 같은 byte 시퀀스 보장. hash 비교 정합성에 필수. RFC 8785. |
