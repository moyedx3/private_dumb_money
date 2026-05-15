# Zchat Deep Dive — Corrections Log (코드 재검증 결과)

> **2026-05-16 일괄 정정**. 이전 분석에서 추론·spec 라벨에 의존했던 부분을 코드 file:line 기준으로 모두 재검증한 결과. 본 파일이 *권위 있는 참조* 이며 다른 파일의 인용은 모두 본 파일을 가리킨다.

원칙: **모든 claim 은 source code file:line 으로 뒷받침되어야 한다.** code reference 없는 추론은 "추가 검증 필요" 로 표기.

---

## §A. Ratchet 영역 (가장 큰 정정)

### A.1 rootKey 저장 위치 (정정됨)

| 이전 표현 | 코드 verify 결과 |
|---|---|
| "rootKey 가 prefs 에 영구 보관" (1차 답변) | ❌ **부정확**. rootKey 는 prefs 에 저장되지 *않음*. |
| 정확한 사실 | **rootKey 자체는 메모리 (`ChatViewModel.messageProcessors: ConcurrentHashMap`) 에만 존재.** 그 *재도출 입력값* (E2E priv/pub + KEX/KEXACK txid + optional PSK) 이 prefs 에 영구 저장. 앱 시작 시 입력값 → root 재도출 후 메모리 캐시. |

**Code references:**
- `ChatViewModel.kt:182` — `private val messageProcessors = java.util.concurrent.ConcurrentHashMap<String, E2EMessageProcessor>()`
- `ChatViewModel.kt:1624-1668` — `getOrCreateMessageProcessor(peerAddress, convId)`:
  - cacheKey = `"$peerAddress:$convId"`
  - cache miss 시 prefs 에서 (sharedKey/ourPub/peerPub/kexTxId/kexAckTxId/psk) 로드 후 `E2ERatchet.deriveRatchetRoot` 호출
- `RatchetStateStore.kt:12-18` — `RatchetConversationState` 스키마: convId + nextCounterA2B/B2A + seenCountersA2B/B2A. **rootKey 필드 없음.**
- `EncryptedPrefsRatchetStateStore.kt:43-49` — JSON 직렬화 시 rootKey 미저장.

### A.2 chain_key advance 메커니즘 (정정됨 — 핵심)

| 이전 표현 | 코드 verify 결과 |
|---|---|
| "Forward secrecy 진짜다 — message key 사용 후 폐기" (§1.3 초기) | ❌ **부정확**. chain_key 가 *advance/wipe 되지 않음*. |
| 정확한 사실 | **`deriveMessageKey` 가 매번 `chain_key_0` 부터 stateless walk.** 호출 끝나면 도출된 chain_key/message_key 가 stack frame 과 함께 해제되지만, **rootKey 가 있으면 어떤 counter 의 키든 즉시 재도출 가능.** |

**Code references:**
- `E2ERatchet.kt:133-142` — `deriveMessageKey(direction, counter)`:
  ```kotlin
  var chainKey = deriveChainKey0(direction)
  var step = 0L
  while (step < counter) {
      chainKey = hmacSha256(chainKey, byteArrayOf(CHAIN_STEP_BYTE))
      step++
  }
  return hmacSha256(chainKey, byteArrayOf(MESSAGE_KEY_BYTE))
  ```
- `E2ERatchet.kt:144-153` — `deriveChainKey0(direction)`: `HKDF(rootKey, salt=null, info=CHAIN_INFO_*, 32)`.
- 클래스 멤버 변수에 `chainKey` 또는 `messageKey` 없음 — `E2ERatchet` 의 instance fields 는 `rootKey`, `convId`, `isLower`, `store`, `myDirection`, `sessionSeenA2B/B2A` (line 25-39) 만.

### A.3 Forward Secrecy 표현 정정

이전 모든 "forward secrecy 제공" 라벨이 **부정확**. 정확한 표현:

| 공격 시나리오 | zchat 의 실제 보호 |
|---|---|
| Network observer (Zcash ciphertext 만 캡처) | ✅ 안전 — Zcash 노트 암호화 (ChaCha20-Poly1305 via receiver IVK). **ratchet layer 가 *추가로* 보장하는 것은 없음** |
| 단일 message_key leak | ✅ 다른 message_key 영향 없음 — HMAC unique outputs |
| Device prefs leak (priv key + peer pub + KEX txid 모두 빼냄) | ❌ rootKey 재도출 → 모든 메시지 복호화 |
| Live 메모리 dump (앱 실행 중) | ❌ `messageProcessors` 안의 rootKey 노출 |
| 새 device 에서 BIP-39 24 단어 복원 | ❌ (의도된 동작) 모든 메시지 복호화 가능 |

**결론:** Signal/Megolm 의 *전통적 의미 forward secrecy* 는 zchat 에 없음. zchat 의 ratchet code 가 *실제로* 제공하는 보호:
1. Replay 보호 (counter window) — `E2ERatchet.kt:78-89`, `MAX_SKIP = 1000` (line 221)
2. Cross-conversation routing 방지 (AAD with convId) — `E2ERatchet.kt:167-171`
3. AES-GCM nonce 충돌 방지 (counter-based nonce + .commit() synchronous) — `EncryptedPrefsRatchetStateStore.kt:30-36`

이 세 가지는 *transport-level integrity* 자산이지 *forward secrecy* 가 아님.

### A.4 isLower 계산 방식 (정정됨)

| 이전 표현 | 코드 verify 결과 |
|---|---|
| "compressed secp256r1 public key 의 lexicographic 비교" (§1.3) | ⚠️ **약간 부정확**. compressed bytes 비교 아님. |
| 정확한 사실 | **Base64 인코딩된 X.509 SubjectPublicKeyInfo 의 string 비교.** `ourPub < peerPub` (Kotlin String compareTo). 결과는 lex-compare 와 동일하지만 *비교 대상 표현*이 다름. |

**Code reference:**
- `ChatViewModel.kt:1639` — `val isLower = ourPub < peerPub`
- `E2EEncryption.kt:143-144` — `generateKeyPair()` 가 `Base64.getEncoder().encodeToString(keyPair.public.encoded)` 반환 (encoded 는 X.509 형식)

### A.5 KEX/KEXACK txid empty fallback (새 발견)

| 이전 표현 | 코드 verify 결과 |
|---|---|
| "ratchet root 도출에 KEX/KEXACK 두 txid 가 binding" (§1.2 / §1.3) | ⚠️ **부분 정확**. txid 가 *없을 때 fallback* 가 있음. |
| 정확한 사실 | prefs 에 `E2EKexTxId` / `E2EKexAckTxId` 가 없으면 **빈 ByteArray 사용**. ECDH shared secret 만으로도 root 도출 가능. 따라서 보안적으로는 *re-KEX 시 root 회전 보장*이 약화됨 — 같은 두 사람의 다른 conversation 이 같은 root 를 가질 수 있음. |

**Code reference:**
- `ChatViewModel.kt:1644-1647`:
  ```kotlin
  val kexTxId = zchatPreferences.getE2EKexTxId(peerAddress)
      ?.toByteArray(Charsets.UTF_8) ?: ByteArray(0)
  val kexAckTxId = zchatPreferences.getE2EKexAckTxId(peerAddress)
      ?.toByteArray(Charsets.UTF_8) ?: ByteArray(0)
  ```
- 주석 line 1641-1643: "Falls back to empty txids for conversations where KEX happened before txid storage was implemented"

### A.6 tryDecryptMessage fail-open 정책 (새 발견)

**Code reference:** `ChatViewModel.kt:1675-1687`

```kotlin
private suspend fun tryDecryptMessage(content: String, peerAddress: String, convId: String?): String {
    if (convId == null) return content
    if (!CiphertextWireFormat.isRatcheted(content)) return content
    return try {
        getOrCreateMessageProcessor(peerAddress, convId)?.decryptIncoming(content) ?: content
    } catch (e: ReplayDetectedException) {
        Log.d("ZCHAT_E2E", "Replay of counter ${e.counter} ...")
        "🔒 Encrypted message"   // 🔒
    } catch (e: Exception) {
        Log.w("ZCHAT_E2E", "Ratchet decrypt failed ...")
        "🔐 Encrypted message (unable to decrypt)"   // 🔐
    }
}
```

**핵심 사실:**
- Replay 감지 시 사용자에게 "🔒 Encrypted message" 표시 (정상 동작 — 이미 봤던 메시지 의미)
- 기타 decrypt 실패 시 "🔐 Encrypted message (unable to decrypt)" placeholder
- Processor 가 null 일 때 (KEX 미완료) 는 content 그대로 통과 = plaintext 가 화면에 표시됨

### A.7 ReplayDetectedException 의 session-scoped 특성 (재확정)

`E2ERatchet.kt:33-39` 주석 그대로:

```kotlin
// Session-scoped seen-counter sets. NOT persisted across restarts.
// Persisting them would break re-scan: on restart, all previously-decrypted
// incoming messages would trigger ReplayDetectedException and show as
// E2E1: blobs. Instead, replay detection is per-session only.
// Send counters (nextCounterA2B/B2A) ARE persisted to prevent GCM nonce reuse.
```

**핵심 사실:**
- `sessionSeenA2B` / `sessionSeenB2A` 는 메모리만 (`MutableSet<Long>`)
- prefs 의 `seenCountersA2B/B2A` 필드는 항상 빈 array 로 직렬화 — `EncryptedPrefsRatchetStateStore.kt:43-49` (dead schema field)
- 앱 재시작 시 모든 seen counter 가 reset → 같은 메시지를 다시 decrypt 해도 ReplayDetectedException 안 남
- 정상 동작이지만 **cross-restart replay 보호는 없음** — sender 가 같은 ciphertext 를 두 transaction 으로 보내면 첫 session 에서 1번, restart 후 1번 = 총 2번 decrypt

### A.8 deterministic-root design 의 진짜 동기 (정정됨)

| 이전 표현 | 정확한 표현 |
|---|---|
| "BIP-39 seed 복원 호환성을 위한 의도적 trade-off" | ❌ **False dichotomy.** Stateful Signal-style ratchet 으로도 BIP-39 복원은 *순차 replay* 로 가능. |
| 정확한 동기 (코드 주석 기반) | **rescan idempotency + 구현 단순성.** Blockchain 메시지를 *어떤 순서로, 몇 번이고* 재처리해도 같은 plaintext 가 나와야 함 (re-scan 발생 잦음). stateless schedule 이 이를 trivially 보장. |

**Evidence:**
- `E2ERatchet.kt:33-39` 주석 — re-scan 시 stateful 이 깨지는 시나리오 명시
- `docs/superpowers/specs/2026-04-12-e2e-ratchet-deterministic-design.md` 라는 spec 파일명 존재 (`E2ERatchet.kt:13` 주석에서 인용) — 정독 시 더 정확한 동기 도출 가능, 본 dive scope 외

---

## §B. Time-lock 영역 (정정됨)

### B.1 ZTL 메시지가 plaintext 로 송신됨 (확정)

| 이전 표현 | 코드 verify 결과 |
|---|---|
| Scenario 11 sequence diagram 의 일반 흐름 | ⚠️ 흐름 정확, 다만 **보안 caveat 누락** |
| 정확한 사실 | **모든 ZTL/ZUNLOCK 메시지가 `rawMemo = true` 로 송신.** ratchet wrap 없음. Wire format 에 plaintext message 가 그대로 박힘. |

**Code references:**
- `ChatViewModel.kt:3250-3268` — `sendScheduledMessage` — `createScheduledMessage(message=plaintext, ...)` + `rawMemo=true`
- `ChatViewModel.kt:3278-3296` — `sendBlockLockedMessage` — 동일 패턴
- `ChatViewModel.kt:3306-3324` — `sendPaymentLockedMessage` — 동일 패턴
- `ChatViewModel.kt:3335-3353` — `sendConditionalMessage` — 동일 패턴
- `ChatViewModel.kt:3360-3382` — `unlockPaymentMessage` (ZUNLOCK PAY) — 동일 패턴 (line 3367 `createUnlockPayment` + `rawMemo=true`)
- `ChatViewModel.kt:3394-3428` — `unlockConditionalMessage` (ZUNLOCK CND) — 동일 패턴 (line 3409 `createUnlockAnswer` + `rawMemo=true`; **answer 도 plaintext on-chain**)

### B.2 ZTL 수신 시 plaintext 가 ChatMessage 에 저장됨 (확정)

| 이전 표현 | 코드 verify 결과 |
|---|---|
| "잠금 메시지 본문은 안 보임" | ❌ **부정확**. plaintext 는 메모리에 있고 UI getter 가 가림. |
| 정확한 사실 | **`ChatMessage.text` 필드에 plaintext message 가 그대로 저장됨**. UI 의 `displayText` getter 가 `isLocked` 일 때만 lock emoji 로 가림. |

**Code references:**
- `ChatViewModel.kt:722-761` — `parseTimeLock` 호출, `TimeLockInfo` 생성 (lockType, unlockTimestamp, requiredPayment, hint, answerHash, isUnlocked 등)
- `ChatViewModel.kt:1022-1028`:
  ```kotlin
  val finalMessage = if (timeLockInfo != null && ZMSGProtocol.isTimeLock(memoText)) {
      val parsedTimeLock = ZMSGProtocol.parseTimeLock(memoText, addressCache)
      parsedTimeLock?.message ?: displayMessage   // plaintext 추출
  } else displayMessage
  ```
- `ChatViewModel.kt:1037-1054` — `ChatMessage(text = messageText, ...)` — text 필드에 plaintext 들어감
- `ChatMessage.kt:133-138` — `displayText` getter:
  ```kotlin
  val displayText: String
      get() = when {
          isLocked -> "🔒 ${timeLock?.lockDescription ?: "Locked message"}"
          isPaymentRequest -> ...
          else -> text
      }
  ```
- `ChatMessage.kt:121-122` — `isLocked` getter: `timeLock != null && !timeLock.isUnlocked`

→ **viewing key 보유자가 raw memo 보면 plaintext 그대로 보임.** zchat 앱 UI 의 잠금은 *Compose getter 한 줄* 의 결정.

### B.3 unlockedMessages 의 in-memory 특성 (새 발견)

**Code reference:** `ChatViewModel.kt:166`
```kotlin
private val unlockedMessages = MutableStateFlow<Map<String, String>>(emptyMap())
```

- **prefs 영구 저장 없음.** 매핑: `originalTxId` → `unlocking_messageId` ("local" 또는 ZUNLOCK txid)
- 앱 재시작 시 reset. blockchain rescan 시 ZUNLOCK 메시지 재발견 (`ChatViewModel.kt:711-715`) → 자동 재구성
- **앱 재시작 후 conditional unlock 한 메시지가 다시 잠긴 것처럼 보일 수 있음** (rescan 이전 시점). rescan 완료 후 자동 unlock.

**Code reference:** `ChatViewModel.kt:711-715`:
```kotlin
if (ZMSGProtocol.isUnlock(memoText)) {
    val parsedUnlock = ZMSGProtocol.parseUnlock(memoText, addressCache)
    if (parsedUnlock != null) {
        unlockedMessages.update { it + (parsedUnlock.originalTxId to messageId) }
    }
    ...
}
```

### B.4 Conditional answer 검증의 약점 (재확정 + 강화)

**Code references:**
- `ZMSGSpecialMessages.kt:382-385` — `verifyConditionalAnswer`:
  ```kotlin
  fun verifyConditionalAnswer(answer: String, answerHash: String): Boolean {
      val computedHash = ZMSGProtocol.generateAddressHash(answer.lowercase().trim())
      return computedHash == answerHash
  }
  ```
- `ZMSGProtocol.kt:106-111` — `generateAddressHash`: SHA-256(input).take(8 bytes).hex
- `ZMSGSpecialMessages.kt:216-228` — `createConditionalMessage`: `val answerHash = ZMSGProtocol.generateAddressHash(answer.lowercase().trim())`

**약점:**
- 64-bit 잘린 SHA-256 + `lowercase().trim()` normalization 만
- **PBKDF2 같은 key stretching 없음** (SecureHash 와 별개)
- 짧은 답 (예: "yes", "blue") 은 dictionary attack 1초 이내
- `answer` 도 ZUNLOCK 메시지에 plaintext 로 onchain — 한 번 unlock 되면 답이 영원히 공개

### B.5 Zcash shielded pool 의 script 부재 (재확정)

zchat 코드 외부 사실이지만 본 dive 결론과 직결:
- Sapling/Orchard 노트는 commitment + nullifier 구조. Bitcoin-style unlock script 없음.
- 따라서 *protocol-level* time-lock enforcement 불가
- zchat 의 ZTL = *application-level UI lock + plaintext on-chain* — viewing key 우회 시 무력화

---

## §C. Group 영역 (재확정)

### C.1 GROUP_MSG 의 AAD 부재 (재확정 — 코드 grep)

| spec 표현 | 코드 실제 |
|---|---|
| ZMSG_PROTOCOL_SPEC.md "AAD = `groupId || senderAddress`" | ❌ **코드에 미구현** |

**Code grep result:** `cipher.updateAAD` 사용처 = 다음 두 곳뿐 (zchat repository 전체):
- `E2EEncryption.kt:800, 821` — `encryptFile` / `decryptFile` (파일 암호화)
- `E2ERatchet.kt:181, 193` — 1:1 ratchet 메시지

→ **`ZMSGGroupProtocol.encryptMessage` (line 402-419) 에 AAD 호출 없음.** spec/code mismatch 확정.

### C.2 createGroupKeyMessage (GY) caller 0건 (재확정)

**Grep result:** `createGroupKeyMessage` 정의 = `ZMSGGroupProtocol.kt:230` 하나. **호출 site 0건.**

→ Group key rotation 은 protocol wire format 만 정의, 실제 호출 없는 **dead function**.

### C.3 leaveGroup 의 자동 키 회전 부재 (재확정)

**Code reference:** `GroupViewModel.kt:777-840` `leaveGroup(groupId)`:
- local member status 를 LEFT 로 update (line 786-793)
- group `isActive = false` 로 update (line 800-804)
- 다른 active members 에게 `createGroupLeaveMessage` (GL) broadcast (line 815-...)
- **GY 호출 없음** → 떠난 멤버는 epoch 0 group key 그대로 보유

---

## §D. Identity Regeneration 영역 (정정됨)

### D.1 ADDR broadcast 미구현 (DEAD CODE — 새 발견)

| 이전 표현 | 코드 verify 결과 |
|---|---|
| §1.7 "수동 broadcast 가능 (가설적 UI)" | ❌ **부정확**. 가설적이 아니라 *완전 미구현*. |
| 정확한 사실 | `createV4ADDRMessage` 는 `ZMSGProtocol.kt:305` 에 정의되어 있지만 **caller 0건**. ChangeIdentityVM 의 `sendAddressChangeNotifications` 가 TODO 처리 + `Log.d` 만 호출. |

**Code reference:** `ChangeIdentityVM.kt:213-232`
```kotlin
private suspend fun sendAddressChangeNotifications(oldAddress: String, newAddress: String) {
    val addressBookContacts = zchatPreferences.getAllContactAddresses()
    val chatContacts = zchatPreferences.getAllConversationPeerAddresses()
    val uniqueContacts = (addressBookContacts + chatContacts).toSet()

    // For each contact, we would send an ADDR message
    // ...
    // TODO: Implement actual notification sending
    // This requires integration with the send flow which is complex
    // For now, the identity change works but notifications are not sent

    android.util.Log.d("ChangeIdentityVM", "Would notify ${uniqueContacts.size} contacts...")
}
```

→ **Identity 회전 후 기존 contacts 는 새 메시지를 *모르는 sender* 로 받게 됨.** ADDR migration UX 자체가 없음.

### D.2 createV4ADDRMessage 도 incoming parse handler 없음

**Grep result:** `parseADDRMessage` 정의 = `ZMSGProtocol.kt:334`. 호출 site:
- `ZMSGProtocol.kt:1493` — wrapper 함수
- `ZMSGProtocolTest.kt` — 테스트 코드만

**즉 ChatViewModel 에서 ADDR 메시지 incoming 분기 처리 없음.** 만약 외부에서 ADDR 메시지가 와도 처리 코드 없음 — `parseMemo` 의 `isADDRMessage` 분기조차 확인 필요. (`isKEXMessage` 분기는 있음, ADDR 분기 없음 추정 — 추가 검증 필요)

---

## §E. Remote Kill / Destroy PIN 영역 (재확정)

### E.1 SecureHash 사용 (PBKDF2 600k + legacy SHA-256)

**Code references:**
- `ZchatPreferences.kt:1133-1134` (`setDestroyPin`):
  ```kotlin
  val hashed = SecureHash.hash(pin)
  prefs.edit().putString(KEY_DESTROY_PIN, hashed).apply()
  ```
- `ZchatPreferences.kt:1139` (`verifyDestroyPin`): `SecureHash.verify(pin, storedHash)`
- `ZchatPreferences.kt:1168-1173` (`setRemoteKillPhrase` / `verifyRemoteKillPhrase`) — 동일 패턴
- `SecureHash.kt:22-26` 상수:
  ```kotlin
  private const val ALGORITHM = "PBKDF2WithHmacSHA256"
  private const val ITERATIONS = 600_000  // OWASP 2023 recommendation
  private const val KEY_LENGTH_BITS = 256
  private const val SALT_LENGTH_BYTES = 16
  ```
- **legacy SHA-256 backward compat** 존재 (`SecureHash.kt:43-51, 77-86`): `verify()` 가 prefix 검사 후 PBKDF2 또는 legacy SHA-256 path 선택

### E.2 prefs key 정정 (사소)

| 이전 표현 | 정확 |
|---|---|
| `remote_kill_phrase_hash` (한국어 텍스트) | `KEY_REMOTE_KILL_PHRASE_HASH = "remote_kill_phrase_hash"` — 정확. `ZchatPreferences.kt:959` |
| `KEY_DESTROY_PIN = "destroy_pin"` | 정확. `ZchatPreferences.kt:957` |
| 주석 "SHA-256 hash, not plaintext" (line 959) | ⚠️ **outdated 주석**. 실제로는 SecureHash (PBKDF2). 코드 동작은 정확하지만 주석이 옛 상태. |

---

## §F. convID 동기화 영역 (정정됨)

### F.1 prefs key 정확한 형식 (정정됨)

| 이전 표현 (claims-to-verify C41) | 정확 |
|---|---|
| `peer_convid_<address>` / `conv_<convId>` | ❌ **부정확**. 실제는 콜론 구분자. |
| **정확한 형식** | `"peer:<address>"` → convId, `"conv:<id>"` → peerAddress. prefs file: `"zchat_conv_mapping"` |

**Code references:**
- `ZchatPreferences.kt:927` — `private const val CONV_MAPPING_PREFS_NAME = "zchat_conv_mapping"`
- `ZchatPreferences.kt:1240-1257` — `getOrCreateConversationId`:
  ```kotlin
  val existing = convMappingPrefs.getString("peer:$peerAddress", null)
  ...
  editor.putString("peer:$peerAddress", newId).putString("conv:$newId", peerAddress)
  ```
- `ZchatPreferences.kt:1260-1275` — `getPeerByConversationId`: `convMappingPrefs.getString("conv:$convId", null)`

### F.2 한 peer 가 여러 convId 를 가질 수 있음 (새 발견 — 의도된 design)

**Code reference:** `ZchatPreferences.kt:1287-1292` 주석:
```kotlin
// Write ONLY the conv→peer direction. A peer can have multiple convIds
// (one generated locally for sending, one received from the remote device).
// The peer→convId direction is managed exclusively by setConversationId()
// and getOrCreateConversationId() for OUR outgoing convId.
// NEVER delete old conv:X entries here — they may belong to the remote side.
```

→ 내 §1.1 의 "convID 동기화는 first INIT 의 한 방향 통보" 는 *불완전*. 실제로는:
- 내 송신 convId: `getOrCreateConversationId` 로 생성 + bidirectional 작성
- 받은 INIT 의 convId: `setConversationMapping` 으로 conv→peer 만 작성 (peer→convId 는 기존 없을 때만)
- **즉 한 peer 가 *내가 만든 convId* + *peer 가 만든 convId* 두 개 가질 수 있음**

이건 정상 동작이고, 코드가 의도적으로 처리.

### F.3 convId 형식 검증 (새 발견)

**Code reference:** `ZchatPreferences.kt:1279-1281`:
```kotlin
if (convId.length != 8 || !convId.all { it in 'A'..'Z' || it in '0'..'9' }) {
    Log.e("ZCHAT_CONVID", "setConversationMapping: REJECTED invalid convId format")
    return
}
```

→ 8자 + uppercase A-Z 또는 0-9 만 허용. 잘못된 format 의 incoming convId 는 silent reject. 이는 attacker 가 malformed convId 로 DOS 시도 방어.

### F.4 inconsistency auto-repair 없음 (새 발견)

**Code reference:** `ZchatPreferences.kt:1264-1273` 주석 + 코드:
- bidirectional mapping 불일치 발견 시 *log 만 남기고 자동 수정 안 함*
- "Auto-repair in a read path is destructive: it can clobber newer mappings written by setConversationId"
- 별도 `validateAndRepairConvIdMappings()` 함수가 startup 시점에 호출되어 일괄 처리

이 함수는 본 dive 에서 자세히 분석 안 했음 — **추가 verify 필요**.

---

## §G. KEX 영역 (보강)

### G.1 ECDSA verify 실패 시 처리 (재확정)

`E2EEncryption.kt:436-453` — `verify` 가 `try-catch` 로 wrap 되어 *어떤 exception 이든 false 반환*:
```kotlin
fun verify(publicKeyBase64: String, message: String, signatureBase64: String): Boolean {
    return try {
        ...
        signature.verify(signatureBytes)
    } catch (e: Exception) {
        Log.e(TAG, "Signature verification failed", e)
        false
    }
}
```

→ verify 실패 시 `parseKEXPayload` / `parseKEXAckPayload` 가 null 반환 (line 496-499, 532-535). caller `handleKEXMessage` 는 null 처리에서 *조용히 무시*. 사용자에게 KEX 실패 알림 없음 — 별도 검증 필요.

### G.2 KEX 의 sender authentication 정정 (정정됨)

§1.2 와 sequence-diagrams Scenario 2 의 "Zcash transaction 송신을 통해 implicit하게 증명" 표현이 **부정확**.

**정확한 사실:**
- KEX 의 ECDSA 서명은 `(senderAddress || A_pub)` 위에 있음 — `E2EEncryption.kt:468-470`
- **서명이 증명하는 것:** A_pub 의 priv 소유자가 이 문자열을 묶었음
- **서명이 증명하지 못하는 것:** senderAddress 가 진짜 sender 임
- Mallory 가 임의의 `senderAddress` 를 *자기* `A_pub` 와 묶어 서명할 수 있음 → 검증 통과
- 진짜 sender authentication 은 **out-of-band contact 등록 (TOFU)** 에 의존

**Spec 인용:** `ZMSG_PROTOCOL_SPEC.md` Security Properties Table 의 Known Gaps:
> "Plain-ZMSG sender authentication" — Impossible in 512-byte memos without a crypto signature — Canonical future fix: Authenticated Reply Addresses via ZIP-231 memo bundles (NU7)

zchat 본인이 정직하게 admit 하는 부분 — 본 dive 가 받아들이지 못한 부분.

### G.3 Plaintext fallback 정책 (재확정)

`ChatViewModel.kt:2495-2501`:
```kotlin
val processor = getOrCreateMessageProcessor(peerAddress, convId)
val outgoingMessage = if (processor != null) {
    processor.encryptOutgoing(message)
    // throws on failure — caught by the outer try/catch, shows error to user
} else {
    message // No E2E for this peer — send plaintext (expected)
}
```

**핵심:**
- processor null (KEX 미완료) → **plaintext 송신 (silent)**
- processor 있음 + encryption 실패 → **abort (try/catch 가 catch, 사용자에게 error)**
- 즉 "encryption 실패 시 silent plaintext fallback 없음" 은 *정확*, 그러나 *KEX 미완료 시* 는 plaintext 송신이 **silent**

`getOrCreateMessageProcessor` null 반환 조건 (`ChatViewModel.kt:1634-1638`):
1. `getE2ESharedKey(peerAddress) == null` (KEX 미완료)
2. `!zchatPreferences.isE2EEnabled(peerAddress)` (E2E 비활성화)
3. `getE2EOurPublicKey(peerAddress) == null`
4. `getE2EPeerPublicKey(peerAddress) == null`

---

## §H. ZFILE 영역 (재확정)

### H.1 ZFILE 은 ratchet wrap 됨 (재확정 — ZTL 과 차이점)

`ChatViewModel.kt:816-817`:
```kotlin
displayMessage = if (ZFILEMessage.isFileMessage(decryptedContent)) {
    val fileMsg = ZFILEMessage.parse(decryptedContent)
    ...
}
```

→ `decryptedContent` 는 **ratchet decrypt *후*** 의 plaintext. 즉 ZFILE 메시지는:
- 송신: plaintext "ZFILE|..." → ratchet encrypt → "E2E1:..." → ZMSG envelope → 송신
- 수신: ZMSG parse → ratchet decrypt → "ZFILE|..." 검출 → ZFILEMessage 객체

**ZTL 과 차이:**
- ZTL: rawMemo=true, ratchet 우회, plaintext 그대로 onchain
- ZFILE: 일반 메시지처럼 ratchet 거침, ciphertext on-chain

**ZFILE 보안 properties:**
- 메시지 자체는 ratchet 으로 보호 (URL/hash/wrappedKey/blurhash 가 wrap 됨)
- 그러나 *파일 본체*는 NIP-96/Blossom server 에 *long-term ECDH shared secret* 으로 wrap 된 key 로 암호화 — *ratchet message key 가 아님*
- 즉 파일 자체는 ratchet 의 forward-secrecy-비슷한-속성 보호 못 받음 (어차피 ratchet 자체가 진짜 FS 가 아니므로 이건 academic)

---

## §I. 새 발견 — Misc

### I.1 Diversified address 처리: `findConversationPartnerByHash` single-partner heuristic 제거됨

**Code reference:** `AddressCacheImpl.kt:229-233` 주석:
```kotlin
// REMOVED: Single-partner heuristic was causing misrouting.
// If we don't have a direct hash match, we MUST NOT guess.
// The message will go to a new conversation, which is better than misrouting.
```

→ 이전 버전에 있던 "단일 conversation partner 라면 무조건 해당 partner 로 매핑" heuristic 이 misrouting 위험으로 *제거됨*. claude.md 어느 버전에서 제거됐는지는 추가 검증 필요.

### I.2 messageProcessors 캐시 무효화 시점

**Grep results:**
- `ChatViewModel.kt:1733, 1782` — KEX/KEXACK 수신 시 같은 prefix 의 processor 제거 (`messageProcessors.keys.removeAll { it.startsWith(senderAddress) }`)
- `ChatViewModel.kt:2114, 2129, 2139` — 다른 처리 (정확한 trigger 추가 검증 필요)

→ 새 KEX 가 도착하면 기존 ratchet processor 가 무효화됨 = root 가 새로 도출됨. 이게 re-KEX 시 root rotation 메커니즘.

### I.3 outdated 주석들 (코드와 spec 불일치 흔적)

verify 중 발견한 주석들 중 *outdated* 가능성 있는 것들:
- `ZchatPreferences.kt:959`: `// SHA-256 hash, not plaintext` — 실제는 PBKDF2 (SecureHash)
- `ZMSGGroupProtocol.kt` 의 일부 createGroupInviteMessage overload `// TODO: Add per-recipient encryption using their public key` — overload 2 (plaintext key) 는 KEX 미완료 fallback 용으로 의도적 잔존
- `E2ERatchet.kt:13` — "Stage B of the 2026-04-12 deterministic-root design" — spec 파일은 본 dive scope 외

---

## §J. 추가 검증 필요 (미해결 항목)

본 corrections-log 작성 후에도 코드 verify 안 한 항목:

1. **`validateAndRepairConvIdMappings()`** 함수 (`ZchatPreferences.kt`) — startup 시 convId mapping 일관성 복구. 정확한 로직 검증 필요.
2. **`parseMemo` 의 ADDR 분기 존재 여부** — `parseMemo` 가 isADDRMessage 검사하는지 확인. 안 한다면 incoming ADDR 메시지가 처리되지 않음 = ADDR migration 도 *수신측*에서 미구현.
3. **`docs/superpowers/specs/2026-04-12-e2e-ratchet-deterministic-design.md`** — ratchet design rationale 의 정식 spec. 본 dive 의 결론과 명시적 비교 필요.
4. **`isKEXMessage` vs `isKEXAckMessage` vs `isADDRMessage` parsing priority** — `parseMemo` 의 정확한 분기 순서가 spec 9-step 과 일치하는지 line-by-line 검증.
5. **Group ECIES wrap 의 정확한 wire format** — `encryptGroupKeyForMember` (E2EEncryption.kt:749) 의 nonce/IV 구조 검증.
6. **`unlockedMessages` rescan 시 자동 재구성 timing** — incoming ZUNLOCK tx 처리 순서가 ZTL tx 처리보다 *반드시 나중*일 보장이 있는지.
7. **`messageProcessors` 캐시 무효화 의 정확한 trigger 들** (line 2114, 2129, 2139) — KEX 외 무엇이 무효화하는지.

각 항목은 별도 verify task 로 다뤄야 함. 본 dive scope 에서는 "추가 검증 필요" 로만 표시.

---

## §K. 이전 dive 표현 정정 매트릭스

| 파일 | 위치 | 기존 표현 | 정정 |
|---|---|---|---|
| `subsystems/03-double-ratchet.md` | Purpose | "per-message key 도출해 forward secrecy 제공" | "stateless deterministic key schedule. transport-level integrity (replay/AAD/nonce 보호) 만 제공. 전통적 forward secrecy 부재." |
| `subsystems/03-double-ratchet.md` | Notes "Forward secrecy는 진짜다" | (전체 단락) | **삭제 후 §A.3 표 인용**으로 대체 |
| `subsystems/03-double-ratchet.md` | Notes "Signal Double Ratchet 아님" | "symmetric ratchet 변종" | "stateless schedule — symmetric *ratchet* 자체가 아님. chain_key 의 advance + wipe 가 없음" |
| `subsystems/03-double-ratchet.md` | Q5 answer (BIP-39 호환) | "deterministic — 새 디바이스에서 복원 가능" | "deterministic, 새 디바이스에서 root 재도출 가능. **단 BIP-39 복원이 stateless 선택의 진짜 이유는 아님 (rescan idempotency 가 진짜 이유)**" |
| `subsystems/01-zmsg-protocol.md` | Q1 answer (convID 동기화) | `peer_convid_<address>` / `conv_<convId>` | `"peer:<addr>"` / `"conv:<id>"`, prefs file `"zchat_conv_mapping"` |
| `subsystems/01-zmsg-protocol.md` | Q1 answer (한 방향 통보) | "한방향 통보 모델" | "내 송신 convId 는 bidirectional 작성. 받은 INIT 의 convId 는 conv→peer 만 작성. **한 peer 가 여러 convId 가질 수 있음 (의도된 design)**" |
| `subsystems/02-kex-e2e-encryption.md` | Walkthrough step 5 (Zcash spending proof) | "Alice 의 주소를 정말 통제하는 사람만이..." | "ECDSA 서명은 *A_pub 의 priv 소유* 만 증명. senderAddress 진위는 별도 trust anchor (out-of-band contact 등록) 필요" |
| `subsystems/02-kex-e2e-encryption.md` | Notes (Quantum Shield) | (그대로 정확) | — |
| `subsystems/04-group-messaging.md` | Notes "AAD 없음" | (정확) | — (재확정만) |
| `subsystems/04-group-messaging.md` | Notes "GY caller 없음" | (정확) | — (재확정만) |
| `subsystems/06-send-receive-flow.md` | Notes (plaintext fallback) | "KEX 미완료 시 plaintext 송신 허용" | (정확) — 추가 명시: "**silent fallback** — 사용자 알림 없음. processor null 조건 4가지: KEX 미완료, E2E disabled, 자기 pubkey 없음, peer pubkey 없음" |
| `subsystems/07-contact-book-address-cache.md` | Identity Regen walkthrough | "수동으로 ADDR broadcast (현재 자동화 X)" | "**완전 미구현 (TODO).** `sendAddressChangeNotifications` 가 `Log.d` 만 호출. `createV4ADDRMessage` caller 0건. ADDR 메시지 수신측 처리도 별도 검증 필요." |
| `subsystems/07-contact-book-address-cache.md` | Remote Kill | "SHA-256 hash 또는 PBKDF2" | "**SecureHash 통합 layer.** 신규는 PBKDF2 600k, 검증 시 stored format detect 로 legacy SHA-256 fallback. SecureHash.kt:43-86" |
| `README.md` | §0.5 Key findings #4 | "symmetric ratchet 으로 FS 있음, PCS 없음" | "**FS 사실상 부재.** chain_key stateless 재도출 — rootKey 메모리 / prefs 입력값 어느 쪽도 leak 시 모든 메시지 복호화. transport-level integrity (replay/AAD/nonce) 만 제공." |
| `README.md` | §0.5 Key findings (Identity) | (해당 없음, 새 추가) | "**ADDR migration 완전 미구현.** ChangeIdentityVM.sendAddressChangeNotifications 가 TODO. createV4ADDRMessage caller 0건." |
| `README.md` | §0.5 Key findings (ZTL) | (해당 없음, 새 추가) | "**Time-lock 메시지 (ZTL/ZUNLOCK) 모두 plaintext on-chain.** `rawMemo = true`, ratchet wrap 없음. ChatMessage.text 에 plaintext 저장, UI getter 가 가림. viewing key 보유자가 zchat 앱 우회로 복호화 가능." |
| `category-A-extraction.md` | §2.3 lift-and-use | "crypto/ratchet/ 패키지 전체 — symmetric ratchet 구현" | "crypto/ratchet/ — *transport integrity* 만 lift. FS 필요시 stateful Signal-style 재작성 필요" |
| `category-A-extraction.md` | §2.4 D-? | (해당 없음, D-6 추가) | "**D-6 진짜 Forward Secrecy.** stateful ratchet + chain_key advance/wipe + rescan replay + skipped key cache. BIP-39 복원은 순차 replay 로 양립." |
| `sequence-diagrams.md` | Scenario 3 (Ratchet root) | (정확하지만 caveat 없음) | **경고 박스 추가**: "⚠️ chain_key 어디에도 저장 안 됨. rootKey + counter 만으로 어떤 시점 메시지든 즉시 복호화 — FS 보장 없음" |
| `sequence-diagrams.md` | Scenario 11 (Time-lock) | 정상 unlock flow | **viewer 우회 시나리오 추가** + caveat: "⚠️ plaintext on-chain, viewing key 우회 가능, app UI lock only" |
| `sequence-diagrams.md` | Scenario 15 (Identity Regen) | "수동 ADDR broadcast (가설적 UI)" | **명확히**: "ADDR broadcast 미구현. ChangeIdentityVM 이 TODO 처리 + Log.d 만" |
| `_claims-to-verify.md` | C41 | `peer_convid_<address>` / `conv_<convId>` | `"peer:<addr>"` / `"conv:<id>"`, prefs `"zchat_conv_mapping"` |
| `_claims-to-verify.md` | C54 (AAD) | `[ ]` | `[✗] 코드에 없음 (재확정)` |
| `_claims-to-verify.md` | C66 (Remote Kill plaintext) | `[ ]` | `[✓] SHA-256 hash 가 아니라 SecureHash (PBKDF2 600k); memo 자체는 plaintext (claude.md "Known Technical Debt")` |
| `_claims-to-verify.md` | C81 (Forward secrecy in pre-ratchet 부재; ratchet 으로 추가됨) | `[ ]` | `[✗] 부정확. ratchet 으로도 FS 추가 안 됨. transport integrity 만 제공.` |
| `_claims-to-verify.md` | C82 (PCS 부재, deterministic root, accepted ceiling) | `[ ]` | `[~] PCS 뿐 아니라 FS 도 부재. deterministic root 의 진짜 동기는 BIP-39 가 아니라 rescan idempotency.` |
| `_claims-to-verify.md` | 새 claim C195~C210 | — | (아래 §L 참조) |

---

## §L. 새 추가 claims (코드 verify 완료)

| ID | Claim | Status | Evidence |
|---|---|---|---|
| C195 | rootKey 는 prefs 에 저장 안 됨, messageProcessors 메모리 캐시만 | ✓ | ChatViewModel.kt:182, EncryptedPrefsRatchetStateStore.kt:43-49 |
| C196 | chain_key 어디에도 저장 안 됨, 매번 root 에서 stateless walk | ✓ | E2ERatchet.kt:133-153 |
| C197 | isLower = Base64 string compare (compressed bytes 아님) | ✓ | ChatViewModel.kt:1639 |
| C198 | KEX/KEXACK txid empty fallback 존재 | ✓ | ChatViewModel.kt:1644-1647 |
| C199 | tryDecryptMessage fail-open (🔒 / 🔐 emoji placeholder) | ✓ | ChatViewModel.kt:1675-1687 |
| C200 | sessionSeenA2B/B2A 메모리만, prefs 빈 array | ✓ | E2ERatchet.kt:33-39, EncryptedPrefsRatchetStateStore.kt:43-49 |
| C201 | ZTL/ZUNLOCK 모두 rawMemo=true, plaintext on-chain | ✓ | ChatViewModel.kt:3250-3428, ZMSGSpecialMessages.kt:170-247 |
| C202 | ChatMessage.text 에 ZTL plaintext 저장, UI getter 가 lock emoji 로 가림 | ✓ | ChatViewModel.kt:1037, ChatMessage.kt:133-138 |
| C203 | unlockedMessages MutableStateFlow in-memory only | ✓ | ChatViewModel.kt:166, 711-715, 3422 |
| C204 | verifyConditionalAnswer = SHA-256(lowercase().trim()) truncated 8B | ✓ | ZMSGSpecialMessages.kt:382-385 |
| C205 | ZUNLOCK answer 도 plaintext on-chain | ✓ | ChatViewModel.kt:3409, ZMSGSpecialMessages.kt:243-246 |
| C206 | GROUP_MSG AAD 누락 (cipher.updateAAD 호출 없음) | ✓ | ZMSGGroupProtocol.kt:402-419, Grep updateAAD 결과 |
| C207 | createGroupKeyMessage (GY) caller 0건 — dead function | ✓ | Grep 결과 |
| C208 | leaveGroup 자동 키 회전 없음, 떠난 멤버 epoch 0 key 보유 | ✓ | GroupViewModel.kt:777-840 |
| C209 | sendAddressChangeNotifications TODO, createV4ADDRMessage caller 0건 | ✓ | ChangeIdentityVM.kt:213-232, Grep 결과 |
| C210 | parseADDRMessage caller 도 production code 에 없음 (test 만) | ✓ | Grep 결과 |
| C211 | SecureHash (PBKDF2 600k) 가 destroy_pin + remote_kill_phrase_hash 둘 다에 사용 | ✓ | ZchatPreferences.kt:1133-1173, SecureHash.kt:22-26 |
| C212 | convId prefs key 형식 = `"peer:<addr>"` / `"conv:<id>"` (콜론) | ✓ | ZchatPreferences.kt:1242, 1250 |
| C213 | 한 peer 가 여러 convId 가질 수 있음 (의도된 design) | ✓ | ZchatPreferences.kt:1287-1292 주석 |
| C214 | convId format 검증: 8자 + A-Z 0-9 만, 잘못된 format silent reject | ✓ | ZchatPreferences.kt:1279-1281 |
| C215 | ZFILE 메시지는 ratchet wrap 됨 (ZTL 과 차이) | ✓ | ChatViewModel.kt:816-817 |
| C216 | findConversationPartnerByHash 의 single-partner heuristic 제거됨 | ✓ | AddressCacheImpl.kt:229-233 주석 |
| C217 | KEX 수신 시 같은 peer prefix 의 messageProcessors 무효화 (re-KEX root rotation 메커니즘) | ✓ | ChatViewModel.kt:1733, 1782 |
| C218 | ECDSA verify 실패 시 silent — log 만 찍고 사용자 알림 없음 | ✓ | E2EEncryption.kt:436-453, parseKEXPayload null 반환 |

---

## §M. 변천 이력 (Honest self-critique)

이전 답변들 중 *틀린* 또는 *부정확한* claim 의 시점별 정리:

1. **§1.3 walkthrough 초기 작성**: "Forward secrecy는 진짜다 — message key 매번 새로 도출 + 사용 후 폐기" → **❌ chain_key 가 wipe 안 됨을 코드로 확인 안 함.**
2. **ratchet 비유 ("쓰면 사라지는 일회용 비밀번호 책")**: **❌ 잘못된 비유.** zchat 은 책이 사라지지 않음 (deterministic re-derivation).
3. **사용자 1차 질문 답변**: "rootKey 영구 보관" → ⚠️ 결론 정확하지만 *위치 표현 부정확* (rootKey 자체가 prefs 에 있는 게 아니라 입력값이 있음).
4. **사용자 2차 질문 답변**: "BIP-39 복원 vs FS 충돌" → **❌ False dichotomy.** Signal/Megolm 으로도 BIP-39 복원 가능.
5. **Time-lock 첫 답변**: "ZTL 은 plaintext" → ✓ 결론은 정확했지만 **ChatViewModel 의 sendXxxMessage 함수들을 코드로 verify 하지 않은 상태에서 추론으로 답함**. 운 좋게 코드와 일치.
6. **Identity Regen 분석**: "수동 ADDR broadcast 가능 (가설적 UI)" → **❌ 코드 확인 결과 완전 미구현 (TODO).**

**근본 원인:**
1. claude.md + spec 문서 라벨 ("Double Ratchet", "FS") 을 *코드 verify 없이* 받아들임
2. ChatViewModel.kt 3736 lines 의 일부만 본 채로 caller 동작을 추론
3. spec/code mismatch 가능성을 처음부터 의심 안 함

**예방책 (향후 dive 방법론):**
- 모든 cryptographic claim 은 *함수 정의 + caller 양쪽* 을 코드로 verify
- spec 라벨과 코드 동작이 다를 가능성을 *default* 로 가정
- 매 claim 마다 file:line 인용 + Grep 결과 첨부
- "추가 검증 필요" 항목을 명시적으로 분리

---

## §N. 이 corrections-log 이후 작업 (이 PR 의 범위)

본 corrections-log 가 권위 있는 참조이며, 다음 파일들이 *본 파일을 인용*하는 형태로 정정됨:

- `subsystems/01-zmsg-protocol.md` — §F 인용 (convID)
- `subsystems/02-kex-e2e-encryption.md` — §G.2 인용 (sender auth)
- `subsystems/03-double-ratchet.md` — §A 전체 인용 (가장 큰 정정)
- `subsystems/04-group-messaging.md` — §C 인용 (재확정만)
- `subsystems/06-send-receive-flow.md` — §A.6, §G.3 인용
- `subsystems/07-contact-book-address-cache.md` — §D, §E 인용
- `README.md` — §A, §B, §D 인용 + 새 key findings
- `category-A-extraction.md` — §A 전체 + D-6 추가
- `sequence-diagrams.md` — Scenario 3 + 11 + 15 caveat
- `_claims-to-verify.md` — §K, §L 표 반영

§J 의 추가 검증 필요 항목들은 *후속 dive* 에서 처리.
