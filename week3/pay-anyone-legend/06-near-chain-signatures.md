# §1.6 NEAR Chain Signatures (NEAR Chain Signatures / MPC)

> **Cross-reference:**
> - §1.5(`05-one-click-bridge.md`)에서 확립된 **two-hop 설계**: 1Click → `swapWallet`(Chain Signatures 파생 EVM 주소) → x402 → 최종 수신자.
> - 이 문서는 그 `swapWallet`이 *어떻게 생성되고*, 그 주소 소유의 *서명이 어떻게 수행되는지*를 설명한다.
> - 서명된 트랜잭션이 x402 프로토콜에서 어떻게 소비되는지는 §1.7에서 다룬다.

---

## 목적 (Purpose)

NEAR Chain Signatures 서브시스템은 **PAL 서버가 destination chain(Base)의 private key를 직접 보유하지 않고도 EVM 트랜잭션에 서명할 수 있도록 해준다.** NEAR MPC(Multi-Party Computation) 네트워크의 `v1.signer` 컨트랙트가 서명 연산을 분산 실행하며, PAL 서버는 오직 NEAR proxy account의 private key만 환경 변수로 보유한다. 이를 통해 x402 결제에 필요한 USDC `transferWithAuthorization` 서명이 서버 사이드에서 자동으로 이뤄지며, `swapWallet` EVM 주소는 MPC 공개키 파생을 통해 결정론적으로 도출된다.

---

## 파일과 함수 (Files & functions)

| 파일 | 라인 | 함수/심볼 | 역할 |
|------|------|-----------|------|
| `lib/chainSig.ts` | 1–53 | 모듈 초기화 | 환경 변수 로드, `ChainSignatureContract`, `EVM` adapter, NEAR `Account` 인스턴스 생성 |
| `lib/chainSig.ts` | 17 | `NEAR_PROXY_CONTRACT` | `process.env.NEAR_PROXY_CONTRACT` (proxy 호출 여부 플래그; default `'false'`) |
| `lib/chainSig.ts` | 18 | `MPC_PATH` | **하드코딩** `'base-1'` — 모든 서명에 동일한 derivation path 사용 |
| `lib/chainSig.ts` | 19 | `accountId` | `process.env.NEAR_PROXY_ACCOUNT_ID` |
| `lib/chainSig.ts` | 20 | `networkId` | `process.env.NEXT_PUBLIC_NEAR_NETWORK \|\| 'mainnet'` |
| `lib/chainSig.ts` | 21 | `contractId` | `process.env.NEAR_PROXY_CONTRACT_ID \|\| 'v1.signer'` |
| `lib/chainSig.ts` | 24–27 | `chainSignatureContract` | `new contracts.ChainSignatureContract({ networkId, contractId })` |
| `lib/chainSig.ts` | 29 | `privateKey` | `process.env.NEAR_PROXY_PRIVATE_KEY as KeyPairString` |
| `lib/chainSig.ts` | 33–39 | `keyPair`, `signer`, `provider`, `account` | `KeyPairSigner` + `JsonRpcProvider`(FastNEAR) + `Account` 구성 |
| `lib/chainSig.ts` | 44–47 | `publicClient` | viem `createPublicClient` — Base mainnet RPC |
| `lib/chainSig.ts` | 50–53 | `evmChain` | `new chainAdapters.evm.EVM({ publicClient, contract })` |
| `lib/chainSig.ts` | 87–103 | `deriveAddressAndPublicKey(path?)` | `evmChain.deriveAddressAndPublicKey(accountId, 'base-1')` 호출 → `{ address, publicKey }` 반환 |
| `lib/chainSig.ts` | 112–117 | `getEthereumAddressFromProxyAccount(path?)` | `deriveAddressAndPublicKey()` 래퍼 — address 문자열만 반환 |
| `lib/chainSig.ts` | 128–201 | `signTypedDataWithChainSignature(domain, types, value)` | EIP-712 typed data 해시 → `chainSignatureContract.sign()` → `(v, r, s)` 반환 |
| `lib/chainSig.ts` | 147–152 | `chainSignatureContract.sign(...)` | MPC 서명 요청 #1 (EIP-712 authorization hash) |
| `lib/chainSig.ts` | 167–186 | v 값 복원 로직 | recovery_id 0/1 → `v = recoveryId + 27` 변환 |
| `lib/chainSig.ts` | 210–401 | `signX402TransactionWithChainSignature(quote)` | **메인 엔트리포인트** — EVM tx 전체를 조립·서명·브로드캐스트 후 tx hash 반환 |
| `lib/chainSig.ts` | 223 | - | `evmChain.deriveAddressAndPublicKey(accountId, MPC_PATH)` — swapWallet address 재파생 |
| `lib/chainSig.ts` | 280 | - | `signTypedDataWithChainSignature(...)` — EIP-712 authorization 서명 (MPC 호출 #1) |
| `lib/chainSig.ts` | 356–363 | - | `evmChain.prepareTransactionForSigningLegacy(...)` — legacy tx 해시 생성 |
| `lib/chainSig.ts` | 372–377 | - | `chainSignatureContract.sign(...)` — MPC 서명 요청 #2 (tx 해시) |
| `lib/chainSig.ts` | 388–391 | - | `evmChain.finalizeTransactionSigningLegacy(...)` — 서명 삽입, 직렬화 |
| `lib/chainSig.ts` | 394–396 | - | `publicClient.sendRawTransaction(...)` — Base mainnet 브로드캐스트 |
| `lib/kdf.ts` | 1–11 | imports | `elliptic`, `bn.js`, `keccak`, `hash.js`, `bs58check`, `xrpl`, `bech32`, `js-sha3`, `near-api-js/serialize` |
| `lib/kdf.ts` | 19–24 | `najPublicKeyStrToUncompressedHexPoint()` | NEAR 공개키 문자열 → `'04' + hex` 형식 변환 |
| `lib/kdf.ts` | 26–50 | `deriveChildPublicKey(parent, signerId, path)` | **secp256k1 타원곡선 child key 파생** — `sha3_256` 스칼라 생성 후 포인트 덧셈 |
| `lib/kdf.ts` | 73–80 | `uncompressedHexPointToEvmAddress()` | keccak256 해시 → 마지막 20 bytes → EVM 주소 |
| `lib/kdf.ts` | 82–107 | `uncompressedHexPointToBtcAddress()` | SHA-256 → RIPEMD-160 → Base58Check — Bitcoin/Dogecoin 주소 |
| `lib/kdf.ts` | 109–180 | `generateAddress({ publicKey, accountId, path, chain, bech32Prefix })` | 멀티체인 주소 파생 — `ethereum`, `btc`, `bitcoin`, `dogecoin`, `xrpLedger`, `cosmos-ethermint`, `cosmos`, `base` 지원 |
| `lib/kdf.ts` | 163–165 | `cosmos` case | `bech32.toWords()` + `bech32.encode()` 사용 (Cosmos 주소용) |
| `lib/kdf.ts` | 222–251 | `uncompressedHexPointToNearImplicit()` | **WIP / 미사용** — NEAR implicit account 파생 (주석: "WARNING WIP DO NOT USE") |
| `lib/near.ts` | 1–11 | imports | `near-api-js`, `bn.js`, `dotenv` |
| `lib/near.ts` | 13–20 | env vars | `NEAR_PROXY_CONTRACT_ID`, `NEAR_PROXY_ACCOUNT_ID`, `NEAR_PROXY_PRIVATE_KEY` 등 로드 |
| `lib/near.ts` | 26–27 | `keyStore` | `new keyStores.InMemoryKeyStore()` — key를 메모리에 저장 |
| `lib/near.ts` | 33–40 | `config` | `networkId: 'mainnet'`, nodeUrl `https://rpc.mainnet.near.org` 하드코딩 |
| `lib/near.ts` | 41–42 | `near`, `account` | `new Near(config)`, `new Account(near.connection, accountId)` |
| `lib/near.ts` | 43–144 | `sign(payload, path)` | `account.functionCall()` → `v1.signer.sign` 직접 호출 (legacy 방식; `lib/chainSig.ts`는 chainsig.js를 대신 사용) |
| `lib/near.ts` | 59–63 | attached deposit | `nearAPI.utils.format.parseNearAmount('1')` — 1 NEAR 첨부 |
| `lib/near.ts` | 116–124 | gas | `new BN('300000000000000')` — 300 TGas |
| `lib/near.ts` | 128–143 | 응답 파싱 | Base64 디코딩 → `{ big_r, s, recovery_id }` → `{ r, s, v }` |
| `scripts/test-sign-x402-transaction.js` | 32 | - | `import('../lib/chainSig')` 동적 임포트 |
| `scripts/test-sign-x402-transaction.js` | 54 | - | `getEthereumAddressFromProxyAccount()` 호출 |
| `scripts/test-sign-x402-transaction.js` | 64–76 | `exampleQuote` | 테스트용 quote: `payTo`, `maxAmountRequired: '0.1'`, `deadline = now+3600`, `nonce = Date.now()` |
| `scripts/test-sign-x402-transaction.js` | 103–105 | - | `signX402TransactionWithChainSignature(exampleQuote)` 실행 |
| `app/api/relayer/register-deposit/route.ts` | 4 | import | `getEthereumAddressFromProxyAccount` import |
| `app/api/relayer/register-deposit/route.ts` | 47 | - | `swapWallet = await getEthereumAddressFromProxyAccount()` → 1Click quote의 `recipientAddress`로 사용 |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 125 | - | `const { signX402TransactionWithChainSignature } = await import('@/lib/chainSig')` 동적 import |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 127–132 | - | `signX402TransactionWithChainSignature({ payTo, maxAmountRequired, deadline, nonce })` 호출 |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 135–140 | - | 반환된 tx hash를 `signedPayload` 컬럼에 저장 |

---

## 연결 (Wiring)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│              NEAR Chain Signatures 서브시스템 데이터 흐름                    │
└─────────────────────────────────────────────────────────────────────────────┘

[환경 변수]
  NEAR_PROXY_ACCOUNT_ID ─────────────────────────────────────────┐
  NEAR_PROXY_PRIVATE_KEY ──── KeyPair.fromString() ──────────────┤
  NEAR_PROXY_CONTRACT_ID (default: v1.signer) ───────────────────┤
  NEXT_PUBLIC_NEAR_NETWORK (default: mainnet) ───────────────────┘
                                                                   │
                                                                   ▼
                                          lib/chainSig.ts (모듈 초기화)
                                          ┌────────────────────────────────┐
                                          │ chainSignatureContract          │
                                          │   (contracts.ChainSignature-   │
                                          │    Contract)                    │
                                          │ evmChain (chainAdapters.EVM)   │
                                          │ account (NEAR Account)          │
                                          └────────────────────────────────┘
                                                       │
                    ┌──────────────────────────────────┤
                    │                                  │
                    ▼                                  ▼
  [§1.5 register-deposit 단계]            [§1.4 cronjob 단계]
  getEthereumAddressFromProxyAccount()    signX402TransactionWithChainSignature()
  ↓                                       ↓
  evmChain.deriveAddressAndPublicKey      (1) EIP-712 authorization 서명 (MPC #1)
  (accountId, 'base-1')                  (2) legacy EVM tx 준비 + 해시
  ↓                                       (3) EVM tx 서명 (MPC #2)
  swapWallet 주소                         (4) Base mainnet 브로드캐스트
  → 1Click quote recipientAddress         ↓
  → §1.5 "two-hop의 중간 지점"           Ethereum tx hash
                                          → Supabase signedPayload 컬럼
                                          → §1.7 (content unlock)

[NEAR MPC 네트워크]
  v1.signer.sign({ payload, path, key_version })
  ← { big_r, s, recovery_id }
```

- **Inputs:**
  - `getEthereumAddressFromProxyAccount()`: 입력 없음 (환경 변수에서 accountId, path `'base-1'` 고정)
  - `signX402TransactionWithChainSignature(quote)`:
    - `quote.payTo` — x402 수신자 EVM 주소 (§1.4 `tracking.recipient`에서 주입)
    - `quote.maxAmountRequired` — USDC 금액 문자열 (예: `"0.1"`)
    - `quote.deadline` — Unix timestamp (초) — 항상 `Date.now()/1000 + 3600` 재계산
    - `quote.nonce` — 문자열 hex (예: `"0x${Date.now().toString(16)}"`)

- **Outputs:**
  - `getEthereumAddressFromProxyAccount()` → EVM 주소 문자열 (`swapWallet`)
  - `signX402TransactionWithChainSignature()` → Ethereum tx hash 문자열 (예: `0x...`) — **브로드캐스트까지 완료된 상태**

- **Dependencies (internal):**
  - `lib/kdf.ts` — 저수준 공개키 파생 유틸리티 (단, `lib/chainSig.ts`의 production path는 `chainsig.js` SDK를 대신 사용; `lib/kdf.ts`는 `lib/chainSig.ts`에서 직접 import하지 않음)
  - `lib/near.ts` — legacy `sign()` 함수 정의 (단, `lib/chainSig.ts` production path는 chainsig.js `ChainSignatureContract.sign()`을 사용; `lib/near.ts`는 독립적으로 존재)

- **Dependencies (external):**
  - `v1.signer` (NEAR MPC 컨트랙트) — 모든 실제 서명 연산의 실행자
  - `chainsig.js@^1.1.14` — MPC 컨트랙트 인터페이스 + EVM tx 어댑터
  - `near-api-js@^0.44.2` — NEAR 계정/키스토어 (lib/near.ts), `base_decode`/`base_encode` (lib/kdf.ts)
  - `@near-js/accounts`, `@near-js/crypto`, `@near-js/providers`, `@near-js/signers` — lib/chainSig.ts가 사용하는 최신 패키지
  - `ethers@^5.7.2` — EIP-712 해시, BigNumber, ABI 인코딩, 주소 체크섬
  - `viem@^2.0.0` — Base mainnet RPC 클라이언트 (`createPublicClient`), `sendRawTransaction`
  - `elliptic@^6.6.1` — secp256k1 타원곡선 연산 (lib/kdf.ts)
  - `bn.js@^5.2.2` — 큰 정수 연산 (lib/near.ts gas/deposit 계산)
  - `keccak@^3.0.4` — keccak256 해시 (lib/kdf.ts EVM 주소 생성)
  - `js-sha3@^0.9.3` — sha3_256 (lib/kdf.ts MPC epsilon 파생 스칼라 생성)
  - `near-seed-phrase@^0.2.1` — WIP 함수에서만 사용 (lib/kdf.ts:236)
  - `hash.js` — RIPEMD-160 (lib/kdf.ts Bitcoin 주소)
  - `bs58check@^4.0.0` — Base58Check 인코딩 (lib/kdf.ts Bitcoin 주소)
  - `bech32@^2.0.0` — Bech32 인코딩 (lib/kdf.ts Cosmos 주소)
  - `xrpl@^4.4.3` — XRP Ledger 주소 파생 (lib/kdf.ts)

---

## 라이브러리 (Libraries)

| Package | Version | 사용 위치 | 용도 |
|---------|---------|-----------|------|
| `chainsig.js` | `^1.1.14` | `lib/chainSig.ts:7` | MPC 컨트랙트 호출 + EVM tx 어댑터 |
| `ethers` | `^5.7.2` | `lib/chainSig.ts:4` | EIP-712 해시, ABI 인코딩, BigNumber, 주소 체크섬 |
| `viem` | `^2.0.0` | `lib/chainSig.ts:5,6` | Base RPC 클라이언트, `sendRawTransaction` |
| `near-api-js` | `^0.44.2` | `lib/near.ts:1`, `lib/kdf.ts:1` | NEAR 계정/키스토어/트랜잭션 (legacy), serialize 유틸 |
| `@near-js/accounts` | (chainsig.js 내 전이) | `lib/chainSig.ts:8` | `Account` 클래스 |
| `@near-js/crypto` | (chainsig.js 내 전이) | `lib/chainSig.ts:9` | `KeyPair`, `KeyPairString` |
| `@near-js/providers` | (chainsig.js 내 전이) | `lib/chainSig.ts:11` | `JsonRpcProvider` |
| `@near-js/signers` | (chainsig.js 내 전이) | `lib/chainSig.ts:12` | `KeyPairSigner` |
| `elliptic` | `^6.6.1` | `lib/kdf.ts:2` | secp256k1 child key 파생 |
| `bn.js` | `^5.2.2` | `lib/near.ts`, `lib/kdf.ts` | 큰 정수 연산 (gas, 포인트 좌표) |
| `keccak` | `^3.0.4` | `lib/kdf.ts:4` | keccak256 → EVM 주소 생성 |
| `js-sha3` | `^0.9.3` | `lib/kdf.ts:9` | sha3_256 → MPC epsilon 스칼라 생성 |
| `near-seed-phrase` | `^0.2.1` | `lib/kdf.ts:14` | WIP implicit account 파생 (미사용) |
| `hash.js` | (indirect) | `lib/kdf.ts:5` | RIPEMD-160 (Bitcoin 주소) |
| `bs58check` | `^4.0.0` | `lib/kdf.ts:6` | Base58Check 인코딩 (Bitcoin 주소) |
| `bech32` | `^2.0.0` | `lib/kdf.ts:8` | Bech32 인코딩 (Cosmos 주소) |
| `xrpl` | `^4.4.3` | `lib/kdf.ts:7` | XRP Ledger 주소 파생 |

---

## 워크스루 — happy path (서명 실행 흐름)

아래는 `signX402TransactionWithChainSignature()` 함수를 중심으로 한 완전한 실행 경로다.

**사전 조건:** 1Click swap 상태가 `SUCCESS`로 전환됨 (`cronjob-check-deposits/route.ts:47`)

```
[1] NEAR proxy account 로드
    lib/chainSig.ts:19   accountId = process.env.NEAR_PROXY_ACCOUNT_ID
    lib/chainSig.ts:29   privateKey = process.env.NEAR_PROXY_PRIVATE_KEY
    lib/chainSig.ts:33   keyPair = KeyPair.fromString(privateKey)
    lib/chainSig.ts:34   signer = new KeyPairSigner(keyPair)
    lib/chainSig.ts:35   provider = new JsonRpcProvider({ url: 'https://rpc.mainnet.fastnear.com' })
    lib/chainSig.ts:39   account = new Account(accountId, provider, signer)
    ── 모듈 로드 시 단 한번 실행; 이후 모든 호출이 같은 account 인스턴스를 재사용 ──

[2] Derivation path 결정
    lib/chainSig.ts:18   const MPC_PATH = 'base-1'   ← 하드코딩
    ── 모든 sign 호출은 동일한 path 'base-1'을 사용 ──

[3] EVM 주소 파생 (swapWallet address)
    lib/chainSig.ts:223  const { address } = await evmChain.deriveAddressAndPublicKey(accountId, MPC_PATH)
    ── chainsig.js가 내부적으로 NEAR MPC 컨트랙트에서 공개키를 조회하고,
       (accountId, 'base-1') 경로의 secp256k1 child key를 계산한 뒤
       keccak256(uncompressedPubKey)[12:] 로 EVM 주소를 도출 ──
    ── 이 address가 §1.5에서 1Click quote의 recipientAddress로 사용된 swapWallet ──

[4] EIP-712 domain + types + value 구성
    lib/chainSig.ts:234–265
      domain = { name: 'USD Coin', version: '2', chainId: 8453, verifyingContract: USDC_CONTRACT }
      types = { TransferWithAuthorization: [ from, to, value, validAfter, validBefore, nonce ] }
      authorizationValue = {
        from: address (swapWallet),
        to: quote.payTo (x402 수신자),
        value: amountInWei (USDC 6 decimals),
        validAfter: 0,
        validBefore: deadline (Unix sec),
        nonce: hexZeroPad(quote.nonce, 32),
      }
    USDC_CONTRACT = '0x833589fcd6edb6e08f4c7c32d4f71b54bda02913' (Base mainnet USDC)

[5] EIP-712 authorization hash 생성 + MPC 서명 #1
    lib/chainSig.ts:138   hash = ethers.utils._TypedDataEncoder.hash(domain, types, authorizationValue)
    lib/chainSig.ts:139   hashBytes = ethers.utils.arrayify(hash)
    lib/chainSig.ts:147   signature = await chainSignatureContract.sign({
                            payloads: [Array.from(hashBytes)],
                            path: 'base-1',
                            keyType: 'Ecdsa',
                            signerAccount: account,
                          })
    ── NEAR cross-contract call: proxy account → v1.signer.sign()
       300 TGas 첨부 (lib/near.ts:116 기준)
       1 NEAR deposit (lib/near.ts:59)
       응답: { big_r: { affine_point: '04...' }, s: { scalar: '...' }, recovery_id: 0|1 }
    lib/chainSig.ts:167–186  recovery_id → v = recoveryId + 27 변환

[6] EIP-712 서명 검증 (ecrecover)
    lib/chainSig.ts:283–300
      recoveredAddress = ethers.utils.recoverAddress(hash, { r, s, v })
      if (recoveredAddress !== address) throw Error
    ── 서명이 swapWallet 주소로 복원되지 않으면 즉시 abort ──

[7] transferWithAuthorization calldata 인코딩
    lib/chainSig.ts:304–342
      iface.encodeFunctionData('transferWithAuthorization', [
        from, to, value, validAfter, validBefore, nonce, v, r, s
      ])
    ── EIP-3009 표준; USDC 컨트랙트가 이 calldata를 받아 서명 검증 + transfer 실행 ──

[8] Legacy EVM 트랜잭션 준비 + 해시 생성
    lib/chainSig.ts:356–363
      const { transaction: preparedTx, hashesToSign } = await evmChain.prepareTransactionForSigningLegacy({
        from: address,
        to: USDC_CONTRACT,
        value: 0n,
        data: (calldata),
        gasPrice: 0.1 gwei,
        gas: 150_000n,
      })
    ── chainsig.js가 RLP 인코딩된 tx의 keccak256 해시(hashesToSign)를 반환 ──

[9] EVM tx 서명 (MPC #2)
    lib/chainSig.ts:372–377
      const signature = await chainSignatureContract.sign({
        payloads: hashesToSign,
        path: 'base-1',
        keyType: 'Ecdsa',
        signerAccount: account,
      })
    ── 두 번째 NEAR cross-contract call; 같은 MPC path 'base-1' ──

[10] 서명 삽입 + 직렬화
     lib/chainSig.ts:388–391
       const signedTx = evmChain.finalizeTransactionSigningLegacy({
         transaction: preparedTx,
         rsvSignatures: signature,
       })
     ── chainsig.js가 v, r, s를 RLP-encoded legacy tx에 삽입, hex 직렬화 ──

[11] Base mainnet 브로드캐스트
     lib/chainSig.ts:394–396
       const broadcastTxHash = await publicClient.sendRawTransaction({
         serializedTransaction: signedTx,
       })
     ── viem이 Base RPC https://mainnet.base.org 에 eth_sendRawTransaction ──

[12] tx hash 반환
     lib/chainSig.ts:401  return broadcastTxHash
     ↓
     cronjob-check-deposits/route.ts:135
       updateDepositTracking(depositAddress, {
         signedPayload: transactionHash,   ← "signedPayload" 컬럼이지만 실제 값은 tx hash
         x402Executed: true,
         confirmed: true,
       })
```

**두 번의 MPC 서명 호출 요약:**

| 단계 | 입력 | 목적 |
|------|------|------|
| MPC #1 (step 5) | EIP-712 `TransferWithAuthorization` 해시 | USDC authorization 서명 (EIP-3009) |
| MPC #2 (step 9) | Legacy EVM tx의 RLP 해시 | 실제 on-chain tx에 대한 서명 |

---

## 노트 / 특이사항 / 주의점

### Trust model (보안 경계)

**무엇이 보호되는가:** Base chain의 `swapWallet` EVM private key는 서버 어디에도 존재하지 않는다. 서명 연산은 NEAR MPC 네트워크의 threshold signature 프로토콜 위에서 분산 수행된다. 단일 노드 침해로는 서명이 불가능하다.

**무엇이 보호되지 않는가:** `NEAR_PROXY_PRIVATE_KEY`는 서버 환경 변수로 평문(plaintext) 보관된다 (`lib/chainSig.ts:29`). 이 키를 가진 모든 주체는 임의의 path에 대해 임의의 payload를 MPC 컨트랙트에 서명 요청할 수 있다. **NEAR proxy account가 곧 보안 경계다** — EVM private key의 분산 보관이 의미를 가지려면 NEAR 환경 변수 관리가 동등하게 엄격해야 한다.

실질적 위험: 서버 환경 변수가 노출되면 공격자가 `swapWallet`의 USDC를 임의 주소로 `transferWithAuthorization`으로 유출할 수 있다. MPC의 threshold 보장은 NEAR 레이어에서만 성립하며, NEAR proxy key 탈취 이후에는 무력화된다.

### Latency / cost (지연 및 비용)

- **MPC 서명 대기 시간:** NEAR MPC 서명 완료까지 일반적으로 5–30초 소요. `lib/near.ts:71`에서는 "this may take approx. 30 seconds to complete"라고 명시한다. `signX402TransactionWithChainSignature()`는 내부적으로 MPC 서명을 **두 번** 수행하므로 총 60초까지 걸릴 수 있다.
- **NEAR gas:** 서명당 300 TGas 첨부 (`lib/near.ts:116` — `new BN('300000000000000')`).
- **NEAR deposit:** 서명당 1 NEAR 첨부 (`lib/near.ts:59` — `parseNearAmount('1')`). MPC 컨트랙트에서 서비스 비용으로 차감된다.
- **Base gas:** `gasPrice = 0.1 gwei` (하드코딩, `lib/chainSig.ts:71`), `gasLimit = 150,000` (하드코딩, `lib/chainSig.ts:351`). Base mainnet에서 `transferWithAuthorization` 호출의 전형적인 가스 소비량이다.

### `lib/kdf.ts` 판정 — (A) NEAR Chain Signatures path derivation

**판정: (A) — NEAR Chain Signatures의 주소 파생 유틸리티**

근거:

- `lib/kdf.ts:26–50` — `deriveChildPublicKey()` 함수는 NEAR MPC의 epsilon derivation 공식을 구현한다: `scalar = sha3_256("near-mpc-recovery v0.1.0 epsilon derivation:${signerId},${path}")`, 이 스칼라를 secp256k1 G에 곱하고 parent public key에 더해 child key를 파생한다. 이는 NEAR 공식 MPC 문서의 표준 알고리즘이다.
- `lib/kdf.ts:109–180` — `generateAddress()` 함수는 `(publicKey, accountId, path, chain)`을 받아 Ethereum(`keccak256`), Bitcoin/Dogecoin(`SHA-256 → RIPEMD-160 → Base58Check`), XRP Ledger, Cosmos(`bech32`), Base(EVM과 동일) 주소를 결정론적으로 도출한다.
- **Zcash 관련 코드 없음:** `bech32` 임포트(`lib/kdf.ts:8`)는 Cosmos 주소에만 사용되고(`lib/kdf.ts:164–165`), `bs58check`(`lib/kdf.ts:6`)는 Bitcoin/Dogecoin 주소에만 사용된다. Zcash `zs1` 또는 `t1` 형식의 주소 생성 코드는 전혀 없다.
- **KDF for symmetric key material 없음:** HKDF, PBKDF2, Argon2 등 대칭키 파생 함수 사용이 없다.

**중요 관찰:** `lib/chainSig.ts`의 production path는 `lib/kdf.ts`를 직접 import하지 않는다. `chainsig.js` SDK가 내부적으로 동일한 NEAR MPC epsilon derivation을 수행한다. `lib/kdf.ts`는 SDK 도입 이전의 레거시 유틸리티이거나, SDK 외부에서 주소를 미리 계산해야 할 때를 위한 보조 모듈이다.

### Fallback / retry 동작

- **retry 없음:** `chainSignatureContract.sign()` 실패 시 `throw new Error('Failed to get signature from MPC contract')` (`lib/chainSig.ts:155`, `lib/chainSig.ts:382`) 발생 후 전파된다.
- **cronjob 수준의 암묵적 retry:** `cronjob-check-deposits/route.ts`는 1분마다 실행되며, `x402Executed: false && signedPayload: null` 조건이 유지되는 한 다음 cron 실행에서 다시 시도된다 (`route.ts:47`). 단, 개별 실행 내에서는 별도의 retry loop가 없다.
- **에러 격리:** 개별 deposit에 대한 오류는 `results` 배열에 `action: 'x402_error'`로 기록되며 다른 deposit 처리를 중단하지 않는다 (`route.ts:149–157`).

### `signedPayload` 컬럼 명칭 혼동

Supabase `deposit_tracking.signed_payload` 컬럼에 저장되는 값은 "서명된 payload bytes"가 아니라 **Base mainnet의 Ethereum 트랜잭션 해시**다 (`cronjob-check-deposits/route.ts:135` — `signedPayload: transactionHash`). 브로드캐스트는 `lib/chainSig.ts` 내부에서 이미 완료되며, 반환값은 `broadcastTxHash`다 (`lib/chainSig.ts:394–401`). 따라서 이 서브시스템은 "서명 후 외부 브로드캐스트 위임"이 아니라 **서명과 브로드캐스트를 하나의 함수에서 모두 처리**한다.

### §1.7과의 경계 (Cross-reference)

`signX402TransactionWithChainSignature()`는 EVM tx를 직접 브로드캐스트한다. §1.7(x402 클라이언트)은 이 함수를 트리거하는 upstream이지만, 브로드캐스트된 트랜잭션을 추가로 처리하지는 않는다. x402 프로토콜의 표준 challenge/response 흐름(서버가 402를 반환하고 클라이언트가 payment proof를 헤더에 첨부)은 이 구현에서 **완전히 다르게** 실행된다 — PAL은 HTTP 402 응답을 받아 payment proof를 재전송하는 대신, MPC로 USDC transfer를 직접 실행하는 방식을 택했다.

### `MPC_PATH` 하드코딩의 함의

`lib/chainSig.ts:18`에서 `const MPC_PATH = 'base-1'`이 하드코딩되어 있다. `deriveAddressAndPublicKey(derivationPath?)` 함수 시그니처에 `path` 파라미터가 존재하지만 함수 내부에서 `const path = 'base-1'`로 재정의되어 파라미터가 무시된다 (`lib/chainSig.ts:94`). 모든 사용자/intent에 대해 동일한 EVM 주소(`swapWallet`)를 사용한다 — 사용자별 또는 주문별 파생은 없다.

### 두 번의 MPC 호출이 필요한 이유

PAL의 x402 구현은 일반적인 ETH 전송이 아니라 **USDC EIP-3009** `transferWithAuthorization`을 사용한다. 이 표준은 "gasless transfer" 패턴으로, 토큰 소유자가 authorization 서명을 생성하면 제3자가 이를 포함한 트랜잭션을 제출할 수 있다. PAL의 구현에서는:
1. MPC #1: USDC 컨트랙트가 검증할 EIP-712 authorization에 서명
2. MPC #2: authorization calldata를 포함한 EVM 트랜잭션 자체에 서명

두 서명 모두 같은 secp256k1 키(`swapWallet`)의 서명이 필요하므로 MPC가 두 번 호출된다.

---

## 답한 open questions (from the spec §7)

**Q: `lib/kdf.ts`가 하는 일은? Chain Signatures path derivation인가, Zcash 관련 KDF인가?**

**A: (A) — NEAR Chain Signatures path derivation (타원곡선 기반 child key/address 파생)**

`lib/kdf.ts`는 NEAR MPC epsilon derivation 알고리즘을 순수 TypeScript로 구현한 것이다. 핵심 함수 `deriveChildPublicKey(parent, signerId, path)`는 `sha3_256("near-mpc-recovery v0.1.0 epsilon derivation:${signerId},${path}")` 스칼라를 secp256k1 G에 곱하고 parent public key에 더하여 child public key를 파생한다(`lib/kdf.ts:26–50`). `generateAddress()` 함수는 이 child key로부터 Ethereum/Bitcoin/Dogecoin/XRP/Cosmos/Base 주소를 도출한다(`lib/kdf.ts:109–180`).

Zcash와는 무관하다. `bech32`는 Cosmos 주소(`lib/kdf.ts:164–165`), `bs58check`는 Bitcoin/Dogecoin 주소(`lib/kdf.ts:82–107`)에만 사용된다. Zcash `zs1` (sapling, bech32m variant) 또는 `t1` (transparent, base58) 형식의 생성 로직은 전혀 없다.

현재 `lib/chainSig.ts`는 `chainsig.js` SDK를 통해 같은 연산을 수행하므로 `lib/kdf.ts`를 직접 import하지 않는다. `lib/kdf.ts`는 SDK 도입 이전 레거시 구현으로 남아 있거나, SDK 밖에서 주소를 수동 계산할 때를 위한 참조 구현이다.
