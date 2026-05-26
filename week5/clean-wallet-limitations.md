# Honest Limitations — Zcash Private Off-Ramp Screening

> 시스템의 알려진 한계를 정직하게 정리. 각 항목은 한 곳에서만 다룬다.
>
> **두 가지를 구분하는 게 핵심**:
> - **본질적 한계** (§1, §3) — Zcash crypto / TEE 모델의 근본 속성. 우리 코드로 못 푼다.
> - **구현 한계** (§2) — 우리 현재 코드가 안 해서. 후속 작업으로 풀 수 있다.

---

## TL;DR — 가장 중요한 4개

이 네 가지가 **시스템 narrative 의 정직한 골격**. 나머지는 디테일.

1. **[Diversifier 회피](#11--diversifier-회피--shielded-audit-의-가장-큰-갭) (§1.1, 본질)** — 한 wallet 의 IVK 하나로 무한 diversifier → 무한 raw 주소를 만들 수 있다. 단일 주소 매칭은 회피 가능. **IVK 매칭 외에는 못 푼다** — shielded audit 의 가장 큰 갭.

2. **[Shielded sender 식별 불가](#12-shielded-sender-식별-불가--1-hopmulti-hop-둘-다) (§1.2, 본질)** — sender 는 chain 에 노출 안 됨. 결과로 **1-hop 검사** ("sanctioned 로부터 받은 적") 도, **multi-hop 추적** ("ZEC 가 어디서 왔는지") 도 본질적 불가능. "clean source of funds" 증명 불가 → 우리는 *downstream outgoing 의 부재* 만 claim.

3. **[TEE narrative 의 두 전제조건](#33--신뢰가능한-감사-결과의-두-가지-전제조건) (§3.3, 운영 필수)** — TEE 가 의미 있으려면:
   - **(a) Viewing key 는 RA-TLS 채널로만** (운영자도 평문 못 봄) — D10
   - **(b) Chain data source 는 검사관 allowlist 의 endpoint 만** (위조 데이터 차단) — D9
   둘 다 충족돼야 "운영자도 못 보고, 검사관도 데이터 진위 신뢰 가능한 audit". 한쪽이라도 빠지면 TEE 쓰는 의미 약함.

4. **[OFAC SDN ZEC 은 현재 transparent 3개만](#41-ofac-sdn-zec-주소--현재-transparent-3개-stale) (§4.1, 현실)** — 2026-05 시점 OFAC SDN list 의 ZEC 주소는 **3개, 모두 `t1...` transparent P2PKH**. 2020-09 designation 이후 6년간 추가 등재 없음. shielded 매칭은 현실에서 발생 거의 없음. 따라서 위 §1.1·1.2 의 shielded 한계가 실 OFAC 운영에는 영향 작음 — 다만 표본 n=3 이라 *통계적 결론 단정은 약함*, narrative 는 honesty 로 명시.

---

## 1. 본질적 한계 (Zcash crypto)

### 1.1 ★ Diversifier 회피 — shielded audit 의 가장 큰 갭

한 wallet 의 IVK 하나로 **무한히 많은 diversifier**, 따라서 무한히 많은 `(diversifier, pk_d)` 쌍을 derive (`pk_d = ivk · g_d`). 외부에서는 같은 wallet 의 다른 diversifier 주소들을 link 할 수 없다.

- OFAC 가 `wallet_X` 의 receive 주소 A 를 SDN 등록해도, 같은 wallet 의 다른 diversifier 주소 B 로 보내면 raw bytes 비교조차 안 됨.
- 결과: 단일 raw bytes 매칭으로는 회피 가능. 같은 wallet 이 외부 관찰자에겐 사실상 무한 identity.

**유일한 완화 — IVK 매칭** (cost 큼):
- 검열 대상의 IVK 가 있으면 `ivk.try_recover(diversifier, pk_d)` 로 어떤 diversifier 든 invariant 매칭.
- 그러나 IVK 보유 자체가 매우 invasive (wallet 의 incoming 전수 노출 + voluntary disclosure 필요).
- → shielded audit 의 강도는 결국 **IVK 가용성에 비례**.

### 1.2 Shielded sender 식별 불가 — 1-hop·multi-hop 둘 다

Shielded transaction 의 sender 는 chain 에 노출되지 않는다:
- spend 는 nullifier 만 보임 — spending key 없이는 어떤 note 와도 link 불가.
- output 의 sender 정보는 OVK 로 self-encrypt 돼 있어 sender 외에는 풀 수 없음.

이 한 가지 cryptographic fact 의 결과들:

| layer | 무엇을 못 하는가 | 영향 |
|---|---|---|
| 1-hop (직접 incoming) | "이 사용자가 sanctioned 로부터 받은 적 있는가" 검사 | 우리 시스템이 outgoing 만 검사하는 이유 |
| multi-hop (upstream chain) | "이 ZEC 가 어디서 왔는지" 추적 | "clean source of funds" 증명 불가 |

→ 1-hop 못 보면 multi-hop 도 당연히 못 봄 (multi-hop = 1-hop × N). 한 wallet 의 IVK 가 있으면 그 hop 만 뚫지만 (§1.1 의 IVK 매칭) 그 다음 hop 의 sender 는 또 모름. **shielded 안에서는 chain 추적 본질적 불가능**.

→ 우리 narrative 가 "non-interaction with sanctioned (downstream outgoing)" 좁은 claim 으로 설정된 이유 (decisions D1, idea.md §3).

---

## 2. 구현 한계 (해결 가능)

| 항목 | 우선순위 | 후속 ID |
|---|---|---|
| 2.1 receiver canonicalization 부재 (★ encoding mismatch 의 진짜 원인) | 🔴 높음 | D13 |
| 2.2 transparent UTXO audit-window 이전 미추적 | 🟡 중간 | F2 |
| 2.3 PoW 검증이 lightwalletd header emission 의존 | 🟡 중간 | F1/F3 |
| 2.4 단일 endpoint, failover 없음 | 🟡 중간 | (신규) |
| 2.5 fail-closed guards 부분적 (6단 명시 X) | 🟡 중간 | (신규) |
| 2.6 outgoing 전용, incoming 본체 통합 부재 | 🟢 낮음 | (신규) |
| 2.7 RA-TLS quote verifier 가 Phala API 의존 | 🟢 낮음 | F4 |
| 2.8 canonical JSON (RFC 8785) 미사용 | 🟢 낮음 | (신규) |
| 2.9 sanctioned IVK 매칭 옵션 부재 (1.1 풀기용) | 🟢 낮음 | (신규) |

### 2.1 ★ receiver canonicalization 부재

`hashAddress(addr) = sha256(addr_string)` — raw string 위에 hash. encoding 차이를 normalize 안 함.

**관찰된 mismatch 케이스**:
- 사용자가 Vizor 의 multi-receiver UA `u1sdly...` 로 송금
- Scanner 가 추출 시 orchard-only UA `u1rrzu...` 로 normalize
- 두 string 비교 → mismatch → false negative

**이건 본질적 한계 아님** — UA decode → iterate → 각 receiver bytes hash 하면 풀림. 게으른 구현.

**Fix (D13 후보)**:
```
canonical_hashes(addr) =
  t-addr     → [ sha256(hash160_bytes) ]              // 20 bytes
  sapling    → [ sha256(diversifier || pk_d) ]         // 43 bytes
  orchard    → [ sha256(diversifier || pk_d) ]         // 43 bytes
  UA         → decode 후 위 hash 들의 *집합* emit
  → sanctioned 도 같은 함수 통과 → set intersection 비교
```

작업량: ~30줄 core + 테스트 갱신.

**임시 우회**: `_debug.derivedRecords[*].recipientAddress` (scanner 추출 형식) 그대로 sanctioned 에 넣어 매칭.

### 2.2 transparent UTXO audit-window 이전 미추적

`OurTransparentTracker` (D12.3) 가 audit window 시작 시 빈 상태로 시작. window *이전*에 받은 UTXO 를 range 안에서 spend 하는 케이스는 vin 의 prevout 이 our_utxos 에 없어 outgoing 으로 감지 안 됨.

**Fix (F2)**: scanner 가 audit start_height 직전 시점에 `GetAddressUtxos` 로 우리 t-addr 들의 미사용 UTXO 를 미리 load → tracker 초기 상태로.

### 2.3 PoW 검증이 lightwalletd header emission 의존

`verify_pow=true` 옵션이 `CompactBlock.header` 가 비어있으면 throw. 다수의 공개 lightwalletd 는 header 를 비워서 send.

**Fix 옵션**:
- (A, F1) `GetBlock(BlockID)` 로 header 별도 fetch — N배 추가 call
- (B, F3) Zebra 직접 운영 — header 항상 있음
- (C) lightwalletd 설정 가이드 docs 에 추가

### 2.4 단일 endpoint, failover 없음

한 endpoint 죽으면 사용자가 수동으로 재시도. 팀원 코드 (`week5/clean-wallet-mvp`) 의 `LIGHTWALLETD_PRIMARY` + `LIGHTWALLETD_BACKUP` + `with_failover` 패턴 도입 필요.

### 2.5 Fail-closed guards 부분적

팀원 코드의 6단 guards (network mismatch / range size / intent expiry / range vs tip / UFVK prefix / sanctioned intersect) 명시화. 우리는 일부만 검증.

### 2.6 outgoing 전용, incoming 본체 통합 부재

`runRealScan` 이 OVK 로 outgoing 만 추출. incoming 은 `scan-incoming` 디버그 도구로만 분리. 본체 통합 가능 (~30줄), 단 1.3 의 본질적 약점으로 narrative 약함.

### 2.7 RA-TLS quote verifier 가 Phala API 의존

`submit-ufvk` 의 RA-TLS verify (D12.1) 가 `cloud-api.phala.com` 호출. local 검증 옵션 없음.

**Fix (F4)**: `@phala/dcap-qvl` (local JS DCAP verifier) 옵션 추가. transitive dep 8개 비용.

### 2.8 Canonical JSON (RFC 8785) 미사용

artifact / policy / deposit-intent hash 가 `JSON.stringify` 기반 → cross-language verifier 가 같은 hash 재현 못 할 수 있음. `serde_jcs` (Rust) + `json-canonicalize` (TS) 도입.

### 2.9 Sanctioned IVK 매칭 옵션 부재

§1.1 의 diversifier 회피를 풀려면 IVK 매칭이 필요. 우리 시스템에 자연스럽게 추가 가능:

```
policy.sanctioned_ivks: [<ivk_or_fvk_strings>]
```

scanner 가 추출한 raw `(diversifier, pk_d)` 에 대해 각 sanctioned IVK 로 `try_recover()` → 매칭되면 FAIL.

운영 전제: 그 IVK 가 어디서 오는가 (공익 fund 의 voluntary disclosure / 법원 강제). 실 운영 빈도 낮음.

---

## 3. 신뢰 모델 한계 (TEE)

### 3.1 TEE 하드웨어·벤더 신뢰 전제

Intel TDX + Phala dstack 에 대한 신뢰 가정. 순수 cryptographic 보장 아님. SGX/TDX 사이드채널 / firmware 버그 / Phala 인프라 compromise 시 attestation 의 의미 약화.

→ 의도된 trust assumption (architecture.md §7).

### 3.2 TEE 는 코드만 보증, 입력은 보증 안 함

D9 chainSource binding + 블록 구간 completeness + D12.2 PoW header chain 으로 많이 닫음. lightwalletd 가 PoW 도 통과하는 위조 블록을 만들면 (난도 무시) 막을 수 있지만 mainnet 난도가 충분히 높아 현실성 X.

→ 2.3 의 PoW 검증 활성화 endpoint 사용 권장.

### 3.3 ★ 신뢰가능한 감사 결과의 두 가지 전제조건

TEE 안에서 "*운영자도 평문 못 보는, 검사관이 인정 가능한* audit" 가 성립하려면 **두 가지 조건이 동시에** 충족돼야 한다. 둘 중 하나라도 빠지면 TEE narrative 가 깨진다.

**조건 1 — Viewing key 는 RA-TLS 채널로만 (운영자 평문 노출 방지)**

- 구현: D10 + D12.1. dstack `getTlsKey({usageRaTls:true})` 로 enclave 안에서 keypair 생성. cert 에 TDX quote 박힘. client 가 quote 풀검증 후에만 본문(UFVK) 전송.
- 깨지는 케이스:
  - UFVK 를 env 변수 / 평문 stdin 으로 받으면 운영자가 process 메모리에서 평문 추출 가능 (D10 narrative 정면 위반).
  - client 가 quote 검증 없이 `--no-verify` 로만 보내면 "임의의 enclave 인 척하는 server" 가 가로채도 모름 (현재 verifier 의존 §2.7 도 link).

**조건 2 — Chain data source 는 검사관 (거래소) 이 인정한 endpoint 만**

- 구현: D9 chainSource binding. policy 의 `approvedChainSources` allowlist + 요청 본문의 `chainSource` + artifact 의 binding payload 에 hash 로 묶임. 검사관 = 거래소가 신뢰하는 lightwalletd 만 허용.
- 깨지는 케이스:
  - allowlist 가 비어있거나 너무 넓으면 (예: 임의 URL 허용) 사용자가 자기 lightwalletd 를 띄워 위조 데이터를 enclave 에 주입 가능. TEE 는 코드만 보증하지 입력 진위 모름 (§3.2).
  - 거래소가 allowlist 를 운영적으로 검토 안 하면 D9 의 binding 이 형식적.

**두 조건이 함께 의미하는 것**:

| 조건 | 답하는 질문 | 빠지면 |
|---|---|---|
| 1 (RA-TLS) | "viewing key 가 진짜 enclave 안에서만 평문인가?" | 운영자가 거래내역 평문 접근 → privacy 붕괴 |
| 2 (chainSource allowlist) | "scanner 가 본 blockchain 데이터가 진짜인가?" | 위조 chain 데이터 → audit 결과 의미 X |

둘 다 충족돼야 "운영자도, 누구도 평문 못 보고, 검사관이 데이터 진위까지 신뢰 가능한 audit" 가 성립. 한쪽만 충족된 시스템은 **TEE 를 쓰는 의미가 약함** — `decisions.md` D9·D10 이 같은 라운드에 추가된 이유.

→ Phala 배포 시 두 조건 모두 점검 필수 (deploy-phala.md §4 의 헬스체크 + §6 quote 외부 검증 + 거래소 측 policy allowlist 검토).

---

## 4. 운영적·비기술적 한계

### 4.1 OFAC SDN ZEC 주소 — 현재 transparent 3개 (stale)

직접 확인 (0xB10C/ofac-sanctioned-digital-currency-addresses, lists branch, 2026-05):

```
t1MMXtBrSp1XG38Lx9cePcNUCJj5vdWfUWL
t1WSKwCDL1QYRRUrCCknEs5tDLhtGVYu9KM
t1g7wowvQ8gn2v8jrU1biyJ26sieNqNsBJy
```

**모두 `t1...` transparent P2PKH. 총 3개. shielded 주소 0건.**

**배경**:
- 2020-09-10 designation — Internet Research Agency 멤버 3명 + Russia 연결 인물 4명 (2016 미국 선거 개입 관련). Dash 와 함께 OFAC 의 **첫 privacy coin sanctions**.
- 2020-09 이후 6년간 ZEC SDN 추가 등재 없음 — list 가 그대로.

**이유 추정**:
- OFAC 가 등록 가능한 건 publicly observable 한 주소. shielded receiver 는 chain trace 불가 → 등록해도 추적 불가능 → 등록 동기 약함.
- 단속·집행 인프라 (Chainalysis 등) 가 shielded 분석에 약함.

**Caveat**:
- n=3 은 통계적 결론에 매우 작은 표본. "거의 100%" 라는 단정보다 "현재 알려진 case 는 모두 transparent" 가 honest.
- 향후 OFAC 가 voluntary disclosure / 법원 강제로 IVK 를 받게 되면 shielded 등록 가능성 열림 — 그때는 1.1 의 diversifier 회피 한계가 audit 강도에 직격.

**현재 운영 함의**:
- shielded 송금에 대한 OFAC 매칭은 현실에서 거의 발생 안 함. §1.1·1.2·1.3 의 shielded 한계가 실 운영 영향 작음.
- 우리 narrative 는 "현재까지 OFAC SDN ZEC = transparent only" + "표본 작음 + 변동 가능" 둘 다 명시해야 함.

### 4.2 사용자 wallet 은닉 — narrow claim

사용자가 UFVK 를 자발적으로 제출. 다른 wallet (UFVK 미제출) 의 거래는 검사 대상 밖.

→ 가장 큰 narrative 한계. 거래소 입장에서 "사용자가 모든 wallet 을 disclose 한 것은 아님" 을 전제. 우리 시스템은 *one of many KYC signals*.

### 4.3 Voluntary disclosure 의존

IVK 매칭 (1.1 풀기) 은 검열 대상이 자기 IVK / FVK 를 공개해야 가능. Zcash 의 voluntary disclosure 모델은 wallet 운영자 의지에 의존. 법원 강제 (subpoena) 시 jurisdiction 한정.

### 4.4 Testnet 생태계 부실

2026-05 시점 공개 testnet faucet 다수 사망 / 봇 차단. Discord voluntary TAZ 응답 느림. 결과: 개발 / 테스트 사이클이 mainnet 의존 (작은 amount 라도 비용 발생).

### 4.5 Wallet 별 default UA encoding 차이

Vizor 는 `shielded_address_request()` (orchard+sapling, transparent omit). Zashi 등 다른 wallet 의 default 다름. 같은 mnemonic 이라도 wallet 별로 보여주는 `u1...` string 이 다름 — 사용자 혼란의 흔한 원인.

→ 2.1 의 receiver canonicalization 적용 시 audit 매칭에는 영향 없게.

### 4.6 원본 ZK 아이디어 (mock-JSON ZK) 폐기 사유

원안 (idea.md): 사용자가 outgoing record 목록을 직접 JSON 제출 + ZK 로 non-intersection 증명. **completeness 못 푼다** — 사용자가 sanctioned 든 record 를 witness 에서 빼면 ZK 통과. attested scanner 로 전환 (D1·D2).

---

## 5. 참고

- `docs/idea.md` — 원안 (mock-JSON ZK)
- `docs/decisions.md` D1·D2 — ZK 한계 + attested scanner 전환
- `docs/decisions.md` D12 — RA-TLS · PoW · transparent-only
- `docs/architecture.md` §7~8 — 신뢰 모델 + 보안 고려사항
- `docs/next-session.md` — 후속 작업 항목 (F1~F4)
- 팀원 reference: `D:/private_dumb_money/week5/clean-wallet-mvp` — failover, fail-closed guards, canonical JSON 패턴
