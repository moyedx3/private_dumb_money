# Section D (55-71) Zcash Project Summary

Week 1 developer cheat sheet 기준으로 Section D의 17개 프로젝트를 읽고 정리한 내용. 회의록 `260502.md`에서 좁힌 4개 방향에 대해 어떤 프로젝트가 reference로 쓸 만한지도 별도 섹션에서 정리한다.

핵심 결론:

- 17개의 절반 정도가 "Zcash ↔ X 브릿지" 또는 일반 wallet/SDK 류이고, 분석/익스플로러가 그 다음 비중이다.
- 회의록의 가장 유력한 방향인 **Zcash memo 기반 키교환 메시징**의 직접적인 prior art는 `Zchat`(#70) 한 개다. 다만 Zchat은 memo를 *메시지 본문* 채널로 쓰고 NOSTR을 보조에 둔다. 우리가 "memo=KEX only, 본문=P2P/NOSTR"로 좁히면 75초 블록 지연, 영구 onchain 노출, forward secrecy 부재 문제를 동시에 회피하면서 차별화할 수 있다.
- **Private but Accountable Payment Toolkit**의 가장 실속 있는 코드 레퍼런스는 `PCZTKit`(#62, ZIP-374 역할 분리), `Miden Zcash`(#57, ivk 기반 클라이언트사이드 노트 디코딩), `Zscreener`(#66, viewing key + Nillion Secret Vault).
- **Private Access Pass**의 최소 구현 패턴은 `Shield Bridge`(#58, `commitment = Hash(recipient ‖ secret)`)에 거의 그대로 있다.
- 일부는 Zcash 통합이 stub이거나 결제 옵션 라벨 수준이다(`Veil Bridge` #60, `ZUSD` #64는 ZEC 토큰 가정만, `Nexa` #67은 3xpl 외부 인덱서 의존, `ai-call-tv` #71).

## Summary Table

| # | 프로젝트 | 문제의식 | 해결 방식 | Zcash/tool 접근 |
|---|---|---|---|---|
| 55 | [Bridger](https://devfolio.co/projects/bridger-c23a) | Miden과 Zcash 사이에 커스터디언 없이 자산을 교환할 방법이 없고, 단순 lock-mint 브릿지는 프라이버시를 깨뜨림 | Miden 쪽에 HTLC 스타일 P2IDE 노트를 만들고, resolver가 양쪽 체인의 secret 공개를 코디네이트해 atomic swap을 수행한다. Zcash 측은 shielded 트랜잭션으로 ZEC를 잠그고 만료 시 자동 환불. Redis 기반 resolver와 React UI | "shielded transfer + proof-of-payment check"라는 표현 외에 lightwalletd, halo2, PCZT, viewing key 같은 구체적 컴포넌트 언급이 없다. 깃허브 레포(`DaniiRix/bridger`)는 404로 실 검증 불가. Zcash 통합 깊이는 표면적이며 ZEC 측은 일반 shielded send 수준 |
| 56 | [Raven Bridge](https://devfolio.co/projects/raven-bridge-00ab) | Zcash가 P2P 송금 외 프로그래머빌리티가 없어, ZEC를 들고 Miden DeFi에 참여하려면 브릿지가 필요 | Zcash Orchard 풀에 입금하면서 **memo 필드에 Miden 수신자 account_id를 박아넣어** 브릿지가 이를 파싱해 Miden에 wrapped 토큰을 발행한다. Rust 백엔드 relayer가 양 체인을 모니터링하며, Miden CLI로 발행을 트리거. 양방향 전환 흐름 일부만 구현 | Orchard pool + memo 필드를 라우팅 채널로 재활용한 점이 특징적. halo2/PCZT/viewing key 같은 protocol-level 통합은 없고 zcashd RPC + memo 디코딩 수준. 레포(`deeakpan/miden-zypherpunk-bridge`)는 Next.js 스캐폴드 + Rust 백엔드 디렉터리 구조가 잡혀있으나 핵심 코드는 작고 prototype 단계 |
| 57 | [Miden Zcash](https://devfolio.co/projects/midenzcash-0eef) | Miden과 Zcash를 따로 관리하면 키/지갑이 이중화되고, shielded 거래는 감사하기도 어려움 | Miden 계정 키에서 HKDF-SHA256 + BIP32로 **Zcash transparent/Sapling 키를 결정론적으로 파생**하고, Miden 브라우저 지갑 안에서 직접 Zcash 트랜잭션을 서명한다. 클라이언트사이드 익스플로러가 ivk로 Sapling 노트를 스캔/복호화해 사용자가 자기 지갑에서 shielded 거래를 감사 | 5개 브릿지류 중 가장 깊은 Zcash 통합. TypeScript로 Sapling 노트 스캐닝(ivk + Jubjub ECDH + BLAKE2s + AES-256-GCM), Merkle witness 관리, P2PKH 트랜스페어런트 트랜잭션 빌드까지 직접 구현. zcashd RPC(`listunspent`, `importaddress`)에 의존하며 lightwalletd/halo2/PCZT는 사용 안 함. 사실상 브릿지가 아니라 "Zcash 라이트 클라이언트 + 키 통합" |
| 58 | [Shield Bridge / ZyphBridge](https://devfolio.co/projects/shieldbridge-e164) | Zcash의 shielded ZEC를 Miden 같은 다른 프라이버시 체인에서 활용할 길이 막혀있음 | 사용자가 `commitment = Hash(recipient ‖ secret)`을 만들어 shielded ZEC를 입금 → 브릿지 operator가 commitment만 보고 Miden에 wZEC private note 발행 → 사용자가 secret을 공개해 클레임. 반대로 Miden에서 wZEC를 burn하면 shielded Zcash 주소로 ZEC 환불 | commitment-based unlinkability 모델이 명시적으로 설계된 5개 중 유일한 케이스. zebrad regtest + zcashd regtest 환경 구성 파일 포함. Zcash 측은 Sapling shielded 송수신 + Groth16 zk-SNARK(기존 회로) 사용, halo2/Orchard나 PCZT 직접 구현은 없음. operator가 viewing key로 입금 검증하는 패턴이 가장 reference-friendly |
| 59 | [Zephyr Wallet](https://devfolio.co/projects/zephyr-wallet-d529) | 프라이버시 체인(Zcash, Aztec, Miden, Starknet)들은 UX가 복잡하고 서로 단절되어 일반 사용자가 접근하기 어렵다 | OASIS 플랫폼 위에 6개 체인(Zcash, Aztec, Miden, Starknet, Solana, Ethereum)을 단일 시드로 묶는 멀티체인 지갑. ZEC 담보 zUSD 스테이블코인, Zcash↔Aztec 프라이빗 브릿지, viewing key 관리 UI를 표방. Zcash 부분은 OASIS의 `ZcashOASIS` provider에 위임 | Next.js + .NET 8 + MongoDB, Andrew Arnott의 Nerdbank Zcash SDK 사용. lightwalletd/halo2/Orchard 직접 통합 없음. shielded tx, viewing key, memo는 모두 UI/문구 수준이며 백엔드(`ZcashCollateralService.cs`)는 "provider가 메서드 미지원"을 반환하는 스텁. 실질 Zcash infra 깊이는 얕고 UX 시연 위주 |
| 60 | [Veil Bridge](https://devfolio.co/projects/veilbridge-43ff) | 일반 크로스체인 브릿지가 트랜잭션 디테일을 모두 노출시킨다는 문제의식 | Starknet ↔ Miden 브릿지로 STRK를 보내고 Miden 쪽에서 private note로 받게 한다. 어드민 패널에서 수동으로 요청을 처리하는 구조 | **Zcash 통합은 "Coming Soon"으로 빠짐** — 해커톤 시간/Verification 복잡도 이유로 abandon. 코드(`Yunusabdul38/VeilBridge`)는 Next.js create-next-app 템플릿 수준. Zcash 참고 자료로서의 가치는 사실상 없음 |
| 61 | [Mumtaz / zec2eth](https://devfolio.co/projects/zeceth-291c) | ZEC는 shielded 상태에서는 담보·이자·DeFi 사용이 불가하고, Zcash 외부 개발자가 Zcash 프라이버시 기능에 접근할 실용적 경로가 없다 | shielded ZEC를 burn하면 watcher가 lightwalletd로 동기화하며 viewing key로 노트를 디코드하고 memo를 추출, ZK 증명을 거쳐 EVM 측에서 FHE로 암호화된 잔액의 "FHZEC"를 mint. Zcash 측은 Rust watcher, 브릿지 측은 Circom + Solidity + cofhe(FHE) | Rust watcher가 `zcash_client_backend`(lightwalletd-tonic), `zcash_client_sqlite`, `zcash_keys`(orchard+sapling), `zcash_protocol` 사용. Sepolia 컨트랙트 배포됨. memo 디코딩이 `zcash_client_backend` 내부까지 파고들었다고 명시. ZK 증명/FHE 부분은 PoC 수준 |
| 62 | [PCZT Kit](https://devfolio.co/projects/pcztkit-289b) | transparent-only 거래소·커스터디는 Zcash cryptography를 재구현하지 않고는 shielded 주소로 직접 송금할 수 없다(Orchard 회로, ZIP 일관성, proving 인프라 부담) | 공식 `pczt` crate를 감싼 Rust 코어(`pcztkit_core` + `pcztkit_cli`)와 stdin/stdout JSON 프로토콜로 호출하는 TypeScript 라이브러리·Go 데모를 제공. propose→prove(Orchard ProvingKey 인-프로세스 빌드)→verify→finalize 역할을 분리하고, 호스트는 transparent 서명만 담당. `getSighash`로 ZIP-244 sighash 노출 | Rust: `pczt`, `zcash_primitives`, `orchard`, `zcash_protocol`, `zcash_transparent`, `zcash_address`, `zcash_keys`, `zcash_script`. ZIP-244/317/321/374 명시적 준수. MVP 한계: 단일 수신자, Orchard change 없음, `append_signature`는 upstream API 미노출로 스텁. Section D에서 가장 실질적인 Zcash infra 기여 |
| 63 | [goZec](https://devfolio.co/projects/gozec-6221) | Go 백엔드/ZK 라이브러리(예: brevis) 생태계에서 Zcash와 직접 통합할 수 있는 라이브러리가 전무하다 | librustzcash를 cdylib로 빌드한 `zcash_rust_ffi` Rust crate를 만들고, cgo로 Go 측 thin wrapper(`gozec`)에서 `Init/Sync/GetAddress/GetBalance/SendTransaction` API를 노출. 내부적으로 `zcash_client_backend`(lightwalletd-tonic-tls), `zcash_client_sqlite`로 unified 주소·Orchard/Sapling 잔액·전송을 처리 | Rust 의존성에 `orchard`, `sapling-crypto`, `zcash_keys`(orchard), `zcash_proofs`(bundled-prover), `zcash_client_backend`(pczt feature 포함), `zcash_protocol`, `zip321` 등 풀스택. memo 파라미터 FFI 시그니처에 존재하나 송신 측 평문 전달만, viewing-key 기반 memo 복호화 노출은 없음. 실질 Zcash infra이지만 표면 API는 좁음 |
| 64 | [ZUSD](https://devfolio.co/projects/zusd-046d) | 스테이블코인은 잔고와 거래가 전부 공개되어 보유자가 표적이 되고, Zcash는 프라이버시는 강하지만 네이티브 스테이블코인을 만들 프로그래머빌리티가 없다 | ZEC를 담보로 예치(최소 담보비 150%)하고 ZUSD를 발행하는 CDP 구조를 Aztec 위에 Noir로 구현. ZStablecoin(볼트), ZUSD 토큰(PrivateSet 기반 비공개 잔고), ZcashOracle(어드민 갱신) 3개 컨트랙트 | Aztec/Noir가 본체. **Zcash 자체 통합은 없음** — README는 "Aztec 네트워크에 ZEC 토큰이 이미 배포되어 있다고 가정"한다고 명시. 즉 ZEC 담보는 추상 토큰이고 실제 Zcash 체인 ↔ Aztec 브리지는 미구현. 오라클은 어드민 1인이 가격을 수동 갱신하는 스텁 |
| 65 | [Zcash Analystics](https://devfolio.co/projects/zcash-analystics-d866) | Google Analytics식 트래킹은 원시 데이터를 노출시키고, 기존 Zcash 익스플로러는 viewing key를 서버에 저장해 보안 리스크를 만든다 | Next.js 대시보드에 (1) Nillion으로 분산 노드에 암호화 저장된 사이트 분석, (2) viewing key를 클라이언트(세션)에서만 보관하는 블록 익스플로러(Sapling/Unified/Orchard 지원, txid별 입출력 분해), (3) 가격·거래량·shielded 채택률을 보여주는 네트워크 대시보드를 결합 | Next.js + Nillion SDK + Zcash API + ZecHub 데이터. 리포(`teslasdev/zechub-dashboard`)는 README가 거의 create-next-app 템플릿 그대로 — 코드 자체는 있으나 문서/완성도는 얕다. shielded "데이터"는 viewing key를 가진 본인 거래 분해와 풀 단위 집계 통계뿐, 임의의 shielded tx 내용을 쿼리하지는 못함 |
| 66 | [Zscreener](https://devfolio.co/projects/zscreener-0762) | 사용자는 완전한 블랙박스(프라이버시) 또는 완전한 투명성 중 하나를 강요받는다. Zcash shielded 거래에 대한 접근 가능한 분석/컴플라이언스 도구가 부족하다 | 모노레포 구조: 백엔드는 `zcashd`에 RPC로 직접 붙는 자체 인덱서로 블록을 파싱해 Sapling/Orchard shielded 풀 가치 흐름을 시계열로 노출. viewing key는 Nillion Secret Vault에서 암호화 저장·복호화해 본인 거래 내역만 보여줌. NEAR Chain Signatures(MPC)로 다른 체인에서 Zcash 자산을 사인·이동하는 cross-chain intent까지 통합 | React/TS + Node 인덱서 + Docker로 묶은 pruned testnet `zcashd` 노드 + Nillion + NEAR. 풀노드 동기화 자원 문제로 testnet/pruned에 그치고 있고 ZSA·ZIP-231 메모는 "계획" 수준. 그래도 4개 분석류 중 zcashd RPC를 실제로 사용하는 가장 깊은 통합 |
| 67 | [Nexa](https://devfolio.co/projects/nexa-e943) | 블록체인 분석툴은 "쓰기 쉬우나 비공개 아님" 또는 "비공개지만 너무 복잡함" 양극단이고, 공개 대시보드는 shielded 거래를 다루지 못함 | "Normal/Privacy" 듀얼 모드 대시보드. ZcashIngestor가 3xpl 샌드박스 API에서 블록·tx·멤풀을 폴링해 윈도우 단위로 집계 → Privacy 모드에서는 EncryptionPreprocessor가 고정소수점 벡터로 변환해 Fhenix CoFHE로 FHE 암호화, nilDB에 ciphertext 저장, nilAI가 임베딩 기반으로 LLM 요약 생성 | React + Node/Express + Solidity + CoFHE + nilDB + nilAI + 3xpl API. **Zcash 자체 통합은 없음** — 모든 데이터가 3xpl 외부 인덱서를 통해 들어오고 노드/RPC/viewing key 사용 없음. README가 명시: "nilDB는 데모 모드에서 인메모리, nilAI는 시뮬레이션, 영속 저장 없음". FHE 파이프라인 골격은 있으나 프로덕션 통합은 스텁 |
| 68 | [SafeMask](https://devfolio.co/projects/safemask-e909) | 일반 지갑 UX는 보유 자산과 거래를 관찰자에게 그대로 노출해, 단말 압수·강요·사회적 감시 상황에서 자기검열을 강제한다 | 평범해 보이는 데코이 앱 안에 비밀 제스처로만 열리는 진짜 지갑을 숨긴다. 시드는 평문 저장 없이 암호화하고 멀티체인 잔액 조회·RPC 폴백·CI 일관성 체크를 붙였다. Zcash는 "프라이버시 앵커"로 포지셔닝 | React Native(Expo) + `circuits/` 디렉터리 존재(자체 ZK 시도 흔적). Sapling shielded, NEAR intents 연계를 표방하나, 실제 shielded send/receive가 클라이언트에서 동작하는지는 데모 영상 외 코드상 명확치 않음 — 부분 통합/일부 데모 수준 |
| 69 | [Zeke](https://devfolio.co/projects/zeke-9a89) | ZK·shielded 트랜잭션 담론이 일반 사용자에게 너무 추상적이라 도달이 안 되고, 프라이버시 옹호 콘텐츠가 부족하다 | Gemini 3.0 + Imagen으로 매일 10+ 종류의 프라이버시 콘텐츠를 트위터에 자동 발행. 사용자가 ZEC를 메모에 토픽 적어 보내면 Zeke가 그 주제로 분석 글을 생성. **메모 본문이 실제 사용자 입력 채널** | Node.js + TypeScript, Zingo(`.zingo/`) 통합 + lightwalletd proto 사용해 viewing key로 incoming shielded tx를 폴링·복호화. 외부 wallet CLI 의존 없이 순수 노드에서 메모 디크립트 — 실제 동작하는 shielded receive + memo 파싱 |
| 70 | [Zchat](https://devfolio.co/projects/zchat-ea53) | 암호화폐는 프라이버시를 강조하지만 정작 거래 협상은 Discord/Telegram 같은 메타데이터 누출 채널에서 이뤄지고, shielded 메모는 지갑 UI 안 고급 필드로만 갇혀 있다 | Zashi 안드로이드 지갑 포크. 모든 메시지를 shielded tx 1건으로 송신하고 **메모 필드(512B)를 ZMSG/ZGRP/ZTL 프리픽스 프로토콜의 transport로 사용**. 400B 초과 시 CHK 청크. NOSTR을 보조 채널(Blossom 파일, WebRTC ICE, presence)로 하이브리드 운용. v4부터 인증 KEX 도입 | **실제 통합**. zebrad → lightwalletd → Rust(`zcash_client_backend`, `zcash_protos`) → Node 백엔드 → React/Compose. 단일 BIP39 시드에서 m/44'/133' Zcash UA + m/44'/1237' NOSTR secp256k1 동시 파생. ECDH+AES-256-GCM, HKDF, ECIES per-recipient group key. 약 75초 블록 지연이 transport 본질 한계 |
| 71 | [ai-call-tv](https://devfolio.co/projects/aicalltv-1fd8) | (해커톤 장난 프로젝트) AI가 친구에게 프랭크 콜을 걸고 그 녹음을 split-screen 영상으로 만드는 서비스에 결제가 필요 | TanStack 기반 웹앱, 자체 Twilio↔OpenAI 통합으로 콜 품질 개선, Fhenix(Base Sepolia)로 PII 암호화. 결제 옵션 다섯 개(USDC Base/Solana, ZTF Starknet ETH, 카드, ZEC) 중 하나로 ZEC 노출 | **Zcash 통합은 사실상 결제 옵션 라벨 수준** — 코드/데모상 shielded 영수증, viewing key 검증, 메모 활용 등 어떠한 ZEC 고유 기능 사용 흔적도 devfolio 페이지에 서술되지 않음. Stub/마케팅 표기 |

## Tooling Categories

### 1. 진짜 Zcash SDK/crypto 근접

- `Mumtaz / zec2eth` (#61) — `zcash_client_backend` lightwalletd-tonic, viewing key memo decode
- `PCZT Kit` (#62) — `pczt` crate, ZIP-244/317/321/374 역할 분리
- `goZec` (#63) — librustzcash cdylib + cgo 풀스택 FFI
- `Miden Zcash` (#57) — TypeScript Sapling 노트 스캔(ivk + Jubjub ECDH + AES-256-GCM)
- `Zchat` (#70) — zebrad → lightwalletd → `zcash_client_backend`, BIP44/UA derivation
- `Zeke` (#69) — Zingo + lightwalletd proto, viewing key incoming polling

이 그룹은 Section A의 `t2z`, Section B의 `CipherScan`/`Zapp`/`Zcash explorer`와 같은 라인. 실제로 lightwalletd, viewing key, memo decode, PCZT를 손대고 있다. 우리가 코드 수준 레퍼런스를 본다면 이 6개 + Section A·B의 위 4개를 묶어서 보면 된다.

### 2. Bridge / cross-chain (Zcash는 부분 통합)

- `Bridger` (#55), `Raven Bridge` (#56), `Shield Bridge` (#58)
- `Zephyr Wallet` (#59) — multichain wallet, Zcash provider 위임

이 그룹은 ZEC를 다른 체인으로 옮기는 게 목적이고 Zcash 측 통합 깊이는 천차만별이다. `Shield Bridge`의 commitment 패턴과 `Raven Bridge`의 memo 라우팅 식별자만 따로 발췌해 보면 충분하다.

### 3. Stub / 마케팅 표기 / Zcash 통합 거의 없음

- `Veil Bridge` (#60) — Zcash "coming soon"
- `ZUSD` (#64) — Aztec 네이티브, Zcash는 가정만
- `Nexa` (#67) — 3xpl 외부 인덱서 의존
- `SafeMask` (#68) — UI/decoy 컨셉 위주, shielded 동작 코드 불명
- `ai-call-tv` (#71) — 결제 옵션 라벨 수준

이 그룹은 reference 가치가 거의 없다. 빠르게 걸러내고 시간 절약.

### 4. 분석 / 익스플로러

- `Zcash Analystics` (#65) — Next.js 대시보드 + Nillion + viewing key 클라이언트 보관
- `Zscreener` (#66) — 자체 zcashd 인덱서 + Nillion Secret Vault + NEAR Chain Signatures

Section B의 `CipherScan`/`Zcash explorer`/`Z-Ray`와 비교해 보면 좋다. 둘 다 viewing key를 외부에 신탁하지 않는 클라이언트사이드 처리에 무게를 실었다는 점이 공통점.

## 회의록 아이디어와의 매핑

회의록 `260502.md` 6장에서 좁힌 4개 후보 방향을 기준으로, Section D에서 reference 가치가 높은 프로젝트를 정리한다. Section A·B에 나온 프로젝트도 비교 대상에 함께 적었다.

### A. Zcash Memo Key Exchange Messenger (회의에서 가장 유력으로 본 방향)

| 우선순위 | 프로젝트 | 왜 보는가 |
|---|---|---|
| ★★★ | `Zchat` (#70) | 직접적인 prior art. **반드시 정독.** memo를 메시지 *본문* 채널로 사용하고 NOSTR을 보조에 둠. 75초 블록 지연, 영구 onchain 노출, forward secrecy 부재가 본질 한계로 드러남. 우리 차별점: "memo=KEX only, 본문=P2P/NOSTR transport"로 좁히면 같은 한계를 회피하면서 검열저항성 핵심은 유지. KEX 부분(ephemeral secp256r1 pubkey를 Zcash spending key로 서명, ECDH→HKDF→AES-256-GCM)은 그대로 차용 가능 |
| ★★ | `Mumtaz / zec2eth` (#61) | viewing key 기반 incoming shielded tx 디코드 + memo 추출이 실제로 동작하는 Rust watcher. memo decoder pipeline 코드 레퍼런스로 가장 직접적 |
| ★★ | `Zeke` (#69) | 메모를 "사용자 → 서비스" 단방향 명령 채널로 운용하는 PoC. Zingo + lightwalletd proto로 viewing key polling. "memo가 small payload channel로 실제로 쓸만한가"의 동작 검증 |
| ★ | `Raven Bridge` (#56) | memo 필드에 라우팅 식별자(account_id)를 박는 minimal example. Bootstrap data를 memo에 싣는 가장 단순한 패턴 |
| 비교 | Section A `Zipher` (#19), `Zucchini Wallet` (#15) | Zipher는 capsule 포맷·ZIP-32 파생까지만 만들고 lightwalletd 브로드캐스트는 stub. UI/UX와 키 파생 참고 |

### B. Private Access Pass (shielded payment + identity-decoupled access proof)

| 우선순위 | 프로젝트 | 왜 보는가 |
|---|---|---|
| ★★★ | `Shield Bridge` (#58) | `commitment = Hash(recipient ‖ secret)`로 입금하고 secret 공개로 클레임하는 흐름이 access pass 발급 로직과 거의 동일. operator는 commitment만 보고 처리. **이 패턴 그대로 차용 가능** |
| ★★ | `Zscreener` (#66) | "shielded 데이터에서 무엇이 실제로 쿼리 가능한가"의 가장 정직한 reference (zcashd RPC + 자체 인덱서). 답: 풀 단위 집계 + 본인 viewing key 범위. access pass 검증 측 한계를 잡는 데 도움 |
| ★ | `Mumtaz / zec2eth` (#61) | burn → ZK proof → 외부 체인 사용 패턴은 access pass의 한 변형 |
| 비교 | Section A `Zcash ↔ Aztec Bridge` (#4) | 동일한 commitment-from-memo 패턴이지만 Aztec 측에 deposit registry + claim 검증이 명시. Shield Bridge보다 검증 흐름이 더 자세 |

### C. Private but Accountable Payment Toolkit

| 우선순위 | 프로젝트 | 왜 보는가 |
|---|---|---|
| ★★★ | `PCZT Kit` (#62) | ZIP-374 PCZT 기반 multi-party signing 역할 분리(Creator/Constructor/Prover/Signer/Combiner/Finalizer). "merchant에게 selective disclosure + 사용자가 서명"의 toolkit 골격으로 직접 참고. Section A `t2z` 두 개와 같은 `pczt` crate 코어를 공유하므로 함께 봐야 함 |
| ★★★ | `Miden Zcash` (#57) | viewing key로 자기 거래를 클라이언트에서 복호화/감사하는 정확한 코드(ivk derivation, Sapling note decrypt, Merkle witness)가 TypeScript로 존재. 회계용 selective disclosure 도구의 코어로 그대로 가져다 쓸 수 있음 |
| ★★ | `Zscreener` (#66) | viewing key를 Nillion Secret Vault에 위탁해 분석/컴플라이언스 뷰 → "merchant analytics on shielded txs + viewing keys for accounting" 아이디어와 가장 가까움. Nillion MPC를 신뢰 가정에 넣는 부분은 평가 필요 |
| ★ | `Zcash Analystics` (#65) | 클라이언트 세션에 viewing key를 두고 본인 tx만 디코드. 회계용 selective disclosure의 약한 원형 |
| ★ | `ZUSD` (#64) | Aztec 사례지만 "비공개 포지션 + 공개 총공급량" 구조가 private-but-accountable 패턴의 레퍼런스 |
| 비교 | Section A `t2z` 두 개 (#20, #21), Section B `CipherScan` (#30) | t2z는 portable PCZT SDK 패키징, CipherScan은 viewing key client-side decrypt + REST/WebSocket API. PCZTKit + t2z + Miden Zcash + CipherScan 조합이 toolkit의 4개 기둥 |

### D. 구체적 use case 기반 private payment

직접 매핑되는 강한 레퍼런스는 Section D에 거의 없다. Section D는 인프라/툴킷/브릿지 비중이 커서 특정 use case(저널리스트/소스, AI agent API 결제, DAO 익명 기여 등)와 1:1로 매칭되는 프로젝트가 드물다.

- `Zeke` (#69)의 "메모로 명령 보내고 viewing key로 응답 처리"는 **AI agent API 결제** use case의 결제+요청 통합 패턴에 가장 근접.
- 그 외에는 Section A의 `ZSPA` (#12, agent + private payment routing)와 Section B의 `Secure Legion` (#26, Tor + Zcash payment request) 쪽이 use case 정의가 더 선명함.

## Practical Takeaways

1. **`Zchat`은 단일 가장 중요한 prior art.** 우리 가설(memo를 KEX 부트스트랩 채널로 쓴다)을 무효화하지 않는다 — 오히려 "memo를 message body로 쓰면 75초 지연 + 영구 노출 + forward secrecy 부재라는 본질 한계가 있다"는 실증을 통해 "memo=KEX only, 본문=별도 transport"라는 우리 분리 모델의 정당성을 보강한다. 정독 + 차별화 지점 명문화 필요.

2. memo decoding pipeline을 직접 만들 거라면 `Mumtaz/zec2eth` (#61)와 `Zeke` (#69)의 viewing key polling 코드를 우선 본다. 둘 다 lightwalletd 또는 Zingo 기반.

3. Private but Accountable Payment Toolkit을 만들 거라면 `PCZT Kit` (#62) + Section A `t2z` 두 개 + `Miden Zcash` (#57) + Section B `CipherScan`의 4개 묶음을 함께 본다. PCZTKit는 ZIP-374 역할 분리, t2z는 portable SDK 패키징, Miden Zcash는 클라이언트사이드 viewing key 디코딩, CipherScan은 explorer/API 패턴.

4. shielded 입금 commitment + secret reveal 패턴은 `Shield Bridge` (#58)에 거의 그대로 있어 Private Access Pass MVP의 출발점으로 쓸 수 있다. Section A `Zcash ↔ Aztec Bridge` (#4)와 함께 보면 검증 흐름이 더 명확.

5. Zcash 통합이 stub인 프로젝트(#60, #64, #67, #71)는 "기존 프로젝트 조사" 단계에서 빠르게 걸러낸다. Zcash 측 코드가 없는 사례를 reference로 보는 시간 낭비를 피하려면 README의 "Zcash" 검색 + 실제 코드 디렉터리 존재 여부만 빠르게 확인하면 된다.
