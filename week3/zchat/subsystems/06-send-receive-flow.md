# §1.6 메시지 송신/수신 흐름 (Layer C → B → A)

> ⚠️ **2026-05-16 정정**: Plaintext fallback 정책의 세부가 부정확했음. **`getOrCreateMessageProcessor` 가 null 반환 시 (KEX 미완료 / E2E disabled / 자기 또는 peer pubkey 없음) 사용자 알림 없이 plaintext 송신** — silent fallback. *encryption 자체 실패* 시에만 abort 가 적용. `getOrCreateMessageProcessor` 의 정확한 null 조건 (`ChatViewModel.kt:1634-1638`) 과 plaintext fallback 코드 (`ChatViewModel.kt:2499-2501`) 를 추가 명시. [`../corrections-log.md` §G.3](../corrections-log.md).

## 목적 (Purpose)

`ChatViewModel`은 zchat의 모든 1:1 메시지 라이프사이클을 통합한다 — 송신 path(UI → 옵티미스틱 pending → ZMSG → Ratchet → ZIP-321 → SDK → JNI → lightwalletd) + 수신 path(SDK `transactions` Flow → debounce → memo extract → ZMSG parse → E2E decrypt → ChatMessage → state flow → UI). 또한 KEX 핸드셰이크 시작/응답, ratchet root 도출, 그룹 메시지 dispatch, payment request·time-lock·reaction·remote-kill 같은 special types 처리, 1분 auto-refresh + 15분 WorkManager + ForegroundService 백그라운드 sync, message queue retry policy까지 모두 이 layer가 담당한다. **Layer C → Layer B → Layer A 위임 경계의 단일 진입점**이라는 점이 가장 중요한 특성이다.

## 파일과 함수 (Files & functions)

### `ui-lib/.../screen/chat/viewmodel/ChatViewModel.kt` (3736 lines — 매우 큰 파일)

- `ChatViewModel.kt:79` — `class ChatViewModel(...)` — Koin DI deps 11개: `TransactionRepository`, `GetSelectedWalletAccountUseCase`, `GetDefaultUnifiedAddressUseCase`, `AccountDataSource`, `CreateChunkedMessageProposalUseCase`, `AddressCache`, `ZchatPreferences`, `SynchronizerProvider`, `ExchangeRateRepository`, `ContactBook`, `WalletSnapshotDataSource`, `PersistableWalletProvider`
- `:94` — `_chatListState: MutableStateFlow<ChatListState>` — 메인 UI state
- `:120` — `pendingMessage: PendingMessageParams?` — disclaimer 대기 중인 한 건의 송신
- `:138` — `pendingMessages: MutableStateFlow<List<ChatMessage>>` — 다수 in-flight 메시지 (UI optimistic)
- `:149` — `messageQueue: MutableList<QueuedMessage>` — note locking 시 대기열
- `:480-568` — Conversation list loading flow (debounce + combine)
- `:570-1700+` — `convertToConversations(transactions, ...)` — Flow 값을 ChatListState로 변환 (긴 함수)
  - `:642` — Status 메시지 skip + peerStatuses 업데이트
  - `:655` — Reaction / ReadReceipt skip
  - `:663` — KEX/KEXACK 분기 → `handleKEXMessage` (incoming) or txid persist (outgoing)
  - `:689` — Group 메시지 분기 → `processGroupMessage`
  - `:698+` — payment request / time-lock / reply / 일반 ZMSG parse + E2E decrypt
- `:1702` — `private fun handleKEXMessage(memoText, ourAddress, receivedTxId)` — incoming KEX 검증 + Ratchet root 도출 + 자동 E2E enable + KEXACK 송신
- `:1849` — `fun sendKEXMessage(peerAddress, ourAddress)` — KEX initiator 진입점 (§1.2)
- `:1899` — `private suspend fun sendKEXAckMessage(peerAddress, ourAddress, convId)` — KEXACK responder
- `:1933` / `:1941` — `getPeerE2EPublicKey(peer)`, `hasCompletedKEX(peer)` — GroupViewModel에서 사용
- `:1949` — `fun refresh()` — pull-to-refresh 수동 sync
- `:1989` — `private fun checkForRemoteKill(amountZatoshi, memo, txId)` — `ZCHAT_DESTROY:<phrase>` 검출 (§1.7)
- `:2018-2068` — Auto-refresh 타이머 (60초 폴링; `AUTO_REFRESH_INTERVAL_SECONDS = 60`)
- `:2312` — `fun sendMessage(peerAddress, message, amountZatoshi = DEFAULT_MESSAGE_AMOUNT)` — UI에서 호출하는 송신 entry. Orchard 잔액 확인 + 비용 disclaimer + 큐 상태 확인 후 `doSendMessage` 호출
- `:2369-2375` — companion constants:
  - `AUTO_REFRESH_INTERVAL_SECONDS = 60` (앱 활성 시 1분 폴링)
  - `DEFAULT_MESSAGE_AMOUNT = 1000L` (zatoshi per output)
  - `MAX_QUEUE_RETRIES = 4`
  - `QUEUE_RETRY_TIMEOUT_MS = 300_000L` (5분 절대 타임아웃)
- `:2429` — `private fun doSendMessage(peerAddress, message, amountZatoshi, existingPendingId, retryCount)` — 핵심 송신 로직
  - `:2447-2467` — pending message 즉시 추가 + prefs 영구화 (`addPendingMessage`)
  - `:2474` — `convIdMutex.withLock { zchatPreferences.getOrCreateConversationId(peer) }` — convID 동기 생성
  - `:2495-2501` — `getOrCreateMessageProcessor(peer, convId)` 호출 후 `processor.encryptOutgoing(message)` (§1.3). **두 가지 분기**: (a) processor null (KEX 미완료 등) → message 그대로 송신 = **silent plaintext fallback**. (b) processor 있는데 `encryptOutgoing` throw → outer try/catch 가 catch 후 사용자에게 error 알림. 즉 "encryption *실패* 시 abort" 는 정확하지만 "*KEX 미완료* 시" 는 plaintext 가 silent 로 나감. processor null 조건은 `ChatViewModel.kt:1634-1638` 참조 (4가지: sharedKey 없음 / E2E disabled / ourPub 없음 / peerPub 없음).
  - `:2505-2518` — `withContext(Dispatchers.Default) { createChunkedMessageProposal(...) }` — proof generation을 Default dispatcher로 — Main thread는 UI recomposition 유지
  - `:2527-2607` — error handling: insufficient balance 분류, queue retry, MAX_QUEUE_RETRIES 도달 시 FAILED
- `:2616` — `private fun processNextQueuedMessage()` — 큐 head 꺼내서 doSendMessage 호출
- `:3505` — `private fun processGroupInvite(...)` — GroupViewModel에 위임하기 전 group state 동기 (§1.4)

### `ui-lib/.../screen/chat/viewmodel/ZchatComposeVM.kt` (411 lines)

- 진입 화면 (chat 시작/검색 + 첫 메시지 작성) state + ViewModel. `ChatViewModel.sendMessage`로 위임.

### `ui-lib/.../screen/chat/viewmodel/ZchatReceiveVM.kt` (79 lines, 작음)

- 수신 표시 / QR + 주소 보여주는 UI state. 메시지 처리는 ChatViewModel만 담당.

### `ui-lib/.../screen/chat/view/ChatDetailView.kt` (2794 lines — UI 디테일은 본 dive scope 외)

- compose UI. ChatViewModel을 collectAsState로 구독. claude.md "Known Technical Debt"에 split 권장.

### Layer B 참조 (호출 site만 표기)

- `common/datasource/ZashiSpendingKeyDataSource.kt:getZashiSpendingKey()` — `ZashiProposalRepository.submit()`이 매 송신마다 호출 (USK 캐시 X)
- `common/datasource/ProposalDataSource.kt` — `createProposal(account, send)` / `submitTransaction(proposal, usk)`
- `common/repository/ZashiProposalRepository.kt` — proposal lifecycle + submit
- `common/repository/TransactionRepository.kt` — `transactions: Flow<List<Transaction>?>` (수신 path의 source)
- `common/provider/SynchronizerProvider.kt` — SDK `Synchronizer` 인스턴스 lazy holder

### 백그라운드 sync 인프라

- `service/SyncForegroundService.kt` (594 lines) — Foreground service for sync continuation; FOREGROUND_SERVICE permission, persistent notification
- `work/SyncWorker.kt:30` — `class SyncWorker : CoroutineWorker`
  - `:38-69` — `doWork()`: synchronizer.status + progress combine + collect until SYNCED / DISCONNECTED
  - `:73` — `SYNC_PERIOD = 15.minutes` (claude.md v2.8.5 매치)
  - `:75` — `newWorkRequest()` — `PeriodicWorkRequestBuilder` + NetworkType.CONNECTED + 1분 initial delay + storage low 회피
- `work/SyncAlarmReceiver.kt` + `work/SyncAlarmScheduler.kt` — AlarmManager fallback for Android 15 FGS timeout (claude.md v2.8.5)
- `work/WorkIds.kt` — `"co.electriccoin.zcash.background_sync"` WorkManager tag
- `common/notification/InAppNotificationManager.kt` + `InAppNotificationBanner.kt` — In-app banner (claude.md v2.8.5)
- `screen/notificationsettings/NotificationSettingsScreen.kt + State + View + VM` — Privacy / sound / vibration / mute 설정 UI

## 연결 (Wiring)

- **Inputs:**
  - User actions (Compose UI): send, refresh, KEX initiate, mute, identity regen, etc.
  - SDK `Synchronizer.transactions` Flow (via `TransactionRepository.transactions`)
  - `SDK Synchronizer.networkHeight`, `walletSnapshotDataSource.observe()` — sync status
  - Prefs read/write (ZchatPreferences): conversation IDs, E2E keys, ratchet state, peer status, drafts, etc.
- **Outputs:**
  - `ChatListState` / `ChatDetailState` / `SendMessageState` flows for UI
  - `pendingMessages`, `peerStatuses`, `hiddenMessages`, `_walletSyncStatus` flows
  - Side effect: `createChunkedMessageProposal.invoke(...)` (§1.5) → Zcash transaction broadcast
  - Side effect: `GroupViewModel.processGroupMessage` (§1.4)
  - Side effect: `DestroyManager.destroyAll()` if remote kill detected (§1.7)
- **Dependencies (internal):**
  - [§1.1 ZMSG](./01-zmsg-protocol.md) — parseMemo, createV4KEXMessage, createV4InitMessage, special types parsers
  - [§1.2 KEX + E2E](./02-kex-e2e-encryption.md) — `E2EEncryption.generateKeyPair`, `createKEXPayload`, ECDH shared secret
  - [§1.3 Double Ratchet](./03-double-ratchet.md) — `E2EMessageProcessor` peer별 캐싱, `E2ERatchet.deriveRatchetRoot`
  - [§1.4 그룹 메시징](./04-group-messaging.md) — `ZMSGGroupProtocol.isGroupMessage` 분기 + `processGroupMessage` 위임
  - [§1.5 ZIP-321 청킹](./05-zip321-tx-chunking.md) — `CreateChunkedMessageProposalUseCase` 주된 caller
  - [§1.7 컨택트 + Identity](./07-contact-book-address-cache.md) — `AddressCache`, `ContactBook`, `ZchatPreferences`, `DestroyManager`
  - [§1.8 NOSTR + 파일](./08-nostr-side-channel.md) — UploadProgressTracker, ZFILE 메시지 처리는 ChatViewModel에서 분기
- **Dependencies (external — SDK Layer A):**
  - `cash.z.ecc.android.sdk.Synchronizer` — `status`, `progress`, `networkHeight`, `transactions`, `refreshTransactions`, `refreshAllBalances`
  - `cash.z.ecc.android.sdk.SdkSynchronizer` — concrete cast for manual refresh
  - `cash.z.ecc.android.bip39.Mnemonics`, `toSeed` — seed restore 시점에 사용 (rare)
  - `cash.z.ecc.android.sdk.model.{TransactionId, Zatoshi}`
- **Dependencies (external — Android infrastructure):**
  - `androidx.work.{CoroutineWorker, PeriodicWorkRequest, Constraints, NetworkType}` — SyncWorker
  - `android.app.{Service, NotificationManager, AlarmManager}` — SyncForegroundService, SyncAlarmReceiver
  - `androidx.lifecycle.{ViewModel, viewModelScope}` — Compose lifecycle
- **Dependencies (external — Ktor http):**
  - `io.ktor.client.{get, body}` — exchange rate fetch (메시징과 무관)

## 라이브러리 (Libraries)

| Package | Version | Used for |
|---------|---------|----------|
| `zcash-android-wallet-sdk` | 2.4.3 | Synchronizer, transactions Flow, USK, Zatoshi, BIP-39 |
| `androidx.work:work-runtime` | (AndroidX) | 15분 주기 PeriodicWorkRequest |
| `androidx.lifecycle:lifecycle-viewmodel` | (AndroidX) | ChatViewModel lifecycle |
| `kotlinx-coroutines-core` | 2.1.10 ecosystem | Flow / StateFlow / debounce / collectLatest / combine / Mutex / withContext |
| `io.koin:koin-android` | 4.0.2 | DI (ChatViewModel 11개 deps 주입) |
| `org.koin.core.component.KoinComponent` | 4.0.2 | SyncWorker가 KoinComponent 패턴으로 sync provider inject |
| `co.electriccoin.zcash.spackle.Twig` | 내부 | 로깅 (`BG Sync:` 태그) |

## 워크스루 — happy path

### A. 송신 — `sendMessage("u1bob...", "Hello", DEFAULT_MESSAGE_AMOUNT)`

**1. `ChatViewModel.sendMessage(peerAddress, message, amountZatoshi)` (line 2312)**

```kotlin
fun sendMessage(peerAddress: String, message: String, amountZatoshi: Long = DEFAULT_MESSAGE_AMOUNT) {
    // - Orchard 잔액 확인
    // - 비용 disclaimer 표시 (첫 송신 시)
    // - 송신 중이면 messageQueue에 추가
    // - 아니면 doSendMessage 즉시 호출
}
```

(코드 본체는 본 dive 범위 외 — 분기는 `_isSendingMessage` + `messageQueue.isEmpty()`)

**2. `doSendMessage` 진입 (line 2429)**

```kotlin
private fun doSendMessage(peerAddress, message, amountZatoshi, existingPendingId, retryCount) {
    viewModelScope.launch {
        _sendMessageState.value = SendMessageState.Sending
        val pendingId = existingPendingId ?: "pending_${System.nanoTime()}"
        try {
            val userAddress = _currentUserAddress.value ?: throw ...
            
            // 1. Pending message 즉시 UI 표시 (optimistic)
            if (existingPendingId == null) {
                pendingMessages.update { it + ChatMessage(id=pendingId, ...) }
                zchatPreferences.addPendingMessage(...)  // 영구화
            }
            
            // 2. ConvID 결정 (atomic at prefs level + mutex)
            val (convId, isFirstMessage) = convIdMutex.withLock {
                zchatPreferences.getOrCreateConversationId(peerAddress)
            }
            
            // 3. E2E ratchet 암호화
            val processor = getOrCreateMessageProcessor(peerAddress, convId)
            val outgoingMessage = if (processor != null) {
                processor.encryptOutgoing(message)  // throws on failure
            } else {
                message  // KEX 미완료 — plaintext 송신
            }
            
            // 4. Proposal + submit (Default dispatcher)
            kotlinx.coroutines.withContext(Dispatchers.Default) {
                createChunkedMessageProposal(
                    destinationAddress = peerAddress,
                    senderAddress = userAddress,
                    message = outgoingMessage,
                    isFirstMessage = isFirstMessage,
                    amountPerOutput = Zatoshi(amountZatoshi),
                    directSubmit = true,
                    skipNavigation = true,
                    conversationId = convId
                )
            }
            
            addressCache.addConversationPartner(peerAddress)
            _sendMessageState.value = SendMessageState.Success
            processNextQueuedMessage()
        } catch (e: Exception) {
            // Insufficient balance / queue retry / FAILED 분류
            ...
        }
    }
}
```

핵심 보안 디테일 (line 2492-2494 주석):
> SECURITY: if encryption fails, ABORT the send rather than falling back to plaintext. Silent plaintext fallback is a confidentiality failure — the user expects E2E and doesn't know it was bypassed.

**3. Layer B → Layer A 위임 (§1.5의 `createMultiOutputProposal` 참조)**

```
createChunkedMessageProposal.invoke
  ↓
zashiProposalRepository.createZip321Proposal(uri)
  ↓
synchronizer.proposeTransferFromUri(account, uri)        [SDK Layer A]
  ↓ (proposal 객체 반환)
zashiProposalRepository.submit()
  ↓
zashiSpendingKeyDataSource.getZashiSpendingKey()         [BIP-39 → ZIP-32 → USK]
  ↓
synchronizer.createProposedTransactions(proposal, usk)   [SDK Layer A]
  ↓ JNI
librustzcash::create_proposed_transactions
  - Orchard Halo2 proof
  - RedPallas spend auth sig
  - Binding sig
  ↓
lightwalletd gRPC submitTransaction
  ↓
zebrad/zcashd mempool → block
```

이후 SDK `transactions` Flow가 새 outgoing tx를 emit. ChatViewModel이 자기 자신의 송신 tx를 다시 받아 pending message → confirmed message 변환.

**4. Queue retry on note locking (line 2527-2607)**

InsufficientFunds 발생 + 이전 송신의 change가 mempool에 있는 상황:
1. 메시지를 queue 앞쪽에 re-insert
2. `_blockHeight.first { it > currentHeight }` — 새 블록 scan될 때까지 대기
3. 새 block 도착 시 자동 retry. MAX_QUEUE_RETRIES=4까지, QUEUE_RETRY_TIMEOUT_MS=5분.
4. 타임아웃 또는 retry 소진 시 FAILED 상태로 표시.

### B. 수신 path — SDK Flow → UI

**1. Flow combine (line 491-563)**

```kotlin
val conversationsFlow = combine(
    transactionRepository.transactions.filterNotNull().debounce(300),
    hiddenMessages,
    pendingMessages
) { transactions, hiddenMsgIds, pending ->
    convertToConversations(txList, userAddress, hiddenMsgIds, pending)
}
```

- `debounce(300ms)`: sync 중 빠른 emission batch
- combine with sync status / wallet snapshot / exchange rate

**2. `convertToConversations(transactions, ...)` (line 570) — 핵심 dispatch**

각 트랜잭션마다:
1. `transactionRepository.getMemos(tx)` — SDK가 노트 복호화 후 평문 memo bytes 반환
2. `memoText = memos.joinToString("\n").trim()`
3. **Remote kill 검사** (line 633-639): 수신 tx + amount + memo 매치 → DestroyManager 트리거 (§1.7)
4. **분기 순서** (parsing priority):
   - **Status** (`isStatus`) → `peerStatuses.update` + skip
   - **Reaction / ReadReceipt** → skip (UI에 별도 처리)
   - **KEX / KEXACK** (`isKEXMessage` / `isKEXAckMessage`):
     - Incoming → `handleKEXMessage(memoText, userAddress, txIdStr)` (line 1702)
     - Outgoing → `setE2EKexTxId(peer, txIdStr)` 또는 `setE2EKexAckTxId` — Ratchet root에 들어갈 자기 송신 txid 저장
   - **GROUP** (`isGroupMessage`) → `processGroupMessage` (§1.4)
   - **Payment request** (`isPaymentRequest`) → `paymentRequestInfo` 채움
   - **Time-lock** → `timeLock` 채움 (§1.1)
   - **Unlock** (`isUnlock`) → 해당 time-lock 메시지 unlock
   - **Reply (RPL)** → `replyToTxId` 채움
   - **Regular** → `ZMSGProtocol.parseMemo` → `ParsedMessage` → E2E1: 검사 후 `E2EMessageProcessor.decryptIncoming` → `ChatMessage` 객체 → `messagesByPeer[peerAddress]` 에 추가
5. Pending list dedup + reorder + return `List<Conversation>`

**3. UI state 갱신 (line 561)**

```kotlin
.collectLatest { state ->
    _chatListState.value = state
}
```

`ChatListView` / `ChatDetailView`가 Compose `collectAsState`로 reactive 렌더링.

### C. KEX 핸드셰이크 — `handleKEXMessage` (line 1702)

(자세한 KEX wire format은 §1.2 / §1.1, ratchet root 도출은 §1.3 참조.)

**1. parse + verify**

```
val (convId, kexPayload) = ZMSGProtocol.parseKEXMessage(memoText)
val peerPublicKey = E2EEncryption.parseKEXPayload(kexPayload, senderAddress)
    ?: return  // 서명 검증 실패
```

**2. peer public key 저장 + 자동 E2E enable (line 1820-1830)**

```kotlin
zchatPreferences.setE2EPeerPublicKey(senderAddress, peerPublicKey)
if (!zchatPreferences.isE2EEnabled(senderAddress)) {
    if (zchatPreferences.getE2EOurPublicKey(senderAddress) == null) {
        val keyPair = E2EEncryption.generateKeyPair()
        zchatPreferences.setE2EOurKeys(senderAddress, keyPair.publicKey, keyPair.privateKey)
        zchatPreferences.setE2EKeyVersion(senderAddress, E2EKeyVersion.V2.value)
    }
    zchatPreferences.setE2EEnabled(senderAddress, true)
}
```

**3. KEXACK 송신** — `sendKEXAckMessage(peerAddress, ourAddress, convId)` (line 1899)

ourPub + 우리 priv로 ECDSA 서명한 payload를 `ZMSGProtocol.createV4KEXAckMessage` 후 `createChunkedMessageProposal(..., rawMemo=true)` 호출 (§1.5)

**4. Ratchet root + processor 생성** (`getOrCreateMessageProcessor` 호출 시)

- ECDH shared secret = `E2EEncryption.deriveSharedSecret(ourPriv, peerPub, V2, psk)`
- KEX/KEXACK 두 txid를 prefs에서 가져옴
- `E2ERatchet.deriveRatchetRoot(ecdhSecret, psk, kexTxid, kexAckTxid)` → 32B root
- `E2EMessageProcessor(rootKey, convId, isLower, EncryptedPrefsRatchetStateStore)` 생성 후 캐시

### D. 백그라운드 sync

**1. Foreground service (앱 활성 시)**

`SyncForegroundService` (594 lines) — Sync 진행 중 persistent notification 표시 + Android의 background kill 회피. 채널 이름 "ZCHAT Sync" (claude.md v2.8.1+v2.8.5).

**2. Periodic worker (15분, 앱 백그라운드)**

`SyncWorker.kt:73` — `SYNC_PERIOD = 15.minutes`. `PeriodicWorkRequestBuilder`로 등록, NetworkType.CONNECTED 제약 + 1분 initial delay. `doWork()`는 `synchronizer.status.combine(progress)`를 collect하여 SYNCED 까지 대기, SYNCED면 `Result.success()`, 아니면 `Result.retry()`.

**3. AlarmManager fallback (Android 15 FGS timeout)**

claude.md v2.8.5에 따르면 Android 15에서 ForegroundService가 timeout될 수 있어 `SyncAlarmReceiver` + `SyncAlarmScheduler`로 15분마다 알람 fallback. 코드 본체는 `work/SyncAlarmReceiver.kt` + `work/SyncAlarmScheduler.kt`.

**4. App 활성 시 60초 auto-refresh (line 2018-2068)**

`AUTO_REFRESH_INTERVAL_SECONDS = 60`. `autoRefreshJob`이 60초마다 `synchronizer.refreshTransactions()` + `synchronizer.refreshAllBalances()` 호출. 사용자가 chat 화면 보고 있을 때 새 메시지가 1분 안에 표시되도록 보장.

## 노트 / quirks / footguns

- **`api.zsend.xyz` 사용 위치 검증.** `Grep "api.zsend|/me/wallet|/wallet/send"` 결과: chat / wallet path 의 `.kt` 파일에는 **0건**. hit 파일들은 `UpdateChecker.kt`(앱 업데이트 확인), `InviteFriendVM.kt`(invite 기능), docs, strings.xml, README/CHANGELOG뿐. → **README claim "No servers, no accounts" + CODEBASE_MAP §9 "wallet 동작에 안 부름" 정확.** wallet/메시지 송신 경로는 lightwalletd 직결.
- **`AUTO_REFRESH_INTERVAL_SECONDS = 60`이 spec 명시.** claude.md v2.8.5는 "15-min WorkManager sync"라 했고 코드도 일치하지만, 그건 *백그라운드*. 사용자가 app을 열어둔 상태에서는 60초 폴링이 작동. ZMSG_PROTOCOL_SPEC.md "~75-second delivery latency"는 Zcash 블록 생성 시간이지 *수신 지연*은 75초 + 0~60초 폴링 ≈ 평균 105초.
- **`MessageQueue` retry는 새 block confirmation까지 대기.** 같은 사용자가 빠른 송신 burst를 만들면 Zcash note locking으로 두 번째 송신이 InsufficientFunds. 5분 안에 새 block 1개 도착 안 하면 FAILED. **메인넷 Zcash block 평균 75초**라 일반적으로 안전하지만 mempool congestion 시 위험. UI는 messageQueue 길이를 표시하지 않음 — 사용자가 N개 빠르게 보내면 첫 1개 외 모두 대기 상태인지 알기 어려움.
- **`getZashiSpendingKey()`는 매 송신마다 호출.** §1.5에 명시. ChatViewModel 자체는 USK 캐시 안 함 — 보안적으로 좋음. `viewModelScope.launch`가 `Dispatchers.Default`로 옮겨가는 line 2505로 proof generation 동안 UI 자유.
- **`directSubmit = true` + `skipNavigation = true` 조합으로 chat 화면에서 모든 송신 처리.** 일반 wallet 흐름의 review screen 우회. 비용 disclaimer는 *첫 송신 시 단 한 번* 표시 — 그 다음부터는 silent. 사용자가 한 번 동의하면 무한히 ZEC 소비 가능 — 충분한 UX warning 필요.
- **`convertToConversations`는 매번 *모든* tx를 다시 처리.** debounce(300)로 batch지만 sync가 활발할 때 큰 tx 리스트가 들어오면 빠른 재처리. cache 없음. Mobile CPU에 부담 가능 — 분명한 hardening 후보.
- **Outgoing KEX/KEXACK의 txid는 prefs에 저장된다 (line 668-682).** Ratchet root 도출에 KEX txid + KEXACK txid가 필요한데(§1.3), 자기가 KEX initiator라면 자기 송신 KEX의 txid도 알아야 한다. `transactions` Flow에서 자기 송신 KEX를 발견하면 `setE2EKexTxId`로 prefs에 저장. 이 사실은 *수신 측만 root을 도출하면 끝*인 게 아니라 *양쪽이 자기 송신 + 수신 txid를 모두 알아야* 한다는 점을 의미. seed restore 시 blockchain 스캔으로 두 txid 복원 가능 (§1.3에서 검증).
- **`processedKillCheckTxIds`로 remote-kill 중복 검사 방지** (line 1996). 같은 tx가 두 번 Flow에 들어와도 destruction이 한 번만 트리거.
- **`@OptIn(FlowPreview::class)` debounce 사용.** Kotlin Coroutines의 preview API. Stable 전환 시 cleanup.
- **3736 lines 단일 ViewModel은 큰 technical debt.** claude.md v2.9.1 audit "ChatViewModel.kt(1150 lines)"는 그 시점 기록이고 현재는 더 큼. 우리 팀 포팅 시 *반드시* split — minimum (send + receive + KEX + group + special types) 분리.
- **`pendingMessage`(line 120, 단수) vs `pendingMessages`(line 138, 복수)는 서로 다른 개념.** 전자는 disclaimer 대기 중인 한 건. 후자는 in-flight 옵티미스틱 UI 목록. 네이밍 혼란.
- **`messageQueue`는 `mutableListOf` + `synchronized` block.** kotlinx.coroutines.sync 가 아닌 JVM monitor. mixed concurrency primitive — uniform Mutex 권장.

## 답한 open question

- **Q14** (research-plan §7): "백그라운드 sync 실제 polling 주기"
  > **Answer:** 두 가지 layer. (1) 앱 활성 시: `AUTO_REFRESH_INTERVAL_SECONDS = 60` — 60초 폴링 (line 2369). (2) 앱 백그라운드: `SyncWorker.SYNC_PERIOD = 15.minutes` — WorkManager PeriodicWorkRequest (work/SyncWorker.kt:73). + Android 15 FGS timeout 대비 AlarmManager fallback (work/SyncAlarmReceiver + SyncAlarmScheduler). + Foreground service (service/SyncForegroundService.kt) 가 sync 진행 중 persistent notification으로 OS kill 회피. — `ChatViewModel.kt:2018-2068`, `work/SyncWorker.kt`

- **Q15** (research-plan §7): "api.zsend.xyz 백엔드는 wallet 동작에 안 부르는가?"
  > **Answer:** ✓ **검증됨**. `Grep "api.zsend|/me/wallet|/wallet/send"` 결과 chat/wallet/common path `.kt` 파일에 0건. hit은 `UpdateChecker.kt` (앱 버전 확인), `InviteFriendVM.kt` (invite 기능), docs / strings.xml / README / CHANGELOG / deploy-apk.sh 뿐 — 모두 메시지 transport 외. 즉 README "No servers, no accounts" claim과 CODEBASE_MAP §9.4 "Wallet 동작에는 안 부름" 정확. — `Grep 결과 + ChatViewModel.kt` 코드 전반

- **Q16** (research-plan §7): "메시지 수신 시 SDK Flow에서 memo를 어떻게 polling/decoding"
  > **Answer:** `TransactionRepository.transactions: Flow<List<Transaction>?>` 를 `ChatViewModel`이 `filterNotNull().debounce(300)`로 구독 (line 493). 새 emission 시 `convertToConversations(txList, ...)` 호출하여 각 tx마다 `transactionRepository.getMemos(tx)` 로 평문 memo 추출. 그 다음 ZMSG parsing priority 순서로 분기. SDK 측은 노트의 ChaCha20-Poly1305 복호화를 자동 수행하고 평문 bytes를 반환 — ZMSG layer는 UTF-8 string으로 받음. — `ChatViewModel.kt:491-694`

- **Q25** (research-plan §7): "75초 블록 지연 + 영구 onchain 노출이라는 본질 한계를 어떻게 사용자에게 노출/해결?"
  > **Answer:** **Optimistic UI**가 핵심 — `pendingMessages` flow가 송신 즉시 UI에 표시 (line 138, 2447). 사용자 시점에서 메시지가 "즉시 sent"로 보이고, 백그라운드에서 트랜잭션 broadcast + block confirmation을 기다림. `MessageStatus` enum (SENDING/SENT/CONFIRMED/READ/FAILED, §1.1 ChatMessage)로 status 시각화 — clock icon → 1 checkmark → 2 checkmark → blue 2 checkmark. 다만 **영구 onchain 노출 문제 자체는 해결 불가** — Zcash 노트 암호화 + Ratchet FS로 *receiver IVK 안전 + root 안전* 가정 하에 미래 복호화 불가지만, viewing key leak 시 모든 과거 메시지 노출. ZMSG_PROTOCOL_SPEC.md Threat model summary가 이를 honest하게 명시. — `ChatViewModel.kt:2447-2467`, `ChatMessage.kt:61-67`

- **C20~C24, C100, C110-C124** (claims-to-verify): 다수 검증
  > **Answer:** 코드와 일치. Koin DI (`KoinComponent` in SyncWorker), Coroutines (Flow + StateFlow + Mutex), EncryptedSharedPreferences (ZchatPreferences), 시드 송신 시 한 번 로드 (`getZashiSpendingKey`), Layer C 의 chat module, 1730/2794/1150 line file 크기. file:line 인용들도 대부분 일치(일부 line 수가 추가된 코드로 인해 약간 어긋남 — 함수 위치는 본 문서가 정확).

- **C26 / Q15** 정확히 확인됨 (위 Q15 참조)
