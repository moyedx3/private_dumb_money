# Section B (19-36) Zcash Project Summary

Week 1 developer cheat sheet 기준으로 Section B의 18개 프로젝트를 읽고 정리한 내용.

핵심 결론: 이 18개 중에서 실제 Zcash SDK/지갑/암호화 레벨로 깊게 들어간 프로젝트는 `t2z`, `CipherScan`, `Zapp`, 일부 explorer류다. 나머지 상당수는 Zcash를 "프라이버시 결제/정산 레이어"로 쓰면서 NEAR Intents, TEE, MPC, ENS, Solana 등을 붙인 형태다.

## Summary Table

| # | 프로젝트 | 문제의식 | 해결 방식 | Zcash/tool 접근 |
|---|---|---|---|---|
| 19 | [Zipher](https://devfolio.co/projects/zipher-a231) | 온체인 트랜잭션은 금액/주소가 숨겨져도 타이밍 메타데이터가 남음 | "캡슐"이라는 암호화된 bearer file로 iMessage/Bluetooth 오프체인 전송, 마지막 수령자가 sweep | Swift/SwiftUI. `aes-256-gcm`, ZIP-32 파생, capsule 포맷은 구현. 실제 Zcash SDK/lightwalletd 브로드캐스트는 아직 stub |
| 20 | [t2z, Prithvish](https://devfolio.co/projects/tz-2bf2) | transparent 주소에 있는 ZEC를 shielded로 옮기는 UX/개발 인프라가 무거움 | transparent -> Orchard shielded tx를 만드는 portable SDK | Rust Zcash crates, Orchard/Halo2, WASM, UniFFI, PCZT/ZIP-374, ZIP-244 sighash, ZIP-317 fee. 실질적 Zcash infra |
| 21 | [t2z, Dominik](https://devfolio.co/projects/tz-transparent-to-shielded-459e) | Zcash shielded tx tooling이 Rust 중심이라 Go/TS/Kotlin/Java 개발자가 막힘 | Rust core + 각 언어 native binding | Rust FFI, Go cgo, TS koffi, Kotlin/Java JNA, PCZT bytes 직렬화. 역시 Zcash devtooling 성격 |
| 22 | [Zord Protocol](https://devfolio.co/projects/zord-protocol-4081) | Zcash에는 Bitcoin Ordinals 같은 inscription/NFT/token 문화와 표준이 없음 | zatoshi/ordinal 기반 메타프로토콜, ZRC-20/ZRC-721/ZNS, indexer/explorer/marketplace | Zcash transparent UTXO만 index. full node JSON-RPC로 체인 replay, OP_RETURN에 작은 JSON envelope 저장, 실제 미디어는 IPFS |
| 23 | [Zerdinals](https://devfolio.co/projects/zerdinals-2f2d) | Zcash에서 온체인 디지털 자산/inscription을 만들 표준과 도구가 없음 | P2SH script/reveal input 기반 inscription + 자체 indexer/marketplace | Zcash에 Taproot가 없어 Bitcoin Ordinals 방식을 못 씀. raw tx, script parsing, custom block parser를 직접 구현 |
| 24 | [Zcast](https://devfolio.co/projects/zighway-0df2) | Zcash 온체인 데이터 분석은 너무 어렵고 대시보드/스프레드시트 중심 | 매일 Zcash 데이터를 AI 팟캐스트로 변환 | Node/Next.js + Python/Pandas/NumPy + Gemini TTS. Zcash 앱이라기보단 Zcash analytics/media pipeline |
| 25 | [Zcash Tunnels of Privacy](https://devfolio.co/projects/zcash-tunnels-of-privacy-09a4) | 프라이버시 교육은 지루하고, 게임은 감시/텔레메트리 중심 | Zcash/프라이버시 개념을 dungeon RPG와 puzzle game에 녹임 | 직접 Zcash tx를 만들지는 않음. HTML/CSS/JS/Python, self-hosted WebSocket, Ollama/OpenRouter. 교육/콘텐츠형 |
| 26 | [Secure Legion](https://devfolio.co/projects/secure-legion-6c44) | 결제 요청/메시징에서 누가 누구에게 요청했는지 메타데이터가 샘 | Tor 기반 E2E encrypted chat 안에서 SOL/ZEC 결제 요청과 확인 | Kotlin/Rust. Tor hidden service, XChaCha20-Poly1305, NLx402 quote hash, Zcash는 shielded payment rail로 사용 |
| 27 | [loofta pay](https://devfolio.co/projects/loofta-2725) | 크립토 결제는 체인/토큰 fragmentation + 급여/거래 내역 공개 문제가 있음 | payment link로 아무 체인/토큰을 받아 원하는 자산으로 정산, private route 강조 | Next.js/TS/Supabase. NEAR Intents 중심 cross-chain routing. Zcash shielded pool을 private settlement/payment route로 사용 |
| 28 | [Zapp](https://devfolio.co/projects/zapp-private-qr-payments-6c44) | 현실 QR 결제는 편하지만 crypto/ZEC 프라이버시와 fiat rail 연결이 어려움 | self-custody Zcash wallet + UPI/Alipay/PIX facilitator dashboard + NEAR/RHEA yield | SwiftUI + ZcashLightClientKit/lightwalletd SDK. 백엔드는 Node/React/MongoDB, NEAR Intents/RHEA. 모바일 Zcash wallet 접근 |
| 29 | [Assura Network](https://devfolio.co/projects/assura-network-a2f4) | ENS/EVM 주소 수신은 지갑 히스토리와 신원이 노출되고, Zcash로 옮기기도 복잡 | ENS subdomain -> 매번 fresh stealth address -> smart account/AutoShield -> Zcash 자동 정산 | Solidity/Next.js. TEE off-chain resolver, stealth address, smart accounts, NEAR Intents로 EVM 자산을 Zcash로 자동 이동 |
| 30 | [CipherScan](https://devfolio.co/projects/cipherscan-fa99) | Zcash explorer/API가 부족하고, viewing key를 서버에 맡겨야 shielded tx 확인 가능 | explorer + REST/WebSocket API + 브라우저 client-side memo decryption + privacy metrics | Rust Zcash crypto crates를 WASM으로 컴파일. Orchard/Sapling memo decrypt, Zebra + lightwalletd + PostgreSQL 운영 |
| 31 | [Zcash explorer](https://devfolio.co/projects/zcash-explorer-35fa) | shielded tx를 사용자가 쉽게 감사/확인하기 어렵고 노드/RPC 지식이 필요 | 브라우저에서 viewing key로 decrypt하는 private explorer + AI assistant | React/Next.js/Socket.IO/Postgres/Rust. Sapling/Orchard viewing key, RPC integration, client-side decryption 패턴 |
| 32 | [Z-Ray](https://devfolio.co/projects/zray-85f8) | Zcash에는 사용자가 viewing key로 개인 금융 대시보드를 보는 UX가 부족 | private explorer + personal finance dashboard | Next.js/React. Web Worker 안에 WASM light client를 꽂는 구조를 설계했지만 실제 crypto engine은 아직 stub/demo data |
| 33 | [Cipher Vault](https://devfolio.co/projects/cipher-vault-fhe-analytical-engine-5422) | Zcash privacy 때문에 aggregate analytics를 만들기 어렵다 | FHE로 client-side encrypt 후 암호문 위에서 집계, threshold decrypt | CKKS FHE, 3-of-5 threshold decryption, Kotlin/Swift SDK 구상. Zcash tx 자체보단 privacy-preserving analytics layer |
| 34 | [Zolanear ShadeLink](https://devfolio.co/projects/zolanear-shadelink-9c31) | cross-chain DeFi 실행은 수동/느림/위험하고 bot에 키를 맡겨야 함 | IFTTT식 cross-chain trigger: ZEC/NEAR 이벤트가 Solana swap/lending 실행 | NEAR Intents + Shade Agents + TEE/MPC + Solana. ZEC를 shielded yield loop의 입출금 자산으로 사용 |
| 35 | [Zaunchpad](https://devfolio.co/projects/zaunchpad-5e0f) | early-stage token sale/launchpad는 공개적이고 private capital market이 없음 | Zcash/NEAR Intents 기반 private launchpad/mixer + Solana capital market 접근 | Next.js/TS, Circom/zkSNARKs, NEAR Intents, Phala TEE, Solana. shielded pool settlement와 anonymity set UX를 강조 |
| 36 | [Templar Protocol](https://devfolio.co/projects/templar-protocol-9e69) | native ZEC를 collateral로 써서 stablecoin을 빌릴 수 있는 trust-minimized lending이 부족 | ZEC 담보로 Solana USDC 차입, Phantom/NEAR/passkey wallet 지원 | NEAR Intents + Chain Signatures + meta-tx relayer. Zcash는 native collateral, 실행은 NEAR/Solana 쪽에서 추상화 |

## Tooling Categories

### 1. 진짜 Zcash SDK/crypto 근접

- `t2z`
- `CipherScan`
- `Zapp`
- `Zcash explorer`

봐야 할 키워드:

- `librustzcash`
- `zcash_primitives`
- `orchard`
- `sapling`
- `PCZT`
- `lightwalletd`
- `ZcashLightClientKit`
- WASM

이 그룹은 실제로 Zcash 지갑/트랜잭션/암호화 계층과 부딪힌다. 개발 난이도가 높지만, Zcash 위에 제대로 앱을 만들 때 가장 참고 가치가 크다.

### 2. Transparent UTXO/indexer 활용

- `Zord Protocol`
- `Zerdinals`

이 그룹은 shielded 앱이라기보다는 Zcash의 Bitcoin-like transparent layer 위에 inscription/indexing을 만든 프로젝트다. 핵심은 full node RPC, raw transaction, script parsing, OP_RETURN/P2SH, custom indexer다.

### 3. Zcash를 private settlement rail로 사용

- `Assura Network`
- `loofta pay`
- `Zolanear ShadeLink`
- `Zaunchpad`
- `Templar Protocol`
- `Zapp`

이 그룹은 Zcash 자체를 깊게 수정하거나 Zcash SDK를 직접 많이 쓰기보다는, ZEC 또는 shielded pool을 private settlement/collateral/payment rail로 둔다. 주변 도구는 NEAR Intents, TEE, MPC, relayer, Solana/EVM smart accounts가 많다.

### 4. 분석/교육/콘텐츠

- `Zcast`
- `Zcash Tunnels of Privacy`
- `Cipher Vault`
- `Z-Ray`

이 그룹은 Zcash 생태계 확장에는 의미가 있지만, 일부는 실제 crypto engine이 stub이거나 demo 중심이다. Zcash 앱 개발 레퍼런스로는 상대적으로 간접적이다.

## Practical Takeaways

Zcash 위에 뭔가 만들 때 경로는 대략 네 가지다.

1. 서버/백엔드가 지갑을 관리한다면 `zcashd` JSON-RPC 또는 `zingolib` 접근.
2. 모바일 wallet UX를 만든다면 `ZcashLightClientKit` 같은 iOS/Android SDK 접근.
3. 브라우저에서 사용자가 키를 들고 있게 하려면 WASM/WebZ.js류 접근. 다만 hackathon 프로젝트들에서도 WASM 컴파일과 crate version 문제가 반복적으로 등장했다.
4. cross-chain 앱을 빠르게 만들려면 Zcash를 private settlement layer로 두고 NEAR Intents/relayer/TEE/MPC를 붙이는 방식이 많았다.

제일 참고할 만한 프로젝트는 `t2z`와 `CipherScan`이다. 둘 다 "Zcash 개발이 왜 어려운지"가 선명하게 드러난다: Rust crates 버전 문제, WASM 컴파일, lightwalletd compact block 처리, PCZT API 부족 같은 실제 병목을 만났다.

