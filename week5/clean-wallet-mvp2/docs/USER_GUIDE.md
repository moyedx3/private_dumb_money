# Clean Wallet PoC User Guide

이 문서는 GitHub 이용자가 Clean Wallet PoC를 직접 실행해 보는 방법을 설명합니다.

Clean Wallet PoC는 사용자가 제출한 Zcash viewing capability(UFVK/FVK/UIVK/IVK)를 서버가 그대로 신뢰하지 않고, **attested enclave 안에서 직접 블록 데이터를 가져와 스캔**한 뒤 blacklist commitment와 겹치는지 확인하는 실험입니다.

> ⚠️ UFVK/FVK/UIVK는 spending key는 아니지만 지갑 활동을 볼 수 있는 민감한 viewing material입니다. 채팅, GitHub issue, shell argument, 로그에 붙여넣지 마세요. 이 레포의 helper는 기본적으로 hidden prompt로 입력받습니다.

## 결과 의미

`PASS`는 다음만 의미합니다.

```text
제출된 network / pool / block range / blacklist manifest 안에서
viewing capability로 발견된 owned commitment와 blacklist commitment의 exact overlap이 없다.
```

`PASS`가 의미하지 않는 것:

- 전역적인 “무죄” 증명
- 사용자가 모든 지갑/account를 제출했다는 증명
- identity ownership 증명
- taint ancestry가 없다는 증명
- 해당 wallet에 다른 거래가 없다는 증명

scanner, lightwalletd, attestation 경계에서 문제가 생기면 `ERROR`가 나오며, `ERROR`는 clean-wallet claim이 아닙니다.

## 빠른 실행: live Phala service에 UFVK 제출

maintainer가 제공한 Phala service URL이 있다면 다음처럼 실행합니다.

```bash
python3 scripts/encrypt_viewing_capability.py \
  --service-url <PHALA_SERVICE_URL> \
  --submit \
  --capability-type ufvk \
  --blacklist-manifest artifacts/blacklist.json \
  --network mainnet \
  --pool orchard \
  --block-start 3363067 \
  --block-end 3363067 \
  --lightwalletd-endpoint https://lightwalletd.mainnet.cipherscan.app:443
```

프롬프트가 뜨면 UFVK를 입력합니다.

```text
Enter Zcash UFVK / viewing capability:
```

입력값은 화면에 보이지 않는 것이 정상입니다.

## 출력 해석

예시:

```json
{
  "result": "PASS",
  "claim": "No exact commitment overlap within declared scope",
  "network": "mainnet",
  "pool": "orchard",
  "block_range": {"start": 3363067, "end": 3363067},
  "blacklist_manifest_hash": "...",
  "measurement": "...",
  "report_hash": "...",
  "signature_or_quote": {"report_data": "..."}
}
```

확인할 것:

1. `result`
   - `PASS`: 해당 범위/blacklist에서 overlap 없음
   - `FAIL`: blacklist commitment와 overlap 있음
   - `ERROR`: scanner/attestation/입력 경계 오류. clean claim 아님
2. `block_range`
   - 내가 의도한 블록 범위와 같은지 확인
3. `blacklist_manifest_hash`
   - 검증자가 합의한 blacklist manifest와 같은지 확인
4. `measurement`
   - 허용된 enclave measurement인지 확인
5. `report_hash == signature_or_quote.report_data`
   - report가 TDX quote에 바인딩됐는지 확인

## 로컬에서 owned commitment artifact 뽑기

public `/proof` 결과는 privacy 때문에 owned commitment 목록을 숨깁니다. 테스트용 데이터를 만들 때만 로컬 helper를 사용하세요.

```bash
python3 scripts/getArtifact.py \
  --capability-type ufvk \
  --network mainnet \
  --pool orchard \
  --block-start 3363067 \
  --block-end 3363067 \
  --lightwalletd-endpoint https://lightwalletd.mainnet.cipherscan.app:443 \
  --out artifacts/owned-commitments-3363067.json
```

출력:

```text
owned_commitment_count=1
first_owned_commitment=<commitment>
default_unified_address=<u1... address>
```

이 helper는 로컬 debug 전용입니다. public attested proof path에서는 owned commitment를 반환하지 않습니다.

## FAIL 테스트용 blacklist 만들기

로컬에서 찾은 첫 owned commitment를 blacklist manifest로 만들려면:

```bash
python3 scripts/getArtifact.py \
  --capability-type ufvk \
  --network mainnet \
  --pool orchard \
  --block-start 3363067 \
  --block-end 3363067 \
  --lightwalletd-endpoint https://lightwalletd.mainnet.cipherscan.app:443 \
  --out artifacts/owned-commitments-3363067.json \
  --blacklist-out artifacts/blacklist-owned-3363067.json
```

이후 proof 요청에서:

```bash
--blacklist-manifest artifacts/blacklist-owned-3363067.json
```

을 사용하면 해당 UFVK가 그 commitment를 decrypt할 수 있는 경우 `FAIL`이 나와야 합니다.

## 로컬 개발 검증

```bash
python3 -m unittest discover -s tests
python3 -m compileall -q clean_wallet scripts/encrypt_viewing_capability.py scripts/getArtifact.py
cargo check --manifest-path zcash_scanner/Cargo.toml -q
cargo build --release --manifest-path zcash_scanner/Cargo.toml -q
```

## 현재 제한

- `PASS`는 범위 제한 claim입니다. 전역 claim이 아닙니다.
- 실제 mainnet blacklist source 자동화는 포함되어 있지 않습니다.
- sender-side outgoing output 복원은 아직 별도 작업입니다.
- local `getArtifact.py`는 민감한 debug artifact를 만들 수 있으므로 결과 파일을 커밋하지 마세요.
