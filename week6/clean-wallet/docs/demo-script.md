# 데모 시나리오

이 프로젝트를 시연하기 위한 대본. CLI / 웹 / 풀 실 모드(RA-TLS) 세 가지.

## 한 문장

> shielded Zcash를 거래소에 입금할 때, 거래내역을 공개하지 않고도 "제재 주소와
> 거래한 적 없음"을 검증 가능하게 보여준다.

## 준비

```bash
npm install
npm test     # core 17 + scanner 19 = 36 통과 — 코어 로직 검증
```

## 시나리오 A — CLI (약 90초)

### A-1. 깨끗한 지갑

```bash
npm run demo
```

말할 것:

- `scan`이 mock 체인의 블록 구간 *전체*를 스캔해 출금 record를 도출했다.
- 결과는 `artifact.json` — 거래소로 가는 유일한 산출물.
- `verify`가 **7개 항목** (D9 chainSource 포함)을 검사하고 전부 통과 → 신뢰 가능 PASS.
- **`artifact.json`을 열어 보여준다** — 수취인 주소·금액·salt 없음. 해시·chainSource·
  PASS/FAIL뿐.

### A-2. 오염된 지갑

```bash
npm run demo:scan -- tainted
npm run demo:verify
```

말할 것:

- 같은 흐름인데 제재 수취인이 있는 지갑 → 결과 FAIL.
- artifact 검증은 *통과*한다 — "유효한 FAIL". 거래소는 거짓 PASS를 받지 않는다.

## 시나리오 B — 웹 (약 2분)

```bash
npm run dev      # http://localhost:3000
```

1. **Prover 페이지** — `scope-clean` 선택 → "스캔 실행" → artifact JSON.
   도출된 record 수는 보이지만 수취인 주소는 보이지 않는다.
2. artifact JSON을 복사.
3. **Verifier 페이지** — 붙여넣고 "검증" → 7개 항목 모두 ✔, 결과 PASS.
4. **변조 시연** — Verifier의 JSON에서 `"result": "PASS"`를 `"FAIL"`로 바꾸고 다시
   검증 → "attestation 서명"이 ✗ → 검증 실패. 서명이 변조를 잡아낸다.
5. `scope-tainted`로 다시 Prover → FAIL artifact → Verifier 통과·결과 FAIL.

## 시나리오 C — 풀 실 모드 (Phala TDX 라이브 + 실 Zcash)

스캐너는 이미 **Phala TDX에 배포되어 라이브**다. 실 mainnet UFVK로 PASS/FAIL이 검증됐다
(샘플 산출물: [docs/examples/artifact-pass.json](./examples/artifact-pass.json) ·
[artifact-fail.json](./examples/artifact-fail.json)). 시나리오 A·B 대신 (또는 추가로) 이
라이브 데모를 보여줄 수 있다. 배포 CVM URL 형식: `https://<app-id>-8080.dstack-<node>.phala.network`.
(아래에선 이 URL을 `<CVM>`으로 줄여 쓴다. 최신 URL은 [ONBOARDING.md §2](../ONBOARDING.md) 참고.)

> PowerShell에선 `curl`이 `Invoke-WebRequest`의 alias다 — 실제 도구는 `curl.exe`.

### C-1. 헬스체크

```bash
curl.exe --insecure https://<CVM>/health
# {"status":"ok","attestationMode":"phala","transport":"https-ra-tls"}
```

`transport: https-ra-tls` → 앱이 enclave 안에서 RA-TLS cert로 HTTPS를 직접 종단하고 Gateway는
TLS passthrough(`-8080s`)로 암호문만 중계한다. cert가 self-signed라 `curl.exe`는 `--insecure`가
필요(진짜 검증은 submit-ufvk가 quote로 한다).

### C-2. 사용자가 UFVK 직접 전달 (본문)

```bash
echo 'uview1...' | node apps/scanner/tools/submit-ufvk.ts \
  --host https://<CVM> \
  --network main \
  --lwd-url https://zec.rocks:443 \
  --start 3363060 --end 3363067
# → screening artifact JSON (위 예제 artifact가 바로 이 mainnet 구간 결과)
```

말할 것:
- **UFVK가 env가 아니라 본문으로** 갔다 — Phala 운영자도 평문에 접근 불가 (D10).
- `--no-verify` 없음 → 클라이언트가 **RA-TLS quote 풀검증 + pubkey 바인딩 + measurement 자동 핀 +
  채널 pin** 을 통과한 뒤에만 UFVK를 보낸다. Gateway는 TLS passthrough(`-8080s`)라 암호문만 중계 =
  신뢰주체 아님. (검증 범위·완성 경로: [demo-architecture-limitations.md](./demo-architecture-limitations.md))
- `--sanctioned` 함정: 스캐너는 outgoing 수취인을 single-receiver(보통 orchard-only) UA로
  normalize한다. 보낸 원본 multi-receiver UA를 그대로 넣으면 매칭이 안 된다 — Rust 사이드카
  `zcash-scanner-rs`의 `recipient_address` 출력값을 그대로 써야 함 ([ONBOARDING.md §2](../ONBOARDING.md)).
- salt는 도구가 random 생성(stderr에 앞 16자 표시) → 사용자가 보관.
- 응답 artifact의 `chainSource`가 요청한 lightwalletd와 일치 (D9).
- `attestation.provider: "phala-tdx"` + `attestation.quote` (hex) → 진짜 TDX.
- Rust 사이드카가 enclave 안에서 lightwalletd 스캔 + **블록 구간 완전성 검증**
  (height·prev_hash) + sapling/orchard/transparent 수취인 추출.

### C-3. 거래소 검증 (시연)

artifact를 (시뮬레이션 거래소가 되어) `verify` 흐름에 넣어 7항목 통과를 보여준다.
실제론 거래소가 자기 정책의 `approvedChainSources`로 7번 체크가 enforce된다.

## 발표 포인트

- **completeness가 핵심.** 사용자가 record를 고르는 게 아니라, attested scanner가
  구간 전체를 스캔한다 ([decisions.md](./decisions.md) D1).
- **TEE 하나가 completeness와 프라이버시를 모두 푼다** (D2).
- **UFVK는 enclave 안에서만 평문** — env가 아니라 RA-TLS 본문(D10).
- **chain source를 정책으로 enforce** — 거래소가 자기 신뢰 lightwalletd만 허용 가능(D9).
- **viewing scope commitment에 salt** — hiding 보장(D11).
- **정직함.** "증명하지 않는 것"을 명확히 한다 — 좁은 신호.

## 자주 나올 질문

- **"그냥 Intel을 믿는 것 아닌가?"** — 맞다, TEE는 하드웨어 신뢰를 전제. 단 pure-ZK는
  completeness를 못 풀어 단독 대안이 아님. 신뢰 모델을 문서에 명시
  ([architecture.md](./architecture.md) §7).
- **"실제 Zcash인가?"** — 시나리오 A·B는 mock, 시나리오 C가 실 UFVK + Rust 사이드카로
  실 lightwalletd 스캔 ([implementation/README.md](./implementation/README.md)).
- **"진짜 TEE에 올렸나?"** — 시나리오 A·B는 시뮬레이션, 시나리오 C는 **이미 Phala TDX에
  배포되어 라이브**다 (실 mainnet UFVK로 PASS/FAIL 검증됨; [deploy-phala.md](./deploy-phala.md)).
- **"UFVK는 누가 본다?"** — D10. **누구도**. 사용자가 RA-TLS 채널로 enclave에 직접 보냄.
  운영자·거래소·우리 둘 다 평문 못 본다.
