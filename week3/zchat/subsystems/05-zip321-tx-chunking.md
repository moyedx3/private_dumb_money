# §1.5 ZIP-321 트랜잭션 청킹

## 목적 (Purpose)

`CreateChunkedMessageProposalUseCase` 서브시스템은 ZMSG layer가 만든 N개 memo strings를 **ZIP-321 payment URI**로 직렬화하여 Synchronizer가 단일 atomic transaction을 N+1 outputs(N개 메시지 outputs + 1개 platform fee output)으로 빌드할 수 있게 한다. 모든 메시지 outputs는 *같은 destination address*로 보내고 memo만 다른 chunk을 담는다. Zashi 계정 / Keystone 하드웨어 지갑 분기, ZIP-321 URI 인코딩, USK 도출 위임, 잔액 부족 분류, "pending change" UX 지연 등 트랜잭션 라이프사이클의 application-side 디테일도 이 layer가 통합한다. 결과적으로 *한 메시지 = 한 atomic transaction* 의 단일 보장이 여기에서 만들어진다.

## 파일과 함수 (Files & functions)

### `ui-lib/.../screen/chat/usecase/CreateChunkedMessageProposalUseCase.kt`

- `CreateChunkedMessageProposalUseCase.kt:26` — `class CreateChunkedMessageProposalUseCase(...)`
  - deps: `KeystoneProposalRepository`, `ZashiProposalRepository`, `AccountDataSource`, `NavigationRouter` (Koin 주입)
- `:33` — `DEFAULT_AMOUNT_PER_OUTPUT = Zatoshi(1000L)` — output 당 0.00001 ZEC (1000 zatoshi)
- `:38` — `PLATFORM_FEE_ADDRESS = ZMSGConstants.PLATFORM_FEE_ADDRESS` — §1.1에서 명시한 178자 unified address
- `:42` — `ESTIMATED_NETWORK_FEE_BUFFER_ZATOSHI = 2000L` — UX 분류용 conservative buffer (실제 fee는 SDK가 계산)
- `:64` — `suspend operator fun invoke(destinationAddress, senderAddress, message, isFirstMessage, amountPerOutput, platformFeeAmount, directSubmit, skipNavigation, rawMemo, conversationId, lastReceivedTxId)` — **단일 진입점**
- `:79-100` — 메모 생성 fallback chain:
  - `rawMemo = true` → message 그대로 (그룹·reactions·receipts·time-lock·payment requests)
  - `conversationId != null + isFirstMessage` → `ZMSGProtocol.createChunkedV4InitMessages`
  - `conversationId != null + !isFirstMessage` → `createChunkedV4ReplyMessages`
  - `conversationId == null + isFirstMessage` → v3 INIT fallback
  - `lastReceivedTxId != null` → v3 REF fallback
  - else → v3 hash reply (deprecated)
- `:104` — `createMultiOutputProposal(destinationAddress, memos, amountPerOutput, platformFeeAmount)` — ZIP-321 URI 빌더 호출 후 repository에 위임
- `:181` — `private suspend fun createMultiOutputProposal(...)` — Repository 분기: Keystone vs Zashi
  - Keystone: `createZip321Proposal(uri)` + `createPCZTFromProposal()` — 하드웨어 서명 대비 PCZT 생성
  - Zashi: `createZip321Proposal(uri)` — 그 후 `submitZashiProposal`이 USK + proof generation 진행
- `:213` — `private fun buildZip321Uri(destinationAddress, memos, amountPerOutput, platformFeeAmount): String` — **ZIP-321 wire format 직접 빌드**
- `:167` — `private suspend fun submitZashiProposal(skipNavigation)` — Zashi 계정용 자동 submit, error propagation 정책 분기
- `:254` — `fun getTotalCost(message, isFirstMessage, amountPerOutput): Zatoshi` — UI에 표시할 비용 계산 (chunkCount + 1) × amountPerOutput
- `:268` — `fun needsChunking(message, isFirstMessage, senderAddress): Boolean` — ZMSGProtocol 위임
- `:275` — `fun getChunkCount(message, isFirstMessage): Int` — ZMSGProtocol 위임
- `:279` — `private fun estimateRequiredSpendableBalance(memoCount, amountPerOutput)` — `(memoCount + 1) * amountPerOutput + 2000` buffer
- `:288` — `private suspend fun hasPendingShieldedBalanceBlockingSpend(required): Boolean` — "내가 보낸 이전 메시지의 change가 confirm 대기 중인가" 분류
- `:297` — `private fun isInsufficientFundsError(throwable): Boolean` — exception chain walk + 메시지 텍스트 검사 (SDK가 InsufficientFundsException으로 직접 throw하지 않을 때 대비)

### Layer B 위임 (참조만 — 본 dive scope 외)

- `ui-lib/.../common/repository/ZashiProposalRepository.kt` — `createZip321Proposal(uri)`, `submit()`, `clear()`. 내부적으로 `synchronizer.proposeTransferFromUri(...)` 호출 후 USK로 sign + submit
- `ui-lib/.../common/repository/KeystoneProposalRepository.kt` — 동일 인터페이스 + `createPCZTFromProposal()` 추가 (PCZT는 ZIP-374)
- `ui-lib/.../common/datasource/AccountDataSource.kt` — `getSelectedAccount()` → `KeystoneAccount` or `ZashiAccount` sealed result
- `ui-lib/.../common/datasource/ZashiSpendingKeyDataSource.kt` — `getZashiSpendingKey()` (BIP-39 → ZIP-32 → USK 매번 새로 도출, 캐시 X) — `ZashiProposalRepository.submit()` 내부에서 호출
- `ui-lib/.../common/datasource/InsufficientFundsException.kt` — 잔액 부족 sealed error type

## 연결 (Wiring)

- **Inputs:**
  - `destinationAddress: String` — 수신자 unified address (1:1) 또는 멤버별 (그룹 fan-out)
  - `senderAddress: String` — 우리 unified address
  - `message: String` — plaintext 메시지 또는 `"E2E1:..."` ciphertext 또는 `"ZMSG:3.0:GROUP:..."` raw (rawMemo)
  - `isFirstMessage: Boolean`, `conversationId: String?`, `lastReceivedTxId: String?` — protocol version 분기
  - `amountPerOutput: Zatoshi` — 각 output의 송금액
  - `platformFeeAmount: Zatoshi` — platform fee output 금액 (default = amountPerOutput)
  - `directSubmit`, `skipNavigation`, `rawMemo` — UX flags
- **Outputs:**
  - 부작용: `ZashiProposalRepository.createZip321Proposal(uri)` → `submit()` (또는 Keystone PCZT)
  - 부작용: navigation router를 통한 화면 전환 (`TransactionProgressArgs` 또는 `InsufficientFundsArgs`)
  - 예외: `InsufficientFundsException` (skipNavigation=true 시 caller로 throw)
- **Dependencies (internal):**
  - [§1.1 ZMSG 프로토콜](./01-zmsg-protocol.md) — `createChunkedV4InitMessages` / `createChunkedV4ReplyMessages` / v3 fallbacks 호출
  - [§1.4 그룹 메시징](./04-group-messaging.md) — `GroupViewModel.createGroup` / `sendGroupMessage`가 `rawMemo = true`로 호출
  - [§1.6 송수신 흐름](./06-send-receive-flow.md) — `ChatViewModel.doSendMessage`가 주된 caller, `ZashiProposalRepository.submit()`가 SDK Synchronizer에 위임
- **Dependencies (external — SDK):**
  - `cash.z.ecc.android.sdk.model.Zatoshi` — 1e-8 ZEC value class
  - `cash.z.ecc.sdk.extension.toZecStringFull` — Zatoshi → "0.00001000" 등 8자리 ZEC 문자열
  - `Synchronizer.proposeTransferFromUri(account, uri)` (Layer B repository 내부) — ZIP-321 URI → Proposal
  - `Synchronizer.createProposedTransactions(proposal, usk)` — proof + sign + 직렬화

## 라이브러리 (Libraries)

| Package | Version | Used for |
|---------|---------|----------|
| `zcash-android-wallet-sdk` (cash.z.ecc.android.sdk) | 2.4.3 (README) | `Zatoshi`, `Synchronizer.proposeTransferFromUri`, `createProposedTransactions` |
| `android.util.Base64` | API 27+ | ZIP-321 memo encoding: `URL_SAFE \| NO_WRAP \| NO_PADDING` |
| Kotlin coroutines | 2.1.10 | `suspend operator fun invoke`, repository 호출 |

## 워크스루 — happy path

### A. 1:1 메시지 송신 — Alice → Bob, "Hello" (단일 청크)

**1. ChatViewModel이 useCase 호출 (§1.6, line 2483+)**

```kotlin
createChunkedMessageProposal.invoke(
    destinationAddress = bobAddress,
    senderAddress = aliceAddress,
    message = "E2E1:00:0000000000000007:abcdef...",  // Ratchet ciphertext (§1.3)
    isFirstMessage = false,
    amountPerOutput = Zatoshi(1000L),
    directSubmit = true,
    skipNavigation = true,
    rawMemo = false,
    conversationId = "ABC12345",
    lastReceivedTxId = null
)
```

**2. 메모 생성 분기 (line 80-100)**

`conversationId != null` + `!isFirstMessage` → `ZMSGProtocol.createChunkedV4ReplyMessages("ABC12345", aliceAddress, message)`. 메시지가 462B 이하면 1개 chunk 반환.

```
memos = [
    "ZMSG|v4|ABC12345|<hash16>|E2E1:00:0000000000000007:abcdef..."
]
```

**3. ZIP-321 URI 빌드 — `buildZip321Uri(bobAddr, memos, 1000zat, 1000zat)` (line 213)**

```kotlin
val firstEncodedMemo = Base64.encodeToString(memos[0].toByteArray(UTF_8),
    Base64.URL_SAFE or Base64.NO_WRAP or Base64.NO_PADDING)

params.append("zcash:$destinationAddress?amount=$amountZec&memo=$firstEncodedMemo")
// 단일 chunk이므로 for loop는 건너뜀

// Platform fee output (paymentIndex = 1)
params.append("&address.1=$PLATFORM_FEE_ADDRESS")
params.append("&amount.1=$platformFeeZec")  // memo는 없음
```

결과:
```
zcash:u1bobaddress...?amount=0.00001000&memo=Wk1TR3x2NHxBQkMxMjM0NXw...
&address.1=u1pm2ju3z...platform...&amount.1=0.00001000
```

**4. ZashiProposalRepository.createZip321Proposal — Layer B**

내부적으로 `Synchronizer.proposeTransferFromUri(account, uri)` 호출. SDK는 ZIP-321 URI를 파싱하여 2개 output에 대한 Proposal 객체 빌드. 노트 선택, ZIP-317 fee 계산, Orchard pool로부터 change 도출.

**5. submitZashiProposal (line 167)**

```kotlin
private suspend fun submitZashiProposal(skipNavigation: Boolean = false) {
    try {
        zashiProposalRepository.submit()
    } catch (e: Exception) {
        if (skipNavigation) throw e
    }
}
```

`ZashiProposalRepository.submit()` 내부:
- `ZashiSpendingKeyDataSource.getZashiSpendingKey()` 호출 → BIP-39 mnemonic → ZIP-32 derivation → `UnifiedSpendingKey`
- `Synchronizer.createProposedTransactions(proposal, usk)` — JNI → librustzcash → Orchard Halo2 proof + RedPallas sig + binding sig
- `lightwalletd.submitTransaction(...)` — gRPC

**6. UI optimistic update — ChatViewModel pendingMessage**

`directSubmit = true` + `skipNavigation = true`이므로 user는 chat 화면에 머무름. ChatViewModel이 `pendingMessage`를 즉시 표시(§1.6) — tx hash는 SDK callback이 나중에 채움.

### B. 긴 메시지 — Alice → Bob, "..." (3 청크)

**메모 생성 (line 89):**

`createChunkedV4ReplyMessages(...)` returns:
```
memos = [
    "ZMSG|v4c|1/3|ABC12345|<hash16>|E2E1:...part1",
    "ZMSG|v4c|2/3|CONT|...part2",
    "ZMSG|v4c|3/3|CONT|...part3"
]
```

**ZIP-321 URI (line 213):**

```
zcash:u1bobaddr...?amount=0.00001000&memo=<chunk1_b64>
&address.1=u1bobaddr...&amount.1=0.00001000&memo.1=<chunk2_b64>
&address.2=u1bobaddr...&amount.2=0.00001000&memo.2=<chunk3_b64>
&address.3=u1pm2ju3z...platform...&amount.3=0.00001000
```

총 4개 outputs (3 chunks + 1 platform fee). 모두 *같은 destination* (Bob)으로 가는 메시지 outputs 3개 + 별도 platform fee output 1개. **단일 atomic transaction** — Synchronizer가 하나의 Proposal로 빌드.

**비용 합계:** 4 × 1000 = 4000 zatoshi = 0.00004 ZEC (plus Zcash network fee ~ZIP-317에 따른 dust mitigation).

### C. 그룹 메시지 — Alice → [Bob, Carol, Dave]

`GroupViewModel.sendGroupMessage`가 for-loop으로 invoke 3번 호출, 각 호출은 `rawMemo = true`로 `destinationAddress`만 다름. 즉 3개 transaction 각각 1개 메시지 output + 1개 platform fee output = 각 2 outputs. 비용 3 × (2 × 1000) = 6000 zatoshi.

ZIP-321 URI (Bob 한 명 분):
```
zcash:u1bobaddr...?amount=0.00001000&memo=<group_msg_b64>
&address.1=u1pm2ju3z...platform...&amount.1=0.00001000
```

### D. 잔액 부족 분류 (line 134-156)

**1. Spendable balance 계산:**

`hasPendingShieldedBalanceBlockingSpend(required)`(line 288):
```kotlin
return account.spendableShieldedBalance < required &&
    account.totalShieldedBalance >= required &&
    account.changePendingShieldedBalance > Zatoshi(0)
```

- spendable < required: 즉시 사용 가능한 노트로는 부족
- total ≥ required: 모든 노트 합치면 충분
- changePending > 0: 이전에 내가 보낸 tx의 change가 아직 confirm 안 됨

→ 세 조건 모두 true면 "사용자가 직전 메시지를 한 번 더 보내려는데 change가 lightwalletd에 아직 인덱싱 안 됨" 상황.

**2. 메시지 분기:**

- "pending change" 상황 → `PENDING_BALANCE_WAIT_MESSAGE = "Please wait for your previous message to confirm on-chain, then try again."`
- 진짜 부족 → `INSUFFICIENT_BALANCE_MESSAGE = "Insufficient balance. Please add ZEC to your wallet to send messages."`

**3. Error propagation:**

`skipNavigation = true` (ZCHAT 직접 send 흐름) → caller(`ChatViewModel`)에게 `InsufficientFundsException` throw. ChatViewModel은 toast / snackbar로 표시. `skipNavigation = false` (review screen 흐름) → router가 `InsufficientFundsArgs` 화면으로 navigate.

## 노트 / quirks / footguns

- **`amountPerOutput = 1000 zatoshi = 0.00001 ZEC` (default).** spec과 claude.md "~0.0001 ZEC per message" 명시와 mismatch — 코드는 *10배 작은* 1000 zatoshi 사용. spec C53도 그룹 0.0001 ZEC라 했지만 실제 code constant는 1000. **fact check 결과 코드가 진실**. 메시지 비용은 (chunks + 1) × 1000 zatoshi + 네트워크 fee. 짧은 메시지 (1 chunk) = 2000 zatoshi + fee ≈ 12000 zatoshi (Zcash ZIP-317 fee 10000) = 0.00012 ZEC.
- **모든 메시지 output에 같은 amount.** chunk마다 개별 amount 변경 불가. 동일 destination + 동일 amount + 다른 memo 패턴. 우리 팀 차별화로 chunk별 다른 amount(예: 첫 chunk는 큰 dust, 나머지는 최소)를 고려할 수 있으나 deterministic ordering이 chunk index에 따라 결정되므로 amount 변화는 ZMSG 측 chunk reassemble과 무관.
- **`PLATFORM_FEE_ADDRESS`로의 송금이 default.** 178자 unified address가 코드에 하드코딩. 모든 메시지(1:1, 그룹, special types)에 항상 1개 platform fee output 추가됨. 우리 팀이 이 모델을 그대로 채택할지는 비즈니스 결정 — fee를 안 받는 경우 `platformFeeAmount = Zatoshi(0)`로 우회 가능하지만 ZIP-321 spec상 amount=0 output이 허용되는지 SDK 동작 확인 필요.
- **Memo Base64 encoding은 `URL_SAFE | NO_WRAP | NO_PADDING`.** URL parameter 자리에 들어가야 하므로 표준 `+/=`가 아닌 `-_` 사용 + 패딩 제거. ZIP-321 spec과 일치. 그러나 *수신 측*에서 ZMSG layer가 받는 memo는 SDK가 이미 decode해서 UTF-8 text를 줌(`tx.memo`) — Base64 round-trip은 transparent.
- **`InsufficientFundsException` 분류는 message text matching.** `isInsufficientFundsError`(line 297)가 SDK exception chain을 walk하며 메시지 텍스트 4종 패턴 검사. SDK가 향후 exception 메시지를 변경하면 분류 깨질 위험. 우리 팀 포팅 시 strongly-typed exception으로 정리 권장.
- **Keystone vs Zashi 분기.** `AccountDataSource.getSelectedAccount()`가 `sealed class { KeystoneAccount, ZashiAccount }`. Keystone은 PCZT (ZIP-374) 흐름 — 디바이스로 PCZT 보내서 사용자가 하드웨어에서 서명, 그 다음 zchat 앱이 받아서 lightwalletd로 broadcast. Zashi는 in-app USK가 자동 서명. 본 dive scope에서는 Zashi 흐름만 깊게 다룸.
- **`directSubmit = true` 시 user가 review screen을 거치지 않는다.** ZCHAT default — chat UX에서 "Send" 누르면 즉시 송신. 비용 disclaimer 처리는 `ChatViewModel`에서 한 번 수락(§1.6) 후 모든 후속 메시지에 자동 적용. legacy non-ZCHAT (일반 wallet) 흐름은 `directSubmit = false`로 review screen 거침.
- **`rawMemo = true`는 ZMSG layer 우회.** 그룹 메시지, reactions, receipts, time-lock, payment requests, ZFILE 등 *이미 wire format이 결정된* 메시지에 사용. 우리 팀이 메시지 layer를 추가하면 (예: NOSTR-side handshake) 같은 `rawMemo` mechanism을 활용 가능.
- **`getZashiSpendingKey()`는 매 송신마다 호출.** USK가 캐시되지 않음 — BIP-39 mnemonic → ZIP-32 derivation을 매번. 보안 측면에서 *good* (USK가 메모리에 오래 살지 않음). 성능 측면에서 약간의 overhead — librustzcash는 이를 fast하게 처리하지만 모바일 burst send 시 측정 필요.
- **Memo Base64 encoding 비용.** 512B raw memo → ~684B Base64 URL-safe (4/3 expansion). ZIP-321 URI는 *URL*이므로 SDK가 다시 decode해서 *raw memo bytes*를 노트에 넣음. 즉 onchain memo는 raw 512B 그대로 — Base64는 ZIP-321 transport에만 존재.
- **Memo가 Zcash protocol의 512B 제한을 초과하면?** ZMSG layer가 chunking으로 보장하므로 도달 안 함. 하지만 `rawMemo = true` 경로(특히 group, time-lock 같은 JSON payload)에서 caller가 512B 초과 memo를 넘기면 ZIP-321 URI는 빌드되지만 Synchronizer가 reject. 우리 팀 hardening 후보: `buildZip321Uri` 진입 시 raw byte len 검증.
- **두 디바이스에서 같은 시드로 동시 송신 시 race.** 같은 USK를 같은 시점에 다른 device가 사용하면 같은 notes를 select해서 nullifier 충돌 → 한쪽 tx reject. 다행히 메시지 nonce / chunk content는 다르지만 lightwalletd가 첫 broadcast만 mempool 받음. ZMSG_PROTOCOL_SPEC.md "multi-device 미지원"의 *또 다른* 이유 (§1.3에서 본 ratchet counter 충돌 외).

## 답한 open question

- **Q11** (research-plan §7): "ZIP-321 multi-output 청킹: N개 chunk가 단일 atomic transaction인가? chunk 순서 보장 메커니즘은?"
  > **Answer:** **단일 atomic transaction**. `buildZip321Uri`(line 213)가 indexed parameter format(`address.1=, amount.1=, memo.1=`)으로 모든 chunk를 한 URI에 넣고, `Synchronizer.proposeTransferFromUri(uri)`가 그 URI 전체를 한 Proposal로 빌드. SDK는 multi-output Proposal에서 모든 outputs을 한 transaction에 포함. 청크 순서는 ZIP-321 URI parameter 순서가 *아니라* memo header (`v4c|1/N`, `v4c|2/N`, ...) 안의 chunk index. 수신측이 `reassembleChunks`(§1.1)에서 same-tx memos를 모은 후 chunk index로 정렬하므로 atomic delivery + index ordering 두 가지가 결합. — `CreateChunkedMessageProposalUseCase.kt:181-249`, `ZMSGProtocol.kt:1058-1156`

- **Q12** (research-plan §7): "chunk 1개의 output recipient는 누구인가? 모두 같은 수신자에게 N번 보내는 multi-output인가, chunk마다 다른 ephemeral 주소를 쓰는가?"
  > **Answer:** **모두 같은 destination address**. `buildZip321Uri`(line 213-242)가 모든 paymentIndex의 `address.X`를 같은 `destinationAddress`로 설정. ephemeral / diversified address 사용 *없음*. 따라서 수신자의 같은 inbox(unified address)에 N개 동시 도착하는 노트로 보임. 다만 diversified address는 transparent 관찰자에게도 unlinkable이므로 outsider 관점에서는 각 output이 별도 noted addresses. — `CreateChunkedMessageProposalUseCase.kt:213-249`

- **Q13** (research-plan §7): "Orchard-only 정책 — 왜 Sapling을 안 쓰는가?"
  > **Answer (partial):** `CreateChunkedMessageProposalUseCase`는 명시적으로 pool을 선택하지 *않는다*. `Synchronizer.proposeTransferFromUri`가 SDK 내부에서 노트 선택. 그러나 **Orchard pool only**의 정책 enforcement는 `ChatViewModel.kt`에서 Orchard 잔액 확인 후에만 진행하도록 강제됨(MESSAGING_CRYPTO.md §4.1 / 향후 §1.6 검증). SDK 자체는 multi-pool fund-pulling을 지원하지만 ZCHAT은 sender 측에서 Sapling tx로 falling back 안 되게 검사. **이유 (추정):** Orchard가 더 작은 proof + 더 효율적 (Halo2 < Groth16), shielded pool 통합 정책. 정확한 정책 위치는 §1.6에서 추가 확인. — `CreateChunkedMessageProposalUseCase.kt` 자체에는 pool selection 코드 없음; §1.6 보완 필요

- **C90** (claims-to-verify): Platform fee address output 추가 검증
  > **Answer:** **✓**. `buildZip321Uri`(line 245-246)가 *모든* ZIP-321 URI 마지막에 `address.X = PLATFORM_FEE_ADDRESS`, `amount.X = platformFeeAmount`를 추가. memo 없음. ZMSG_PROTOCOL_SPEC.md Transaction Structure와 일치. — `CreateChunkedMessageProposalUseCase.kt:38, 245-246`

- **C48** (claims-to-verify): ZIP-321 URIs로 multi-output tx 구성, Base64 URL-safe encoded memos
  > **Answer:** **✓**. `buildZip321Uri`(line 213)가 ZIP-321 indexed parameter format 정확히 사용. memo는 `Base64.URL_SAFE \| NO_WRAP \| NO_PADDING`(line 227, 236). — `CreateChunkedMessageProposalUseCase.kt:213-249`

- **C44, C45** (claims-to-verify): MAX_CHUNKS 1000, chunk sizes
  > **Answer:** **✓ (§1.1에서 검증). 본 layer는 ZMSGProtocol에 위임. CreateChunkedMessageProposalUseCase 자체는 chunk 개수 검증 안 함 — ZMSGProtocol이 MAX_CHUNKS 초과 시 `require` 예외 throw.** — `ZMSGProtocol.kt:63, 368, 404`
