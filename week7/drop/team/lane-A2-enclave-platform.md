# 레인 A2 — Enclave 플랫폼 (서버의 껍데기 + 신뢰의 척추)

> **담당: Rust #2.** 서버가 도는 TEE(아무도 못 들여다보는 밀봉된 하드웨어 방, Intel TDX) 안의 "껍데기"를 만든다.
> A1이 만드는 결제 엔진은 *내용물*이고, A2는 그 엔진이 살 **집(HTTP 서버) + 비밀을 안전하게 받는 현관(provisioning) + "나 진짜 그 코드 그대로 돈다"는 증명서 발급소(attestation) + 배포 파이프라인**이다.
> 인프라 역할(public bucket 스텁 포함)도 A2로 흡수됨.
>
> **A1과 공유하는 것:** 같은 indexer 크레이트. A2는 그 안에서 `provision` / `attest` / `server` 모듈 + 배포를 소유한다. A1은 `Catalog::lookup`으로 A2가 채운 카탈로그를 *읽기만* 한다.
> **정확한 데이터 모양은 항상 [`interfaces.md`](./interfaces.md) 참조** (특히 I3, I5, I6).

---

## 1. 한 줄 요약

크리에이터가 콘텐츠 열쇠(`K_drop`)를 **측정된 enclave만 열 수 있게** 봉인해 넣는 `POST /provision`(I5)과, "나 진짜 그 오픈소스 코드다"를 Intel 서명 quote로 증명하는 `GET /attest`(I6)를 만들고, 받은 비밀을 내부 카탈로그(I3-b)에 저장해 A1이 읽게 한 뒤, 이 전부를 Docker로 재현빌드해 Phala에 띄운다 — **스파이크 #3가 진짜 Phala 하드웨어에서 이미 "된다"를 증명한 그 흐름**을 진짜 런타임 엔드포인트로 구현하는 것.

---

## 2. 큰 그림에서 내 위치

```
  크리에이터(C)                    [ A2 = enclave 껍데기 ]                      A1(엔진)
  (남의 컴퓨터)                    Phala TDX 안, 운영자도 못 봄                (같은 enclave 안)
      │                                                                          │
      │ ① GET /attest ─────────────▶ ┌──────────────────────────────────┐        │
      │ ◀── quote + provisioning ──── │ /attest  (I6)                    │        │
      │     pubkey                    │   report_data = sha256(pubkey)   │        │
      │                               │   → quote에 공개키를 못 박음       │        │
      │ ② quote를 Intel 체인까지 검증  │                                  │        │
      │   + 측정값=공개레포 재현빌드?  │  [provisioning keypair]           │        │
      │                               │   = dstack KMS 파생 (측정값당 고정)│        │
      │ ③ sealed = crypto_box_seal(   │                                  │        │
      │     {drop_id,price_zat,k_drop,│ /provision (I5)                  │        │
      │      ufvk,h_content}, pubkey) │   crypto_box_seal_open(sealed)    │        │
      │   POST /provision ──────────▶ │   → DropConfig 저장 (I3-b)  ──────┼──▶ Catalog::lookup
      │                               └────────────┬─────────────────────┘    (A1이 읽음)
      │                                            │ /catalog (공개 JSON, I3-a)
      └────────────────────────────────────────────┴──▶ 구매자 앱(B)이 목록 조회

  운영자(Phala)는 ②③에서 오가는 게 전부 암호문(sealed)이라 K_drop을 절대 못 봄.  ← 핵심 보안 속성
```

핵심 한 문장: **A2는 "비밀이 들어오는 문"과 "그 문이 진짜임을 증명하는 도장"을 만든다.** 엔진(A1)은 그 비밀을 쓸 뿐, 받지 않는다.

---

## 3. 내가 받는 것 / 내보내는 것

| 방향 | 인터페이스 | 엔드포인트 / 함수 | 모양 (정확값은 `interfaces.md`) |
|---|---|---|---|
| **받음** | **I5** provisioning | `POST /provision` (body = `sealed`) | `crypto_box_seal({drop_id, price_zat, k_drop[32], creator_ufvk, h_content}, enclave_provisioning_pubkey)` — libsodium sealed box |
| **내보냄** | **I6** attestation | `GET /attest` → `{ quote_hex, provisioning_pubkey, code_measurement, event_log, vm_config }` | TDX quote. `report_data[0..32] = sha256(provisioning_pubkey)` |
| **내보냄 (A1에게, enclave 내부)** | **I3-b** internal DropConfig | `Catalog::lookup(drop_id) -> Option<DropConfig{price_zat, k_drop, creator_ufvk}>` | 절대 공개 안 함. enclave 메모리/파일에만 |
| **내보냄 (B에게, 공개)** | **I3-a** public catalog | `GET /catalog` → `[{drop_id, price_zec, h_content, title}]` | 공개 JSON. **k_drop / ufvk 절대 미포함** |

> 용어 빠른 정의(처음 한 번):
> - **sealed box** = "상대 공개키로만 열 수 있게" 포장하는 libsodium 표준. 보내는 쪽은 매번 임시 키쌍을 만들어 쓰므로 **나(보내는 사람)도 다시 못 연다.** Rust는 `dryoc`(A1과 동일 크레이트), JS(크리에이터 앱)는 `libsodium-wrappers`. 곡선 양쪽 Curve25519.
> - **attestation(인증)** = 하드웨어가 "지금 이 enclave 안에서 *정확히 이 코드*가 돈다"를 Intel 개인키로 서명해 주는 증명서(=quote).
> - **report_data** = quote 안에 우리가 **64바이트 자유 데이터**를 넣을 수 있는 칸. 여기에 공개키 해시를 넣으면 "이 quote ↔ 이 공개키"가 위조 불가능하게 묶인다.
> - **measurement (MRTD / RTMR)** = enclave 안에서 도는 코드/이미지의 해시. 크리에이터는 이걸 공개 레포의 재현빌드 해시와 대조해 "운영자가 코드를 몰래 안 바꿨다"를 확인한다.
> - **KMS-derived key** = dstack(Phala의 TEE SDK)가 **measurement로부터 결정적으로** 만들어 주는 enclave 전용 키. 같은 코드면 항상 같은 키, 코드가 바뀌면 다른 키. (→ 8장 `[C4]` 함정의 원인.)

---

## 4. 만드는 것 (단계별)

스택: `axum 0.7`, `dryoc 0.5`(sealed box), `sha2`, `serde`/`serde_json`/`ciborium`(CBOR), `tokio`. dstack 호출은 `attest.rs`의 `post_uds_json`(unix 소켓 HTTP)을 그대로 재사용. 모듈은 indexer 크레이트의 `provision` / `attest` / `server`.

### (a) `GET /attest` — 공개키를 quote에 못 박기 (`attest.rs` 재사용)

clean-wallet의 `DstackAttestor`(`attest.rs`)는 이미 `get_quote(report_data: &[u8;32])`를 구현해 둠 — 32바이트 해시를 64바이트 reportData 슬롯에 zero-pad해서 dstack `/GetQuote`로 POST한다. **A2는 그 `report_data`에 무엇을 넣을지만 바꾸면 된다.**

- clean-wallet은 `report_data = sha256(screening artifact)` (결과를 묶음).
- **드롭은 `report_data = sha256(enclave_provisioning_pubkey)`** (그 quote로 검증한 사람이 **바로 그 공개키로** I5를 암호화하게 못 박음).

```rust
// server.rs 의 attestation 핸들러를 이렇게 바꾼다 (clean-wallet은 report_data=[0u8;32]였음)
async fn attest(State(s): State<AppState>) -> Response {
    let pubkey = s.provisioning_pubkey;                 // [u8; 32], (b)에서 파생
    let report_data: [u8; 32] = Sha256::digest(pubkey).into();   // ← 공개키를 quote에 바인딩
    match s.attestor.get_quote(&report_data).await {
        Ok(q) => Json(AttestResponse {
            quote_hex: q.quote_hex,
            provisioning_pubkey: hex::encode(pubkey),    // 크리에이터가 검증 후 이 키로 암호화
            code_measurement: s.code_measurement.clone(),
            event_log: q.event_log, vm_config: q.vm_config,
        }).into_response(),
        Err(_) => err(StatusCode::SERVICE_UNAVAILABLE, "Attestation hardware unavailable, retry."),
    }
}
```

크리에이터 쪽(레인 C) 검증 순서: ① quote를 Intel 체인까지 검증(`@phala/dcap-qvl-web` 또는 t16z) → ② `Mrtd/Rtmr`가 공개 레포 재현빌드 해시와 일치하는지 → ③ **`report_data[0..32] == sha256(응답의 provisioning_pubkey)` 인지 직접 다시 계산.** 셋 다 통과해야 그 공개키를 신뢰하고 I5 암호화에 쓴다. ③을 빼먹으면 운영자가 quote는 진짜인데 공개키만 자기 걸로 바꿔치기할 수 있으니, **이 바인딩이 secret-IN의 전부다.**

### (b) provisioning 키쌍 — dstack KMS 키에서 파생 (measurement당 고정)

enclave는 자기 X25519 키쌍(`provisioning_pub` / `provisioning_priv`)이 필요하다. **이 키를 enclave 안에서 매번 랜덤 생성하면 안 된다** — 재시작/스케일아웃마다 공개키가 바뀌어 크리에이터가 검증한 공개키로 다시 못 보낸다. 대신 **dstack KMS가 measurement로부터 결정적으로 파생해 주는 seed**를 받아서 키쌍을 만든다 (같은 코드 = 항상 같은 키쌍).

- dstack 게스트 API는 `/GetQuote`/`/Info`와 **같은 unix 소켓**에 KMS 키 파생 호출(`GetKey` 계열)을 제공한다. `attest.rs`의 `post_uds_json(socket, "/GetKey", &json!({ "path": "drop/provisioning" }))`로 호출 → 응답의 키 바이트를 시드로 사용.
- 그 시드로 X25519 키쌍을 만든다: `let kp = dryoc::keypair::KeyPair::from_secret_key(seed_to_x25519(&kms_key));`. 부팅 시 1회 파생해 `AppState.provisioning_pubkey/_secret`에 보관.
- **오프라인 경로도 존재(스파이크 #3가 확인):** `phala envs encrypt`가 바로 이 CVM 키로 암호문을 만든다 — 즉 크리에이터가 푸시 없이 `K_drop`을 미리 암호화할 수 있다. 우리 `/attest`는 그 공개 절반을 **런타임에** 노출하는 것뿐이다.

> 왜 KMS 파생이 핵심인가: 운영자가 이미지를 바꾸면 measurement가 바뀌고 → KMS가 **다른** 키를 파생하고 → 그 키로 만든 quote의 `report_data`도 달라진다. 그래서 "공개키 + measurement"가 한 묶음으로 위조 불가. (단점은 `[C4]`, 8장.)

### (c) `POST /provision` — sealed box 열어서 DropConfig 저장

```rust
async fn provision(State(s): State<AppState>, body: Bytes) -> Response {
    // body = crypto_box_seal(payload, provisioning_pub).  운영자는 이 암호문만 본다.
    use dryoc::sealedbox::SealedBox;
    let kp = s.provisioning_keypair();                  // (b)의 KMS 파생 키쌍
    let plain = match SealedBox::unseal_to_vec(&body, &kp) {
        Ok(p) => p,
        Err(_) => return err(StatusCode::BAD_REQUEST, "sealed box does not open with this enclave key."),
        // ↑ 키가 안 맞으면(=다른 빌드 대상으로 암호화됨) 여기서 깔끔히 실패. C4 진단 지점.
    };
    let cfg: ProvisionPayload = match ciborium::from_reader(&plain[..]) {   // CBOR (또는 JSON)
        Ok(c) => c,
        Err(_) => return err(StatusCode::BAD_REQUEST, "decrypted payload is malformed."),
    };
    s.catalog.insert(cfg);                              // I3-b 내부 저장 → A1이 lookup
    StatusCode::NO_CONTENT.into_response()
}
```

- `ProvisionPayload` = `{ drop_id: u64, price_zat: u64, k_drop: [u8;32], creator_ufvk: String, h_content: String }` (정확값 I5).
- 저장 시 **공개 카탈로그 엔트리(I3-a)도 같이 파생**해 둔다: `{drop_id, price_zec, h_content, title}` (여기엔 `k_drop`/`ufvk`가 절대 안 들어감).
- 복호화 평문(`plain`, `k_drop`)은 **로그 금지.** measurement-bound 키로만 열리므로 평문이 enclave 밖으로 나갈 경로가 코드상 없어야 한다.

### (d) 카탈로그 store + 공개 카탈로그 JSON 엔드포인트

`Catalog`는 A1이 `lookup`으로 읽고 A2가 `insert`로 쓴다 (A1 플랜의 mock 경계와 정확히 같은 타입).

```rust
pub trait Catalog: Send + Sync { fn lookup(&self, drop_id: u64) -> Option<DropConfig>; }
// A2가 구현 (데모 = 인메모리; restart 시 재-provision 필요. 영속화는 Open-Q):
pub struct MemCatalog { inner: RwLock<HashMap<u64, (DropConfig, PublicEntry)>> }
```

- `GET /catalog` → 저장된 모든 `PublicEntry`의 JSON 배열 (구매자 B가 목록 표시).
- **DropConfig는 절대 노출하는 라우트가 없어야 한다.** (`/catalog`는 public, `lookup`은 프로세스 내부 함수 — HTTP 경로 없음.)
- 데모는 인메모리로 충분하되, "재시작=비밀 증발"을 코드 주석에 명시. (영속 시 enclave-encrypted 파일 = `[C4]`와 같은 measurement 잠금.)

### (e) Docker 이미지 + 재현빌드 + `phala deploy`

clean-wallet의 `apps/scanner/Dockerfile` 2-스테이지 빌드(`rust:1.88-bookworm` → `debian:bookworm-slim`)와 `scripts/deploy-cvm.sh`를 그대로 베이스로 쓴다. 바꿀 것:

- **dstack 소켓 마운트**(필수): `docker-compose.yml`의 `volumes: [ /var/run/dstack.sock:/var/run/dstack.sock ]` — 이게 있어야 `/attest`/KMS가 동작. clean-wallet compose에 이미 있음.
- **재현빌드(load-bearing).** `--locked`로 `Cargo.lock` 고정 + base 이미지 **digest 핀**(`rust@sha256:…`) + 빌드 인자 제거 → 누가 빌드해도 같은 이미지 해시 = 같은 measurement. 이게 깨지면 크리에이터의 측정값 대조(③)가 무의미해진다(8장).
- **배포 = `phala deploy --name drop-indexer --compose <file> --instance-type tdx.small --wait`** (스파이크 #3과 동일 명령). GHCR 패키지는 **public**이어야 함(Phala가 익명 pull — `deploy-cvm.sh` 주석 참조).
- 배포 후 measurement 캡처: `phala cvms attestation --cvm-id drop-indexer` → quote의 `mr_td`를 공개 레포 README에 게시(크리에이터가 대조할 기준값).

### (f) public bucket 스텁 API (팀용)

A1(blob put)·B(blob get)·C(content put)가 붙을 **최소 버킷**. 데모는 로컬 파일/S3 둘 중 단순한 쪽.

```rust
#[async_trait] pub trait Bucket: Send + Sync {
    async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()>;     // A1/C
    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>>;      // B
}
```

- HTTP로도 노출: `PUT /bucket/:key`, `GET /bucket/:key`. 키는 불투명(`blake2b(ek_pub‖txid)` 등 — I2/I4), 버킷은 **암호문만** 본다(콘텐츠/열쇠 둘 다 enclave 밖에선 암호문).
- 스텁이어도 인터페이스(`Bucket` trait + 2개 라우트)는 첫날 확정 → A1/B/C가 mock으로 개발.

### 스파이크 #3의 `phala envs` ↔ 진짜 런타임 엔드포인트 매핑

스파이크 #3은 `phala envs update`로 **배포자(나)가 config-time에** 시크릿을 봉인했고, enclave가 그걸 풀어 `sha256`을 로그에 찍는 걸 확인했다(평문은 절대 안 나옴). 드롭의 진짜 흐름은 **크리에이터(제3자)가 attestation을 직접 검증한 뒤 런타임에** `K_drop`을 봉인하는 것 — 차이는 *누가/언제* 봉인하느냐 뿐이고, **암호 메커니즘은 동일**하다.

| 스파이크 #3 (검증됨) | 진짜 드롭 (A2가 빌드) |
|---|---|
| `phala envs update -e K_DROP_TEST=…` (배포자가 봉인) | `POST /provision` (크리에이터가 봉인) |
| dstack가 CVM KMS 키로 복호화 → compose `${VAR}` | enclave가 (b)의 KMS 파생 키로 `crypto_box_seal_open` |
| 로그의 `DECRYPTED-INSIDE-ENCLAVE sha256=…` | `Catalog`에 `DropConfig` 저장 |
| `phala envs encrypt`(오프라인 암호문) | 크리에이터가 `/attest`로 받은 pubkey로 `crypto_box_seal` |

→ 스파이크 #3가 "비밀이 measured enclave에만 들어가고 운영자는 암호문만 본다"를 **진짜 하드웨어로 이미 증명**했으므로, A2가 남긴 건 *증명*이 아니라 **그 봉인을 받는 런타임 문(`/provision`)을 짜는 일**뿐이다 (feasibility-review §3.3, spec §4.1 `[C3]`).

---

## 5. 재사용 (복붙 출처)

| 가져올 것 | 파일 | 무엇을 |
|---|---|---|
| dstack `/GetQuote` 래퍼 + report_data 패킹 | `week5/clean-wallet-mvp/apps/scanner/src/attest.rs` | `DstackAttestor::get_quote(&[u8;32])`, `post_uds_json`(unix 소켓 HTTP), `Attestor` trait + `MockAttestor`(테스트). **report_data 값만 `sha256(pubkey)`로 교체.** |
| axum 라우터 + 핸들러 패턴 | `.../apps/scanner/src/server.rs` | `Router::new().route(...)`, `AppState`, `err()` 헬퍼, `DefaultBodyLimit`, CORS, `oneshot` 테스트 패턴. `attestation` 핸들러를 본떠 `/attest`/`/provision`/`/catalog` 작성. |
| dstack 부팅 + measurement 캡처 | `.../apps/scanner/src/main.rs` | `attestor.info()`로 `code_measurement` 받아 `AppState`에 넣는 부팅 시퀀스 그대로. |
| Docker → GHCR → Phala 배포 | `.../scripts/deploy-cvm.sh`, `.../apps/scanner/Dockerfile`, `.../docker-compose.yml` | 2-스테이지 빌드, GHCR public 단계, dstack 소켓 마운트, `phala deploy --wait`. |
| 봉인/언봉인 + 키 타입 | A1 플랜 `plan-a1-payment-flow.md` Task 4 | `dryoc::sealedbox::SealedBox`, `KeyPair`, `StackByteArray<32>` 사용법 (A1과 **같은 크레이트/버전 0.5** → 양쪽 sealed box 호환). |
| secret-IN end-to-end 절차 | `week7/drop/spike3/RUNBOOK.md` + `docker-compose.yml` | `phala deploy → envs update → logs grep → cvms attestation` 명령 흐름. 소켓 마운트가 없으면 시크릿이 컨테이너에 안 들어오는 함정(compose `environment:` 매핑)도 여기 기록됨. |

---

## 6. 테스트하는 법

**로컬 (flow 로직 — Intel 서명은 못 받지만 빠름):**

1. **유닛 테스트(mock):** `MockAttestor`(attest.rs) + `server.rs`의 `oneshot` 패턴으로 `/attest`·`/provision`·`/catalog`를 HTTP 레벨 검증. 핵심 케이스:
   - `/provision`에 `crypto_box_seal(payload, provisioning_pub)`를 보내면 `Catalog::lookup(drop_id)`가 그 `DropConfig`를 돌려준다 (왕복).
   - 잘못된 키로 봉인한 body → `400` ("sealed box does not open").
   - `/catalog` 응답에 `k_drop`/`ufvk` 문자열이 **절대 없음**(부정 단언).
   - `/attest` 응답의 `report_data`(quote에서 추출) `== sha256(provisioning_pubkey)`.
2. **로컬 dstack 시뮬레이터:** `attest.rs`의 `#[ignore]` 라이브 테스트(`live_simulator_get_quote_returns_quote_hex`)가 가리키는 소켓(`~/.phala-cloud/simulator/.../dstack.sock`)으로 `/attest`·KMS 파생까지 실제 호출. **단, 시뮬레이터 quote는 dev-키 서명이라 Intel 검증(Check 1)은 항상 실패** — flow 배선 확인용일 뿐.

**진짜 Phala (genuine quote — Check 1 통과):** 스파이크 #3 RUNBOOK 명령 그대로.

```bash
phala deploy --name drop-indexer --compose week7/drop/... --instance-type tdx.small --wait
# 1) attestation이 진짜 Intel 서명인지: quote_hex → https://proof.t16z.com → "Genuine TDX quote" (Check 1 PASS)
phala cvms attestation --cvm-id drop-indexer
# 2) secret-IN 진짜 동작: 오프라인 암호문으로 /provision 왕복 (스파이크 #3의 envs encrypt 경로)
#    또는 크리에이터 앱(C)으로 /attest 검증 → seal → POST /provision → /catalog에 엔트리 확인
# 3) 운영자 불가시성: phala cvms logs 에 k_drop 평문이 없고 sha256/엔트리만 보이는지 확인
```

> 데모 신뢰성: 시뮬레이터로 전 로직을 통과시킨 뒤, **마지막에 진짜 Phala 한 번**으로 Check 1만 증명하는 게 비용/시간 효율적(스파이크 #3가 검증한 패턴).

---

## 7. 완료 기준 (Definition of Done)

- [ ] `GET /attest` → `{quote_hex, provisioning_pubkey, code_measurement, …}`, `report_data == sha256(provisioning_pubkey)` (유닛+시뮬레이터 통과).
- [ ] provisioning 키쌍이 **dstack KMS에서 파생**되어 재시작해도 동일 (랜덤 생성 아님). 부팅 1회 파생.
- [ ] `POST /provision`이 `crypto_box_seal_open` 후 `DropConfig`를 저장, `Catalog::lookup`가 A1에게 반환. 안 맞는 키 → 400, 평문 로그 없음.
- [ ] `GET /catalog` 공개 JSON에 `k_drop`/`creator_ufvk`가 **절대 없음**(부정 테스트로 보장).
- [ ] `Bucket` trait + `PUT/GET /bucket/:key` 스텁 동작 (A1/B/C가 붙을 수 있음).
- [ ] Dockerfile **재현빌드**: `--locked` + base digest 핀 → 두 번 빌드 시 동일 이미지 해시. `mr_td` 기준값 README 게시.
- [ ] `phala deploy`로 실제 CVM 기동, `proof.t16z.com`에서 **Genuine TDX quote (Check 1 PASS)**.
- [ ] 진짜 Phala에서 secret-IN 왕복 1회(스파이크 #3 RUNBOOK) — 로그에 평문 없음 확인.
- [ ] interfaces.md의 I3-a/I3-b/I5/I6 모양과 1:1 일치 (Day-1 kickoff에서 C·A1과 확정).

---

## 8. 주의 / 함정

- **`[C4]` — KMS 키는 measurement에서 파생된다 → 이미지를 다시 빌드하면 enclave 키가 바뀐다.** 그러면 **이전 빌드에 provisioning했던 크리에이터는 새 빌드에 도달 불가** (그가 봉인한 공개키가 더는 존재하지 않음 → `/provision`이 400으로 떨어짐). 해커톤에선 재배포가 잦으니 **이게 반드시 터진다.** 대응:
  - 배포 측정값을 고정(이미지 digest + `Cargo.lock` 핀)해 **불필요한 재빌드를 막고**, 측정값이 바뀌면 **재-provisioning을 명시적 절차로** 둔다(크리에이터가 `/attest` 다시 받아 다시 봉인).
  - 카탈로그를 영속화하려면 그 저장소도 measurement-bound라 같은 함정 — 재배포 전 "비밀이 증발한다"를 전제로 운영(또는 dstack state migration을 직접 배선해야 키/카탈로그 연속성 확보, 자동 아님).
  - **참고:** Phase 2의 in-enclave Zcash *지출키*는 이 함정이 곧 **자금 동결**이 된다(같은 원인, 더 치명적 — spec §7.6). A2 Phase 1은 지출키 없음(`K_drop`/`ufvk`만) → 최악이 "재-provision 필요"지 자금 손실은 아님.
- **재현빌드가 load-bearing.** 이미지가 재현 불가면 크리에이터의 측정값 대조(③)가 의미를 잃고 → attestation 전체가 "운영자를 믿어라"로 퇴화 = 프로젝트 헤드라인("운영자도 못 본다")이 무너진다. `--locked` + base digest 핀은 옵션이 아니라 **보안 요건**. CI가 이미지 해시를 게시해 제3자가 재빌드로 검증 가능해야 함(spec §6/§7.1).
- **report_data 바인딩을 빼먹으면 secret-IN이 통째로 뚫린다.** quote가 진짜여도 `report_data != sha256(pubkey)`면 운영자가 자기 공개키를 끼워넣어 `K_drop`을 가로챌 수 있다. 크리에이터(C)가 ③(`report_data == sha256(응답 pubkey)`)를 **직접 재계산**하도록 C 스펙에 못 박을 것.
- **운영자는 DoS만 가능, 읽기는 절대 불가.** Phala 운영자는 CVM을 죽이거나 네트워크를 끊을 수 있다(가용성 위협). 하지만 TDX 메모리 격리 + measurement-bound 키 때문에 `K_drop`/콘텐츠는 못 읽는다. 이건 신뢰 모델의 의도된 경계(spec §6) — A2 코드에 평문이 enclave 밖으로 새는 경로(로그·공개 라우트·에러 메시지)를 **하나도** 만들지 말 것.
- **dstack 소켓 마운트 누락 = 조용한 실패.** `docker-compose.yml`에 `/var/run/dstack.sock` 볼륨이 없으면 `/attest`·KMS 파생이 부팅부터 실패한다(스파이크 #3가 이걸로 배포 한 번 날림 — RUNBOOK "Gotcha"). compose `environment:`/`volumes:` 매핑을 부팅 헬스체크로 확인.
- **A1과 sealed box 호환 맞추기.** `dryoc` 버전·곡선(Curve25519)·CBOR vs JSON을 A1/C와 **같은 값**으로 첫날 고정(interfaces.md). 한쪽만 JSON, 한쪽만 CBOR이면 `/provision`이 조용히 malformed로 떨어진다.
