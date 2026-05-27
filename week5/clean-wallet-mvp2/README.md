# Clean Wallet PoC

Zcash shielded note에 대해 **제출된 범위 안에서 blacklist commitment와 겹치지 않음**을 확인하는 PoC입니다.

현재 구현은 실제 Zcash client/TEE가 아니라 **fixture + mock TEE** 기반입니다.

---

## 무엇을 증명하는가

`PASS`가 의미하는 것:

```text
제출된 network / pool / viewing scope / block range / blacklist root 안에서
사용자의 note commitment와 blacklist commitment의 exact overlap이 발견되지 않았다.
```

즉 핵심 조건은 다음입니다.

```text
MyCommitments ∩ BlacklistCommitments = ∅
```

`PASS`가 증명하지 않는 것:

```text
- 전역적 무죄
- 모든 wallet/account를 제출했다는 사실
- identity ownership
- taint ancestry 부재
- hidden wallet / 다른 seed 부재
- trustless proof
```

scanner/runtime 문제가 있으면 결과는 `ERROR`이며, `PASS`로 처리하지 않습니다.

---

## 구성도

### 실제 구현 목표 구조

```text
┌──────────────────────────────┐
│ Client / User Wallet          │
│ - UFVK/FVK/IVK 보유           │
│ - attestation 확인            │
│ - proof 요청                  │
└──────────────┬───────────────┘
               │ encrypted viewing capability
               ▼
┌──────────────────────────────┐
│ Verification Server           │
│                              │
│ Untrusted Host                │
│ - API gateway                 │
│ - chain/lightwalletd access   │
│ - report delivery             │
│                              │
│ Trusted TEE Enclave           │
│ - shielded note scan          │
│ - commitment extraction       │
│ - blacklist overlap check     │
│ - attested report 생성        │
└──────────────┬───────────────┘
               │ PASS / FAIL / ERROR report + quote
               ▼
┌──────────────────────────────┐
│ Verifier / Compliance Desk    │
│ - blacklist root 확인         │
│ - enclave measurement 확인    │
│ - quote 검증                  │
│ - bounded claim만 수용        │
└──────────────────────────────┘
```

### 현재 레포의 mock 구조

```text
fixtures/*.json
   │
   ▼
FixtureScanner
   │
   ▼
set intersection
   │
   ▼
MockAttestor
   │
   ▼
artifacts/*-report.json
   │
   ▼
verify-report
```

---

## 주요 파일

```text
clean_wallet/
  scanner.py        # fixture scanner; 실제 Zcash scanner 교체 지점
  attestation.py    # MockAttestor; 실제 TEE 교체 지점
  blacklist.py      # blacklist manifest/root/signature
  proof.py          # PASS/FAIL/ERROR report 생성 및 검증
  cli.py            # CLI

fixtures/
  blacklist_commitments.txt
  pass_scan.json
  fail_scan.json
  error_scan.json

scripts/
  demo-pass.sh
  demo-fail.sh
  demo-error.sh
  verify-report.sh
```

---

## 사용법

### 1. 전체 데모 실행

```sh
scripts/demo-pass.sh
scripts/demo-fail.sh
scripts/demo-error.sh
```

### 2. 테스트 실행

```sh
python3 -m unittest discover -s tests
```

### 3. Blacklist 생성

```sh
python3 -m clean_wallet.cli build-blacklist \
  --commitments fixtures/blacklist_commitments.txt \
  --output artifacts/blacklist.json \
  --network regtest \
  --pool orchard \
  --issuer demo-issuer \
  --version v0
```

### 4. PASS report 생성

```sh
python3 -m clean_wallet.cli request-proof \
  --fixture fixtures/pass_scan.json \
  --blacklist artifacts/blacklist.json \
  --output artifacts/pass-report.json \
  --viewing-scope-id alice-orchard-account-0 \
  --network regtest \
  --pool orchard \
  --start-block 100 \
  --end-block 110
```

### 5. FAIL report 생성

```sh
python3 -m clean_wallet.cli request-proof \
  --fixture fixtures/fail_scan.json \
  --blacklist artifacts/blacklist.json \
  --output artifacts/fail-report.json \
  --viewing-scope-id alice-orchard-account-0 \
  --network regtest \
  --pool orchard \
  --start-block 100 \
  --end-block 110
```

### 6. Report 검증

```sh
python3 -m clean_wallet.cli verify-report \
  --report artifacts/pass-report.json \
  --blacklist artifacts/blacklist.json
```

---

## TEE 처리

현재:

```text
clean_wallet/attestation.py::MockAttestor
```

- mock measurement 생성
- `report_hash`를 quote의 `report_data`에 바인딩
- HMAC 기반 mock signature 생성/검증

실제 구현 시 교체:

```text
MockAttestor → SGX DCAP / TDX / Nitro attestor
```

유지할 인터페이스:

```python
quote(report_hash) -> quote
verify_quote(quote, expected_report_hash, allowed_measurements) -> None
```
