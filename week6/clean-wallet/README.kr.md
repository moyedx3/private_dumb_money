# Zcash Private Off-Ramp Screening

Zcash 지갑이 제재 대상 주소와 거래한 적이 없음을 — **거래내역을 거래소에 공개하지
않고** — 증명한다.

> MVP / 해커톤 데모. 로컬 데모는 mock 체인 + 시뮬레이션 attestation, 실 모드는 Rust
> 사이드카 + RA-TLS로 실 Zcash + 실 TDX(Phala) 지원.
> 좁은 스크리닝 신호이며 **컴플라이언스 제품이 아니다.**

> **팀원 시작점 → [`ONBOARDING.md`](ONBOARDING.md)** (셋업 · 테스트 · 배포 빠른 안내).

## 문제

거래소는 shielded 출처 ZEC를 고위험으로 본다 — 자금 출처를 들여다볼 수 없기 때문이다.
그래서 프라이버시를 위해 shielded를 쓴 사용자가 입금을 거부당하거나, viewing key를
통째로 넘기라는(=프라이버시 파괴) 요구를 받는다.

## 접근

**attested scanner**가 TEE(신뢰 실행 환경) 안에서 동작한다:

1. 사용자가 **viewing key + salt**를 **RA-TLS 채널의 본문**으로 enclave에 직접 보낸다
   (거래소도 운영자도 평문에 접근 못 함 — env 아님).
2. 스캐너가 요청 블록 구간 **전체**를 스캔하고 **완전성 검증**(높이·prev_hash 체인)을
   거친 뒤, 그 scope로 보이는 모든 출금 수취인(sapling/orchard/transparent)을 도출한다.
3. 그 수취인을 제재 주소 집합과 대조한다.
4. **screening artifact**를 만든다 — `PASS`/`FAIL` + attestation, 정책·입금 요청·
   **실제 사용한 chain source**에 바인딩됨. **raw 거래내역도 salt도 TEE 밖으로 나가지
   않는다.**
5. 거래소가 artifact를 7항목으로 검증한다.

TEE는 두 문제를 한꺼번에 푼다: 스캔 **완전성**(사용자가 고른 record에 대한 ZK 증명으로는
풀 수 없다)과 수취인 **프라이버시**. ZK 회로는 MVP에서 의도적으로 뺐다 —
[`docs/decisions.md`](docs/decisions.md) 참고.

## 증명하는 것 / 증명하지 않는 것

**증명한다:** 선언한 viewing scope와 블록 구간 안에서, 출금 수취인(shielded·
transparent)이 제공된 제재 집합과 매칭되지 않았다. 결과는 특정 정책·입금 요청·chain
source에 바인딩된다.

**증명하지 않는다:** 사용자가 가진 모든 지갑이 깨끗하다는 것, 완전한 OFAC/AML
컴플라이언스, 자금의 전체 upstream 출처, lightwalletd 운영자가 정직하다는 것 (chain
source는 묶이지만 데이터 정확성은 별도 PoW 검증 필요 — 후속). 이것은 *좁은* 신호다.

## 빠른 시작

```bash
npm install
npm test          # core 17 + scanner 19 = 36
npm run demo      # CLI: 깨끗한 지갑 스캔 -> artifact -> 검증
npm run dev       # 웹 데모 http://localhost:3000
```

FAIL 경로:

```bash
npm run demo:scan -- tainted   # 제재 수취인이 있는 지갑 스캔
npm run demo:verify            # artifact는 검증 통과, 신뢰 결과 = FAIL
```

실 모드 (Phala TDX + 실 UFVK via RA-TLS) — [`docs/deploy-phala.md`](docs/deploy-phala.md).

조회 뷰어 (Phala 스캐너 + AWS Amplify/DynamoDB) — UFVK는 CLI 전용이고, 웹은 비밀이
아닌 artifact만 저장·재검증한다 — [`docs/deploy-web-amplify.md`](docs/deploy-web-amplify.md).

## 저장소 구조

```
clean-wallet/
├─ packages/core/    코어 라이브러리 — 스캐너·attestation·artifact·검증기
├─ apps/web/         Next.js 데모 — Prover / Exchange Verifier / Results(DB) 페이지
├─ apps/scanner/     배포 가능한 스캐너 HTTP/HTTPS 서비스
│  ├─ src/           server.ts (phala 모드 = HTTPS+RA-TLS, 현재 배포 기본; SCANNER_TRANSPORT=http면 평문 HTTP 대안, 신뢰 뉘앙스는 docs/limitations.md §3.3), phala-attestation.ts
│  └─ tools/         submit-ufvk.ts — RA-TLS 본문으로 UFVK 보내는 도구
├─ apps/zcash-scanner-rs/   Rust 사이드카 — 실 Zcash 스캔 (sapling+orchard+transparent)
└─ docs/             전체 문서
```

## 상태

| Phase | 범위 | 상태 |
|---|---|---|
| 1–2 | 코어 파이프라인, attestation, 검증기, CLI | 완료 |
| 3 | Next.js 웹 데모 | 완료 |
| 4 | 실 TEE(Phala) + 실 Zcash (Rust 사이드카) | **배포 완료** — 스캐너가 Phala TDX에서 라이브, mainnet에서 실 UFVK PASS/FAIL 검증 완료 (`docs/examples/`) |
| 5 | 웹 조회 뷰어 + DynamoDB + CLI `--save` | 완료 — [`docs/deploy-web-amplify.md`](docs/deploy-web-amplify.md) |

Phase 4는 `Dockerfile`, `docker-compose.dstack.yml`, `PhalaAttestation` (RA-TLS 자격증명
포함), `apps/scanner/tools/submit-ufvk.ts` 클라이언트, Rust 사이드카(블록 완전성 검증 +
transparent vout 처리 + Orchard unified 인코딩 + UFVK zeroize)를 포함한다. 스캐너는 Phala
Cloud에 배포되어 있고 (dstack Gateway **TLS passthrough** → `SCANNER_TRANSPORT=ratls`, client 가
enclave quote 직접 검증 + measurement 핀; [`docs/deploy-phala.md`](docs/deploy-phala.md) §1), 실
mainnet UFVK 스캔이 엔드투엔드로 검증되었다 (샘플 artifact는 `docs/examples/`).

## 문서

전체 문서는 [`docs/`](docs/)에 있다. [`docs/one-pager.md`](docs/one-pager.md)부터
시작해 `planning`, `architecture`, `decisions`(D1–D12) 순으로 읽으면 된다. 모듈별
설명은 [`docs/implementation/`](docs/implementation/)에 있다.
