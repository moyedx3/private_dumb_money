# Phase 4 — Phala dstack 배포 가이드 (D9·D10·D11 반영)

> **상태:** 코드 완성 (PhalaAttestation + RA-TLS + Rust 사이드카 + 멀티스테이지 Dockerfile +
> dstack compose). 실 배포·실 TDX·실 UFVK는 사용자 환경에서 수행 (디버그 라운드 가능).

## 0. 무엇이 준비됐나

| 항목 | 상태 |
|---|---|
| `apps/scanner` HTTP/HTTPS 서비스 | ✅ — sim=HTTP, phala=HTTPS(RA-TLS) |
| `apps/scanner/tools/submit-ufvk.ts` 클라이언트 | ✅ |
| `apps/zcash-scanner-rs` Rust 사이드카 (Phase B 강화) | ✅ — transparent·완전성·Orchard·zeroize |
| `PhalaAttestation` (attest + getRaTlsCredentials) | ✅ |
| `Dockerfile` 멀티스테이지 (Rust + Node) | ✅ |
| `docker-compose.yml`(로컬) / `docker-compose.dstack.yml`(dstack) | ✅ |
| 실제 Phala 배포 | ✅ 배포됨 (dstack TDX, **ratls passthrough** = 단계 1) |
| 실 Zcash UFVK 검증 | ✅ mainnet UFVK PASS/FAIL 검증됨 (`docs/examples/`) |
| TDX quote 풀 검증 | ⚠️ Phala verifier 위임 (§6) |
| RA-TLS 클라이언트 측 quote 검증 | ✅ 구현·가동 (passthrough 기본) · measurement 자동 핀(TOFU) + 채널 pin 포함 |

## 1. 동작 모드

스캐너는 환경변수 한 개로 attestation 모드를 고른다. UFVK·chainSource·scanRange는 더 이상
환경변수가 아니라 **RA-TLS 본문**으로 받는다 (D10).

| env | 값 | 의미 |
|---|---|---|
| `ATTESTATION_MODE` | `simulated`(기본) / `phala` | 시뮬레이션(HTTP) ↔ 실 TDX (전송은 `SCANNER_TRANSPORT`) |
| `PORT` | (기본 8080) | listen 포트 |
| `SCANNER_TRANSPORT` | (기본 `ratls`) / `http` | phala 모드 전송. `http`면 앱이 평문 HTTP 서빙(TLS는 gateway가 종단) |
| `ZCASH_SCANNER_BIN` | (Docker 기본 `/usr/local/bin/zcash-scanner-rs`) | Rust 사이드카 경로 |
| `RATLS_ALT_NAMES` | (선택) | cert SAN에 들어갈 호스트 콤마 구분 |

> ✅ **현재 배포 = 단계 1 (ratls passthrough).** `SCANNER_TRANSPORT=ratls` 로 앱이 enclave 안에서
> RA-TLS HTTPS를 8080에 직접 종단하고, Gateway는 **TLS passthrough**(엔드포인트 포트 뒤 `s`, 예
> `<id>-8080s.<domain>`)로 암호문만 중계한다. 클라이언트(submit-ufvk)는 `--no-verify` 없이 TDX
> quote를 직접 풀검증 + measurement 자동 핀 → enclave를 직접 검증하므로 **gateway가 신뢰주체가
> 아니다**. (cert가 self-signed라 `curl`엔 `--insecure` 필요. 검증 범위: demo-architecture-limitations.md)
>
> ⚠️ **대안(약한 모드) — gateway TLS termination:** passthrough를 못 쓰는 환경이면
> `SCANNER_TRANSPORT=http` + non-`s` 엔드포인트. 그럼 dstack Gateway가 Let's Encrypt cert로 TLS를
> **종단**하고 컨테이너엔 HTTP 포워딩 → 클라이언트는 `--no-verify` 필요(quote 직접검증 불가, gateway
> TEE를 신뢰). attestation quote는 artifact 본문엔 그대로 들어가지만 채널 보장은 약화 (limitations.md §3.3).

조합:

- `simulated` + body `mode:mock` — 로컬 sim 데모 (HTTP).
- `phala` + `SCANNER_TRANSPORT=ratls` + gateway **passthrough(`s`)** + submit-ufvk `--no-verify`
  제거 — **현재 배포·권장.** client가 enclave를 직접 검증하는 end-to-end RA-TLS (단계 1).
- `phala` + `SCANNER_TRANSPORT=http` + non-`s` 엔드포인트 + submit-ufvk `--no-verify` — 대안(약한
  모드, gateway 종단). passthrough 불가 환경에서만.

## 2. 필요한 것 (사용자가 준비)

- **Phala Cloud 계정** — 배포용.
- **DockerHub 계정** — 이미지 푸시용.
- *(real 모드)* **lightwalletd 엔드포인트** — 예: `https://lwd.zec.pro:443` (mainnet) 또는
  testnet 노드 (zechub.wiki/zcash-tech/lightwallet-nodes).
- *(real 모드)* **UFVK** — 자기 Zcash 지갑(또는 testnet 테스트 지갑)의 Unified Full
  Viewing Key. read-only 키지만 거래내역 가시성 있음 — 신중히.
- *(real 모드)* **submit-ufvk 도구 실행 환경** — Node 24+ 가 깔린 사용자 머신.

> ⚠️ UFVK·salt는 **레포지토리·이미지·env에 절대 넣지 말 것.** D10 — RA-TLS 본문으로만.

## 3. 배포 절차

Phala Cloud는 이미지를 레지스트리에서 pull → 먼저 빌드·푸시 후 배포.

### 3.1 이미지 빌드 + 푸시 (DockerHub)

`clean-wallet/`에서:

```bash
docker login
docker build -t <DOCKERHUB_USER>/zcash-screening-scanner:latest -f apps/scanner/Dockerfile .
docker push  <DOCKERHUB_USER>/zcash-screening-scanner:latest
```

(Dockerfile은 멀티스테이지 — Rust 사이드카까지 자동 빌드. 첫 빌드 5~15분.)

`docker-compose.dstack.yml`의 `image:`를 푸시한 이미지로 바꾼다.

### 3.2 배포 — Phala Cloud 대시보드 (권장)

1. Phala Cloud 콘솔 로그인.
2. CVM 생성 → Docker Compose / Advanced. OS = 최신 stable dstack 이미지.
3. `docker-compose.dstack.yml` 내용 붙여넣기.
4. 환경변수: `ATTESTATION_MODE=phala` + **`SCANNER_TRANSPORT=ratls`** (단계 1 — 앱이 enclave 안에서
   RA-TLS HTTPS 직접 종단). UFVK 등 비밀은 절대 넣지 않는다.
5. 네트워크: dstack Gateway ON, **TLS passthrough**(포트 `8080s`). Restrict mode면 8080 허용목록 추가.
6. 배포.

배포 후 Gateway가 발급한 passthrough endpoint(예:
`https://<app-id>-8080s.dstack-<node>.phala.network` — 포트 뒤 **`s`**)를 대시보드에서 확보한다.
이게 `--host`/스캐너 주소. (passthrough를 못 쓰면 §1 대안: `http` + non-`s`.)

### 3.3 (대안) CLI

`phala cvms create`는 deprecated → `phala deploy`. PowerShell에서 불안정하므로
**WSL/Linux 셸**에서 실행.

## 4. 동작 확인

아래 `<cvm-host>`는 §3.2에서 확보한 Gateway endpoint. **PowerShell은 `curl` 대신 `curl.exe`.**

### 4.1 헬스체크

```bash
# 현재 배포: ratls passthrough(-8080s). cert가 self-signed(enclave RA-TLS)라 --insecure 필요.
curl --insecure https://<cvm-host>/health
# {"status":"ok","attestationMode":"phala","transport":"https-ra-tls"}
```

`transport: https-ra-tls` = 앱이 enclave 안에서 RA-TLS 종단(단계 1). cert 안 TDX quote 풀검증은
submit-ufvk(§4.3)가 자동으로 한다. (대안 `http` 모드면 LE cert라 `--insecure` 없이 `transport:http`.)

### 4.2 mock 데이터로 attestation 만 검증

```bash
curl -X POST https://<cvm-host>/scan \
  -H 'content-type: application/json' \
  -d '{"mode":"mock","scope":"tainted"}'
# → screening artifact JSON, attestation.provider="phala-tdx"
```

### 4.3 실 UFVK로 풀 스캔 (submit-ufvk 도구 사용)

자기 머신에서:

```bash
cd clean-wallet
# UFVK를 stdin으로 (process list에 안 남게)
echo 'uview1...' | node apps/scanner/tools/submit-ufvk.ts \
  --host https://<cvm-host> \
  --network test \
  --lwd-url https://lwd.testnet.example:443 \
  --start 2500000 --end 2500050
# → screening artifact JSON
# (<cvm-host> = §3.2 의 passthrough -8080s 엔드포인트. --no-verify 없음 = quote 풀검증 +
#  measurement 자동 핀 + 채널 pin. 대안 http 배포에서만 --no-verify 필요.)
```

도구가 random salt를 생성·표시(보관 안내)하고, UFVK·salt·chainSource·scanRange를 본문에
실어 `/scan`에 POST. 응답이 artifact JSON.

## 5. 남은 작업 / 후속

### 5.1 PhalaAttestation 실 환경 검증
TDX 없이 테스트 못함. 배포 후 `getQuote`/`info`/`getTlsKey` 호출이 dstack SDK
버전과 맞는지 확인 → 안 맞으면 `apps/scanner/src/phala-attestation.ts` 조정.

### 5.2 실 Zcash 통합 검증
Rust 사이드카는 컴파일·IPC·완전성 로직 검증됨. 실 UFVK + lightwalletd로 end-to-end는
사용자 첫 실행에서 검증.
잠재 이슈:
- HTTPS lightwalletd 연결 → `tonic` `tls` feature 필요할 수 있음 (없으면 build 재시도).
- 큰 블록 범위는 느림 (tx마다 GetTransaction). 50~100블록 권장.

### 5.3 RA-TLS 클라이언트 측 quote 검증 (D12.1 — 구현됨)
`apps/scanner/tools/ra-tls-verify.ts` 에 들어 있다. submit-ufvk 가 기본으로 RA-TLS 풀검증을
수행하고, 풀검증 통과한 cert 를 pin 한 채 본문 POST 한다. 항목:
- cert 의 dstack OID `1.3.6.1.4.1.62397.1.1` 추출 → TDX quote.
- Phala verifier API (`cloud-api.phala.com/api/v1/attestations/verify`) 위임.
- `report_data == sha512("ratls-cert:" || SPKI_DER)` 바인딩 확인 (anti-substitution).
- **measurement 자동 핀** — `expected-measurements.json` 의 MRTD/RTMR3 와 비교 (기본 ON, TOFU 등급).
  `--expected-mrtd/--expected-rtmr3` 덮어쓰기, `--no-measurement-pin` 으로 끄기. 완성(단계 2):
  [demo-architecture-limitations.md](./demo-architecture-limitations.md) §4.
- 풀검증 통과 cert 를 **fingerprint pin** 한 채로만 UFVK POST (postScan).
디버그용 escape hatch: `--no-verify` (검증 전부 끔 — 대안 http 배포 전용).

## 6. TDX quote 외부 검증

`/scan` artifact의 `attestation.quote`(hex TDX quote)를 **Phala 공개 verifier /
dstack-verifier**에 가져가면 진짜 TDX인지 cryptographic 검증된다. 데모에서는 그 단계를
"이 quote는 Phala verifier에서 검증됨"으로 보여준다.

`PhalaAttestation.verify()`는 풀 DCAP를 안 함 — 순수 JS 풀 구현이 비현실적이라
구조·measurement 일치만 확인하고 quote는 외부에 위임.

## 7. 정직한 한계

- TDX/Phala attestation은 하드웨어·벤더(Intel/Phala) 신뢰를 전제 — 순수 암호학 아님
  ([architecture.md](./architecture.md) §7).
- enclave는 *코드*만 보증, *입력*은 아님 — D9 chainSource + D12.2 PoW 헤더 체인 검증으로
  많이 좁혔지만, `verify_pow=true` 가 동작하려면 lightwalletd 가 `CompactBlock.header` 를
  보내야 한다 (공개 lightwalletd 다수 미지원 — 후속 F1/F3 참고).
- 사용자가 *다른 지갑(UFVK)*을 숨기면 막을 수 없음 — 클레임이 "제출된 UFVK 범위 내"로 좁다.
- D12.1 로 RA-TLS 클라이언트 측 quote 풀검증이 들어왔지만 Phala verifier API 가용성에 의존.
  사내 verifier / local `@phala/dcap-qvl` 로 옮기는 후속(F4) 검토 가능.
- D12.3 transparent-only 송금 감지는 audit window 시작 *이전* 받은 UTXO 는 못 잡는다 — 시작
  시점 UTXO 를 `GetAddressUtxos` 로 미리 불러오는 보강이 후속(F2).
- 이대로 배포해도 완전한 OFAC/AML 컴플라이언스 아님 — 좁은 스크리닝 신호다.
