# Clean Wallet PoC Architecture

## 목표

증명자가 자신에게 유리한 commitment 목록을 조작해서 제출하는 것을 막기 위해, 검증 서버가 prover-submitted owned commitments를 받지 않습니다.

대신 flow는 다음과 같습니다.

```text
User Wallet
  └─ UFVK/FVK/UIVK/IVK를 hidden prompt에 입력
      ↓
Client Helper
  └─ /attestation에서 enclave public key + quote 확인
  └─ viewing capability를 X25519 + ChaCha20-Poly1305로 암호화
      ↓
Phala dstack TDX CVM
  └─ enclave-local private key로 복호화
  └─ lightwalletd에서 compact blocks 직접 fetch
  └─ Rust Zcash scanner로 owned commitments 추출
  └─ blacklist manifest와 exact set overlap 검사
  └─ report_hash를 TDX quote report_data에 바인딩
      ↓
Verifier
  └─ report result, block range, blacklist hash, measurement, quote binding 확인
```

## 주요 컴포넌트

```text
scripts/encrypt_viewing_capability.py
  Client helper. Attestation을 확인하고 viewing capability를 암호화해 /proof에 제출합니다.

clean_wallet/service.py
  HTTP service. Plaintext viewing key field를 거부하고 encrypted capability path만 production 기본값으로 허용합니다.

clean_wallet/enclave_key.py
  Enclave encryption key descriptor, runtime ephemeral key, decrypt boundary.

clean_wallet/lightwalletd.py
  lightwalletd gRPC compact block client. chainMetadata를 포함해 arbitrary height scan을 가능하게 합니다.

zcash_scanner/
  Rust scanner. zcash_client_backend + zcash_keys로 UFVK/FVK/UIVK를 decode하고 compact block output을 trial-decrypt합니다.

clean_wallet/proof.py
  PASS / FAIL / ERROR report 생성, blacklist root/hash, report_hash binding.

clean_wallet/attestation.py
  Mock attestor와 Phala dstack attestor adapter.
```

## Report model

- `PASS`: owned commitment와 blacklist commitment의 exact overlap이 없음
- `FAIL`: exact overlap이 있음
- `ERROR`: scanner, input, attestation, chain boundary 오류. clean claim 아님

## Privacy boundary

Public proof report에는 다음을 넣지 않습니다.

- UFVK/FVK/UIVK/IVK plaintext
- decrypted note plaintext
- owned commitment list
- wallet address list
- viewing scope id 원문

로컬 debug helper `scripts/getArtifact.py`만 owned commitments와 default unified address를 artifact로 저장할 수 있습니다.

## Chain metadata issue

Zcash scanner가 중간 block height에서 시작하려면 Sapling/Orchard note commitment tree size가 필요합니다. 이 PoC는 lightwalletd compact block의 `chainMetadata`를 Python converter와 Rust scanner까지 전달합니다.

예: block `3363067`에서 확인된 metadata:

```json
{
  "saplingCommitmentTreeSize": 73916603,
  "orchardCommitmentTreeSize": 50059617
}
```
