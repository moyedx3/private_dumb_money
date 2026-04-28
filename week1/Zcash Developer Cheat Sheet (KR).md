
Zcash 앱을 만들기 위한 실용 가이드. 암호학자가 아닌 개발자를 위해 작성됨.

---

## 1. Zcash 작동 방식 (전체 그림)

```
Zcash 블록체인 (수천 개의 노드에 분산)
                         │
                  풀 노드 (Zebra / zcashd)
                  블록 검증, 체인 저장
                         │
          ┌──────────────┴──────────────┐
          │                             │
     lightwalletd                  JSON-RPC API
     (compact block 서버)          (포트 8232)
          │                             │
     gRPC 엔드포인트              코드에서 직접 호출
          │                     z_sendmany 등
          │                     (접근법 A)
          │
     ┌────┴─────────────────────────────────┐
     │          librustzcash                │
     │  (핵심 Rust 크레이트 — 모든 라이트    │
     │   클라이언트가 이 위에 구축됨)        │
     └────┬─────────────┬──────────┬────────┘
          │             │          │
     zingolib      Android/    WebZ.js
     (Rust)        iOS SDK     (WASM)
          │             │          │
     백엔드 서비스   모바일 앱    브라우저 앱
                  (접근법 B)
```

### Layer 1: 블록체인

Zcash는 비트코인에서 포크된 분산 원장이다. 수천 개의 노드가 동일한 트랜잭션 히스토리 사본을 유지한다. 블록은 약 75초마다 추가된다.

비트코인과의 핵심 차이: Zcash 트랜잭션은 **shielded** 될 수 있다. 네트워크가 송신자, 수신자, 금액을 보지 않고도 트랜잭션이 유효한지(무에서 돈이 생성되지 않았는지) 검증한다. 영지식 증명을 사용.

### Layer 2: 주소 풀

Zcash에는 같은 체인 위에 두 가지 자금 풀이 있다:

- **Transparent (t-주소)** — 비트코인과 동일하게 작동. 잔액과 트랜잭션이 공개적으로 보임. 하위 호환성을 위해 존재.
- **Shielded (z-주소)** — 프라이버시 레이어. 세 세대: Sprout (deprecated), Sapling (널리 사용), Orchard (최신, Halo 2 증명, trusted setup 없음). Shielded-to-shielded 트랜잭션은 외부 관찰자에게 아무것도 드러내지 않음.

### Layer 3: 노드

노드 소프트웨어는 P2P 네트워크에 연결하여 모든 블록을 다운로드/검증하고, 전체 체인을 저장 (~300GB 메인넷)하며, 합의 규칙을 적용한다.

두 가지 구현체가 존재:

| 노드         | 언어   | 상태                                       |
| ---------- | ---- | ---------------------------------------- |
| **zcashd** | C++  | 원본, 단계적 폐지 중                             |
| **Zebra**  | Rust | Zcash Foundation이 개발, 미래 (NU7 이후 유일한 노드) |

노드 자체는 검증자일 뿐이다. 지갑 기능을 추가하지 않으면 "당신의" 돈에 대해 아무것도 모른다.

### Layer 4: 지갑 기능

완전히 다른 두 가지 접근법:

**접근법 A — 내장 지갑 (zcashd 내부):** 노드에 지갑이 내장되어 있다. 키 생성, note 스캐닝, 트랜잭션 구성, 증명 생성이 모두 하나의 프로세스 안에서 이루어짐. JSON-RPC로 통신. 편리하지만 밀접하게 결합됨. 다이어그램의 오른쪽.

**접근법 B — 외부 지갑 (라이트 클라이언트):** 지갑이 lightwalletd 서버에 연결하는 완전히 별개의 소프트웨어. 다이어그램의 왼쪽이며 여러 레이어로 구성:

### Layer 5: lightwalletd (미들웨어)

lightwalletd는 풀 노드와 라이트 클라이언트 사이에 위치한다. 전체 블록을 "compact block"으로 축소한다. 라이트 클라이언트가 자기 트랜잭션을 스캔하는 데 필요한 최소한의 데이터만 제공. 키를 보관하지 않고, 지갑에 대해 모른다. 단순한 데이터 프록시.

### Layer 6: librustzcash (핵심 라이브러리)

[github.com/zcash/librustzcash](https://github.com/zcash/librustzcash)

모든 라이트 클라이언트는 librustzcash 위에 구축된다. 실제 암호학적 작업을 처리하는 공식 Rust 크레이트. Zcash 지갑 로직의 단일 진실 소스(single source of truth). lightwalletd와 직접 상호작용하지 않고, librustzcash가 대신 해준다.

핵심 크레이트:

- `zcash_primitives` — 트랜잭션 데이터 구조
- `zcash_keys` — 키 파생 (spending key, viewing key)
- `zcash_client_backend` — 지갑 로직 (스캐닝, note 추적, tx 구성)
- `zcash_client_sqlite` — 지갑 상태용 SQLite 저장소
- `zcash_proofs` — 증명 생성 (Sprout)
- `pczt` — 부분 구성 트랜잭션 (다자간 서명 / FROST용)

### Layer 7: SDK와 지갑 라이브러리 (하나 선택)

librustzcash 위에서, 플랫폼에 맞는 라이브러리를 선택:

**zingolib** (Rust) — [github.com/zingolabs/zingolib](https://github.com/zingolabs/zingolib) 가장 올인원인 옵션. librustzcash를 고수준 API로 래핑:

- "지갑 생성" → 키 파생, DB 초기화, 파라미터 설정 처리
- "동기화" → 전체 동기화 루프, compact block 가져오기, trial decryption 처리
- "주소로 전송" → tx 구성, 증명, 브로드캐스트 처리
- 자체 동기화 엔진 (pepper-sync)과 lightwalletd gRPC 클라이언트 포함

**Android / iOS SDK** (Kotlin / Swift) — librustzcash의 플랫폼 네이티브 래퍼. 모바일 앱에 사용.

**WebZ.js** (JavaScript / WASM) — [github.com/ChainSafe/WebZjs](https://github.com/ChainSafe/WebZjs) librustzcash를 WebAssembly로 컴파일하여 브라우저에서 사용. 유일한 브라우저 전용 Zcash SDK. 아직 활발히 개발 중 (감사 미완료). lightwalletd 앞에 gRPC-web 프록시 필요.

**비유하자면:** librustzcash = React + Redux + Router. zingolib = Next.js.

이들은 모두 같은 일을 한다. lightwalletd와 동기화, 키 관리, shielded 트랜잭션 구성. 단지 다른 플랫폼을 타겟할 뿐. Node.js나 Python SDK는 없다. 팀이 Rust를 쓰지 않으면 접근법 A (아무 언어, HTTP 호출만) 또는 WebZ.js (브라우저 전용)를 사용.

---

## 2. 접근법 A vs 접근법 B

### 접근법 A: JSON-RPC (자체 노드 운영)

```
풀 노드 (Zebra / zcashd)      →  직접 운영해야 함
JSON-RPC API                   →  노드에 내장
API 래퍼                       →  직접 구축
앱 로직                        →  직접 구축
```

- **키 보관 주체:** 노드가 내부적으로 보관
- **인프라:** 자체 풀 노드 운영 필요 (~10-30GB 테스트넷, ~300GB 메인넷)
- **코드 복잡도:** 낮음. JSON-RPC 엔드포인트에 HTTP 호출만 (`z_sendmany`, `z_getnewaddress` 등)
- **사용 시점:** 결제를 처리하는 백엔드 서비스, 에이전트용 수탁형 지갑, 거래소 통합
- **다른 사람의 노드를 쓸 수 있는가?** 아니오. RPC 포트가 지갑에 대한 루트 접근을 부여. 키가 해당 머신에 존재.

### 접근법 B: 라이트 클라이언트 (공용 인프라 사용)

```
풀 노드 + lightwalletd         →  다른 사람이 운영
라이트 클라이언트 라이브러리    →  이미 존재 (Rust 크레이트, 모바일 SDK, WASM)
지갑 관리 레이어               →  직접 구축
앱 로직                        →  직접 구축
```

- **키 보관 주체:** 당신의 코드가 보관 (폰, 브라우저, 백엔드)
- **인프라:** 제로. 공용 lightwalletd 서버에 연결
- **코드 복잡도:** 높음. 라이브러리 API를 통해 지갑 상태, 동기화 루프, 키 저장 관리
- **사용 시점:** 사용자 대상 지갑, 자기 수탁형 제품, 키가 서버에 닿지 않아야 하는 웹 앱
- **대부분의 앱 개발자가 사용하는 방식**

### 트레이드오프

| | 접근법 A (JSON-RPC) | 접근법 B (라이트 클라이언트) |
|---|---|---|
| 인프라 | 무거움 (노드 운영) | 없음 (공용 서버 사용) |
| 코드 복잡도 | 낮음 (HTTP 호출) | 높음 (Rust 라이브러리) |
| 키 위치 | 노드 위 | 당신의 코드 안 |
| 신뢰 모델 | 수탁형 (노드가 키 보관) | 자기 수탁형 가능 |
| 언어 | 아무거나 (HTTP만 됨) | Rust (또는 플랫폼 SDK) |

---

## 3. lightwalletd: 공용 서버

접근법 B를 쓰는 데 자체 노드를 운영할 필요가 없다. 140개 이상의 공용 lightwalletd 서버가 추적되고 있다:

**서버 상태 대시보드:** [hosh.zec.rocks/zec](https://hosh.zec.rocks/zec)

### 주요 서버

**테스트넷:**

| 서버 | 가동률 (30일) |
|---|---|
| `testnet.zec.rocks:443` | ~99.75% |
| `lightwalletd.testnet.cipherscan.app:443` | 가용 |
| `zcash.mysideoftheweb.com:19067` | ~52% (불안정) |

**메인넷:**

| 서버 | 가동률 (30일) | 핑 |
|---|---|---|
| `zec.rocks:443` | 99.95% | 16ms |
| `na.zec.rocks:443` | 99.54% | 24ms |
| `lwd.zcashexplorer.app:9067` | 99.83% | 92ms |
| `z3.deepikaw.xyz:443` | 99.97% | 118ms |
| `lightwalletd.mainnet.cipherscan.app:443` | 99.95% | 412ms |

전체 목록 (실시간 상태): [hosh.zec.rocks/zec](https://hosh.zec.rocks/zec)

### 자체 lightwalletd를 운영하고 싶다면?

참고: lightwalletd는 뒤에 풀 노드가 필요하다. 독립 실행 불가.

zcash-stack 프로젝트를 통한 Docker 빠른 시작:

```bash
git clone https://github.com/zecrocks/zcash-stack.git
cd docker
./download-snapshot.sh          # 수일간의 동기화 건너뛰기
docker compose up -d            # Zebra + lightwalletd 실행
```

---

## 4. 레퍼런스 프로젝트

### Zipher (Atmosphere Labs)

[github.com/atmospherelabs-dev/zipher-app](https://github.com/atmospherelabs-dev/zipher-app)

프라이버시 우선 Zcash 지갑. 인간과 AI 에이전트 모두를 위한 것. 하나의 Rust 엔진, 세 가지 인터페이스 (모바일 앱, CLI, MCP 서버). zingolib 위에 제품을 만드는 방법의 최고 레퍼런스.

아키텍처:

```
Flutter 모바일 앱 ──(FFI)──→ Rust 엔진 ──→ zingolib ──→ librustzcash ──→ lightwalletd
CLI 바이너리 ───────────────→ Rust 엔진
MCP 서버 (22개 도구) ───────→ Rust 엔진
```

학습할 핵심 패턴: 단일 엔진 / 다중 컨슈머, 지출 정책 엔진, 2단계 전송 흐름 (제안 후 확인), 암호화된 시드 볼트.

### CipherScan

[github.com/Kenbak/cipherscan](https://github.com/Kenbak/cipherscan)

무료 공용 lightwalletd 인프라도 제공하는 Zcash 블록 익스플로러. Next.js로 구축. 앱에 블록 익스플로러 API가 필요할 때 유용.

공용 엔드포인트:

- 메인넷 gRPC: `lightwalletd.mainnet.cipherscan.app:443`
- 테스트넷 gRPC: `lightwalletd.testnet.cipherscan.app:443`
- REST API: `api.mainnet.cipherscan.app/api/*`

### Zingo Wallet (Zingo Labs)

[github.com/zingolabs/zingolib](https://github.com/zingolabs/zingolib)

zingolib이 구동하는 지갑. lightwalletd 연결을 즉시 테스트할 수 있는 CLI (`zingo-cli`) 포함:

```bash
cargo build --release --package zingo-cli
./target/release/zingo-cli --server https://testnet.zec.rocks:443
```

### zcash-devtool (공식)

[github.com/zcash/zcash-devtool](https://github.com/zcash/zcash-devtool)

Zcash 기능 프로토타이핑을 위한 공식 CLI 도구. `zcash_client_backend`와 `zcash_client_sqlite`를 직접 사용. zingolib 없음. 래퍼 라이브러리 없이 librustzcash를 연결하는 방법을 보고 싶다면 좋은 레퍼런스 코드.

---

## 5. 주요 리소스

| 리소스 | URL | 용도 |
|---|---|---|
| ZecHub Developers | [zechub.wiki/developers](https://zechub.wiki/developers) | 가장 잘 정리된 개발자 디렉토리 |
| 공식 문서 | [zcash.readthedocs.io](https://zcash.readthedocs.io/) | zcashd 문서, 통합 가이드, RPC 레퍼런스 |
| 서버 상태 | [hosh.zec.rocks/zec](https://hosh.zec.rocks/zec) | 실시간 lightwalletd 서버 모니터링 |
| Zcash GitHub | [github.com/zcash](https://github.com/zcash) | librustzcash, lightwalletd, SDK, zcash-devtool |
| Zcash Foundation | [github.com/ZcashFoundation](https://github.com/ZcashFoundation) | Zebra 노드, FROST |
| Zingo Labs | [github.com/zingolabs](https://github.com/zingolabs) | zingolib |
| 커뮤니티 포럼 | [forum.zcashcommunity.com](https://forum.zcashcommunity.com/) | 기술 토론, 그랜트, 공지 |
| 테스트넷 Faucet | "Zcash testnet faucet" 검색 | 개발용 무료 테스트 ZEC |

---

## 6. 빠른 결정 가이드

**"사용자/에이전트를 위해 지갑을 관리하는 백엔드 서비스를 만든다"** → 가장 빠른 경로는 접근법 A (VPS에 zcashd), 표준 아키텍처는 접근법 B (백엔드에 zingolib).

**"사용자가 자기 키를 직접 보유하는 웹 앱을 만든다"** → 접근법 B, WebZ.js (브라우저) 또는 zingolib/zcash_client_* (서버 사이드).

**"모바일 지갑을 만든다"** → Android SDK (Kotlin) 또는 iOS SDK (Swift). 둘 다 librustzcash를 래핑.

**"웹사이트에서 Zcash 결제를 받고 싶다"** → BTCPay Server + Zcash, 또는 접근법 A에 간단한 결제 리스너.

**"빠르게 프로토타이핑하고 테스트하고 싶다"** → `zingo-cli` 설치, `testnet.zec.rocks:443`에 연결, 몇 분 안에 테스트 트랜잭션 전송 시작.

**"팀이 Rust를 모른다"** → (a) 접근법 A, zcashd 사용. 아무 언어로 HTTP 호출 가능, 또는 (b) 브라우저용이면 WebZ.js.
