# Project References by Idea — ★★★ Deep Dive

회의록 `260502.md`에서 발산한 5개 아이디어에 대해, Section A~D 71개 프로젝트 중 **★★★로 평가된 레퍼런스만** 모아 심층 정리한 문서. 각 프로젝트별로 (1) **프로젝트 개요**, (2) **누구를 위한 건가**, (3) **우리 아이디어와의 연결**을 적는다.

> **v2 갱신 (2026-05-04).** 처음에는 Section A/B 요약을 그대로 입력으로 썼지만, 우리 5-카테고리 프레임이 그 요약 작성 시점에는 없었으므로 36개 프로젝트(A+B)를 재조사. 그 결과 ★★★ 누락 9개를 추가 발견하고 카테고리 E의 정체를 확인했다.

5개 카테고리:

- **A. Zcash Memo Key Exchange Messenger** — Zcash memo를 검열저항적 키교환/부트스트랩 채널로 사용하는 P2P 메시징
- **B. Private Access Pass** — shielded payment + identity-decoupled access proof
- **C. Private but Accountable Payment Toolkit** — selective disclosure, viewing key 영수증/감사, PCZT/multi-party signing
- **D. 구체적 use case 기반 private payment** — AI agent API 결제, journalist/source, 카피 트레이딩, 대리 구매, 자선 등
- **E. x402 + Zcash** — HTTP 402 결제 프로토콜과 Zcash의 결합 (회의록 5.2 추가 카테고리)

---

## 카테고리 A — Zcash Memo Key Exchange Messenger

★★★ 5개. Memo encoding 측 3개 + memo retrieval 측 1개 + 직접 메시지 prior art 1개.

### #18 Rime [primary; retrieval 측 단일 ref]

**프로젝트 개요.** Zcash unified address light client를 ZIP-314 reference 구현으로 새로 짠 프로젝트. 일반 light client는 transaction 내용은 숨기지만 *어떤 note를 누가 언제 조회했는지*가 lightwalletd 서버 입장에서 다 보인다 — block-range query, memo fetch 패턴, 타이밍, 네트워크 식별자로 거래 그래프 재구성 가능. Rime은 **8개 방어층**을 모듈로 묶어 해결: XOR-PIR (2-server, non-colluding) + dummy traffic + full-memo 다운로드 + note 밀도 smoothing + bucketing + Tor isolation (RPC vs PIR 분리, 지터) + ephemeral mode + stateless mode. 합의 변경 없이 클라이언트 측 sync 방식만 바꾼다. Rust + SQLite + Tor.

**누구를 위한 건가.** "shielded tx 내용 비공개" 만으로는 부족하고 메타데이터까지 hardening해야 하는 사용자 — 저널리스트, 활동가, 그리고 본 문서의 맥락에서는 **"검열저항적 메시징을 만들겠다"는 사람**.

**우리 아이디어와의 연결.** Memo KEX Messenger는 두 레이어가 모두 필요하다: (1) memo encoding (키교환 정보를 memo에 박는 우리 프로토콜), (2) memo retrieval (그 memo를 안전하게 꺼내는 클라이언트). lightwalletd가 "이 사람이 이 시점에 이 z-주소의 memo를 풀어봤다"를 알면 검열저항성이 깨진다. **Rime은 (2)의 거의 유일한 prior art**이고, 코드도 모듈화돼 있어 우리 메신저 데모 클라이언트에 plumbing 단위로 가져올 수 있다 — `--sync-mode full-memo`, `--pir-dummy-interval`, `--bucket-size`, `--tor-only --tor-isolate`, `--const-cost --dummy-decryptions` 같은 플래그가 그대로 우리 옵션이 된다. 단 `rime-core`는 receive-only UFVK 한정이라 spending 측은 별도 처리해야 한다.

---

### #70 Zchat [primary; 메시지 prior art]

**프로젝트 개요.** Zashi 안드로이드 지갑 포크. 모든 메시지를 shielded tx 1건으로 송신하고 **memo 필드(512B)를 ZMSG/ZGRP/ZTL 프리픽스 프로토콜의 transport로 사용** (400B 초과 시 CHK 청크). 단일 BIP39 시드에서 m/44'/133' Zcash UA + m/44'/1237' NOSTR secp256k1 동시 파생, ECDH+AES-256-GCM, HKDF, ECIES per-recipient group key. NOSTR은 명시적 *보조* — Blossom 파일, WebRTC ICE, presence만. v4부터 ephemeral secp256r1 pubkey를 Zcash spending key로 서명한 인증 KEX 도입. zebrad → lightwalletd → Rust(`zcash_client_backend`, `zcash_protos`) → Node 백엔드 → React/Compose.

**누구를 위한 건가.** Discord/Telegram이 새는 메타데이터를 안 새고 싶지만 별도 인프라 없이 지갑만 있으면 통신할 수 있어야 하는 사용자. 기본 타겟은 OTC 거래 협상 트레이더, state actor 위협 모델 가진 활동가.

**우리 아이디어와의 연결.** **단일 가장 중요한 prior art.** Zchat은 memo를 *메시지 본문 채널*로 쓰면서 다음 본질 한계가 드러난다: 블록 시간 ~75초 → latency 끔찍, 영구 onchain 저장 → forward secrecy 불가능 (Phase 3 Double Ratchet 예정), 512B 제한 → 청크 필요, 단일 시드에서 ZEC+NOSTR 동시 파생 → 키 손상 시 동시 노출, 컨택 발견 미정의. 우리가 "**memo = KEX only, 본문 = P2P/NOSTR**"로 좁히면 위 한계 5개 중 4개를 회피한다. KEX 부분(ephemeral 키를 spending key로 서명, ECDH→HKDF→AES-256-GCM)은 코드를 그대로 차용 가능. **Zchat은 우리 아이디어를 무효화하지 않고, 오히려 "memo=transport"의 한계를 실증해 우리 분리 모델의 정당성을 보강한다.**

반드시 정독해야 할 문서: `decentrathai/zchat/blob/main/ARCHITECTURE.md`, `DECISIONS.md`, `PRODUCT.md`.

---

### #4 Zcash ↔ Aztec Bridge [primary; 가장 깨끗한 memo decoder 코드]

**프로젝트 개요.** 사용자가 `secret`을 만들고 `ticket_id = hash(secret, user)`를 Zcash shielded memo(`ticket:{ticketId}`)에 넣어 deposit → **Rust watcher가 viewing key로 memo 복호화** → Aztec ZecBridge에 `register_ticket` → 사용자가 `claim_with_ticket(ticketId, secret)`로 bZEC mint. PoC 코드는 `PraneshASP/zcash-aztec-bridge-poc`.

**누구를 위한 건가.** "Zcash 입금을 받아서 Aztec(또는 다른 검증 가능 환경)에서 처리해야 하는 인프라 운영자". 본 프로젝트는 bridge지만, 똑같은 워치 패턴이 우리 메신저 백엔드의 KEX 등록 처리에도 그대로 적용된다.

**우리 아이디어와의 연결 (A 측).** **memo decoder 코드의 lift-and-use reference가 가장 깨끗한 케이스**. `zec-watcher-rs/src/bin/bridge_watcher.rs`에 (1) UFVK로 incoming shielded tx 폴링, (2) memo 복호화, (3) prefix(`ticket:`) 기반 라우팅이 모두 한 곳에 있다. 우리가 KEX 메시지를 `kex:` prefix memo로 정의한다면 같은 watcher 구조에 prefix만 바꾸면 된다. Section D Mumtaz/zec2eth(#61)도 같은 패턴이지만 #4의 코드가 더 작고 의도가 명확.

(B 측 본문은 카테고리 B에서 별도 처리.)

---

### #7 Liquid Privacy (PLSP) [primary; memo 인코딩 포맷 ref]

**프로젝트 개요.** Zcash memo를 cross-chain intent 채널로 써서 shielded ZEC를 Starknet으로 가져가 ERC4626 기반 private liquid staking을 제공하는 풀 워킹 데모. **`memoParser.js` 안에 실제 memo 포맷이 정의돼 있다**: `01:<address>`(account binding), `02:<commitment>`(commitment-based deposit). Relayer가 shielded tx의 memo를 파싱해 Starknet 컨트랙트에 mint를 트리거. 즉 memo가 단순 라벨이 아니라 *구조화된 명령 페이로드*로 운용되는 사례.

**누구를 위한 건가.** Zcash 사용자를 다른 체인의 DeFi/staking에 끌어오려는 cross-chain 프로토콜 빌더.

**우리 아이디어와의 연결.** 카테고리 A에서 가장 이른 시점에 "memo가 production에서 데이터 채널로 쓸 만한가?"의 답을 데모 수준으로 구현해 둔 사례. 우리가 KEX 정보를 memo에 박을 때 포맷은 거의 같은 모양이 된다 — `kex01:<ephemeral_pubkey>`, `kex02:<session_invite_hash>` 같은 식. 원 프로젝트의 `01:`/`02:` 두 포맷 분기가 우리 KEX 프로토콜의 메시지 타입 분기 그대로 매핑된다.

---

### #19 Zipher [primary; 오프라인 bearer capsule]

**프로젝트 개요.** "캡슐"이라는 암호화된 bearer 파일(`.zpr`)을 iMessage / Bluetooth(MultipeerConnectivity)로 오프라인 전송한 뒤 마지막 수령자가 sweep하는 iOS 지갑. 캡슐 = 일회용 shielded 주소의 spending key를 AES-256-GCM으로 암호화한 페이로드 + HMAC-SHA256. ZIP-32 namespace 분리(`m/32'/133'/0'/99'/n'`), 7-day restore timelock, double-spend 감지. crypto layer는 done, lightwalletd 브로드캐스트는 stub.

**누구를 위한 건가.** "전혀 모르는 사람한테 봉투 건네듯 ZEC를 줘야 하는데 onchain까지 가지 않고도 종료될 수 있어야 하는" 시나리오 — 마켓플레이스, 더치페이, 팁, 노출 없는 P2P 양도.

**우리 아이디어와의 연결.** Memo KEX의 **오프라인 delivery 채널** reference. 우리 메신저가 처음 만나는 두 사람이 키교환을 시작할 때, 그 KEX 정보를 *반드시 onchain Zcash tx로* 전달해야 하는가는 별개 질문이다. Zipher 모델을 빌리면 "QR 또는 NFC로 KEX capsule을 직접 건네고, 첫 메시지부터 P2P transport에 올린다"는 변형이 가능 — onchain 트래픽 0회로 시작할 수 있다. `.zpr` 포맷 spec과 ZIP-32 namespace 분리 패턴이 우리 KEX capsule 설계에 거의 그대로 들어간다.

---

## 카테고리 B — Private Access Pass

★★★ 6개. Commitment-from-memo + secret-reveal 패턴이 핵심.

### #4 Zcash ↔ Aztec Bridge [primary; B 측]

(A 측 본문 위 참조.)

**B 측 연결.** `ticket_id = hash(secret, user)`를 memo에 넣어 deposit → operator가 viewing key로 ticket_id + amount만 확인 → 사용자가 secret 제시로 claim. 입금자와 클레임자를 operator가 연결할 수 없다. **회의록 6.1 Private Access Pass의 거의 그대로의 코드 패턴**:

- 결제 = shielded deposit + ticket_id memo
- 증명 = ticket_id (operator viewing key 검증)
- 사용 = secret 제시로 access 발급

API/콘텐츠 access pass에 적용하면 사용자가 ticket_id memo로 결제 → 서버는 viewing key로 amount만 확인 → 사용자가 secret로 API 호출 시 인증. **MVP 7-10일** 안에 구현 가능. README의 "Why Two-Step Ticket Model" 섹션은 product narrative에도 그대로 활용 가능.

---

### #58 Shield Bridge / ZyphBridge [cross-ref; Section D 요약 참조]

**B 측 핵심.** `commitment = Hash(recipient ‖ secret)` 패턴을 explicit하게 사용하는 Zcash↔Miden 브릿지. Operator가 commitment만 보고 wZEC 발행, 사용자는 secret 공개로 클레임. zebrad regtest + zcashd regtest 환경 구성 포함. **#4와 같은 패턴이지만 Zcash 측 구현이 별도로 자세히 노출**돼 두 프로젝트를 묶어 보면 "memo에 commitment 박기 (#4) + operator의 viewing key 입금 검증 (#58)" 두 절반이 모두 갖춰진다.

---

### #10 Shadow Swap [primary; HTLC 패턴]

**프로젝트 개요.** Starknet ↔ Zcash 간 HTLC 기반 atomic swap으로 STRK ↔ ZEC를 익명 cross-chain swap. Starknet 측에서 nullifier로 입출금 unlinkability, Zcash 측은 **별도로 만든 `zcash-htlc-builder` Rust 라이브러리**가 Sapling/Orchard shielded address 지원 + ZIP-300 스타일 HTLC + address parsing + keygen/signing까지 wrap.

**누구를 위한 건가.** Zcash와 다른 체인 사이 trustless swap을 만들고 싶은 빌더. 그리고 HTLC를 access pass primitive로 재사용하려는 빌더.

**우리 아이디어와의 연결.** HTLC = "secret-hash 잠금 → secret 공개로 unlock"의 정확한 패턴. Access Pass와 구조 동형: 사용자가 결제할 때 secret 만들어 hash 락 걸고, 나중에 access를 사용할 때 secret을 reveal. **`zcash-htlc-builder`의 `generate_hash_lock`, `redeem_htlc` 함수를 그대로 우리 access pass 백엔드에 가져갈 수 있다**. #4의 ticket 패턴과 비교하면, HTLC 쪽은 timelock 환불 흐름이 자연스럽게 들어 있어서 "결제했지만 access 못 쓴" edge case를 명시적으로 처리하기 좋다.

---

### #29 Assura Network [primary; ENS → stealth address]

**프로젝트 개요.** ENS subdomain을 입력하면 TEE off-chain resolver가 매번 fresh stealth address를 반환하고, smart account의 AutoShield 모듈이 EVM 자산을 NEAR Intents로 자동으로 Zcash shielded pool로 옮겨주는 ENS-친화 private receive endpoint. 즉 사람이 외우는 이름(`alice.eth`) ↔ 매번 다른 결제 주소(stealth) ↔ 최종 정산 자산(shielded ZEC)이 세 단계로 분리된다.

**누구를 위한 건가.** "프리랜서/크리에이터처럼 받는 사람이 고정 ENS 이름을 공개해야 하는데, 그 이름이 곧 지갑 히스토리로 풀리는 게 싫은" 사용자.

**우리 아이디어와의 연결.** Private Access Pass의 **identity ↔ payment 디커플링** 부분을 정확히 구현한 prior art. Access pass 발급은 두 단계로 볼 수 있다: (1) "이 access는 누구의 권리인가" (사용자가 가진 secret), (2) "이 access는 어디서 결제됐는가" (commitment/ticket). Assura가 (1)을 ENS subdomain + TEE resolver로 처리하는 방식은 우리가 access pass에 사용자 핸들을 붙일 때(예: `alice.api`로 access를 사라고 하면 그때마다 fresh commitment를 뱉어주는 server) 그대로 모델로 쓸 수 있다.

---

### #48 Obscura V2 [primary; D에도 ★★★]

**프로젝트 개요.** ZK 카피 트레이딩 플랫폼. 트레이더의 win-rate/PnL은 Cairo ZK-STARK로 증명, 거래소 API 키는 Nillion blind compute에 보관, **Zcash Unified Address 기반 shielded 구독 결제**로 누가 누구를 카피하는지 자체를 은닉. Python/FastAPI + Cairo + Next.js + zecwallet-light-cli (Docker wrapped). README에 "running in MOCK mode" 폴백 경로가 있어 메인넷 결제 검증 여부는 불투명.

**누구를 위한 건가.** "내가 누구의 트레이딩을 따라 하는지를 플랫폼에도, 다른 사용자에게도 보이고 싶지 않은" 카피 트레이더.

**우리 아이디어와의 연결.** Private Access Pass의 **구독 변형**. 보통 access pass는 1회성 결제 + N회 access인데, Obscura는 *recurring shielded payment + 지속적 구독*으로 확장한다. "월 구독" use case에서는 결제 정기성 + 구독 expiry 검증을 어떻게 viewing key 만으로 처리할지가 직접 이슈가 된다. zcash-payment-modal + subscription manager 코드가 그 출발점.

---

### #52 Overpay [primary; D에도 ★★]

**프로젝트 개요.** 역경매형 personal shopper 서비스. 사용자가 원하는 상품(Amazon 등 미국 머천트)을 지정하고 **ZEC로 지불**하면 운영자가 대신 구매해서 배송. Tor + 무로깅 인프라. 백엔드는 **Zcash UFVK (Unified Full Viewing Key)** 를 모니터링해 Postgres에 입금 동기화 — 실제 shielded payment 수신 처리.

**누구를 위한 건가.** "익명으로 실물 상품을 사고 싶은데 카드/계좌가 없거나 쓰기 싫은" 사용자. 또는 "ZEC로 실물 결제를 받아보고 싶은" 머천트 입장의 PoC.

**우리 아이디어와의 연결.** Private Access Pass의 **off-chain action 변형**. 사용자가 ZEC를 보내면 → operator가 UFVK로 입금 확인 → 약속한 off-chain 행동(상품 구매)을 한다. 정확히 "결제는 증명, 지갑은 identity로 만들지 않음" 패턴. UFVK는 amount + memo만 노출하고 spending key는 사용자가 보유 — selective disclosure의 가장 단순한 형태. UFVK 모니터링 + Postgres 동기화 코드는 우리 access pass 백엔드의 출발점이 될 수 있다 (단 GitHub repo 비공개라 아키텍처만 참고).

---

## 카테고리 C — Private but Accountable Payment Toolkit

★★★ 6개. PCZT 4부작 + viewing key client-side decoding 1개 + explorer/API 1개.

### #54 Temi [primary; TypeScript PCZT 가장 깊은 구현]

**프로젝트 개요.** **ZIP-374 PCZT API 8개 함수를 TypeScript로 완전 구현**한 라이브러리. propose → prove → sign → combine → finalize 5단계로 트랜잭션 구성과 서명을 분리해 하드월렛/멀티시그/에어갭/기관 커스터디 워크플로우를 지원. 공식 Rust `pczt` crate를 napi-rs FFI로 직접 호출, ZIP-244 sighash + ZIP-321 결제 URI + 실제 Orchard ZK proof 생성(9347바이트 검증)까지 동작.

**누구를 위한 건가.** Zcash cryptography를 직접 다시 구현하지 않고 shielded send를 지원해야 하는 비-Rust 환경 — transparent-only 거래소, 하드웨어 지갑/에어갭 시그너, 멀티파티/MPC 서명 셋업.

**우리 아이디어와의 연결.** Toolkit의 본질은 "여러 당사자가 한 트랜잭션에 협력하는데 각자 보는 정보가 다른" 구조다. 그 협력의 표준 직렬화 포맷이 PCZT이고, **TypeScript 백엔드에서 PCZT를 다루는 가장 좋은 코드 레퍼런스가 Temi**.

---

### #62 PCZT Kit [primary; cross-language bridge]

**프로젝트 개요.** 공식 `pczt` Rust crate를 감싼 `pcztkit_core` + `pcztkit_cli`를 stdin/stdout JSON 프로토콜로 노출 → TypeScript 라이브러리, Go 데모 제공. `propose`/`prove`(Orchard ProvingKey 인-프로세스 빌드)/`verify`/`finalize` 4역할 구조. ZIP-244/317/321/374 명시적 준수.

**우리 아이디어와의 연결.** Temi와 비교: **Temi = 단일 TypeScript에서 깊이, PCZTKit = 여러 언어에서 얕게**. 라이브러리 distribution 모델이 외부 팀에 배포할 거라면 PCZTKit의 JSON CLI 패턴이 안전. 한계 (해커톤 MVP): 단일 수신자만 지원, Orchard change output 없음, `append_signature` upstream API 미공개로 stub.

---

### #20 t2z (Prithvish/d4mr) [primary; WASM-first]

**프로젝트 개요.** ZIP-374 PCZT 기반으로 transparent → Orchard shielded 전환을 portable SDK로 만든 프로젝트. **WASM이 first-class** — `@d4mr/t2z-wasm` npm 패키지 + Mintlify 문서 사이트 + Halo 2 proving key를 클라이언트에서 ~10초 빌드 후 캐시. 브라우저/모바일 친화적 포지셔닝.

**우리 아이디어와의 연결.** Toolkit을 *브라우저 확장 또는 모바일 앱*으로 배포할 거라면 t2z d4mr 버전이 reference. 사용자가 자기 키로 직접 PCZT를 propose할 수 있게 하려면 client-side proving이 필요한데, 본 프로젝트가 그 시간/메모리 footprint를 실측해뒀다.

---

### #21 t2z (Dominik/gstohl) [primary; multi-party signing 측에서 더 강함]

**프로젝트 개요.** 동일한 ZIP-374 PCZT 기반 t→z SDK이지만 **네이티브 바인딩** (koffi/CGO/JNA) 중심. macOS/Linux/Windows 사전빌드 라이브러리 번들 + Java 추가 지원 + regtest infra(Docker compose). 핵심 차별점: **`combine()` 병렬 다자 서명 머지 API + `verify_before_signing` + `parse_pczt`/`serialize_pczt` 저장·전송 포맷 + `calculate_fee` ZIP-317 헬퍼**.

**우리 아이디어와의 연결.** **C에서 d4mr보다 gstohl 쪽이 더 적합**. Multi-party signing 워크플로우(merchant + 사용자 + accountability service)를 만들 때 직접 필요한 함수가 모두 노출돼 있다. d4mr는 *클라이언트사이드 proving 데모*에 가깝고, gstohl은 *서버/인프라 multi-party orchestration*에 가깝다. PCZT 4부작에서 둘이 나뉜 이유가 이거다.

---

### #57 Miden Zcash [primary; client-side viewing key 측]

**프로젝트 개요.** Miden 브라우저 지갑 안에서 사용자가 자기 Zcash 거래를 직접 감사할 수 있게 하는 프로젝트. **TypeScript로 Sapling 노트 스캐닝(ivk + Jubjub ECDH + BLAKE2s + AES-256-GCM), Merkle witness 관리, P2PKH 트랜스페어런트 트랜잭션 빌드까지 직접 구현**. zcashd RPC에 의존, lightwalletd/halo2/PCZT는 미사용.

**우리 아이디어와의 연결.** Toolkit의 "accountable" 측은 결국 "viewing key를 받은 사람이 무엇을 볼 수 있는가"의 질문. Miden Zcash는 **클라이언트사이드 ivk-기반 노트 디코딩의 정석 코드**를 TypeScript로 제공한다. PCZT 4부작이 "트랜잭션 *생성* 측 협력"이라면, Miden Zcash는 "트랜잭션 *조회/감사* 측"을 채운다.

---

### #30 CipherScan [primary; explorer/API 측]

**프로젝트 개요.** Zcash explorer + REST/WebSocket API + **브라우저 client-side memo decryption** + privacy metrics. Rust Zcash crypto crates(Orchard/Sapling memo decrypt)를 **342KB WASM**으로 컴파일해 viewing key를 서버에 안 맡기고 브라우저에서 처리. Zebra + lightwalletd + PostgreSQL 운영. README 강조: "**Viewing keys never leave your browser**".

**우리 아이디어와의 연결.** Toolkit의 **사용자 대면 UI 측 reference**. PCZT 4부작이 트랜잭션 생성 도구라면, Miden Zcash가 client-side 디코딩 코어라면, CipherScan은 그 위에 얹는 **API + WebSocket 형태의 production stack**이다. WASM client-side decrypt는 우리가 영수증/감사 PWA를 만들 때 거의 그대로 활용 가능.

---

## 카테고리 D — 구체적 use case 기반 private payment

★★★ 7개 (Obscura, Overpay는 B 본문 cross-ref).

### #37 Pay Anyone Legend [primary; E와 함께]

**프로젝트 개요.** AI 기반 cross-chain 결제 어시스턴트. 자연어 의도 인식(NEAR AI/TEE) + 시맨틱 검색으로 서비스를 매칭하고, 사용자가 QR로 ZEC을 입금하면 NEAR 1Click API로 USDC(Base/Solana)로 브릿지한 뒤 NEAR Chain Signatures(MPC)로 **x402 결제를 자동 실행**. README 명시: "x402 Payment Protocol: HTTP 402 standard with automatic payment verification and execution". Z-address 생성은 mock(`crypto.getRandomValues`로 랜덤 hex에 `zs1` prefix), shielded tx는 1Click API에 위임 — Zcash 측 구현은 얕고, x402 + AI agent 결제 플로우 쪽이 본체.

**누구를 위한 건가.** 사용자가 *상품/서비스 이름*만 말하면 가장 빠른 결제 경로를 자동 선택해주는 결제 어시스턴트가 필요한 가맹점, OnlyFans 스타일 pay-per-access 컨텐츠 가맹점.

**우리 아이디어와의 연결.** 회의록 6.3 use case 예시 중 "AI agent의 유료 API 사용 내역 보호"가 정확히 이 영역. **AI agent가 결제까지 자동 처리할 때 agent의 결제 trace가 사용자 신원으로 역추적되는 위험**이 핵심 동기. AI intent → MPC signing → x402 execute 플로우를 출발점으로 삼고, Zcash 통합 깊이를 우리 쪽이 더 채우면 된다. (E 측 본문은 카테고리 E에서 별도 처리.)

---

### #5 SIP Protocol / Sipher [primary; AI agent SDK]

**프로젝트 개요.** "Shielded Intents Protocol" — NEAR Intents 위에 stealth address + Pedersen commitment + Noir ZK proof + viewing key를 얹어 cross-chain swap 발신/금액/수신을 모두 숨기는 SDK + 모바일/웹 앱 + Arcium MPC program. **부속 프로젝트 `sip-protocol/sipher`는 "Privacy-as-a-Skill for Multi-Chain Agents"** — REST API + OpenClaw skill file로 LangChain/CrewAI/Claude 같은 autonomous agent에게 stealth address/Pedersen commitment/viewing key를 제공. README: "any autonomous agent — Claude, LangChain, CrewAI", "privacy-demo-agent.ts runs 20 steps across 34 endpoints with zero human intervention".

**누구를 위한 건가.** AI agent를 production에서 돌리는데 그 agent의 결제·실행 trace가 한 지갑으로 모이는 게 위험한 사용자/기업.

**우리 아이디어와의 연결.** **카테고리 D의 "AI agent API payments" use case에서 가장 직접적인 prior art**. Pay Anyone Legend(#37)는 사용자 ↔ 상점 결제 흐름인 반면, Sipher는 *agent-as-customer* 자체에 초점. 우리가 D를 골라 "agent infra"로 narrative를 잡는다면 Sipher가 reference 1순위. Sipher 자체는 x402 직접 사용은 없지만 REST API 방식이라 x402 facilitator를 끼우는 변형이 자연스럽다 (E와 인접).

---

### #49 Zypher Trade [primary; perp trading]

**프로젝트 개요.** DeFi 영구선물 거래 시 지갑 연결로 온체인 정체성과 거래 내역이 영구 노출된다는 문제. 사용자가 **ZEC를 지정 주소로 보내며 memo 필드에 JSON 거래 지시를 인코딩**, 서버가 lightwalletd로 입금 감지 후 NEAR Intent로 USDC 스왑하여 Hyperliquid에서 레버리지 거래 실행, 종료 후 shielded ZEC로 환원. 실제 zcashd→Zebra 마이그레이션, lightwalletd, mempool/confirmed 모니터링, 메모 스키마 검증, NEAR Intent + Hyperliquid 실연동.

**우리 아이디어와의 연결.** **memo를 데이터 채널로 쓰는 가장 직접적인 production PoC**. 회의록 6.4 Memo KEX Messenger와 다르게 여기서는 memo가 *사용자 → 서비스* 단방향 명령 채널이지만, **"memo에 구조화된 페이로드(JSON)를 박는 것이 production에서 통한다"는 실증** 자체가 우리 KEX 메시저의 fallback 데이터 모델로 가치 있다. D 카테고리에서는 "trading"이라는 use case가 ZEC를 써야 하는 이유가 가장 명확한 사례 — 거래 전략 노출 위험은 ZEC 외 체인에서는 본질적으로 막을 수 없다.

---

### #12 ZSPA (Zcash Shielded Philanthropy Agent) [primary; donation]

**프로젝트 개요.** AI 에이전트가 15+ trust signal로 자선 단체를 평가하고 NEAR Intents로 ZEC를 익명 도네이션·크로스체인 변환해주는 시스템. 자연어 intent("Donate 5 ZEC to privacy NGOs in Africa") → AI agent가 평가 → 익명 송금. NEAR Intent로 ZEC→USDC/Arbitrum/Ethereum 라우팅. 메모는 swap intent ID 추적용으로만 사용 (KEX/메시징 아님).

**누구를 위한 건가.** "익명으로 기부하고 싶은데 수혜처 검증과 cross-chain 라우팅을 직접 하고 싶지 않은" 기부자. 활동가 후원, journalism 후원 같은 high-stakes 기부.

**우리 아이디어와의 연결.** D의 *donation/philanthropy use case 단일 reference*. AI agent + 자연어 intent + Zcash anonymity의 조합이 그대로 동작. 단 ZSPA의 Zcash 측 깊이는 얕다(메모는 단순 ID, viewing key는 추상 언급) — 우리가 D로 가면 ZSPA의 narrative + agent orchestration을 빌리고 Zcash 측 구현은 더 깊이 채우는 식이 된다.

---

### #28 Zapp [primary; cross-border QR]

**프로젝트 개요.** SwiftUI + **ZcashLightClientKit** 기반 self-custody iOS 지갑. 사용자가 ZEC를 보내면 facilitator가 UPI(인도)/Alipay(중국)/PIX(브라질) 같은 현지 fiat QR rail로 수신자에게 정산. 백엔드는 Node/React/MongoDB + NEAR Intents + RHEA yield. ZcashLightClientKit를 직접 사용한다는 점에서 mobile shielded tx의 표준 SDK 통합 사례.

**누구를 위한 건가.** Cross-border 송금이 필요한 노동자, 가족 송금, 그리고 ZEC를 fiat rail로 받아야 하는 노점/소상공인.

**우리 아이디어와의 연결.** D의 **cross-border real-world payment** use case 가장 구체. Zapp는 "사용자 = ZEC 송금자, 수신자 = fiat QR 수령"이라는 비대칭 결제를 다뤄, 결제의 한쪽 끝이 fully off-chain일 때의 facilitator 설계가 들어 있다. Pay Anyone Legend(#37)와 비교하면 #37은 AI/x402 자동화 측, #28은 직접 사용자가 누르는 UX 측. 두 프로젝트가 D 내에서 서로 보완.

---

### #26 Secure Legion [primary in E; 여기는 cross-ref]

(E 측 본문 아래 참조.)

**D 측 핵심.** Tor hidden service 위에서 동작하는 E2E encrypted chat + 결제 요청 시스템. SOL/ZEC 결제 요청과 확인을 채팅 안에서 처리. P2P payment request라는 use case가 `decentralized OTC`/`activist 후원`/`whistleblower bounty` 같은 회의록 D 예시와 정확히 매칭.

---

## 카테고리 E — x402 + Zcash

★★★ 2개. 이번 재조사로 카테고리 E의 정체가 분명해졌다.

### #26 Secure Legion / NLx402 [primary; 카테고리 E의 압도적 prior art]

**프로젝트 개요.** Tor hidden service 위 E2E 암호화 채팅 안에서 **NLx402 quote/hash로 SOL/ZEC 결제 요청·검증을 in-chat으로 실행**하는 serverless P2P 메신저. Kotlin/Rust + XChaCha20-Poly1305 + Tor + zcashd. SecurePayManager.kt + secure-legion-core/src/securepay/ 모듈.

**NLx402 정체.** 처음에는 자체 명명인 줄 알았으나 **재조사 결과 NLx402는 PCEF (Perkins Coie Entrepreneur Fund, 501(c)(3) nonprofit)이 운영하는 Solana 기반 x402 facilitator**임이 확인됨. 다른 프로젝트(`aiindigo925/btcfi-api`)의 README에 "**Dual-network x402 micropayments in USDC** — Base via Coinbase facilitator, Solana via **NLx402 by PCEF (nonprofit)**"라고 명시돼 있다. 즉:

- **L402** = Lightning Service Authentication Tokens (Lightning Labs)
- **x402** (Coinbase) = HTTP 402 기반 micropayment 프로토콜, Base 위 Coinbase facilitator로 실행
- **NLx402** (PCEF) = 같은 x402 family의 Solana 변형 facilitator, USDC denominated

Secure Legion의 README 인용:

> **NLx402 Payment Protocol:** Inspired by the HTTP 402 code ('Payment Required'). Creates a quote_id and hash for every transaction. Prevents 'replay attacks' (claiming the same payment twice) by tracking these hashes in a local SQLCipher database.

> Secure Pay Protocol: *Built on NLx402 payment protocol core logic*

> Acknowledgment: PCEF — 501(c)(3) nonprofit supporting open source crypto projects; **NLx402 payment protocol core logic**

**Zcash 측 통합 방식.** Secure Legion이 한 일은 NLx402의 *quote 흐름*을 Zcash로 가져온 것. Merchant가 `(quote_id, quote_hash, amount, currency, expiry)`를 만들면 Tor + XChaCha20 chat을 통해 사용자에게 전달, 사용자가 ZEC tx를 보낼 때 **memo 필드에 `NLx402:<quote_hash>`** 를 박는다. Verification = chain을 스캔해 memo가 local SQLCipher의 hash와 매칭되는 tx를 찾고, hash uniqueness로 replay 차단. **30개 분석 프로젝트 중 x402-family quote token을 Zcash shielded memo에 명시적으로 carry하는 유일한 케이스**.

**누구를 위한 건가.** API/콘텐츠 micropayment를 받는 머천트 + 그 결제를 Zcash로 처리하고 싶은 운영자. Tor 내 P2P 결제 use case는 부수적 (호스팅 머천트 antonlivaja/Xenush가 만들었으니 자연스러운 표적).

**우리 아이디어와의 연결.** 카테고리 E의 **단일 가장 중요한 prior art**. 패턴이 거의 그대로 access pass + x402 결합:

1. Server: `quote_hash` 발급
2. Client: ZEC shielded tx + memo `NLx402:<quote_hash>`
3. Server: viewing key로 memo 확인 + amount 검증 + replay DB 갱신
4. Server: 402 응답 해제, 리소스 access 제공

우리가 카테고리 E를 잡으면 위 흐름을 그대로 차용하고 차별화 포인트를 (a) selective disclosure 추가 (어떤 amount를 어디까지 공개), (b) NLx402 의존을 줄이고 Zcash 자체를 facilitator로 재설계, (c) refund / dispute 흐름 정식화 — 이 셋 중 하나로 잡을 수 있다.

추가 조사: NLx402 자체의 spec 문서를 PCEF에서 더 찾아봐야 한다. Secure Legion이 wrap한 부분과 spec 본체의 경계를 구분해야 우리 차별화가 정확해진다.

---

### #37 Pay Anyone Legend [본문은 D 참조]

위 D 본문 참조. 핵심 추출:

- README에 **x402 명시 사용** ("x402 Payment Protocol: HTTP 402 standard")
- 사용자 ZEC 입금 → 1Click 브릿지 → MPC 서명 → x402 결제 자동 실행
- "Private cross-chain x402 payments for merchants"가 self-positioning
- **Zcash 측 깊이는 얕다 (1Click API에 위임, Z-address 생성도 mock)**

Secure Legion(#26)과 비교: **Secure Legion = Zcash가 x402 영수증의 carrier (직접 통합), Pay Anyone Legend = x402 결제 이전에 ZEC가 USDC로 변환되는 funding 단계 (외부 swap에 위임)**. 즉 Pay Anyone Legend의 패턴은 우리가 차별화하기 가장 명확한 지점 — Zcash를 x402 facilitator의 settlement asset으로 직접 만든다는 narrative가 자연스럽게 나온다.

---

## Cross-cutting Projects

여러 카테고리에서 ★★★인 프로젝트들 (= 시간 압박 시 가장 ROI 높은 정독 대상):

| 프로젝트 | ★★★ 카테고리 | 한 줄 |
|---|---|---|
| #4 Zcash↔Aztec Bridge | A + B | memo decoder lift-and-use + commitment-from-memo access pass 둘 다 |
| #5 SIP / Sipher | C + D | viewing key SDK + AI agent privacy skill |
| #20 t2z d4mr | C | WASM-first PCZT |
| #21 t2z gstohl | C | 네이티브 + multi-party combine API (multi-sig 측 더 강함) |
| #26 Secure Legion | D + E | NLx402 = Solana x402 facilitator를 Zcash memo로 carry |
| #37 Pay Anyone Legend | D + E | x402 명시 + AI agent 결제 |
| #48 Obscura | B + D | shielded 구독 결제 + 카피 트레이딩 |
| #54 Temi | C | TypeScript PCZT 가장 깊은 구현 |
| #70 Zchat | A | Memo KEX 메시저 직접 prior art |

이 9개는 **어느 방향을 선택하든 한 번은 봐야 한다**.

## v2 변경 요약 (재조사로 추가/수정된 항목)

기존 v1 → v2:

- **카테고리 A**: 2개 → 5개 (+#4, #7, #19)
- **카테고리 B**: 5개 → 6개 (+#10, #29; #58 cross-ref 유지)
- **카테고리 C**: 6개 → 6개 (#20과 #21 차별화 명시, #21이 multi-party 측에서 더 강함)
- **카테고리 D**: 4개 → 7개 (+#5, #12, #28; #26 cross-ref)
- **카테고리 E**: 2개 → 2개 (NLx402 정체 확인; **Secure Legion이 카테고리 E의 압도적 prior art로 격상**)

가장 큰 결정 변수는 **NLx402 = Solana x402 facilitator (PCEF)** 라는 사실 확인. 카테고리 E가 더 이상 "thin prior art"가 아니라 "concrete reference + clear differentiation angle"로 위치가 바뀐다.

## 결론 — 우리 다음 액션

5개 카테고리 ★★★ 풍성도와 우리 차별화 여지를 함께 보면:

1. **카테고리 A (Memo KEX Messenger)** — ★★★ 5개 (Rime, Zchat, #4, #7, #19). 이번 재조사로 *encoding 측 코드 reference가 풍부해졌다* (#4 Rust watcher, #7 memo 포맷, #19 capsule 포맷). retrieval은 Rime 단독. 우리 차별화("memo=KEX only, 본문=P2P/NOSTR"로 좁히기)가 가장 명확. **우선 후보 1**.

2. **카테고리 E (x402 + Zcash)** — ★★★ 2개지만 NLx402 정체 확인으로 prior art 밀도가 v1 대비 크게 높아졌다. Secure Legion 코드를 정독하고 "Zcash 자체를 x402 facilitator로 직접 만들기"로 차별화하면 narrative가 명확. Pay Anyone Legend가 "ZEC funding → USDC → x402"로 우회한 부분을 우리가 직접 채우는 자세. **우선 후보 2**.

3. **카테고리 C (Accountable Toolkit)** — ★★★ 6개로 가장 풍성하지만, 그만큼 차별화 압력이 크고 "왜 또 만드는가"의 question에 답해야 한다. 우리만의 추가 측면(merchant 영수증 표준, refund 흐름, selective disclosure UI)이 정해지지 않은 상태에서는 위험.

4. **카테고리 B (Private Access Pass)** — ★★★ 6개, MVP 7-10일 가능. 차별화 narrative가 단순 paywall과 어떻게 다른지 product-side 보강 필요. 다만 #4 + #58 + #10 + #29의 4가지 commitment/HTLC/stealth 변형이 모두 lift-and-use 가능해서 *구현 측 위험은 가장 낮다*.

5. **카테고리 D (구체적 use case)** — ★★★ 7개로 풍성하지만 각각이 다른 use case라 단독 카테고리라기보다는 다른 카테고리(특히 A 또는 E)의 narrative wrapping에 가깝다.

**가장 추천하는 다음 단계:**

1. **A 또는 E를 골라 정독.** A를 선택하면 Zchat (memo=transport 한계 실증) + Rime (retrieval 측) + #4 (memo decoder 코드) 순으로 본다. E를 선택하면 Secure Legion (NLx402 carry 코드) + NLx402 PCEF spec + Pay Anyone Legend (x402 facilitator 흐름) 순.
2. **회의록 "아직 확인해야 할 점" 12개 답 채우기.** 특히 Zcash memo 크기 제한 + 암호화 + 지갑 지원 + transport 후보 (NOSTR/libp2p), x402 + Zcash 결합의 정확한 spec.
3. **MVP 범위를 1-2주 단위로 자르기.** A를 고르면 KEX-only 메신저 PoC + Rime fork retrieval, E를 고르면 NLx402 in Zcash memo + 영수증 검증 pipeline.
