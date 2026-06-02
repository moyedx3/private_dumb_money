# Attestation 신뢰 모델 — 현재 검증 범위와 완성 경로 (단계 2)

> **범위:** RA-TLS/attestation 으로 *무엇이 검증되고 무엇이 안 되는지*, 그리고 완전한 보장까지의
> 경로를 정리한다. Zcash crypto / TEE 모델 일반의 본질적 한계는 [`limitations.md`](./limitations.md).
>
> **현재 상태(한 줄):** 데모는 **단계 1.5** — Gateway TLS passthrough + 클라이언트 RA-TLS 풀검증 +
> 채널 fingerprint pin + measurement 자동 pin. **gateway 는 더 이상 신뢰주체가 아니다.** 남은 단
> 하나 = measurement 비교값을 *소스유도*로 올리는 것(단계 2).

---

## 1. 배포 구성 (현재)

| 요소 | 값 |
|---|---|
| Scanner 전송 | `SCANNER_TRANSPORT=ratls` — enclave 안에서 HTTPS(RA-TLS) 직접 종단 |
| Gateway | **TLS passthrough** (`<app-id>-8080s.<domain>`) — 암호문만 중계, 복호화 안 함 |
| Client | `submit-ufvk` (`--no-verify` 없음) — quote 풀검증 + measurement pin + 채널 pin |

→ TLS 가 **enclave 안에서 종단**되고 gateway 는 ciphertext 만 본다. (과거 gateway-HTTP 종단 모드는
[`deploy-phala.md`](./deploy-phala.md) §1 "대안"으로만 남김.)

---

## 2. 지금 무엇이 검증되나 (UFVK 전송 전, 순서대로)

1. **quote 암호학 검증** — cert 의 dstack 확장(OID `1.3.6.1.4.1.62397.1.1`)에서 TDX quote 추출 →
   Phala verifier(`cloud-api.phala.com`)가 DCAP(서명·PCK chain·TCB) 검증, `success:true` 확인.
   → "진짜 Intel TDX 하드웨어가 만든 quote". (`ra-tls-verify.ts` `extractTdxQuoteFromCert`/`verifyQuoteViaPhalaApi`)
2. **pubkey 바인딩** — `report_data == sha512("ratls-cert:" || SPKI_DER)`. → "이 TLS cert(=채널)가
   그 attested enclave 가 만든 키". 채널 substitution 차단. (`verifyPubkeyBinding`)
3. **measurement 핀** — quote 의 **MRTD + RTMR3** 가 게시값(`expected-measurements.json`)과 일치.
   (`verifyRaTlsCert` + `submit-ufvk` 자동 로드 — §3)
4. **채널 fingerprint pin** — 실제 UFVK POST 커넥션의 leaf cert fingerprint 가 위에서 검증한 cert 와
   동일할 때만 본문 전송. (`postScan`)

→ **하나라도 실패하면 UFVK 미전송 + 종료.**

---

## 3. measurement 가 "어떤 부분만" 검증하나 (★ 핵심)

현재 핀은 **두 레지스터를 게시된 hex 와 바이트 비교**만 한다:

| 레지스터 | 의미 | 핀? |
|---|---|---|
| **MRTD** | dstack 베이스 VM 이미지(펌웨어/커널/initrd) 해시 | ✅ 게시값 비교 |
| **RTMR3** | dstack 이 앱 `compose_hash`(도커 구성/이미지)를 extend 한 값 | ✅ 게시값 비교 |
| RTMR0–2 | 부팅 단계 measurement | ❌ 안 봄 (필요 시 추가 가능) |

**핵심 한계 — 비교 "대상값"의 출처.** 지금 게시값은 **라이브 엔드포인트에서 읽은 TOFU 스냅샷**
(`expected-measurements.json`, `trust:"tofu-snapshot"`):

- ✅ **검증됨:** "지금 도는 enclave 가 **게시 시점에 본 그 코드/구성과 동일한가**" (= 드리프트 감지).
- ❌ **미검증:** "그 코드가 **내가 감사한 정직한 소스코드인가**". 게시값을 라이브에서 떠왔으므로,
  게시 시점에 이미 악성이었다면 그대로 통과한다(순환). MRTD/RTMR3 가 *무슨 코드의* 해시인지를
  소스와 묶지 못한다.

→ 현재 수준 = **"동일 배포 고정 + 변경 감지"**. **"정직한 코드 보증"은 아직 아님.**

---

## 4. 완벽해지려면 — 단계 2 모범안

measurement 핀이 "정직한 코드"를 보증하려면 **게시값을 라이브가 아니라 소스/재현빌드에서 유도**해야
한다.

1. **재현가능 빌드(reproducible build)** — 같은 소스 → 같은 도커 이미지 바이트. 빌드 비결정성 제거
   (타임스탬프 고정 `SOURCE_DATE_EPOCH`, 빌드 경로/사용자 고정, 패키지 순서 고정, 핀된 베이스 이미지).
2. **이미지 digest 고정** — `docker-compose.dstack.yml` 의 `image:` 를 태그가 아니라 `@sha256:...`
   digest 로 핀. 그래야 `compose_hash`(→RTMR3)가 특정 이미지 바이트에 묶인다.
3. **measurement 오프라인 계산** — dstack 측정 도구 **`dstack-mr`** 로:
   - **MRTD** = 배포할 dstack OS 이미지 버전에서 계산 (또는 Phala 가 OS 버전별로 게시한 값 사용).
   - **RTMR3** = digest 고정한 app-compose 의 event log 를 replay 해 계산.
   둘 다 **라이브 CVM 없이** 소스/이미지만으로 산출 → TOFU 순환 끊김.
4. **게시 + trust 승급** — 산출값을 `expected-measurements.json` 에 넣고 `trust:"source-derived"` 로.
   소스·빌드 스크립트·이 값을 함께 repo 에 두면 **누구나 재빌드해 같은 값을 재현·검증** 가능.
5. **(선택) 로컬 verifier** — quote DCAP 검증을 `cloud-api.phala.com` 대신 local `@phala/dcap-qvl`
   로 옮기면 Phala verifier 가용성/신뢰 의존도 제거 (`limitations.md` §2.7 F4).

→ 1–4 완료 시: operator 가 악성 코드를 올리면 RTMR3 가 달라져 **클라이언트가 거부**하고, 팀원은
소스만 믿고 게시값을 독립 재현해 확인한다. = **"정직한 코드"까지 보증 = 완성형.**

---

## 5. 신뢰 경계 — 단계 비교

| 단계 | 구성 | client 가 검증 | 막는 것 / 못 막는 것 |
|---|---|---|---|
| 0 (과거) | gateway-HTTP + `--no-verify` | (없음) | 사람 운영자 평문만(gateway 신뢰). **MITM 무방비** |
| **1.5 (현재)** | passthrough + RA-TLS verify + MRTD/RTMR3 pin(**TOFU**) | 진짜 enclave + 채널 pin + 동일배포 고정 | MITM·substitution·드리프트 차단. **게시시점 악성코드** 못 막음 |
| 2 (완성) | 위 + measurement = **source-derived** | + 정직한 코드 | 운영자 악성코드 차단 + 팀 독립 검증. (HW/Phala 본질 신뢰만 남음) |

---

## 6. 남는 신뢰주체 (단계 2 후에도)

- **Intel TDX HW + Phala dstack** — TEE 본질 가정 (제거 불가; `architecture.md` §7).
- **Phala verifier API** — quote DCAP 검증 위임 (local `@phala/dcap-qvl` 로 제거 가능, F4).
- **gateway 는 신뢰주체 아님** — passthrough 로 ciphertext 만 중계.

---

## 7. 참고

- `apps/scanner/tools/ra-tls-verify.ts` · `submit-ufvk.ts` · `expected-measurements.json` — 검증/핀 코드
- `apps/scanner/src/server.ts` · `docker-compose.dstack.yml` — RA-TLS 종단 / 전송 모드
- [`limitations.md`](./limitations.md) §3.3 (두 전제조건) · §2.7 (verifier 의존)
- [`deploy-phala.md`](./deploy-phala.md) §1 (전송 모드 전환)
