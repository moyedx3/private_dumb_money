# A1 → 최신 A2 통합: 예상 충돌과 해소 기준

> 기준 시점: `a1` (`e3b38b6`)과 `origin/master` (`c4a7c77`)를 비교했다.
> 공통 조상은 `f6df5b3`이다. 이 문서는 **병합 전에** 충돌을 예측하고,
> A1 결제 스캐너를 A2 TEE/HTTP 런타임에 붙일 때 무엇을 정답으로 삼을지 기록한다.

## 목표

`origin/master`의 A2 서버를 기반으로 A1 모듈을 통합한다.

```text
Creator /provision
  → A2 CatalogStore (비밀 보관)
  → A1 scan_loop / Engine (UFVK로 결제 감지)
  → A2 dispatch FsBucket
  → Buyer GET /dispatch, GET /dispatch/:key
```

이미 master에 들어간 아래 Lane-B 요청을 A1에서 재구현하지 않는다.

- `GET /dispatch`, `GET /dispatch/:key`와 content/dispatch store 분리 (`aa302c9`)
- public catalog의 `deposit_addr` 및 투명 주소 거부 (`cf3ec4d`)
- A1 `A1B64:` memo 폴백을 포함한 최신 인터페이스 문서 (`75bcbec`)

## Git이 표시할 직접 겹침

| 파일 | 충돌 종류 | 원인 | 해소 원칙 |
| --- | --- | --- | --- |
| `.gitignore` | add/add | A1은 `/.pnpm-store/`, A2는 agent artifact ignore를 추가했다. | 양쪽 항목을 모두 유지한다. |
| `indexer/Cargo.toml` | add/add | 두 브랜치가 각자 `drop-indexer` crate를 생성했다. A1은 Zcash/lightwalletd 의존성, A2는 axum/TEE/HTTP 의존성을 가진다. | A2 manifest를 골격으로 하고 A1의 필요한 의존성·build-dependency를 합친다. 동일 crate의 중복 버전은 Cargo가 하나로 해소하게 한다. |
| `indexer/Cargo.lock` | add/add | 위 manifest가 완전히 달라 lockfile도 독립적으로 만들어졌다. `dryoc`도 A1은 0.7, A2는 0.8이다. | 수동 병합 금지. 통합 `Cargo.toml` 확정 뒤 `cargo generate-lockfile` 또는 `cargo check`로 재생성한다. |
| `indexer/src/lib.rs` | add/add | A1은 scanner/engine 모듈과 좁은 `Catalog`·`Bucket` 경계를 정의했고, A2는 provision/server/storage와 실제 타입을 정의했다. | A2의 실제 도메인 타입을 기준으로 하고, A1 모듈 선언을 추가한다. `DropConfig`에는 `h_content`를 반드시 보존하며 `Bucket`은 A2의 `put/get/list` 형태를 채택한다. |
| `team/interfaces.md` | changed/changed | A1은 I1의 raw + `A1B64:` 계약을 추가했고, A2는 I2 dispatch API와 I3/I5의 `h_content`·`deposit_addr`를 추가했다. | 두 변경을 모두 유지한다. 최종 문서는 `origin/master:team/interfaces.md`를 기준으로 I1 텍스트 폴백까지 포함해야 한다. |

## Git 충돌은 아니지만 반드시 해결할 의미적 충돌

### 1. A1의 mock API와 A2의 실제 HTTP API가 중복된다

| A1 현재 | A2 실제 계약 | 결정 |
| --- | --- | --- |
| `api.rs`의 `/api/catalog`, `/api/buyers/dispatch...` 벡터 | `/catalog`, `/dispatch`, `/dispatch/:key`, `/bucket/:key` | A2 경로를 production canonical contract로 사용한다. `ApiVectors`는 단위 테스트용 mock으로만 남기거나 제거한다. |
| `ApiVectors`가 `Catalog + Bucket`을 함께 구현 | `CatalogStore`와 content/dispatch `FsBucket`이 분리됨 | 실제 Engine에는 `CatalogStore`와 **dispatch** `FsBucket`을 주입한다. content store에 dispatch를 쓰면 안 된다. |

### 2. 공유 타입의 필드와 trait 폭이 다르다

- A1 `DropConfig`에는 `price_zat`, `k_drop`, `creator_ufvk`, `deposit_addr`만 있다.
- A2 `DropConfig`에는 Buyer가 content를 찾는 데 필요한 `h_content`도 있다.
- A1 `Bucket`은 `put`만, A2 `Bucket`은 `put/get/list`를 제공한다.

**해소:** A2 `DropConfig`과 `Bucket` trait을 단일 정의로 채택한다. A1 Engine은
필요한 `put`만 호출하므로 넓어진 trait에도 그대로 맞아야 한다. A1의 테스트 mock도
`get/list`를 구현해 컴파일하게 고친다.

### 3. 스캐너는 아직 서버에 실행 경로가 없다

A2 `src/main.rs`는 `CatalogStore`와 두 `FsBucket`을 만들지만, A1 scan loop를
spawn하지 않는다. A1 `scan_loop::scan_once`는 단일 UFVK와 명시 block range를 받는
라이브러리 함수다.

통합 시 결정할 항목:

1. provision된 모든 drop/UFVK를 열거할 내부 catalog API를 추가한다.
2. 각 UFVK의 마지막 스캔 높이와 replay txid를 저장할 state keying을 정한다.
3. lightwalletd endpoint, network, poll interval, 시작 높이를 런타임 설정으로 정의한다.
4. state 저장 암호키는 개발용 `A1_STATE_KEY_HEX` 대신 dstack/KMS 또는 TEE sealing key로
   공급한다.
5. HTTP 서버와 scanner task의 종료·오류·재시작 정책을 정한다.

이 항목이 완료되기 전에는 A1 코드가 존재해도 실제 결제가 dispatch로 이어지지 않는다.

### 4. 의존성과 프레임워크 버전

- A1: `dryoc 0.7`, tonic/prost, Zcash scanning crates, build-time protobuf 생성.
- A2: `dryoc 0.8`, axum 0.7, serde, tower-http CORS, zeroize.

**해소:** 먼저 `dryoc 0.8`에서 A1의 sealed-box 호출과 state API가 빌드되는지 확인한다.
호환되지 않으면 A2를 0.7로 내리지 말고, A1 호출부를 0.8 API에 맞춰 수정한다.
TEE server의 보안 수정과 배포 검증을 보존하기 위함이다.

## 권장 통합 순서

1. 작업 트리를 깨끗하게 만든 뒤 `origin/master`를 기준으로 통합 브랜치를 만든다.
2. 위의 다섯 직접 겹침을 해소하고 lockfile을 재생성한다.
3. A1 모듈을 A2 crate에 선언하고, 공유 `DropConfig`/`Catalog`/`Bucket`을 하나만 남긴다.
4. A1 Engine이 `CatalogStore` + dispatch `FsBucket`으로 dispatch를 발행하는 통합 테스트를 만든다.
5. 그 다음에만 server startup에 scanner task와 영속 상태를 연결한다.
6. Buyer/Creator 앱을 포함한 HTTP 통합 검증과 실제 wallet memo 검증을 한다.

## 병합 완료의 최소 검증

```text
cargo fmt --check
cargo test --manifest-path indexer/Cargo.toml
cargo check --manifest-path indexer/Cargo.toml
```

추가로 아래 흐름을 자동 테스트한다.

```text
sealed /provision
→ CatalogStore lookup
→ A1 Engine.on_note
→ dispatch FsBucket put
→ GET /dispatch 에 key 노출
→ GET /dispatch/:key 에 80-byte blob 반환
```

## 범위 밖 (후속)

- `R-A2-4`: buyer 읽기 트래픽을 TEE host에서 분리할 외부 bucket/CDN 도입.
- catalog 영속화: 현재 `CatalogStore`는 메모리 저장소라 재시작 시 re-provision이 필요하다.

이 둘은 데모용 A1/A2 통합의 선행 조건은 아니지만, 운영 배포 전 해결해야 한다.
