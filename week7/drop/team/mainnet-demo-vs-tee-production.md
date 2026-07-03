# 로컬 메인넷 데모 vs 실제 TEE 프로덕션

- **대상:** 이번 세션의 "실 메인넷 결제 → A1 감지 → 키교환 → buyer 복호·재생" end-to-end.
- **한 줄:** **앱 프로토콜은 실 메인넷에서 완전 입증.** 단 **TEE 신뢰 레이어(attestation·measurement 바인딩·호스트 blindness)는 이번 실행에서 꺼져 있었다** — 로컬 프로세스 + 호스트가 아는 dev seed + `/attest` 미사용. → "인덱서/호스트가 콘텐츠 못 본다"는 핵심 보장은 **미적용**.

## 1. REAL vs MOCK

| 요소 | 이번 데모 | 비고 |
|---|---|---|
| 결제·체인스캔·memo·dispatch·복호 | **REAL** — 실 ZEC, mainnet lightwalletd, dryoc↔libsodium, AES-GCM | 앱 계층 전부 실제 |
| memo 형식 (M6 실측) | **REAL** — Zashi가 raw 바이너리 memo를 떨궈 `A1B64:` 텍스트 사용 | 실측 결과 |
| 인덱서 실행 | **MOCK** — Windows 로컬 프로세스 | TEE 아님 |
| provisioning seed | **MOCK** — `A2_DEV_PROVISIONING_SEED_HEX`(호스트가 앎) | **"host가 k_drop 못 봄" 미적용** (`main.rs:94-103`) |
| `/attest` · creator 검증 | **미사용** — dstack 없어 quote 없음. pubkey를 dev seed서 파생, provision을 스크립트로 | attestation 검증 통째로 skip |
| 전송 | 평문 HTTP | TLS 없음 |
| 규모 | 단일 creator/drop | N1/N2는 멀티테넌트서 중요 (`a1-a2-security-N1-N2.md`) |

> spike #3(`spike3/RUNBOOK.md`)가 encrypt-to-enclave를 실 TDX 하드웨어서 별도 검증. 이 통합 데모만 로컬.

## 2. TEE 속성 — 구현 상태

**✅ 구현됨 (실 dstack/wiring 필요):**
- Quote 발급 + `report_data=sha256(pubkey)` 바인딩 — `attest.rs`
- measurement-bound KMS seed(재빌드 시 변경) — `main.rs:101`, `dstack.rs get_key`
- creator측 검증: measurement 핀 대조 + pubkey 바인딩 — `attestation.ts:51,58`
- dev-seed override를 live TEE서 부팅 거부 — `main.rs:83-89`
- 비밀 Debug redaction · C4 연속성(재-provision idempotent)

**⚠️ 가정/외부 (이번엔 미충족):**
- **DCAP QVL(quote 서명검증)** — 외부 플러그인, 미번들. 없으면 검증 throw. `attestation.ts:118-142`
- expected measurement(재현빌드 해시) — creator가 핀으로 제공해야. CI 미검증
- dstack 런타임(Phala TDX) — 로컬 부재

**❌ 미구현 / 설계상 대체:**
- **RA-TLS** — spec C3의 옵션(a). 구현은 **(b) encrypt-to-enclave 채택** → 부재는 "다른 경로 선택". 서버 TLS도 없음(메타데이터 미보호).

## 3. 핵심 통찰

- **creator의 secret-IN에 RA-TLS 불필요.** K_drop·UFVK은 `crypto_box_seal`로 attested pubkey에 봉인(`provision.rs`) → 평문 HTTP라도 enclave만 복호, pubkey 바꿔치기는 quote 위조 불가라 차단. **encrypt-to-enclave = RA-TLS (비밀 한정).** RA-TLS가 더 주는 건 `?title=` 평문 파라미터·메타데이터 보호뿐.
- **buyer는 attestation 불필요** — 읽기 전용, enclave에 비밀 안 보냄. dispatch blob은 자기 e_pub로만 열림.
- **진짜 신뢰의 뿌리 = quote 검증(DCAP QVL).** RA-TLS든 encrypt-to-enclave든 이게 있어야 의미. **QVL 미배선이 유일한 실질 구멍.**
- **enclave 키 = KMS가 measurement서 결정론적 파생**(랜덤 X, 디스크 X, host 불가시). 랜덤이면 재시작마다 키 갈려 연속성 붕괴. 대가: 재빌드→키 변경→옛 봉인/자금 좌초(C4, `spec.md §7.6`).

## 4. 실 TEE 배포 전 체크리스트

- [ ] **DCAP QVL 배선**(@phala/dcap-qvl-web) — 없으면 attestation 무의미
- [ ] 재현빌드 CI + measurement 게시 → creator 핀
- [ ] Phala/dstack 배포 → `get_key`/`get_quote` 활성 + `A2_DEV_PROVISIONING_SEED_HEX` 제거
- [ ] creator 앱 경유 provision(스크립트 우회 대신 `/attest`→검증→seal)
- [ ] 최소 TLS(메타데이터) + R-A2-4(버킷을 TEE 밖으로)
- [ ] 멀티-creator로 N1/N2 실측(현재 미검증)

> 이번 세션의 로컬 도구(gen-wallet·seal-dispatch·통합 테스트·인덱서 tracing/heartbeat·dstack cfg)는 프로덕션 경로 아님 — 별도 취급.
