# §1.8 NEAR Rust contract (NEAR Rust 컨트랙트)

> **Cross-reference:**
> - [§1.7 x402 client](./07-x402-client.md) — x402 런타임 실행 경로는 TS `lib/chainSig.ts`에만 존재하며 Rust 컨트랙트는 전혀 관여하지 않음을 확립.
> - [§1.6 NEAR Chain Signatures](./06-near-chain-signatures.md) — 프로덕션에서 NEAR MPC를 사용하는 것은 `v1.signer` 컨트랙트뿐이며, `anyone-pay.near` 컨트랙트는 호출되지 않음을 확립.
> - [§1.4 deposit tracking](./04-deposit-tracking.md) — 모든 결제 상태는 Supabase에 저장됨; NEAR 컨트랙트에 상태가 기록되지 않음을 확립.

---

## 목적 (Purpose)

`contract/src/lib.rs`의 `AnyonePay` NEAR Rust 컨트랙트는 **NEAR 네이티브 x402 facilitator 설계를 위한 인텐트 에스크로 + x402 실행 허브**로 구상되었다. 설계 의도는 다음과 같다: (1) 사용자의 결제 인텐트를 on-chain에 기록하고(`create_intent`), (2) NEAR Intents를 통해 ZEC 입금을 검증하며(`verify_deposit`), (3) 입금 확인 후 `x402.near` facilitator에 `pay()` cross-contract call을 실행하는 것이었다(`execute_x402_payment`). 그러나 프로덕션 TypeScript 코드는 이 컨트랙트의 어떤 메서드도 호출하지 않는다 — 모든 상태는 Supabase에 저장되고, x402 결제는 `lib/chainSig.ts`가 직접 Base mainnet에 브로드캐스트한다. **판정 (f): 설계된 역할이 있으나 라이브 TS 경로에서 완전히 우회(bypassed)된 dead code다.** 컨트랙트는 배포 스크립트(`contract/deploy.sh`)와 테스트 스크립트(`contract/test-contract.sh`)에서만 호출된다.

---

## 파일과 함수 (Files & functions)

| 파일 | 라인 | 함수/심볼 | 역할 | 런타임 도달 가능성 |
|------|------|-----------|------|--------------------|
| `contract/src/lib.rs` | 9–18 | `struct Intent` | 결제 인텐트 데이터 구조 (8개 필드) | (자료구조) |
| `contract/src/lib.rs` | 20–28 | `enum IntentStatus` | `Pending`, `Funded`, `Executing`, `Completed`, `Failed` | (자료구조) |
| `contract/src/lib.rs` | 30–36 | `struct AnyonePay` | 컨트랙트 state: `intents`, `x402_facilitator`, `intents_contract` | (자료구조) |
| `contract/src/lib.rs` | 38–46 | `impl Default for AnyonePay` | `x402.near`, `intents.near`를 기본값으로 초기화 | (init 기본값) |
| `contract/src/lib.rs` | 50–57 | `pub fn new(...)` `#[init]` | 컨트랙트 초기화 — `x402_facilitator`, `intents_contract` 설정 | deploy.sh 전용 |
| `contract/src/lib.rs` | 60–81 | `pub fn create_intent(...)` | 인텐트 생성 및 `UnorderedMap`에 삽입 | deploy.sh / test-contract.sh 전용 |
| `contract/src/lib.rs` | 84–102 | `pub fn verify_deposit(...)` | `intents.near.mt_batch_balance_of()` Promise 생성 (결과 무시, 항상 `true` 반환) | **dead — 아무데서도 호출 없음** |
| `contract/src/lib.rs` | 105–142 | `pub fn execute_x402_payment(...)` | `x402.near.pay()` cross-contract call 후 콜백 등록 | **dead — TS에서 호출 없음** |
| `contract/src/lib.rs` | 144–150 | `pub fn on_x402_payment_success(...)` `#[private]` | `execute_x402_payment` callback — status를 `Completed`로 변경 | **dead (callback of dead method)** |
| `contract/src/lib.rs` | 153–155 | `pub fn get_intent(...)` | `intent_id`로 `Option<Intent>` 조회 (view 메서드) | deploy.sh / test-contract.sh 전용 |
| `contract/src/lib.rs` | 157–163 | `pub fn mark_funded(...)` `#[private]` | status를 `Funded`로 변경 — `#[private]` = self-call 전용 | **dead — TS에서 호출 없음** |
| `contract/Cargo.toml` | 1–22 | — | crate 설정, `near-sdk = "5.1.0"`, release 프로파일 | (빌드 설정) |
| `contract/build.sh` | 1–6 | — | `cargo build --target wasm32-unknown-unknown --release` + `.wasm` 복사 | 빌드 전용 |
| `contract/deploy.sh` | 1–75 | — | mainnet 배포, 초기화, smoke test (`create_intent` + `get_intent`) | 배포 전용 |
| `contract/test-contract.sh` | 1–34 | — | `create_intent` + `get_intent` 두 단계 테스트 | 테스트 전용 |
| `contract/update-env.sh` | 1–56 | — | `.env.local`에 `NEXT_PUBLIC_CONTRACT_ID`, `NEXT_PUBLIC_INTENTS_CONTRACT` 등 기록 | 배포 후 설정 전용 |

---

## 연결 (Wiring)

### 입력 (Inputs)

- **배포 시 (deploy.sh only):**
  - `deploy.sh:31` — `near contract deploy ... with-init-call new json-args {"x402_facilitator":"x402.near","intents_contract":"intents.near"}`
  - `deploy.sh:52–59` — smoke test: `create_intent` args (`intent_id`, `intent_type`, `deposit_address: "zs1test123"`, `amount`, `redirect_url`)
  - `test-contract.sh:12–19` — `create_intent` 호출: `deposit_address: "zs1test123456789"` (하드코딩 테스트 리터럴)

- **런타임에서의 입력: 없음.** TS 코드 어디서도 `anyone-pay.near` 컨트랙트 메서드를 호출하지 않는다. `NEXT_PUBLIC_CONTRACT_ID`는 `next.config.js:6`에 기본값 `'anyone-pay.near'`으로 정의되어 있으나, `.ts`/`.tsx` 파일 어디에서도 `process.env.NEXT_PUBLIC_CONTRACT_ID`를 읽지 않는다.

### 출력 (Outputs)

- **설계 의도 (dead code):**
  - `execute_x402_payment()` 성공 시: `x402.near`에 `pay()` cross-contract call → `on_x402_payment_success()` callback → intent status `Completed`
  - `create_intent()`: `UnorderedMap`에 `Intent` 저장
  - `mark_funded()`: intent status → `Funded` (self-call 전용)

- **프로덕션 실제 출력: 없음.** 컨트랙트는 배포 스크립트 실행 외에는 호출되지 않으며, 프로덕션 결제 흐름에 기여하는 on-chain 상태 변경이 없다.

### 내부 의존성 (Dependencies — internal)

없음. Rust 컨트랙트는 동일 레포지토리의 다른 Rust crate를 의존하지 않는다.

### 외부 의존성 (Dependencies — external)

| 대상 | 참조 위치 | 도달 가능성 |
|------|-----------|-------------|
| `x402.near` — x402 facilitator | `lib.rs:42`, `lib.rs:126`, `deploy.sh:15` | **dead** — `execute_x402_payment()` 내부에서만 참조; TS에서 호출 없음 |
| `intents.near` — NEAR Intents contract | `lib.rs:43`, `lib.rs:90`, `deploy.sh:16` | **dead** — `verify_deposit()` 내부에서만 참조; Promise 결과도 무시됨 |
| NEAR mainnet RPC | `deploy.sh:35` `network-config mainnet` | 배포 전용 |

---

## 라이브러리 (Libraries)

| Package | Version | 사용 위치 | 용도 |
|---------|---------|-----------|------|
| `near-sdk` | `5.1.0` (features: `legacy`) | `Cargo.toml:11`, `lib.rs:1–5` | `#[near_bindgen]`, `UnorderedMap`, `Promise`, `AccountId`, `NearToken`, `env`, `Gas`, `U128` |
| `borsh` | `1.5.1` | `Cargo.toml:12`, `lib.rs:1` | `BorshSerialize`/`BorshDeserialize` — NEAR 상태 직렬화 |
| `serde` | `1.0` (features: `derive`) | `Cargo.toml:13`, `lib.rs:4` | JSON 직렬화 (`Serialize`/`Deserialize`) — ABI 입출력 |
| `serde_json` | `1.0` | `Cargo.toml:14`, `lib.rs:92–96`, `lib.rs:129–135` | `serde_json::json!` 매크로 — cross-contract call args 직렬화 |

**release 프로파일 (`Cargo.toml:16–22`):**

```toml
[profile.release]
codegen-units = 1    # LLVM 최적화 단위 = 전체 LTO 허용
opt-level = "z"      # 최소 바이너리 크기 최적화 (NEAR WASM 바이너리 크기 제한 고려)
lto = true           # Link-Time Optimization (LTO) 활성화
debug = false        # 디버그 심볼 미포함
panic = "abort"      # 패닉 시 스택 언와인드 없이 즉시 abort (WASM 권장)
strip = true         # 심볼 스트립으로 추가 크기 절감
```

이 프로파일은 NEAR 컨트랙트 표준 권장 설정과 일치한다. `wasm-opt` 후처리는 `build.sh`에 없다 (`build.sh:4–5` — 단순 복사만 수행).

---

## 워크스루 (공개 메서드 상세 분석)

### 1. `new(x402_facilitator, intents_contract)` — 초기화 (`#[init]`)

```rust
// contract/src/lib.rs:50–57
#[init]
pub fn new(x402_facilitator: AccountId, intents_contract: AccountId) -> Self {
    Self {
        intents: UnorderedMap::new(b"i".to_vec()),
        x402_facilitator,
        intents_contract,
    }
}
```

- **역할:** 컨트랙트 상태를 초기화한다. `UnorderedMap`의 storage key prefix는 `b"i"`로 고정.
- **호출 제한:** `#[init]` — 한 번만 호출 가능. 배포 시 `deploy.sh:28–36`에서 호출.
- **프로덕션 TS 도달 가능성:** `deploy.sh` 전용 — 런타임 TS에서 호출 없음.

---

### 2. `create_intent(...)` — 인텐트 생성

```rust
// contract/src/lib.rs:60–81
pub fn create_intent(
    &mut self,
    intent_id: String,
    intent_type: String,
    deposit_address: String,
    amount: U128,
    redirect_url: String,
) -> Intent {
    let intent = Intent {
        id: intent_id.clone(),
        user: env::predecessor_account_id(),   // 호출자 계정
        intent_type,
        deposit_address,
        amount: amount.0,
        status: IntentStatus::Pending,         // 초기 상태
        redirect_url,
        created_at: env::block_timestamp(),    // nanoseconds
    };
    self.intents.insert(&intent_id, &intent);
    intent
}
```

- **역할:** `IntentStatus::Pending` 상태의 새 인텐트를 `UnorderedMap`에 삽입하고 반환.
- **호출 제한:** 없음 — 누구나 호출 가능. `predecessor_account_id()`가 `user` 필드로 기록된다.
- **로그 출력:** 없음 (no `env::log_str`).
- **cross-contract call:** 없음.
- **프로덕션 TS 도달 가능성:** `deploy.sh:52–59` 및 `test-contract.sh:12–19`에서 smoke test 목적으로 호출. 런타임 TS에서 호출 없음.

---

### 3. `verify_deposit(intent_id)` — 입금 검증 (broken no-op)

```rust
// contract/src/lib.rs:84–102
pub fn verify_deposit(&self, intent_id: String) -> bool {
    let intent = self.intents.get(&intent_id).expect("Intent not found");

    // "In production, this would call intents.near to verify deposit"
    let gas = env::prepaid_gas().saturating_sub(near_sdk::Gas::from_tgas(10));
    Promise::new(self.intents_contract.clone())
        .function_call(
            "mt_batch_balance_of".to_string(),
            serde_json::to_vec(&serde_json::json!({
                "account_id": intent.deposit_address,
            })).unwrap(),
            NearToken::from_yoctonear(0),
            gas,
        );
    // ↑ Promise 객체가 생성되지만 반환되지 않음 — fire-and-forget

    true  // ← 항상 true 반환; Promise 결과는 절대 확인되지 않음
}
```

- **역할 (설계 의도):** `intents.near`에 `mt_batch_balance_of()` 호출로 실제 ZEC 입금을 on-chain 검증.
- **실제 동작:** Promise가 fire-and-forget이다. `&self` (불변 참조)이기 때문에 `.then()` 콜백을 등록할 수 없으며, 함수는 Promise 반환 없이 즉시 `true`를 반환한다. 검증은 **항상 성공으로 간주**된다.
- **호출 제한:** 없음.
- **프로덕션 TS 도달 가능성:** **없음.** 어떤 TS 파일도 `verify_deposit`을 호출하지 않는다.

---

### 4. `execute_x402_payment(intent_id, amount, recipient)` — x402 결제 실행 (dead code)

```rust
// contract/src/lib.rs:105–142
pub fn execute_x402_payment(
    &mut self,
    intent_id: String,
    amount: U128,
    recipient: AccountId,
) -> Promise {
    let intent = self.intents.get(&intent_id).expect("Intent not found");

    assert_eq!(
        intent.status,
        IntentStatus::Funded,
        "Intent must be funded first"    // Funded 상태 필수 (mark_funded 선행 필요)
    );

    let mut updated_intent = intent.clone();
    updated_intent.status = IntentStatus::Executing;
    self.intents.insert(&intent_id, &updated_intent);

    let gas = env::prepaid_gas().saturating_sub(near_sdk::Gas::from_tgas(10));
    Promise::new(self.x402_facilitator.clone())           // x402.near
        .function_call(
            "pay".to_string(),
            serde_json::to_vec(&serde_json::json!({
                "amount": amount.0.to_string(),
                "recipient": recipient.to_string(),
                "token": "usdc",
            })).unwrap(),
            NearToken::from_yoctonear(amount.0),          // NEAR token 첨부
            gas,
        )
        .then(
            Self::ext(env::current_account_id())          // self callback
                .on_x402_payment_success(intent_id),
        )
}
```

- **역할 (설계 의도):** `x402.near`의 `pay()` 메서드에 NEAR token을 첨부한 cross-contract call로 x402 결제를 NEAR 네이티브하게 실행.
- **사전 조건:** `intent.status == IntentStatus::Funded` (assert_eq 실패 시 패닉).
- **cross-contract call:** `Promise::new(self.x402_facilitator)` → `x402.near.pay({ amount, recipient, token: "usdc" })` → `.then(Self::ext(...).on_x402_payment_success(intent_id))`
- **`x402.near`의 실제 존재 여부:** 확인 불가. NEAR mainnet에 이 계정이 실제로 배포되어 있는지 검증되지 않음.
- **프로덕션 TS 도달 가능성:** **없음.** `rg -n "execute_x402_payment" --type ts --type js`가 결과를 반환하지 않는다. 완전한 dead code.

---

### 5. `on_x402_payment_success(intent_id)` — x402 콜백 (`#[private]`)

```rust
// contract/src/lib.rs:144–150
#[private]
pub fn on_x402_payment_success(&mut self, intent_id: String) -> Intent {
    let mut intent = self.intents.get(&intent_id).expect("Intent not found");
    intent.status = IntentStatus::Completed;
    self.intents.insert(&intent_id, &intent);
    intent
}
```

- **역할:** `execute_x402_payment`의 Promise callback — intent status를 `Completed`로 업데이트.
- **호출 제한:** `#[private]` — `env::predecessor_account_id() == env::current_account_id()` 강제. 즉, `anyone-pay.near` 컨트랙트 자신이 발생시킨 callback만 허용.
- **프로덕션 TS 도달 가능성:** **없음.** `execute_x402_payment`가 dead code이므로 이 콜백도 dead code.

---

### 6. `get_intent(intent_id)` — 인텐트 조회 (view method)

```rust
// contract/src/lib.rs:153–155
pub fn get_intent(&self, intent_id: String) -> Option<Intent> {
    self.intents.get(&intent_id)
}
```

- **역할:** `intent_id`로 `Option<Intent>`를 반환하는 read-only view 메서드.
- **호출 제한:** 없음. NEAR view call은 gas가 필요 없고 누구나 호출 가능.
- **프로덕션 TS 도달 가능성:** `deploy.sh:62–67` 및 `test-contract.sh:26–31`에서 smoke test 목적으로만 호출. 런타임 TS에서 호출 없음.

---

### 7. `mark_funded(intent_id)` — 입금 확인 표시 (`#[private]`)

```rust
// contract/src/lib.rs:157–163
#[private]
pub fn mark_funded(&mut self, intent_id: String) {
    let mut intent = self.intents.get(&intent_id).expect("Intent not found");
    intent.status = IntentStatus::Funded;
    self.intents.insert(&intent_id, &intent);
}
```

- **역할:** intent status를 `Funded`로 변경. `execute_x402_payment` 사전 조건(Funded 상태)을 충족시키기 위한 setter.
- **호출 제한:** `#[private]` — **self-call 전용** (`env::predecessor_account_id() == env::current_account_id()`). 즉, 외부 계정(사람, relayer)이 직접 호출할 수 없고, 오직 `anyone-pay.near` 컨트랙트 자신의 Promise callback을 통해서만 호출 가능하다.
- **DEPLOY_CONTRACT.md 모순:** 문서는 "relayer only"라고 기술하지만 `#[private]` semantics는 relayer 계정이 아닌 self-call만을 허용한다. 실제로 외부 relayer가 `mark_funded`를 호출하면 `predecessor_account_id() != current_account_id()` 조건으로 패닉이 발생한다. (→ §노트 참조)
- **프로덕션 TS 도달 가능성:** **없음.** 어떤 TS 코드도 `mark_funded`를 호출하지 않는다.

---

## 노트 / 특이사항 / 주의점 (Notes / quirks / footguns)

---

### 판정: (f) — 설계된 역할을 가지나 라이브 TS 경로에서 완전히 우회됨

**판정 근거:**

1. **rg 결과 증거:** `rg -n "anyone-pay\.near|NEXT_PUBLIC_CONTRACT_ID|create_intent|mark_funded|execute_x402_payment|verify_deposit|get_intent" -g "*.ts" -g "*.tsx" -g "*.js"` 실행 결과:
   - `app/api/relayer/register-deposit/route.ts:56` — `senderAddress: senderAddress || 'anyone-pay.near'` : 이것은 컨트랙트 메서드 호출이 아니라 1Click quote 요청의 `senderAddress` 파라미터에 fallback 문자열로 사용될 뿐이다 (컨트랙트 RPC 호출 없음).
   - `next.config.js:6` — `NEXT_PUBLIC_CONTRACT_ID: process.env.NEXT_PUBLIC_CONTRACT_ID || 'anyone-pay.near'` : 환경변수 정의만. `.ts`/`.tsx` 파일 어디서도 `process.env.NEXT_PUBLIC_CONTRACT_ID`를 읽지 않는다.
   - **위 두 개 외에 매칭 없음.** `create_intent`, `mark_funded`, `execute_x402_payment`, `verify_deposit`, `get_intent` 중 어느 것도 런타임 TS 코드에 없다.

2. **lib/near.ts 사용 현황:** `lib/near.ts`는 `anyone-pay.near` 컨트랙트 호출 코드가 아닌 NEAR MPC `v1.signer` 호출용 레거시 모듈이다 (`lib/near.ts:24` — `contractId = NEAR_PROXY_CONTRACT_ID`). 그리고 이 파일 자체도 런타임에서 import되지 않는다 (§1.6 확립 사항 — `lib/chainSig.ts`가 `lib/near.ts`를 import하지 않음).

3. **프로덕션 결제 경로:** ZEC 입금 → 1Click swap `SUCCESS` → `lib/chainSig.ts:394` Base `sendRawTransaction` — NEAR 컨트랙트를 거치는 hop이 없다.

---

### Dead-code Matrix — 메서드별 호출자 현황

| 컨트랙트 메서드 | 파일:라인 | Called by (런타임 TS) | Called by (스크립트) | 판정 |
|----------------|-----------|----------------------|---------------------|------|
| `new(...)` `#[init]` | `lib.rs:51` | (none — dead) | `deploy.sh:28–36` | dead (배포 전용) |
| `create_intent(...)` | `lib.rs:60` | (none — dead) | `deploy.sh:52–59`, `test-contract.sh:12–19` | dead (테스트 전용) |
| `verify_deposit(...)` | `lib.rs:84` | (none — dead) | (none) | **완전 dead** |
| `execute_x402_payment(...)` | `lib.rs:105` | (none — dead) | (none) | **완전 dead** |
| `on_x402_payment_success(...)` | `lib.rs:145` | (none — dead) | (none) | **완전 dead (unreachable callback)** |
| `get_intent(...)` | `lib.rs:153` | (none — dead) | `deploy.sh:62–67`, `test-contract.sh:26–31` | dead (테스트 전용) |
| `mark_funded(...)` | `lib.rs:159` | (none — dead) | (none) | **완전 dead** |

---

### `mark_funded()` `#[private]` — DEPLOY_CONTRACT.md 모순

`contract/src/lib.rs:157–158`:

```rust
#[private]
pub fn mark_funded(&mut self, intent_id: String) {
```

NEAR SDK에서 `#[private]` macro는 다음 assertion을 함수 진입부에 주입한다:

```rust
assert_eq!(
    near_sdk::env::predecessor_account_id(),
    near_sdk::env::current_account_id(),
    "Method is private"
);
```

이는 **컨트랙트 자신(`anyone-pay.near`)이 발생시킨 cross-contract Promise callback**만 허용한다. 즉, 외부 NEAR 계정(relayer account, 사용자 wallet)이 직접 `mark_funded`를 트랜잭션으로 호출하면 반드시 패닉이 발생한다.

**DEPLOY_CONTRACT.md의 설명 오류:** "Called by relayer only" — 이것은 `#[private]`의 의미가 아니다. 실제 NEAR의 "relayer only" 접근 제어를 구현하려면 `assert_eq!(env::predecessor_account_id(), RELAYER_ACCOUNT_ID, ...)` 형태의 명시적 assertion이 필요하다. `#[private]` 대신 relayer 계정을 state에 저장하고 비교하거나, `owner_id` 패턴을 사용해야 한다. 현재 코드는 relayer가 `mark_funded`를 호출하는 것이 구조적으로 불가능하다.

이 모순은 설계 단계에서 `mark_funded`가 "relayer가 외부에서 호출할 수 있는 방법"으로 의도되었으나, 구현 시 잘못된 접근 제어 매크로가 적용되었음을 시사한다.

---

### `verify_deposit()` — Promise fire-and-forget no-op

`contract/src/lib.rs:84–102`에서 `verify_deposit`은 다음 구조를 가진다:

```rust
pub fn verify_deposit(&self, intent_id: String) -> bool {
    // ...
    Promise::new(self.intents_contract.clone())
        .function_call("mt_batch_balance_of".to_string(), /* ... */);
    // Promise 변수에 할당되지 않음, 반환되지 않음

    true  // 항상 true
}
```

세 가지 근본 문제가 있다:

1. **`&self` — 불변 참조:** Promise callback을 등록하려면 `&mut self`와 `.then(Self::ext(...)...)` 패턴이 필요하다. `verify_deposit`은 `&self`이므로 상태를 변경하는 비동기 결과를 처리할 수 없다.

2. **Promise 반환 없음:** 생성된 `Promise` 객체가 바인딩되지 않고 즉시 drop된다. NEAR runtime이 이 Promise를 스케줄할지 여부도 불확실하다.

3. **무조건 `true` 반환:** 비동기 결과를 기다리지 않고 함수가 즉시 `true`를 반환한다. 입금 금액이 0이든 충분하든 함수는 동일하게 `true`를 반환한다.

코드 내 주석 `"In production, this would call intents.near to verify deposit"` (`lib.rs:87`)가 이 함수가 완성되지 않은 stub임을 스스로 인정하고 있다.

---

### `x402.near` facilitator — 설계 의도와 현실의 거리

`contract/src/lib.rs:42`의 `Default` 구현과 `deploy.sh:15`의 `X402_FACILITATOR="x402.near"`는 NEAR mainnet에 `x402.near` 계정이 x402 facilitator 컨트랙트로 실제 동작하고 있다는 가정을 내포한다. 그러나:

- PAL 코드 어디에도 `x402.near`의 ABI나 인터페이스 정의가 없다.
- `execute_x402_payment`가 호출하는 메서드 이름 `"pay"`와 args (`amount`, `recipient`, `token: "usdc"`)는 임시 설계로 보인다.
- NEAR mainnet에서 `x402.near` 계정의 실제 컨트랙트 존재 여부는 이 코드베이스에서 확인되지 않는다.

이 컨트랙트 설계가 우리 팀에게 가치 있는 이유는 **NEAR 네이티브 x402 facilitator 아키텍처의 청사진**을 제공하기 때문이다 (§2 Category-E 추출에서 상세 논의). 설계가 실제로 구현되었다면: ZEC 입금 → `verify_deposit()` (NEAR Intents 잔액 확인) → `mark_funded()` (self-call) → `execute_x402_payment()` → `x402.near.pay()` 의 순서로 완전히 NEAR 컨트랙트 레이어에서 결제가 처리되는 구조가 된다. 이는 서버리스 cron + offchain MPC 경로(현재 PAL의 실제 구현)보다 훨씬 trustless하다.

---

### 이 설계를 실제로 사용하려면 무엇이 필요한가

현재 PAL의 `contract/` 설계를 프로덕션에서 실제로 사용하려면 다음이 필요하다:

1. **`x402.near` facilitator 컨트랙트 구현:** `pay(amount, recipient, token)` 인터페이스를 구현한 NEAR 컨트랙트 배포. 현재 이 계정에 무엇이 있는지 불명확.

2. **`verify_deposit()` 재구현:** `&mut self` + `.then()` callback 패턴으로 실제 비동기 검증 로직을 구현. 혹은 `mt_batch_balance_of` 대신 NEAR Intents의 올바른 balance query API를 사용.

3. **`mark_funded()` 접근 제어 수정:** `#[private]` 제거 후 `verify_deposit` callback에서 자동 호출되도록 연결하거나, relayer account ID를 state에 저장하고 비교하는 방식으로 변경.

4. **TS 코드 연결:** `lib/near.ts` 또는 새 모듈에서 `anyone-pay.near.create_intent()` 및 `execute_x402_payment()`를 실제로 호출하도록 `register-deposit/route.ts` 및 `cronjob-check-deposits/route.ts` 수정.

5. **`update-env.sh` 실행 결과 사용:** `NEXT_PUBLIC_CONTRACT_ID`를 실제로 읽는 TS 코드 작성. 현재 `next.config.js:6`에만 정의되어 있고 사용 코드가 없다.

---

### `update-env.sh` — 환경변수 순환의 단절

`update-env.sh:44–47`은 배포 후 `.env.local`에 `NEXT_PUBLIC_CONTRACT_ID=anyone-pay.near`를 기록한다. 그러나 `next.config.js:6`에서 이미 동일 값이 기본값으로 하드코딩되어 있고 (`'anyone-pay.near'`), 어떤 `.ts`/`.tsx` 파일도 이 환경변수를 읽지 않으므로, `update-env.sh`가 수행하는 환경변수 기록이 실제 애플리케이션 동작에 아무런 영향을 미치지 않는다.

---

### 빌드 파이프라인 — `wasm-opt` 없음

`build.sh:4–5`:

```bash
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/anyone_pay.wasm ./res/
```

`wasm-opt` (Binaryen 도구)가 없다. NEAR 커뮤니티 표준 빌드 파이프라인(예: `cargo-near`)은 `wasm-opt -Oz`로 추가 최적화를 수행하지만, PAL은 이 단계를 생략한다. `Cargo.toml`의 `opt-level = "z"` + `lto = true` + `strip = true`로 상당한 크기 최적화가 이미 적용되므로 기능상 문제는 없다.

---

## 답한 open questions (from the spec §7)

**Q: "Does the NEAR Rust contract participate in the payment flow, or is it just service-registry / metadata?"**

**A: NEAR Rust 컨트랙트는 결제 흐름에 전혀 참여하지 않는다. service-registry 역할도 수행하지 않는다 (Supabase가 담당). 컨트랙트는 배포 스크립트와 테스트 스크립트 외에는 호출되지 않는 dead code다.**

증거:

| 항목 | 증거 |
|------|------|
| 런타임 TS에서 `execute_x402_payment` 호출 없음 | `rg` 검색 결과 0건 |
| 런타임 TS에서 `create_intent` 호출 없음 | `rg` 검색 결과 0건 |
| `NEXT_PUBLIC_CONTRACT_ID` TS에서 읽지 않음 | `.ts`/`.tsx` 검색 결과 0건 |
| x402 경로는 `lib/chainSig.ts:394` Base `sendRawTransaction` | `07-x402-client.md` §연결 확립 |
| 상태는 Supabase `deposit_tracking`에만 저장 | `lib/depositTracking.ts:26`, `supabase-deposit-tracking.sql:5` |
| 서비스 메타데이터는 Supabase `payment_services`에만 저장 | `supabase-setup.sql:8`, `lib/serviceRegistry.ts:52` |
| `register-deposit/route.ts:56`의 `'anyone-pay.near'`는 1Click `senderAddress` 파라미터 문자열 fallback | `app/api/relayer/register-deposit/route.ts:56` |
| `next.config.js:6`의 `NEXT_PUBLIC_CONTRACT_ID`는 정의만 되고 읽히지 않음 | `.ts`/`.tsx` 전체 검색 결과 0건 |
