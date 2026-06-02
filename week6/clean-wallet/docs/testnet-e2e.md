# Testnet e2e 가이드

> 목적: 우리 Rust 사이드카 + Node 스캐너 흐름이 실 Zcash testnet 데이터에서 안 깨지는지
> 로컬/testnet에서 검증. (배포는 이미 완료 — 이건 사전 게이트가 아니라 로컬 검증 가이드다.)
>
> 배포된 CVM에 실 mainnet e2e를 돌리려면 → [ONBOARDING.md §2](../ONBOARDING.md).

## 0. 준비물

- 본 레포지토리 (`D:/zcash_auditor/clean-wallet`).
- Node 22+ (이미 설치됨).
- Rust 1.94+ (이미 설치됨).
- Zashi testnet 같은 외부 Zcash 지갑 — mnemonic import 와 송금 가능해야 함.
  - Zashi: <https://electriccoin.co/zashi/> (testnet 모드 제공).
  - 대안: Ywallet testnet, zecwallet-lite testnet.
- Testnet lightwalletd 엔드포인트 — 예: `https://lightwalletd.testnet.electriccoin.co:9067`
  (zechub.wiki 의 lightwallet-nodes 페이지 참고).

## 1. testnet 지갑 생성

```powershell
cd D:/zcash_auditor/clean-wallet/apps/zcash-scanner-rs
cargo run --release --bin gen-testnet-wallet
```

출력:
- `mnemonic` (BIP-39 24단어) — **비밀**. 메모장에 보관, 외부 지갑에 import.
- `ufvk` (uviewtest1...) — submit-ufvk 에 사용. 비밀이지만 송금 권한은 없음.
- `transparent_address_index_0` (tm...) — faucet 으로 받기용.
- `unified_address_default` (utest1...) — shielded 받기용.

같은 키를 재현하려면: `cargo run --bin gen-testnet-wallet -- --mnemonic "word1 word2 ..."`.

⚠️  여기서 출력된 mnemonic 으로 **실 메인넷 자금을 절대 받지 마라**. testnet 전용.

## 2. ZEC 받기 (faucet)

`transparent_address_index_0` 을 testnet faucet 에 넣고 받기:

- <https://faucet.zecpages.com> (community faucet, t-addr/z-addr 둘 다 가능)
- <https://faucet.testnet.z.cash> (공식 — 동작 여부 시점에 따라 변동)

받기 트랜잭션의 block height 를 메모. (보통 faucet 페이지가 보여준다.)

## 3. Zashi 등에 mnemonic import

- Zashi testnet 앱 설치 → "Restore wallet" → 24단어 입력.
- 동기화 완료까지 대기 (light client 기준 1~5분).
- transparent balance 가 보이는지 확인.

## 4. outgoing 송금 (외부 지갑에서)

Zashi 에서 임의의 testnet 주소로 작게 송금 (예: 0.001 ZEC).

- 보낼 주소는 임의로 — 자기 자신이 만든 다른 testnet 주소도 가능.
- 송금 블록 height 메모 (Zashi 의 tx detail 에 보임).
- 이 송금이 **우리 scanner 가 잡아내야 할 outgoing 거래**.

## 5. 로컬 스캐너 띄우기

```powershell
cd D:/zcash_auditor/clean-wallet
# 시뮬레이션 모드 (HTTP, TDX 없음)
$env:ATTESTATION_MODE = "simulated"
node apps/scanner/src/server.ts
```

`[scanner] attested scanner — http://localhost:8080` 가 떠야 함.

## 6. submit-ufvk 로 스캔

별도 터미널에서:

```powershell
cd D:/zcash_auditor/clean-wallet
$ufvk = "uviewtest1..."        # 1번에서 출력된 UFVK
$lwd  = "https://lightwalletd.testnet.electriccoin.co:9067"

# block range 는 송금 height 를 포함해야 함. 50~100 블록 권장 (느림).
$start = 2500000
$end   = 2500050

# UFVK 는 stdin 으로 (process list 노출 방지). 로컬 sim 모드라 --no-verify 필요.
$ufvk | node apps/scanner/tools/submit-ufvk.ts `
    --host http://localhost:8080 `
    --network test `
    --lwd-url $lwd `
    --start $start --end $end `
    --no-verify
```

응답으로 screening artifact JSON 이 출력됨. 안에 `derivedRecords` 가 비어 있으면
(혹은 송금이 잡히지 않으면) 가능한 원인:

- block range 가 송금 block 을 포함 안 함 → 범위 조정.
- lightwalletd 가 응답을 안 보냄/연결 실패 → 다른 testnet lwd 시도.
- Rust 사이드카 빌드 안 됨 → `cargo build --release` 먼저.
- shielded 송금이라 transparent path 가 아닌 sapling/orchard 로 잡힘 — `pool` 필드 확인.

## 7. 추가 옵션 시험

### 7.1 PoW 헤더 검증 (C1)
ScanRequest 의 `verify_pow: true` 를 켜면 각 블록 헤더의 Equihash + target 을 검증한다.
대부분의 공개 lightwalletd 는 `CompactBlock.header` 를 빈 채로 보내므로 throw 가
정상 — header 가 들어오는 lightwalletd 인지 확인하는 용도.

submit-ufvk 가 verify_pow 를 노출 안 한 상태라 직접 server 의 `/scan` 본문에 추가:
```json
{ "mode": "real", "ufvk": "...", "salt": "...",
  "chainSource": {"kind":"lightwalletd","url":"...","network":"test"},
  "scanRange": {"startHeight":N,"endHeight":N+50},
  "verify_pow": true }
```

### 7.2 시작 앵커 (C1)
`start_anchor_hash_hex` 로 첫 블록의 prev_hash 를 강제. testnet block explorer 에서
`start - 1` 의 hash 를 hex(LE) 로 가져와 넣으면 시작점 위조도 차단된다.

## 8. 보고서

다음 정보가 있으면 다음 디버그 라운드에서 빠르게 도와줄 수 있음:

- 사용한 lightwalletd 엔드포인트.
- start/end 블록 height.
- 송금 tx 의 hash (또는 block explorer 링크).
- scanner 응답 JSON (UFVK 빼고).
- 에러가 났으면 server.ts 의 stderr 와 stdout.
