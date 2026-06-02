# Clean Wallet PoC

Zcash shielded note에 대해 **제출된 범위 안에서 blacklist commitment와 겹치지 않음**을 확인하는 PoC입니다.

현재 레포는 두 경로를 분리합니다. 로컬 demo는 fixture scanner를 사용할 수 있지만,
HTTP service/container 기본값(`CLEAN_WALLET_ATTESTOR=phala`)에서는 fixture proof를
거부하고 **encrypted viewing capability + real lightwalletd compact blocks + enclave-local scanner command**
경로만 허용합니다. 로컬 mock TEE는 명시적으로 `CLEAN_WALLET_ATTESTOR=mock`을 설정할 때만 사용합니다.

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

### 현재 레포의 로컬 mock 구조

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
  enclave_key.py    # attested encrypted viewing capability key descriptor
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

---

## Phala Cloud / dstack TEE 목표 구조

HTTP service와 Docker image 기본값은 `CLEAN_WALLET_ATTESTOR=phala`입니다.
Phala Cloud CVM에서 `/var/run/dstack.sock`과 `dstack-sdk`를 통해 실제 TDX quote를
생성합니다. dstack이 없으면 `/health`, `/measurement`, `/attestation`은 503으로
실패합니다. 로컬 fixture demo만 돌릴 때는 `CLEAN_WALLET_ATTESTOR=mock`을 명시하세요.

Production Phala/default mode now rejects fixture proof submissions. The real scanner path accepts only an encrypted viewing capability plus a lightwalletd chain source. The enclave/container decrypts the viewing capability, fetches compact blocks via lightwalletd `GetBlockRange`, and invokes `CLEAN_WALLET_ZCASH_SCANNER_CMD` for Sapling/Orchard trial-decryption and owned note commitment extraction. The Docker image builds and wires the Rust `clean-wallet-zcash-scanner` command by default. If the command is missing, unsupported, or fails, the report result is `ERROR`, never `PASS`.

### TEE 내부 목표 흐름

```text
대상자
  └─ attestation/compose-hash 확인 후 encrypted viewing capability 제출

Phala Cloud CVM / Intel TDX
  ├─ clean-wallet container
  ├─ /var/run/dstack.sock
  ├─ ZcashViewingKeyScanner  # decrypts capability, fetches lightwalletd, invokes scanner command
  ├─ blacklist exact-overlap check
  └─ PhalaDstackAttestor
       └─ report_hash를 TDX quote reportData에 바인딩
```

### 추가된 코드 seam

```text
clean_wallet/attestation.py
  MockAttestor             # local demo/default
  PhalaDstackAttestor      # CVM 내부 quote generation; dstack-sdk + dstack.sock 필요
  PhalaDstackVerifier      # CVM 외부 report verification; Phala verify API 사용
  build_attestor           # proof generation factory
  build_verifier           # report verification factory

clean_wallet/scanner.py
  FixtureScanner           # current MVP scanner
  ZcashViewingKeyScanner   # decrypts viewing capability, fetches lightwalletd blocks, invokes scanner command fail-closed

zcash_scanner/
  clean-wallet-zcash-scanner  # Rust librustzcash scanner command for encrypted UFVK/FVK input

clean_wallet/service.py
  /health
  /info
  /measurement
  /attestation?report_hash=<sha256hex>
  /attestation?purpose=enclave-key&nonce=<client_nonce>
  /proof                  # Phala/default: encrypted viewing capability + lightwalletd only; mock: fixture allowed
```

### Phala 배포 파일

```text
Dockerfile
  - Python service image
  - dstack-sdk 설치

docker-compose.phala.yml
  - registry image digest placeholder; replace before Phala deploy
  - /var/run/dstack.sock mount
  - CLEAN_WALLET_ATTESTOR=phala
```

Phala CVM service 실행:

```sh
python3 -m clean_wallet.service
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/info
```

로컬 mock service 실행:

```sh
CLEAN_WALLET_ATTESTOR=mock python3 -m clean_wallet.service
curl http://127.0.0.1:8080/health
```

사용자가 viewing capability를 암호화하기 전에는 enclave key attestation을 받아야 합니다.
현재 코드는 public key descriptor와 quote binding 계약만 구현했으며, TEE-local private key/public key가 설정되지 않으면 `encryption_key.status`가
`unconfigured`입니다. 설정 시 encrypted viewing capability를 enclave 내부에서 복호화합니다.

```sh
curl "http://127.0.0.1:8080/measurement"
curl "http://127.0.0.1:8080/attestation?purpose=enclave-key&nonce=client-nonce-1"
```

환경변수:

```text
CLEAN_WALLET_ENCLAVE_KEY_ID=attested-key-1
CLEAN_WALLET_ENCLAVE_PUBLIC_KEY=<x25519-public-key-or-provider-output>
# preferred for Phala PoC: process-local key, private key never in compose/env
CLEAN_WALLET_AUTO_GENERATE_ENCLAVE_KEY=1
CLEAN_WALLET_ENCLAVE_KEY_SCHEME=x25519-chacha20poly1305-v0

# optional local deterministic provisioning only
CLEAN_WALLET_ENCLAVE_PRIVATE_KEY_B64=<base64-raw-32-byte-x25519-private-key>
```

`/attestation?purpose=enclave-key` 응답의 `attestation_payload_hash`가 Phala/dstack
quote `reportData`에 바인딩됩니다. client는 measurement/compose hash allowlist와
이 hash binding을 확인한 뒤 해당 public key로 IVK/FVK/UFVK를 암호화해야 합니다.

### FVK/UFVK/UIVK 제공 helper

FVK/UFVK/UIVK는 채팅, shell argument, `/proof` plaintext field로 제공하지 않습니다.
`scripts/encrypt_viewing_capability.py`는 attested enclave public key를 가져온 뒤 터미널에서 숨김 입력으로 viewing capability를 받아 encrypted `/proof` payload를 만듭니다.

Payload만 생성:

```sh
python3 scripts/encrypt_viewing_capability.py \
  --service-url http://127.0.0.1:8080 \
  --capability-type fvk \
  --blacklist-manifest artifacts/blacklist.json \
  --network mainnet \
  --pool orchard \
  --block-start 3360000 \
  --block-end 3363864 \
  --viewing-scope-id local-test-scope \
  --lightwalletd-endpoint https://lightwalletd.mainnet.cipherscan.app:443 \
  --out /tmp/clean-wallet-proof-payload.json
```

생성 후 직접 submit:

```sh
python3 scripts/encrypt_viewing_capability.py \
  --service-url http://127.0.0.1:8080 \
  --submit \
  --capability-type fvk \
  --blacklist-manifest artifacts/blacklist.json \
  --network mainnet \
  --pool orchard \
  --block-start 3360000 \
  --block-end 3363864
```

테스트/자동화에서만 `--viewing-key-stdin`을 사용할 수 있습니다. 실제 사용 시에는 shell history에 남지 않도록 기본 hidden prompt를 사용하세요. helper output에는 `ciphertext`, `ephemeral_public_key`, `nonce`만 포함되고 plaintext FVK는 포함되지 않습니다.

Phala PoC compose는 `CLEAN_WALLET_AUTO_GENERATE_ENCLAVE_KEY=1`을 사용합니다. 이 경우 private key는 프로세스 메모리에만 있고, `/attestation?purpose=enclave-key`는 public key와 `key_origin=runtime-ephemeral`만 reportData에 바인딩합니다. CVM이 재시작되면 이전 encrypted payload는 만료되므로 attestation/public key를 다시 받아 암호화해야 합니다.

`/proof`는 `CLEAN_WALLET_ATTESTOR=mock` 또는 `CLEAN_WALLET_ALLOW_FIXTURE_PROOFS=1`에서만 기존 fixture payload를 지원합니다. Phala/default mode에서는 아래 production 계약만 허용하며, scanner command가 설정되지 않았거나 실패하면 결과는 의도적으로 `ERROR`입니다.

```json
{
  "request": {
    "network": "regtest",
    "pool": "orchard",
    "block_range": {"start": 100, "end": 110},
    "viewing_scope_id": "client-scope-id-not-returned-in-report",
    "encrypted_viewing_capability": {
      "scheme": "x25519-chacha20poly1305-v0",
      "capability_type": "ufvk",
      "ciphertext": "<base64 ciphertext+tag>",
      "key_id": "attested-key-1",
      "ephemeral_public_key": "<base64 requester x25519 public key>",
      "nonce": "<base64 12-byte nonce>"
    },
    "chain_source": {
      "type": "lightwalletd",
      "endpoint": "https://lightwalletd.example:9067"
    }
  },
  "blacklist_manifest": {}
}
```

보안 경계:

- `viewing_key`, `ivk`, `uivk`, `fvk`, `ufvk`, `seed_phrase`, `mnemonic` 같은 plaintext key field는 거부합니다.
- ciphertext, raw scope id, decrypted notes/addresses/amounts/tx metadata는 report에 넣지 않습니다.
- 현재 real PoC는 `lightwalletd` chain source만 받습니다. `full_node_rpc`/`compact_block_bundle`은 실제 fetch/검증 경로가 없으므로 허용하지 않습니다.

Phala CVM용 compose 핵심:

```yaml
services:
  clean-wallet:
    image: ghcr.io/YOUR_ORG/clean-wallet-mvp2@sha256:PINNED_DIGEST
    ports:
      - "8080:8080"
    volumes:
      - /var/run/dstack.sock:/var/run/dstack.sock
    environment:
      - CLEAN_WALLET_ATTESTOR=phala
```

배포 예시:

```sh
npm install -g phala
phala login
# first push Dockerfile image to a registry and replace the image digest in docker-compose.phala.yml
phala deploy -c docker-compose.phala.yml -n clean-wallet-mvp2
phala logs --cvm-id clean-wallet-mvp2
```

### 아직 구현되지 않은 production 항목

- RTMR3 event log replay 및 compose-hash allowlist 운영 정책
- pool-specific raw IVK byte formats. 현재 Rust scanner는 ZIP-316 UFVK/FVK와 UIVK 형태를 지원합니다.

scanner command가 없거나, 지원되지 않는 capability type이거나, scan이 실패하면 `ZcashViewingKeyScanner`는 `ERROR`를 반환합니다. 실제 trial-decrypt 없이 PASS가 나오는 것을 방지하기 위함입니다.


## Real Zcash scanner command contract

Production `/proof` no longer accepts prover-submitted owned commitments in Phala/default mode. The encrypted viewing capability path does this inside the enclave/container:

1. Decrypt `request.encrypted_viewing_capability` using `CLEAN_WALLET_ENCLAVE_PRIVATE_KEY_B64`.
2. Fetch compact blocks from `request.chain_source.endpoint` using lightwalletd `GetBlockRange`.
3. Invoke `CLEAN_WALLET_ZCASH_SCANNER_CMD` inside the same container/enclave.
4. Compare the scanner-produced owned note commitments against the signed blacklist.
5. Generate PASS/FAIL/ERROR report and bind its `report_hash` into TDX quote `reportData`.

The scanner command receives JSON on stdin:

```json
{
  "schema_version": "clean-wallet-zcash-scan-request-v0",
  "viewing_key": "uview... UFVK/FVK or uivk... UIVK string",
  "viewing_capability_type": "ufvk | fvk | uivk | ivk",
  "key_id": "attested key id",
  "network": "mainnet | testnet",
  "pool": "orchard | sapling",
  "block_range": {"start": 0, "end": 0},
  "compact_blocks": []
}
```

It must return JSON on stdout:

```json
{"owned_commitments": ["64-hex-character-note-commitment"]}
```

If the command is missing, fails, emits invalid JSON, or emits invalid commitments, `/proof` returns `ERROR`; it cannot return `PASS`.

Implementation note: `zcash_scanner/` uses current `zcash_client_backend 0.22` + `zcash_keys 0.13` APIs. It supports UFVK/FVK and UIVK strings, scans compact blocks, and returns the on-chain `cmu`/`cmx` commitments corresponding to decrypted wallet outputs. For UIVK/IVK mode it builds `ScanningKeys::new` with nullifier-less Sapling/Orchard IVK scanners, so commitment discovery works without exposing spending/nullifier authority.
