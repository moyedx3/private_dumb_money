# 온보딩 — Zcash Private Off-Ramp Screening

TEE(Phala TDX) 안의 attested scanner가 "내 Zcash 지갑이 제재 주소와 거래한 적 없다"를
**거래내역 공개 없이** PASS/FAIL로 증명한다. 배경·개념은
[`README.md`](README.md) · [`docs/one-pager.md`](docs/one-pager.md).

이 문서는 **빠른 시작 + 테스트 방법**만 다룬다. 세부는 아래 문서 맵 참고.

## 문서 맵

| 보고 싶은 것 | 문서 |
|---|---|
| 한 장 요약 | [docs/one-pager.md](docs/one-pager.md) |
| 아키텍처 | [docs/architecture.md](docs/architecture.md) |
| 설계 결정 (D1–D12) | [docs/decisions.md](docs/decisions.md) |
| 용어 | [docs/glossary.md](docs/glossary.md) |
| 정직한 한계 | [docs/limitations.md](docs/limitations.md) |
| **스캐너 → Phala 배포** | [docs/deploy-phala.md](docs/deploy-phala.md) |
| **웹 조회 → AWS Amplify + DynamoDB 배포** | [docs/deploy-web-amplify.md](docs/deploy-web-amplify.md) |
| 실 testnet/mainnet e2e | [docs/testnet-e2e.md](docs/testnet-e2e.md) |
| 데모 시연 대본 | [docs/demo-script.md](docs/demo-script.md) |
| 모듈별 구현 노트 | [docs/implementation/](docs/implementation/) |

## 0. 셋업

```bash
git clone <repo> && cd clean-wallet
npm install
```
Node 22+ 필요(`.ts` 직접 실행). Rust 사이드카는 Docker 이미지 빌드에 포함됨 — **로컬 실 스캔**만
직접 빌드: `cd apps/zcash-scanner-rs && cargo build --release`.

### 0.1 지갑 키 도구 — mnemonic → UFVK 추출 (`gen-testnet-wallet`)

니모닉(또는 raw seed)에서 **UFVK·주소들을 derive** 해 출력하는 로컬 도구. §2 실 스캔에 넣을
**UFVK 를 여기서 뽑는다.** (Rust 사이드카와 같은 crate 의 bin — 이름은 testnet 이지만
`--network main` 으로 mainnet 키도 derive 한다.)

위 `cargo build --release` 면 `apps/zcash-scanner-rs/target/release/gen-testnet-wallet(.exe)` 생성.
또는 `cargo run --release --bin gen-testnet-wallet -- <옵션>` 으로 바로 실행.

```powershell
# (a) 내 기존 니모닉에서 UFVK 추출 — 가장 흔한 용도
.\apps\zcash-scanner-rs\target\release\gen-testnet-wallet.exe --network main --mnemonic "word1 word2 ... word24"

# (b) 새 testnet 지갑 생성 (인자 없음 → 새 24단어 mnemonic)
.\apps\zcash-scanner-rs\target\release\gen-testnet-wallet.exe

# (c) raw seed hex 에서 (>= 64 hex 문자 = 32바이트)
.\apps\zcash-scanner-rs\target\release\gen-testnet-wallet.exe --seed-hex <hex>
```

| 옵션 | 의미 |
|---|---|
| (없음) | 새 24단어 mnemonic 생성 (기본 testnet) |
| `--mnemonic "<24 words>"` | 기존 mnemonic 으로 동일 키 재현 |
| `--seed-hex <hex>` | raw seed(≥32바이트 = 64 hex)에서 derive |
| `--network main\|test` | 기본 `test`. mainnet 키는 `main` |

출력 핵심:
- `ufvk: uview...` — **이 값을 §2 `submit-ufvk --ufvk`(또는 stdin)에 넣는다.**
- `mnemonic` / `seed_hex` — 복원용 비밀.
- `transparent_address_index_0` · `ua_transparent_receiver` · `unified_address_*` — 받기 주소 (faucet·외부 지갑 import 용). (UA 표현 차이 설명은 출력에 함께 나옴.)

⚠️ **mnemonic·seed 는 진짜 비밀.** repo·이미지·env·DB 커밋 금지(§보안). 파일로 저장하면
(`... > wallet.txt`) gitignore 확인. **mainnet mnemonic = 진짜 자금 — 노출 = 손실.**

## 1. 빠른 검증 (네트워크 불필요)

```bash
npm test          # 단위 테스트: core 17 + scanner 19
npm run demo      # CLI: 깨끗한 지갑 스캔 → artifact 생성 → 검증 (sim attestation)

# FAIL 경로
npm run demo:scan -- tainted
npm run demo:verify
```

## 2. 배포된 TEE 스캐너에 실 스캔 (핵심 e2e)

현재 dev CVM (바뀔 수 있음 — 최신 URL은 팀/Phala 대시보드 확인):
```
https://8a7763ac129cce9f021cc78db11a275d9dfaa2fd-8080s.dstack-pha-prod9.phala.network
```
아래 명령은 이 URL을 `<CVM>`로 줄여 쓴다 (포트 뒤 **`s`** = Gateway TLS passthrough). **PowerShell에선 `curl` 아니라 `curl.exe`.**

**(a) 헬스 + mock 스캔 — UFVK 불필요:** passthrough RA-TLS cert는 self-signed라 `curl.exe`는 `--insecure` 필요(진짜 검증은 submit-ufvk가 quote로 함).
```powershell
curl.exe --insecure https://<CVM>/health
# {"status":"ok","attestationMode":"phala","transport":"https-ra-tls"}

curl.exe --insecure -X POST https://<CVM>/scan -H "content-type: application/json" -d "{\"mode\":\"mock\",\"scope\":\"tainted\"}"
# → artifact JSON, "provider":"phala-tdx" + 진짜 TDX quote
```

**(b) 실 UFVK 스캔 — `submit-ufvk`.** UFVK는 **본인 머신에서만** CLI로 enclave에 전송(웹/DB엔 안 감).
`--no-verify` 없이 실행 → RA-TLS quote 풀검증 + measurement 자동 핀(`expected-measurements.json`) +
채널 pin 을 모두 통과한 뒤에만 UFVK 전송. `$ufvk`에 본인 UFVK 세팅 후:

```powershell
# PASS — 제재 주소 안 줌(데모 mock 집합과 비교 → 실 수취인 안 맞아 PASS)
node apps/scanner/tools/submit-ufvk.ts `
  --host https://<CVM> `
  --network main --lwd-url https://zec.rocks:443 `
  --start 3363060 --end 3363067 `
  --ufvk $ufvk
# → "result":"PASS",  _debug.derivedRecordsCount = 발견한 outgoing 수

# FAIL — 특정 수취인을 제재로 지정
node apps/scanner/tools/submit-ufvk.ts `
  --host https://<CVM> `
  --network main --lwd-url https://zec.rocks:443 `
  --start 3363060 --end 3363067 `
  --ufvk $ufvk `
  --sanctioned "<normalize된 recipient_address>"
# → "result":"FAIL"
```
> lightwalletd(`zec.rocks`)가 `UNAVAILABLE` 등으로 죽으면 지역 노드로 교체: `--lwd-url https://ap.zec.rocks:443`
> (또는 `eu`/`na`). 자동 failover는 미구현 ([limitations.md §2.4](docs/limitations.md)).

⚠️ **`--sanctioned` 함정**: 스캐너는 outgoing 수취인을 **그 pool의 single-receiver UA(보통
orchard-only `u1...`)로 normalize**한다. 네가 보낸 원본 multi-receiver UA를 그대로 넣으면
hash가 안 맞아 FAIL이 안 뜬다. 정확한 값 = 스캐너가 뽑은 `recipient_address`를 직접 확인:
```powershell
$req = @{ network="main"; lightwalletd_url="https://zec.rocks:443"; ufvk=$ufvk; start_height=3363060; end_height=3363067 } | ConvertTo-Json -Compress
$req | .\apps\zcash-scanner-rs\target\release\zcash-scanner-rs.exe
# outgoing_records[].recipient_address ← 이 값을 그대로 --sanctioned 에
```
(UA를 분해해 receiver 종류·raw bytes 진단: `decode-ua.exe <u1...>`)

**플래그 의미:**
- (기본·플래그 없음) — RA-TLS quote 풀검증 + pubkey 바인딩 + **measurement 자동 핀**
  (`expected-measurements.json`) + 채널 fingerprint pin. 전부 통과해야 UFVK 전송. 현재 CVM이
  `SCANNER_TRANSPORT=ratls` + Gateway **TLS passthrough**(`-8080s`)라 이게 기본 작동. 검증 범위·신뢰
  모델: [docs/demo-architecture-limitations.md](docs/demo-architecture-limitations.md).
- `--no-verify` — **디버그 전용.** quote/measurement/cert 검증을 전부 끈다(능동 MITM 무방비). gateway-HTTP
  종단 모드(대안)에서만 필요. 평소 쓰지 말 것.
- `--no-measurement-pin` / `--expected-mrtd,--expected-rtmr3` — measurement 핀 끄기 / 게시값 덮어쓰기.
- `--save <url>` — 결과 artifact를 웹 DB에 적재 (예: `https://<amplify-app>/api/artifacts` —
  [docs/deploy-web-amplify.md §4](docs/deploy-web-amplify.md)).
- `--start/--end` — 작게(50~100블록 이하). tx마다 GetTransaction이라 큰 범위는 느림.

**로컬 스캐너로 테스트하려면**(Phala 없이): `docker compose up --build`(HTTP sim) 또는
`docker compose -f docker-compose.local-tee.yml up --build`(실 PhalaAttestation 코드 경로 +
dstack simulator). `--host`를 `http://localhost:8080`으로.

## 3. 배포

- **스캐너 → Phala**: [`docs/deploy-phala.md`](docs/deploy-phala.md)
- **웹 조회 → Amplify + DynamoDB**: [`docs/deploy-web-amplify.md`](docs/deploy-web-amplify.md)

## 보안 (필수)

- **UFVK·salt·mnemonic·seed는 repo·이미지·env·DB에 절대 커밋 금지.** UFVK는 CLI로 enclave에만 전송.
- **artifact(PASS/FAIL + attestation)는 비밀이 아니다** — 수취인 주소·UFVK가 들어가지 않으므로
  공유·DB·웹 조회 OK. (`docs/examples/`의 예제도 실 mainnet 스캔 결과지만 비밀 없음.)
- `.env*.local`(AWS 키 등)은 gitignore됨.

## 부록 A — 웹 조회 UI (선택 · 데모 핵심 경로 아님)

> 핵심 데모는 §1(빠른 검증) → §2(실 스캔). 아래 웹 UI는 **선택**이며 절차상 필수 단계가 아니다.
> `/prover`·`/verifier`는 sim 데모(네트워크 없이 artifact 생성·검증), `/results`는 저장된
> artifact를 DB에서 조회·재검증하는 화면이다 — 라이브 TEE 데모(§2)에는 쓰이지 않는다.
> (웹 데모 시연 대본은 [docs/demo-script.md](docs/demo-script.md) 시나리오 B.)

```bash
npm run dev       # http://localhost:3000
```
- `ARTIFACTS_TABLE` 미설정 → **in-memory 저장**(비영속, 개발용).
- `/results` → **"artifact 직접 업로드"** → 아래 예제 붙여넣기 → 저장·재검증:
  - [`docs/examples/artifact-fail.json`](docs/examples/artifact-fail.json) — 실 Phala TDX **FAIL** artifact
  - [`docs/examples/artifact-pass.json`](docs/examples/artifact-pass.json) — 실 Phala TDX **PASS** artifact
- 기대: 바인딩 항목 ✓ 통과 + attestation은 `⧉ 위임`(phala-tdx quote는 Phala verifier 외부검증)
  + 신뢰 결과 = PASS/FAIL.
- `/prover`·`/verifier` = sim 데모(네트워크 없이 artifact 생성·검증).
