# §1.3 Double Ratchet (forward secrecy)

> ⚠️ **2026-05-16 대규모 정정.** 이 파일의 초기 분석에 *forward secrecy* 표현 + *BIP-39 trade-off* 설명이 부정확했음. 정확한 코드 기반 결론은 [`../corrections-log.md` §A](../corrections-log.md) 참조. 본문은 정정된 표현으로 갱신됨.

## 목적 (Purpose)

본 서브시스템은 §1.2 의 정적 ECDH derived key 대신 **per-message key 를 도출하는 stateless deterministic key schedule** 이다. 이름은 "ratchet" 이지만 코드 상으로는 *진정한 ratchet 의 핵심 성질 (사용 후 비가역적 상태 advance)* 이 **없다.** `chain_key` 가 어디에도 저장되지 않고 매 encrypt/decrypt 마다 `rootKey` 에서 stateless 로 재도출되므로, rootKey 와 counter 만 알면 어떤 시점의 message key 든 즉시 도출 가능하다.

**이 layer 가 실제로 제공하는 보호 (코드 verify 됨):**
1. Replay 보호 (counter window, `MAX_SKIP = 1000`) — `E2ERatchet.kt:78-89, 221`
2. Cross-conversation routing 방지 (AAD 에 convId 포함) — `E2ERatchet.kt:167-171`
3. AES-GCM nonce 충돌 방지 (counter-based nonce + `.commit()` synchronous) — `EncryptedPrefsRatchetStateStore.kt:30-36`

**이 layer 가 제공하지 *않는* 보호:**
- 전통적 forward secrecy (사용 후 비가역적 폐기) — chain_key advance + wipe 가 없음. rootKey 또는 그 재도출 입력값 (priv key + peer pub + KEX/KEXACK txid) leak 시 모든 메시지 복호화 가능.
- Post-compromise security — DH ratchet 단계 없음.

이 디자인의 진짜 동기는 **BIP-39 seed 복원 호환성이 아니라 rescan idempotency + 구현 단순성** ([§A.8](../corrections-log.md) 참조).

## 파일과 함수 (Files & functions)

`ui-lib/src/main/java/co/electriccoin/zcash/ui/screen/chat/crypto/ratchet/` 디렉토리.

- `E2ERatchet.kt:25` — `class E2ERatchet(rootKey, convId, isLower, store)` — symmetric ratchet 코어
  - `:46` — `suspend fun encrypt(plaintext): Ciphertext` — 송신; `myDirection` chain의 다음 counter 가져와서 message key 도출 + AES-GCM
  - `:70` — `suspend fun decrypt(ciphertext): ByteArray` — 수신; direction + counter로 message key 재계산
  - `:133` — `private fun deriveMessageKey(direction, counter)` — chain_key_0부터 N번 HMAC step → `HMAC(chain_key_N, 0x01)` = message_key_N
  - `:144` — `private fun deriveChainKey0(direction)` — root + info = "ZCHAT_CHAIN_A2B_V1" / "ZCHAT_CHAIN_B2A_V1" → HKDF로 chain_0
  - `:155` — `private fun counterNonce(direction, counter)` — 12B nonce = `[direction(1B)][zero(3B)][counter big-endian(8B)]`
  - `:167` — `private fun aadFor(direction, counter, convId)` — AAD = `[direction(1B)][counter(8B)][convId UTF-8]`
  - `:247` — `companion object.deriveRatchetRoot(ecdhSharedSecret, psk, kexTxid, kexAckTxid): ByteArray` — root 도출 (정적 함수)
  - 상수:
    - `:221` — `MAX_SKIP = 1000L` — receiver가 미래로 walk 가능한 최대 counter 거리 (DoS 방어)
    - `:227` — `MAX_SEND_COUNTER = 1_000_000L` — sender 측 cap (re-KEX 필요 임계)
    - `:208-209` — `CHAIN_INFO_A2B = "ZCHAT_CHAIN_A2B_V1"`, `CHAIN_INFO_B2A = "ZCHAT_CHAIN_B2A_V1"`
    - `:210-211` — `MESSAGE_KEY_BYTE = 0x01`, `CHAIN_STEP_BYTE = 0x02`
    - `:212` — `DIRECTION_A2B = 0x00` (0x01 = B2A)
    - `:213` — `ROOT_SALT = "ZCHAT_RATCHET_ROOT_V1"`
- `E2EMessageProcessor.kt:18` — `class E2EMessageProcessor(rootKey, convId, isLower, store)` — high-level wrapper
  - `:30` — `suspend fun encryptOutgoing(plaintext): String` — `ratchet.encrypt(...)` 후 `CiphertextWireFormat.serialize` → `"E2E1:..."` 문자열
  - `:44` — `suspend fun decryptIncoming(wireContent): String` — `E2E1:` prefix 없으면 그대로 통과 (plaintext fallback), 있으면 parse + decrypt
- `Ciphertext.kt:10` — `data class Ciphertext(direction: Byte, counter: Long, bytes: ByteArray)` — ratcheted 메시지의 in-memory 표현
- `CiphertextWireFormat.kt:21` — `object CiphertextWireFormat`
  - `:23` — `PREFIX = "E2E1:"`
  - `:27` — `serialize(ct): String` — `"E2E1:<dir_hex2>:<counter_hex16>:<base64>"`
  - `:34` — `parse(wire): Ciphertext?` — 검증: dir ∈ {0x00, 0x01}, counter ≥ 0, base64 valid
  - `:62` — `isRatcheted(wire): Boolean` — prefix 검사
- `RatchetStateStore.kt:12` — `data class RatchetConversationState(convId, nextCounterA2B, nextCounterB2A, seenCountersA2B, seenCountersB2A)` — persisted state schema
- `RatchetStateStore.kt:26` — `interface RatchetStateStore`
  - `load(convId): RatchetConversationState?`
  - `save(state)`
  - `mutexFor(convId): Mutex` — per-conversation lock
- `EncryptedPrefsRatchetStateStore.kt:15` — production impl; `SharedPreferences`(EncryptedSharedPreferences via Tink) 기반 JSON 직렬화
  - `:35` — `save` uses `.commit()` **synchronous** — `.apply()` async는 nonce reuse 위험 (주석에 catastrophic 명시)
- `InMemoryRatchetStateStore.kt:11` — test impl; HashMap 기반
- `RatchetExceptions.kt:10` — `class ReplayDetectedException(direction, counter)` — 같은 (dir, counter) 두 번 수신
- `RatchetExceptions.kt:20` — `class CounterOutOfRangeException(direction, counter, maxAllowed)` — counter가 MAX_SKIP 초과
- `E2EMessageProcessor.kt:57` — `class MalformedCiphertextException(message)` — wire format parse 실패

## 연결 (Wiring)

- **Inputs:**
  - Root key (32B) — `E2ERatchet.deriveRatchetRoot(ecdhSecret, psk, kexTxid, kexAckTxid)` (§1.2 KEX 완료 후 ChatViewModel이 호출)
  - `convId` — §1.1에서 정해진 8자
  - `isLower: Boolean` — 양쪽 pubkey의 compressed secp256r1 lex 비교 결과 — ChatViewModel이 계산해서 주입
  - Persistence: `RatchetStateStore` (EncryptedSharedPreferences 또는 InMemory)
  - Plaintext bytes (송신) 또는 `Ciphertext` (수신)
- **Outputs:**
  - `"E2E1:<dir>:<counter>:<base64-ct>"` 문자열 (송신; ZMSG payload로 들어감, §1.1)
  - 복호화된 plaintext UTF-8 String (수신)
  - 예외: `ReplayDetectedException` / `CounterOutOfRangeException` / `MalformedCiphertextException` / `AEADBadTagException`
- **Dependencies (internal):**
  - [§1.2 KEX + E2E](./02-kex-e2e-encryption.md) — `HKDF` object를 import, root 도출은 ECDH shared secret을 IKM으로 사용
  - [§1.1 ZMSG 프로토콜](./01-zmsg-protocol.md) — `E2E1:` ciphertext가 ZMSG 메시지 payload 자리에 들어감
  - [§1.6 송수신 흐름](./06-send-receive-flow.md) — `ChatViewModel`이 `E2EMessageProcessor` 인스턴스를 peer별로 캐싱하고 호출
  - [§1.7 컨택트 + Identity](./07-contact-book-address-cache.md) — `EncryptedPrefsRatchetStateStore`가 사용하는 prefs 파일 (별도 namespace `ratchet_state_<convId>`)
- **Dependencies (external):**
  - `javax.crypto.Cipher` (`AES/GCM/NoPadding`), `Mac` (HMAC-SHA256), `SecretKeySpec`, `GCMParameterSpec`
  - `kotlinx.coroutines.sync.Mutex` — per-convId 동기화
  - `org.json.JSONObject` / `JSONArray` — state persistence
  - `android.content.SharedPreferences` — production storage
  - `java.security.MessageDigest` (SHA-256) — root 도출의 `sha256(kex_txid || kexack_txid)`

## 라이브러리 (Libraries)

| Package | Version | Used for |
|---------|---------|----------|
| `kotlinx-coroutines-core` | (Kotlin 2.1.10 ecosystem) | `suspend fun`, `Mutex.withLock`, per-conversation lock |
| JDK `javax.crypto` | Android API 27+ | AES-256-GCM, HMAC-SHA256 |
| `org.json` (Android API) | API 27+ | JSON serialization in EncryptedPrefsRatchetStateStore |
| Kotlin stdlib | 2.1.10 | data class, ByteArray ops |

## 워크스루 — happy path

### 0. Root 도출 (KEX 완료 직후 한 번만)

**`E2ERatchet.deriveRatchetRoot(ecdhSecret, psk, kexTxid, kexAckTxid)` (line 247):**

```kotlin
fun deriveRatchetRoot(
    ecdhSharedSecret: ByteArray,
    psk: ByteArray?,
    kexTxid: ByteArray,
    kexAckTxid: ByteArray,
): ByteArray {
    val ikm = if (psk != null) ecdhSharedSecret + psk else ecdhSharedSecret
    val kexContext = sha256(kexTxid + kexAckTxid)
    return HKDF.deriveKey(
        ikm = ikm,
        salt = ROOT_SALT,   // "ZCHAT_RATCHET_ROOT_V1"
        info = kexContext,
        length = 32,
    )
}
```

핵심 두 가지:
1. **PSK는 IKM에 concat** (V2 ECDH derive와 같은 패턴, §1.2)
2. **KEX/KEXACK txid가 info로 들어감** — 같은 두 사람이 두 번 KEX 시작하면 다른 root. Re-KEX로 root rotation 가능. *Blockchain-derivable info*이기 때문에 양쪽이 동일한 root 도출 가능 (BIP-39 + recipient viewing key + KEX/KEXACK txid만으로 복원 가능).

ChatViewModel에서:
1. `ZchatPreferences`에서 `(ourPriv, peerPub)` 로드
2. `E2EEncryption.deriveSharedSecret(ourPriv, peerPub, V2, psk = QuantumShield.psk)` 호출하지 *않고*, 대신 raw ECDH만 추출하여 `deriveRatchetRoot`에 입력. (자세한 caller는 §1.6에서 확인)
3. `E2EMessageProcessor(rootKey, convId, isLower = (ourPubCompressed < peerPubCompressed), store)` 인스턴스 생성

### A. 송신 — `encryptOutgoing(plaintext)`

**1. `E2EMessageProcessor.encryptOutgoing("Hi")` (E2EMessageProcessor.kt:30)**

```kotlin
suspend fun encryptOutgoing(plaintext: String): String {
    val ct = ratchet.encrypt(plaintext.toByteArray(Charsets.UTF_8))
    return CiphertextWireFormat.serialize(ct)
}
```

**2. `E2ERatchet.encrypt(bytes)` (E2ERatchet.kt:46)**

```kotlin
suspend fun encrypt(plaintext: ByteArray): Ciphertext {
    val mutex = store.mutexFor(convId)
    return mutex.withLock {
        val state = loadOrInit()
        val counter = counterFor(state, myDirection)
        require(counter < MAX_SEND_COUNTER) { ... }
        val messageKey = deriveMessageKey(myDirection, counter)
        val nonce = counterNonce(myDirection, counter)
        val aad = aadFor(myDirection, counter, convId)
        val cipherBytes = aesGcmEncrypt(messageKey, nonce, aad, plaintext)
        store.save(advanceSendCounter(state))
        Ciphertext(myDirection, counter, cipherBytes)
    }
}
```

순서:
- per-convId Mutex 획득 — 동시 송신 race 방지
- store에서 현재 state 로드 (`nextCounterA2B`, `nextCounterB2A`, seen sets)
- 내 방향의 counter 추출 (예: A2B면 `state.nextCounterA2B`)
- counter ≥ 1_000_000이면 reject (re-KEX 필요)
- `deriveMessageKey(myDirection, counter)` — chain_key_0부터 N step HMAC walk → `HMAC(chain_key_N, 0x01)` = message_key_N
- `counterNonce(myDirection, counter)` — `[direction][zero×3][counter big-endian u64]` 12B
- `aadFor(myDirection, counter, convId)` — `[direction][counter big-endian u64][convId UTF-8]`
- AES-256-GCM 암호화 (AAD bound)
- counter 증가시켜 store에 저장 — **`.commit()` synchronous** (line 35의 critical comment)
- `Ciphertext(direction, counter, bytes)` 반환

**3. `deriveMessageKey` 상세 (E2ERatchet.kt:133)**

```kotlin
private fun deriveMessageKey(direction: Byte, counter: Long): ByteArray {
    var chainKey = deriveChainKey0(direction)
    var step = 0L
    while (step < counter) {
        chainKey = hmacSha256(chainKey, byteArrayOf(0x02))  // CHAIN_STEP_BYTE
        step++
    }
    return hmacSha256(chainKey, byteArrayOf(0x01))  // MESSAGE_KEY_BYTE
}
```

`chain_key_0 = HKDF(root, salt=null, info="ZCHAT_CHAIN_A2B_V1" or "B2A", 32)`. 그 다음:
- `chain_key_{N+1} = HMAC(chain_key_N, 0x02)`
- `message_key_N = HMAC(chain_key_N, 0x01)`

이건 **stateless** chain walk — counter만 알면 어디서나 message key 재계산 가능. 그래서:
- O(N) HMAC cost per encrypt — counter가 1M 가까이 되면 매 송신마다 1M HMAC, 성능 저하
- skipped-message-keys 캐시 *없음* — 모든 step을 매번 다시 walk (Signal과의 큰 차이)

→ **MAX_SEND_COUNTER = 1M의 근거**: 1M HMAC ≈ 안드로이드 모바일 CPU 1~5초. user-visible latency가 너무 커지기 전 re-KEX 강제.

**4. `CiphertextWireFormat.serialize(ct)` (CiphertextWireFormat.kt:27)**

```
"E2E1:<dir_hex2>:<counter_hex16>:<base64-of-bytes>"
```

예: `E2E1:00:0000000000000007:5LZ3v8a...`

→ 이 문자열이 §1.1 ZMSG 위에 payload로 wrap됨: `ZMSG|v4|ABC12345|<hash16>|E2E1:00:0000000000000007:5LZ3v8a...`

### B. 수신 — `decryptIncoming(wire)`

**5. `E2EMessageProcessor.decryptIncoming("E2E1:...")` (E2EMessageProcessor.kt:44)**

```kotlin
suspend fun decryptIncoming(wireContent: String): String {
    if (!CiphertextWireFormat.isRatcheted(wireContent)) return wireContent
    val ct = CiphertextWireFormat.parse(wireContent)
        ?: throw MalformedCiphertextException("Invalid E2E1 wire format")
    val plainBytes = ratchet.decrypt(ct)
    return String(plainBytes, Charsets.UTF_8)
}
```

prefix 없으면 plaintext fallback로 그대로 반환 (KEX 미완료 상대로부터의 메시지 등). prefix 있는데 parse 실패 → exception(절대 raw wire를 plaintext로 노출하지 않음 — `// must surface as a decrypt failure`).

**6. `E2ERatchet.decrypt(ct)` (E2ERatchet.kt:70)**

```kotlin
suspend fun decrypt(ciphertext: Ciphertext): ByteArray {
    val isOwnOutgoing = ciphertext.direction == myDirection
    val mutex = store.mutexFor(convId)
    return mutex.withLock {
        if (!isOwnOutgoing) {
            val sessionSeen = sessionSeenFor(ciphertext.direction)
            if (ciphertext.counter in sessionSeen) {
                throw ReplayDetectedException(...)
            }
            val maxSeen = sessionSeen.maxOrNull() ?: 0L
            if (ciphertext.counter > maxSeen + MAX_SKIP) {
                throw CounterOutOfRangeException(...)
            }
        }
        val messageKey = deriveMessageKey(ciphertext.direction, ciphertext.counter)
        val nonce = counterNonce(ciphertext.direction, ciphertext.counter)
        val aad = aadFor(ciphertext.direction, ciphertext.counter, convId)
        val plaintext = aesGcmDecrypt(messageKey, nonce, aad, ciphertext.bytes)
        if (!isOwnOutgoing) sessionSeenFor(ciphertext.direction).add(ciphertext.counter)
        plaintext
    }
}
```

핵심:
- **`isOwnOutgoing` 처리**: 자기 송신 메시지를 자기 디바이스 re-scan 시 다시 decrypt하는 경우 replay 검사 우회. 이게 가능한 이유 = chain이 deterministic이라 같은 counter로 같은 plaintext 도출.
- **`sessionSeen`** 은 in-memory 만 (RatchetConversationState 가 *seenCounters* 필드도 가지지만 코드 본체는 session set만 사용). restart 시 비워짐 — re-scan 호환의 핵심.
- **MAX_SKIP = 1000**: 신뢰할 수 있는 peer는 1000개 미래 counter까지 jump 허용. malicious peer가 huge counter 보내도 1000 HMAC 이상 work 강제 안 됨 (DoS bound).

### C. State persistence — EncryptedPrefsRatchetStateStore

**7. `save(state)` (EncryptedPrefsRatchetStateStore.kt:30)**

```kotlin
prefs.edit().putString(key(state.convId), toJson(state).toString()).commit()
```

`.commit()` synchronous. 주석 (line 31-34):
> "If app crashes after encrypt() advances the counter but before the state flushes to disk, the sender would re-use the same counter on restart → same GCM nonce + same key = catastrophic nonce reuse."

JSON schema (line 43):
```json
{
  "convId": "ABC12345",
  "nextA2B": 7,
  "nextB2A": 4,
  "seenA2B": [],
  "seenB2A": []
}
```

`seenA2B/B2A`는 항상 빈 배열로 직렬화됨 (코드의 `sessionSeen`은 별도 in-memory). 즉 RatchetConversationState의 `seenCountersA2B / B2A` 필드는 **현재 코드에서 사용되지 않는 dead field** — 향후 hardening을 위한 reservation으로 보임.

## 노트 / quirks / footguns

- **Signal Double Ratchet 아님 — 사실은 *ratchet 자체도 아님*.** 코드 주석에 "symmetric ratchet" 이라고 표현되어 있지만(line 12-14), 진정한 ratchet 의 핵심 성질 (chain_key 의 비가역적 advance + 이전 값 wipe) 이 *없다*. `deriveMessageKey` 가 매번 chain_key_0 부터 stateless walk — rootKey 만 있으면 어떤 counter 든 즉시 도출 가능. **MESSAGING_CRYPTO.md §1 "Receiver IVK leak 시 과거 메시지 노출 → Double Ratchet으로 방어" 표현은 부정확** — ratchet layer 가 추가로 보장하는 FS 가 없으므로 receiver IVK leak 만으로는 노트가 풀리고, *추가로* E2E priv key + peer pub + KEX/KEXACK txid 까지 leak 되면 rootKey 재도출 → 모든 메시지 평문. ([corrections-log §A.3](../corrections-log.md))
- **Forward secrecy 사실상 부재.** chain_key 가 advance + wipe 되지 않으므로 일반적 FS 정의 (사용 후 비가역적 폐기) 충족 안 함. Code references:
  - `messageProcessors: ConcurrentHashMap` (`ChatViewModel.kt:182`) 가 rootKey 보유한 `E2EMessageProcessor` 인스턴스를 *앱 수명 동안* 캐시.
  - `RatchetConversationState` 스키마 (`RatchetStateStore.kt:12-18`) 에 chain_key 필드 없음 — counter + seen sets 만.
  - prefs leak (E2E priv + peer pub + KEX txid) → rootKey 재도출 가능 (`ChatViewModel.kt:1624-1668`).
  - 즉 "ratchet 으로 FS 보장" 라벨은 정확하지 않음. ([corrections-log §A.1, A.2, A.3](../corrections-log.md))
- **`docs/superpowers/specs/2026-04-12-e2e-ratchet-deterministic-design.md`** 가 정식 설계 문서 (코드 주석 line 13). 더 자세한 보안 분석을 보려면 그 파일 참조. 본 dive scope에서는 외부 reference로만 명시.
- **`isLower` 결정 책임은 caller (ChatViewModel) 에 있다.** Ratchet 자체는 단순히 `Byte` 를 받음. `ChatViewModel.kt:1639`: `val isLower = ourPub < peerPub` — **Base64 인코딩된 X.509 SubjectPublicKeyInfo 의 Kotlin String 비교**. 코드 주석 (E2ERatchet.kt:20) 의 "compressed secp256r1 public key lexicographic" 표현은 정확하지 않음 (compressed bytes 가 아니라 Base64 string). 결과는 lex-compare 와 동일하지만 비교 대상 표현이 다름. ([corrections-log §A.4](../corrections-log.md))
- **KEX/KEXACK txid empty fallback 존재.** prefs 에 `E2EKexTxId` / `E2EKexAckTxId` 가 없으면 (claude.md 의 이전 버전과의 호환) `ByteArray(0)` 로 root 도출 (`ChatViewModel.kt:1644-1647`). 이 경우 같은 두 사람의 다른 conversation 이 같은 root 를 가질 수 있어 *re-KEX 시 root rotation 보장*이 약화됨. ([corrections-log §A.5](../corrections-log.md))
- **`tryDecryptMessage` fail-open 정책.** decrypt 실패 시 UI 에 emoji placeholder 표시 (`ChatViewModel.kt:1675-1687`): ReplayDetectedException → "🔒 Encrypted message", 기타 exception → "🔐 Encrypted message (unable to decrypt)". processor 가 null 인 경우 (KEX 미완료) 는 content 그대로 통과 = plaintext 표시. ([corrections-log §A.6](../corrections-log.md))
- **`seenCountersA2B/B2A` 필드는 persisted state schema에 있지만 사용 안 됨.** EncryptedPrefsRatchetStateStore는 빈 배열로 serialize/deserialize. 향후 더 강한 replay 보호(across-restart)를 위한 reserve로 보임. 우리 팀이 채택 시 어떻게 사용할지 검토.
- **MAX_SKIP = 1000 이지만 *session* basis.** receiver가 앱을 껐다 켜면 session set이 reset되어 maxSeen이 0L부터 다시 시작 — 즉 counter 500인 메시지를 받고 앱을 끄고 다음 session에서 counter 1500 메시지가 와도 (1500 - 0) > 1000이면 reject. 결과적으로 long-offline 후 따라잡기 어려울 수 있음 — 코드는 그러나 first re-scan 시 maxSeen이 message stream 따라 자연스럽게 갱신되므로 실용적 문제 작다.
- **Skipped key cache 없음.** Signal과 달리 zchat은 out-of-order 메시지를 받으면 매번 chain key를 처음부터 walk. 1000개의 미수신 메시지가 buffer에 있다가 reverse order로 처리되면 1000² HMAC operations 발생. 우리 팀 차별화로 lazy skipped-key cache 추가 가능.
- **Send counter advance + state save가 atomic.** Mutex로 보호. crash 시 .commit() 동기 flush로 nonce 재사용 방지 — *good*. 하지만 `commit()`은 disk I/O 동기 호출이라 **매 메시지 송신마다 fsync** 비용. battery 영향 있을 수 있음.
- **`hmacSha256` reflection 비용 없음.** Mac instance 매 호출 재생성. JIT 최적화 의존 — 핫 path라 BC provider 미리 캐싱하는 게 더 빠를 수 있음. 우리 팀 hardening 후보.
- **AAD에 convId 포함.** ciphertext가 다른 conversation으로 routing되어도 AES-GCM decryption이 fail. 좋은 defensive design. 그러나 chain key 자체가 root에서 도출되고 root는 KEX/KEXACK txid에 binding되니, *실제로* 다른 conversation의 ratchet이 같은 chain key를 만들 확률 = 0. 따라서 convId AAD는 단순 strict bound.
- **MalformedCiphertextException는 silently raw bytes를 plaintext로 노출하지 않는다.** 이건 잘 설계됨 — 사용자가 E2E1: blob을 메시지로 보는 일을 막음. UI 측에서 "복호화 실패 placeholder" 처리 필요 (§1.6에서 확인).
- **plaintext fallback은 `E2EMessageProcessor.decryptIncoming`에서 일어난다.** `CiphertextWireFormat.isRatcheted(wireContent) = false`면 그대로 통과. 즉 KEX 미완료 상대로부터의 메시지는 자동으로 plaintext 표시. 사용자가 "암호화됐는지" 시각적으로 구분할 메커니즘이 §1.6 UI에 필요 (Conversation.isE2EReady).

## 답한 open question

- **Q5** (research-plan §7): "Double Ratchet의 root key는 BIP-39 seed restore와 어떻게 호환?"
  > **Answer (정정됨):** Root 는 deterministic 으로 *재도출 가능* — `HKDF(ECDH_secret || optional_psk, salt="ZCHAT_RATCHET_ROOT_V1", info=sha256(kex_txid || kexack_txid), 32)`. rootKey 자체는 prefs 에 저장되지 *않음* — 메모리 캐시 (`messageProcessors`) 만. 새 디바이스에서 BIP-39 seed 로 복원 시 입력값 (priv/pub/txid/psk) 이 prefs 로부터 복원되거나 blockchain rescan 으로 발견되어 root 재도출 가능.
  >
  > **단, BIP-39 복원이 stateless schedule 선택의 진짜 이유가 아님.** Signal 식 stateful ratchet 으로도 BIP-39 복원 양립 가능 (blockchain 메시지 순차 replay + skipped key cache). zchat 의 진짜 동기는 **rescan idempotency + 구현 단순성** — re-scan 시 같은 메시지를 *임의 순서로, 몇 번이고* decrypt 해도 같은 plaintext 보장이 필요. stateless 가 이를 trivially 만족.
  >
  > **Multi-device 미지원의 진짜 이유:** counter state 가 prefs 에 있어 두 device 가 같은 counter 부터 송신 시도 시 GCM nonce 충돌. ([corrections-log §A.8](../corrections-log.md)) — `E2ERatchet.kt:247-261`, `EncryptedPrefsRatchetStateStore.kt:30-36`

- **Q4 (partial)** (research-plan §7): "Replay 보호"
  > **Answer:** Session-scoped seen-counter sets (`sessionSeenA2B/B2A`)가 in-memory만 — **restart 시 비워짐**. 같은 session 내에서 same (direction, counter) 재수신 시 `ReplayDetectedException`. Counter가 `maxSeen + 1000` 초과 시 `CounterOutOfRangeException` (DoS bound). 자기 outgoing 메시지에는 replay 검사 우회 — re-scan 시 자기 ciphertext 재복호화 가능하게 (deterministic chain의 활용). 즉 **session-level replay protection only** — 첫 메시지의 cross-session replay는 catch 못 함. — `E2ERatchet.kt:70-104, 221`

- **C145, C146** (claims-to-verify): "E2ERatchet의 정확한 알고리즘 / DH ratchet 존재 여부"
  > **Answer:** **Signal Double Ratchet 아님 — symmetric ratchet (KDF chain) only.** DH ratchet 없음. Root은 KEX 1회로 고정 (단, re-KEX 시 새 KEX/KEXACK txid로 root rotation 가능). Forward secrecy provided per-direction, per-counter. Post-compromise security **부재** — root leak되면 미래 모든 메시지 복호화 가능. ZMSG_PROTOCOL_SPEC.md Security Properties Table의 "Accepted ceiling: session-level FS per KEX epoch (Megolm-style)"가 정확. — `E2ERatchet.kt:12-14, 133-153`

- **C135, C136, C137** (claims-to-verify): Replay 방어 / MAX_SKIP / AEAD tag
  > **Answer:** 모두 일치 — Replay `ReplayDetectedException` (line 80), MAX_SKIP=1000 with `CounterOutOfRangeException` (line 83-87), AEAD tag tampering은 `AEADBadTagException` (JDK 표준; `decrypt` catch 안 함, caller로 throw). — `E2ERatchet.kt:70-104`

- **MESSAGING_CRYPTO.md §11 추가 의문**
  > **§11 의 "Ratchet 정확한 알고리즘"** → 확정: symmetric ratchet (DH ratchet 없음)
  > **§11 의 "MAX_SKIP 값"** → 1000 (session-scoped)
  > **§11 의 "Out-of-order 처리"** → 가능 (counter MAX_SKIP 윈도우 내) but skipped-key cache 없음 — receiver가 매번 chain을 처음부터 walk (성능 O(N))
