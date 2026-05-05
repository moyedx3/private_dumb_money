# Section C (37-54) Zcash Project Summary

Week 1 developer cheat sheet 기준으로 Section C 프로젝트를 읽고 간단히 정리한 내용.

결론:
- 대부분의 프로젝트는 ZCash의 shielded pool을 private layer로 이용, **활용은 오프체인 또는 외부 체인**에서 이루어지는 것으로 보임
- 결제 시 프라이버시 보장을 위해 ZEC를 채택하다보니 lightwalletd 기반으로 비공개 트랜잭션 열람만 수행하는 것으로 보임
- 대부분 체인 통합, UX 강화, 신뢰 강화를 위해 Near intent, Near chain signature(MPC), Near TEE를 활용하여 구현함
- ZCash로 가치를 창출한다기 보다는 ZCash의 비공개성만 골라 사용하거나, 그마저도 옵션으로 제공하고 있는 프로젝트들이 다수
- 만약 로우 단에서 Rust 라이브러리 개발, 개선해야 한다면 `Temi` 프로젝트를 참고하면 도움이 될 것 같음

## Summary Table

| #   | 프로젝트                                                                                       | 문제의식                                                                                                              | 해결 방식                                                                                                                                                                                                           | Zcash/tool 접근                                                                                                                              |
| --- | ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| 37  | [Pay Anyone Legend](https://devfolio.co/projects/pay-anyone-legend-733c)                   | 온체인 결제는 거래 내역이 공개되고 KYC가 요구되며, 크로스체인 결제는 복잡한 수동 조작이 필요                                                            | 자연어 인텐트를 NEAR AI TEE에서 처리, NEAR Intent로 Zcash 실딩 주소를 자동 생성해 익명 결제. x402(HTTP 402) 프로토콜로 머천트 결제 표준화, QR 코드로 즉시 검증                                                                                                | NEAR Chain Signatures(MPC)를 이용해 Zcash Shielded Address 자동 생성 및 서명, NEAR Intent를 통한 ZEC 크로스체인 라우팅                                           |
| 38  | [Privora](https://devfolio.co/projects/privora-859d)                                       | 신원을 드러내야(KYC or공개 블록체인) 자금을 받을 수 있기에 기부 금액과 수혜자를 공개적으로 노출함                                                        | Reclaim Protocol로 신원 공개 없이 실력(GitHub 커밋 등)을 ZK 증명. Fhenix FHE로 기부 금액을 암호화한 ImpactSBT 발행. NEAR AI TEE로 익명 프로필과 기부자 인텐트를 매칭, Zashi 지갑으로 실딩 결제                                                                     | Zcash Shielded Payment(Zashi 지갑 연동), Reclaim Protocol ZK 증명, Fhenix FHE(euint128/ebool)                                                    |
| 39  | [ZKputer](https://devfolio.co/projects/zkputer-2900)                                       | 크립토 트레이딩은 혼란스럽고 반응적이며, 개인 트레이더에게 프라이버시와 준법 감시 체계가 없음                                                              | 계산기 앱으로 위장한 로컬 멀티체인 지갑·트레이딩 인프라 구축. Zcash 실딩 지갑으로 자금 조달 후 NEAR Intents로 브릿징.<br>AI 모델의 비결정적 행동을 막는 아키텍처 레벨 컴플라이언스 게이트(스키마 검증, 사전 거부권, 인간 승인 바인딩) 구현                                                             | Zcash Shielded Wallet(온램프), NEAR Intents(크로스체인 브릿징)                                                                                        |
| 40  | [Aicall.tv](https://devfolio.co/projects/aicalltv-1fd8)                                    | AI 장난 전화 서비스를 만들고 싶었음. 실시간 통화 품질, 음성→영상 변환 파이프라인, 전화번호·이름·지갑 데이터를 다루는 문제를 해결해야 했음                                 | AI가 직접 장난전화를 걸고(Twilio↔OpenAI 연동), 그 오디오 녹음을 받아 분할화면 영상을 생성해 사용자에게 전송. Fhenix 스마트 컨트랙트를 Base Sepolia에 배포해 PII 데이터를 온체인 암호화. 결제 수단으로 ZEC, ZTF(Ztarknet ETH), USDC(Base), USDC(Solana), 신용카드 5가지 지원               | ZEC 결제 통합, Fhenix FHE(PII 암호화), Ztarknet(ZTF 결제)                                                                                           |
| 41  | [OIF-Starknet](https://devfolio.co/projects/oifztarknet-7ca4)                              | 기존 크로스체인 브리지는 최종 사용자 입장에서 느림                                                                                      | Open Intents Framework(OIF)를 Ztarknet으로 확장해 브리지 속도 개선. EIP-7683 크로스체인 인텐트 표준을 Cairo로 이식. Hyperlane-7683으로 Sepolia, Base, Arbitrum, Optimism, Starknet, Ztarknet 총 6개 네트워크 연결                                    | Ztarknet(Zcash 기반 L2), CairoVM, Hyperlane-7683, EIP-7683<br>                                                                               |
| 42  | [Starknet Lightning](https://devfolio.co/projects/starknet-lightning-privacy-mixer-v-431e) | 퍼블릭 블록체인의 모든 거래(발신자, 수신자, 금액, 시점)가 영구적으로 공개되어 포괄적 금융 프로파일링이 가능                                                    | 4레이어 아키텍처: ① Noir 회로 + Garaga 온체인 검증 + 머클트리(256 commitment) ZK 믹서 ② Lightning Network 오프체인 라우팅 ③ Cashu Ecash 무기록 전송 ④ ZEC ↔ STRK atomic swap.                                                                   | starknet 위에서의 여러 방식의 믹서 구현.<br>Zcash <-> STARK atomic swap 브릿징 구현                                                                          |
| 43  | [Zarlink](https://devfolio.co/projects/zarklink-934f)                                      | 기존 브리지는 브리징 사실과 금액을 온체인에 공개하며, 중앙화 커스터디언에 의존                                                                      | 쉴드풀에서 ZEC를 특정 주소(볼트)에 잠금, 실제 금액은 동형 값 커밋먼트로 숨김. starknet위의 컨트랙트로 Zcash pow와 커밋먼트 루트를 증명. 증명 시 wZEC 발행하여 사용 가능                                                                                                   | Sapling Commitment Root를 사용한다고 적혀있음.<br>BLAKE2b-256, Equihash PoW 검증을 Circom ZK 회로로 구현. Starknet에서 Zcash 상태를 온체인 신뢰 없이 검증                  |
| 44  | [ShieldNet](https://devfolio.co/projects/shieldnet-a777)                                   | 퍼블릭 블록체인의 투명성으로 급여, 기부, 사업 결제 등 모든 금융 활동이 공개되어 해킹 타겟팅, MEV 프론트러닝, 금융 프로파일링 등 실질적 위험 발생                            | UTXO 방식 노트 시스템: 예금 시 Hash(amount, asset, blinding, owner_key) 커미트먼트를 머클트리에 등록. 출금 시 ZK 증명으로 노트 소유권만 증명(어떤 노트인지 비공개). 릴레이어가 트랜잭션을 제출해 사용자 지갑과 출금의 연결 차단. Nullifier로 이중 지출 방지                                     | Ztarknet(Zcash 롤업), Noir 회로, Cairo 컨트랙트, Nullifier 기반 이중지출 방지<br><br>zcash와 비슷한 방식으로 구현되었으나 zcash와 직접적인 연관은 없는 것으로 보임.                     |
| 45  | [Umbra](https://devfolio.co/submissions/umbra-6c09)                                        | Zcash 보유자가 DeFi에 참여하려면 중앙화 커스터디언에 의존하거나 프라이버시를 포기해야 함                                                             | zcash를 쉴드풀에서 잠근 뒤, 입금에 대한 zk proof 생성. mina의 o1js(ZkProgram)을 이용해 증명 후 zkZEC를 미나 체인에서 민팅.<br>미나에서 래핑 토큰 소각 시 타임락(24시간) 이후 ZEC 반환                                                                                | Zcash ZK 증명(예금 검증), Zcash Nullifier(이중지출 방지), Mina zkApp 연동                                                                                |
| 46  | [MinaBarter](https://devfolio.co/projects/barter-swap-8a6d)                                | Zcash는 강력한 프라이버시를 갖지만 스마트 컨트랙트가 없어 에스크로·조건부 거래가 불가능. 기존 해결책은 래핑 토큰과 신뢰 기반 커스터디언에 의존                               | 스왑 요청 시 전용 에스크로 인스턴스 생성. mina에서는 MINA를 zkApp 컨트랙트에 예치 후 zk proof로 온체인 검증, ZEC는 실드 풀 에스크로에 전송 후 lightwalletd가 노트 감지.<br>양쪽 입금이 확인되면 스왑을 실행. 타임아웃 시 환불.                                                           | Zcash Shielded Escrow(lightwalletd 기반), Encrypted Memo Field, zcashd/Zebra, z_sendmany<br><br>zcash에 스마트 컨트랙트를 만들 수 없으니, 외부 인스턴스를 만들어 의존함. |
| 47  | [Caution](https://devfolio.co/projects/caution-platform-248a)                              | 기존 클라우드 서비스는 실제로 어떤 코드가 실행되는지 신뢰에 의존                                                                              | 완전 오픈소스 소프트웨어로 재현 가능한 전체 소스 부트스트랩 빌드를 통해 enclave에서 실행되는 정확한 소스 코드를 증명. EnclaveOS(AWS Nitro)로 원격 attestation 구현. 블록체인 노드, LLM, VPN 등 배포 가능. Passkey 기반 CLI/웹 인증                                                  | ZCash를 이용하는 프로젝트는 아닌 것 같고, Zcash 노드 등 블록체인 인프라를 검증 가능한 방식으로 호스팅할 때 사용 가능할듯<br><br>글에 단일 TEE 역시 단일 실패 지점이라는 언급이 있는데, 이에 대한 해법은 안 보임         |
| 48  | [Obscura](https://devfolio.co/projects/obscura-12d1)                                       | 전통 카피 트레이딩 플랫폼은 트레이딩 활동, 포트폴리오 크기, 전략이 중앙화 운영자에게 공개됨. 성과 주장도 신뢰에 의존                                               | ZK 증명(Cairo/Ztarknet)으로 트레이더 성과를 실제 거래 공개 없이 검증. TEE(Citadel 모듈)로 거래소 API 키를 암호화 보관. Zcash 실딩 트랜잭션으로 구독료를 익명 결제.                                                                                                | 카피 트레이딩에 대한 구독 결제를 Zcash Shielded Transaction으로 진행하여 익명화함. zecwallet-light-cli 연동, ZK 증명(Cairo)                                            |
| 49  | [Zypher Trade](https://devfolio.co/projects/zypher-trade-f540)                             | DeFi 퍼프 트레이딩 플랫폼은 지갑 연결과 온체인 신원 공개를 요구해 거래 활동이 영구적으로 공개됨                                                          | 메모 필드에 JSON으로 거래 요청을 담아 ZEC를 전송. 플랫폼이 ZEC → USDC(NEAR Intent) → Hyperliquid → 퍼프 거래 → 수익을 다시 ZEC 실딩하여 반환까지 자동 처리. Temporal 워크플로우로 다단계 처리 보장                                                                     | Zcash Shielded Transaction(예금/반환), Encrypted Memo Field(트레이딩 지시), lightwalletd/Zebra(트랜잭션 모니터링), NEAR Intent(ZEC→USDC 스왑)                  |
| 50  | [Shadow Mesh](https://devfolio.co/projects/veil-8c33)                                      | 여러 지갑과 블록체인 간 자산 이동이 복잡하고 단편화되어 있으며 지갑 데이터가 노출됨                                                                   | Ethereum, Solana, Cardano, Bitcoin, Zcash 등 다중 체인 통합 인터페이스에서 프라이버시 보호 툴(Fhenix, Arcium, Zcash 실딩, Mina ZK 증명)을 통해 크로스체인 거래 실행. AI 에이전트가 자연어 명령을 해석해 복잡한 크로스체인 작업 자동 처리. 모든 지갑 연결 해제 시 세션 데이터 완전 삭제              | Zcash Shielded Transaction(크로스체인 자산 이동), NEAR Protocol 연동                                                                                  |
| 51  | [Cipher](https://devfolio.co/projects/cipher-981d)                                         | Zcash 보유자는 강력한 프라이버시를 갖지만 Ethereum, Solana 등의 DeFi 기회에서 격리됨                                                       | ZEC를 실딩해 NEAR에서 프라이빗 pZEC 발행 → 멀티체인 DeFi(대출, 스왑, 스테이킹) 이용 → 완료 후 ZEC로 실딩 해제. ZK 증명과 NEAR 인텐트 시스템으로 프라이버시 유지                                                                                                     | Zcash Shielded Transaction(실딩/언실딩), ZK 증명, NEAR Intent                                                                                     |
| 52  | [Overpay](https://devfolio.co/projects/overpaycom-99ce)                                    | ZEC로 실제 상품을 구매하기가 너무 어렵고, Web2 개발자가 빠르게 ZEC 결제를 수용하기 어려움                                                          | 사용자가 원하는 상품을 말하고 ZEC를 전송하면 AI shopper가 미국 내 모든 merchant(Amazon 포함)에서 대리 구매. Tor + IP 미기록으로 메타데이터 프라이버시 보호. UFVK(Unified Full Viewing Key)로 Postgres에 거래 동기화                                                     | Zcash Shielded Payment(비즈니스 수락), UFVK(Unified Full Viewing Key), lightwalletd, Tor                                                         |
| 53  | [onyx](https://devfolio.co/projects/onyx-edfc)                                             | Zcash는 강한 프라이버시를 제공하지만 ZEC는 변동성이 크고 안정적인 "머니 레이어"가 없음. 기존 달러 스테이블코인(USDC, USDT)은 완전 투명한 체인에서 발행되어 모든 잔액·이체가 추적 가능 | ZEC를 담보로 사용하는 알고리즘 기반 스테이블 코인 USDO를 만듦.<br>보수적 ZEC 바닥 가격(예: $100/ZEC)을 설정해 USDO 발행. ZEC 시장가가 바닥가 초과 시 초과분은 xUSDO(변동성 흡수 토큰)와 변동성 흡수 풀(VAP)로 분리해 ZEC 폭락 시에도 USDO 보호. 모든 볼트·잔액은 Aztec 프라이빗 노트로 저장, Noir 회로로 ZK 증명 | Zcash를 담보로 사용(ZEC 볼트), Aztec, Noir 회로                                                                                                      |
| 54  | [Temi](https://devfolio.co/projects/temi-d908)                                             | 투명 주소만 지원하는 하드웨어 지갑·거래소·커스터디언 사용자는 실딩 Orchard 출력으로 ZEC를 쉽게 전송할 수 없음. 기존 pczt Rust 크레이트는 외부 서명 API 미지원             | PCZT(부분 구성 zcash 트랜잭션)이라는 방식으로, 트랜잭션을 단계별로 조립할 수 있게 하는 표준(zip-374)를 구현함.<br>TypeScript로 API 제공, 내부적으로 Rust의 pczt를 호출해서 orchard zk proof를 생성함.                                                                   | PCZT(ZIP 374), Orchard ZK 증명 생성, ZIP 244 서명, ZIP 321 결제 URI, pczt Rust 포크                                                                  |
## Tooling Categories
### 1. Bridge & Cross-Chain
- `Zarklink`
- `Umbra`
- `Barter Swap`
- `OIF-Starknet`
- `Cipher`
- `Starknet Lightning`
타 체인으로의 확장을 제공하는 프로젝트들이다.
`zcashd`, `lightwalletd`를 이용하여 Zcash에서의 정보를 동기화하는 방식을 공통적으로 채택하여 구현하였다.

### 2. Privacy & Trading
- `Zypher Trade`
- `Obscura`
- `ZKputer`
- `Onyx`
- `ShieldNet`
신원이나 포지션을 노출하지 않기 위해 Zcash를 채택하였다.
1번과 유사하게 활용은 타 체인에서 이루어졌으며, 자금의 출처를 ZCash를 통해 감추었다.

### 3. Payment & Application
- `Overpay`
- `Pay Anyone Legend`
- `aicall.tv`
- `Privora`
- `Shadow Mesh`
일반 사용자가 ZEC 쉽게 사용할 수 있도록 하는 프로젝트들이다.
대부분 Near의 Chain Signature(MPC)와 Intent 시스템을 활용하여 개발하였다. `Pay Anyone Legend`는 Intent+x402를 이용하여 자연어 결제 의도 분석, 실제 결제까지 연결했고, Overpay는 ZEC를 지불하여 미국 내 판매처에서 대리구매를 진행하는 서비스를 구현했다. 

### 4. Infra & Development Tools
- `Caution`
- `Temi`
Caution은 Zcash와 직접적으로 관련있다고 보기는 어렵다.
Temi는 트랜잭션을 나누어 생성 및 서명할 수 있게하는 ZIP-374를 이용하여 비공개 트랜잭션을 제공하지 않는 지갑에서도 Zcash 비공개 풀을 사용할 수 있도록 인프라를 만들었다.
Rust `pczt` crate를 직접 포크하여 수정하는 등 가장 로우 단에서 개발이 이루어졌을 것으로 추측된다.
만약 생태계 기여를 위해 로우 단에서의 라이브러리 제작, 개선이 요구된다면 `Temi`의 개발 내역을 참고삼아 수행하면 도움이 되지 않을까 싶다.
