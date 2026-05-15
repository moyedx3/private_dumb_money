# §1.7 컨택트북 + 주소 캐시 + Identity Regeneration + Destroy PIN

> ⚠️ **2026-05-16 정정**: (a) convID prefs key 형식 `peer_convid_<addr>` / `conv_<convId>` 는 **부정확** — 실제는 `"peer:<addr>"` / `"conv:<id>"` (콜론). (b) Identity Regen 의 ADDR migration 은 "수동 broadcast 가능 (가설적 UI)" 가 아니라 **완전 미구현 (TODO)** — `ChangeIdentityVM.sendAddressChangeNotifications` 가 `Log.d` 만 호출, `createV4ADDRMessage` caller 0건. (c) Destroy PIN / Remote kill phrase 는 단순 SHA-256 hash 가 아니라 **SecureHash (PBKDF2 600k iter, OWASP 2023, legacy SHA-256 backward compat 포함)**. [`../corrections-log.md` §D, §E, §F](../corrections-log.md) 참조.

## 목적 (Purpose)

본 서브시스템은 zchat의 *local-only identity / contact 인프라*를 묶는다 — sender hash → unified address 매핑 캐시(diversified address 문제 처리), 사용자가 명시적으로 추가한 contact book(displayName + alias), DEC-016 **Identity Regeneration** ("masks" — 한 wallet 안에 여러 messaging identity), **Destroy PIN** (remote kill memo + 11단계 폭파 시퀀스). 모두 EncryptedSharedPreferences(또는 일반 SharedPreferences) 에만 저장 — **외부 서버 의존 0건**. 위 4가지 기능은 서로 prefs 파일이 다르고 독립적이지만 다 같이 "이 디바이스가 누구이며 누구를 안다"를 정의한다.

## 파일과 함수 (Files & functions)

### `ui-lib/.../screen/chat/datasource/ContactBookImpl.kt` (87 lines)

- `:15` — `class ContactBookImpl(context) : ContactBook`
- `:22-25` — `PREFS_NAME = "zchat_contact_book"`, `KEY_CONTACTS = "contacts"` — 단일 prefs 파일에 contact list JSON
- `:27` — `addContact(contact)` — 기존 같은 address 제거 후 추가 (idempotent update)
- `:35` / `:41` / `:45` / `:49` / `:53` — `removeContact`, `getContact`, `getAllContacts` (정렬 by name lowercase), `hasContact`, `updateContactName`

### `ui-lib/.../screen/chat/model/Contact.kt` (24 lines)

- `:8` — `data class Contact(address: String, name: String, addedAt: Instant)`
- `:17` — `interface ContactBook` — implementation-agnostic 인터페이스

### `ui-lib/.../screen/chat/datasource/AddressCacheImpl.kt` (241 lines)

- `:15` — `class AddressCacheImpl(context) : AddressCache`
- `:17` / `:236-238` — `PREFS_NAME = "zchat_address_cache"`, `KEY_PREFIX = "addr_"`, `KEY_CONVERSATION_PARTNERS = "conversation_partners"`
- `:25` — `memoryCache: mutableMapOf<String, String>` — hash → address in-memory cache
- `:28` — `conversationPartners: synchronizedSet(mutableSetOf<String>())` — 우리가 주도적으로 송신한 address 목록
- `:34-41` — init: SharedPreferences에서 모든 캐시 동기 로드 + `cacheLoaded` 플래그
- `:65` — `cacheAddress(hash, address)` — unvalidated path
- `:69` — `cacheAddressValidated(hash, address)` — trusted source (INIT message / contact book) path
- `:77` — `cacheAddressWithValidation(hash, address, validated)` — **collision guard**:
  - `isValidZcashAddress` 실패 시 reject
  - 기존 매핑 있고 다른 address이면 `validated = false` 인 한 거부 (line 86-91)
  - validated = true 면 overwrite 허용
- `:98` — `getAddress(hash): String?` — synchronized memory lookup
- `:113` — `getAddressWithLegacyFallback(hash, address?)` — 16-char (v4) 매핑이 없으면 12-char (legacy v3) prefix로 fallback + migrate
- `:144` — `isValidZcashAddress`: `u1*` (UA, length > 100) 또는 `zs*` (Sapling, length > 70)
- `:178` — `addConversationPartner(address)` — 송신 시 ChatViewModel이 호출. 동시에 hash 자동 캐시 + StringSet persist
- `:208` — `findConversationPartnerByHash(hash): String?` — diversified address 문제 처리:
  - direct match 우선
  - 16-char hash + partner의 legacy hash prefix match
  - **single-partner heuristic은 misrouting 위험으로 제거됨** (line 229 주석)

### `ui-lib/.../screen/chat/datasource/ZchatPreferences.kt` (1801 lines — Layer C의 prefs hub)

본 서브시스템 scope에서는 *어떤 종류의 키를 저장하는지 카테고리*만 다룬다. 자세한 surface는 코드 자체 참조.

- E2E keypair / E2E peer pub key / E2EKeyVersion (per peer) — KEX/Ratchet (§1.2 / §1.3) 의존 storage
- ConvID mapping (`"peer:<addr>"` / `"conv:<id>"`, prefs file `"zchat_conv_mapping"`) — ZMSG v4 threading (§1.1). **한 peer 가 여러 convId 가질 수 있음** (의도된 design, `ZchatPreferences.kt:1287-1292`). [정정 2026-05-16]
- KEX/KEXACK txid (`E2EKexTxId`, `E2EKexAckTxId`) per peer — Ratchet root 도출 (§1.3)
- Ratchet state (별도 prefix `ratchet_state_<convId>`) — §1.3에서 backed by EncryptedSharedPreferences
- Group state: GroupInfo / GroupMembers / GroupKey (epoch별) / GroupKeyEpoch / GroupMessageSequence / GroupDraft
- Quantum Shield: ourSecret / peerSecret / PSK per peer
- Conversation drafts (`drafts.<addr>`)
- Peer status updates (`peer_status_<addr>`)
- Pending message persistence (`pending_messages` list — restart survive 용)
- Sound / Vibration / Privacy / Mute (notification settings)
- Hidden message IDs (사용자가 숨긴 메시지)
- Last worker sync timestamp
- **Remote kill**: `remote_kill_enabled`, `remote_kill_amount`, `remote_kill_phrase_hash` (SHA-256 of phrase, not plaintext per claude.md v2.9.1 audit fix)
- **Destroy PIN**: SecureHash (PBKDF2WithHmacSHA256, 600k iter, OWASP 2023) 저장, 검증은 `verifyDestroyPin` API. legacy plain SHA-256 backward-compat. prefs key `"destroy_pin"`. plaintext `getDestroyPin` 제거됨 per claude.md v2.9.1. — `ZchatPreferences.kt:1133-1148`, `SecureHash.kt:22-86` [정정 2026-05-16: 단순 SHA-256 아님]

### `ui-lib/.../screen/chat/util/DestroyManager.kt` (295 lines)

- `:25` — `class DestroyManager(context, zchatPreferences, walletCoordinator, synchronizerProvider, standardPreferenceProvider, encryptedPreferenceProvider, flexaRepository)`
- `:34-40` — `KILL_MEMO_PREFIX = ZMSGConstants.REMOTE_KILL_PREFIX` (`"ZCHAT_DESTROY:"`), `MIN_KILL_PHRASE_LENGTH = 12`
- `:49` — `isKillSignal(amountZatoshi, memo): Boolean` — enabled + 정확한 amount + memo prefix + phrase hash 일치 시 true
- `:76` — `suspend fun destroyAll(requestUninstall: Boolean = true)` — **11단계 폭파 시퀀스**:
  1. `flexaRepository.disconnect()` — 외부 서비스
  2. `(synchronizer as? SdkSynchronizer)?.closeFlow()?.first()` — SDK 종료 (DB lock 해제 위해 critical)
  3. `walletCoordinator.deleteSdkDataFlow().first()` — wallet DB + derived data 삭제
  4. `zchatPreferences.clearAll()` — 채팅 prefs
  5. `standardPreferenceProvider().clearPreferences()` — 표준 prefs
  6. `encryptedPreferenceProvider().clearPreferences()` — 암호화 prefs (mnemonic 포함)
  7. `clearAllSharedPreferences()` — shared_prefs/ 디렉토리 파일 직접 삭제 (backup)
  8. `clearCache()` — cacheDir + externalCacheDir
  9. `clearDatabases()` — databases/ 디렉토리 + `deleteDatabase(name)`
  10. `clearFilesDir()` — filesDir + externalFilesDir
  11. `requestUninstall()` (`Intent.ACTION_DELETE`) + `forceKillApp()` (`Process.killProcess(myPid())`)
- `:229` — `isValidKillPhrase(phrase): Boolean` — length ≥ 12
- `:238` — `setupRemoteKill(phrase, amountZatoshi): Boolean`

### `ui-lib/.../screen/changeidentity/` (5 파일 — DEC-016 Identity Regeneration)

- `IdentityManager.kt:17` — `data class Identity(id: String, name: String, address: String, createdAt: Long, isDefault: Boolean)`
  - `:29-33` — `generateId()`: 16자 random `[A-Z a-z 0-9]` (62-char charset)
- `IdentityManager.kt:41` — `interface IdentityManager`
- `IdentityManager.kt:76` — `class IdentityManagerImpl(context)`
  - prefs: `"zchat_identity_manager"`
  - storage: JSON `List<Identity>` + active id
  - `_activeIdentityFlow: MutableStateFlow<Identity?>` — UI subscribe
- `IdentityManager.kt:200` — `initializeDefaultIdentity(address, name = "Default")` — wallet 첫 생성 시 호출
- `IdentityManager.kt:222` — `createDiversifiedIdentity(address, name)` — 새 diversified address로 새 identity 추가
- `IdentityManager.kt:147` — `removeIdentity(id)` — **default identity 또는 active identity 제거 금지** (안전 가드)
- `ChangeIdentityScreen.kt` / `ChangeIdentityState.kt` / `ChangeIdentityView.kt` / `ChangeIdentityVM.kt` — Compose UI: 현재 identity 목록, 새 identity 추가 (새 diversified address 도출), active switch, name 수정

## 연결 (Wiring)

- **Inputs:**
  - 송신/수신 path (`ChatViewModel`)에서 `cacheAddress`, `addConversationPartner`, `getAddress`, `findConversationPartnerByHash` 호출
  - Contact UI (별도 화면)에서 `addContact`, `removeContact`, `updateContactName`
  - Identity 화면에서 `getAllIdentities`, `setActiveIdentity`, `createDiversifiedIdentity` (`SDK.proposeDiversifiedAddress` 활용)
  - Remote kill: ChatViewModel `checkForRemoteKill(amountZatoshi, memo, txId)` (§1.6 line 1989)
- **Outputs:**
  - `String?` (address lookup)
  - `Boolean` (hasContact / isConversationPartner)
  - `activeIdentityFlow` (Compose UI)
  - 부작용: `destroyAll()` 시 OS-level uninstall intent + process kill
- **Dependencies (internal):**
  - [§1.1 ZMSG](./01-zmsg-protocol.md) — `generateAddressHash` / `generateLegacyAddressHash` AddressCacheImpl 안에서 호출
  - [§1.6 송수신 흐름](./06-send-receive-flow.md) — ChatViewModel 주요 caller
  - [§1.4 그룹 메시징](./04-group-messaging.md) — `contactBook.getContact(address)` 멤버 nickname 조회
  - [§1.2 KEX + E2E](./02-kex-e2e-encryption.md) — ZchatPreferences가 E2E keypair / PSK 저장 (별도 keyspace)
- **Dependencies (external):**
  - `android.content.SharedPreferences` + `Context.getSharedPreferences(MODE_PRIVATE)` — 세 별도 prefs 파일
  - `org.json.{JSONArray, JSONObject}` — Contact / IdentityManager 직렬화
  - `kotlinx.serialization.json.Json` — IdentityManager kotlinx-serialization
  - `cash.z.ecc.android.sdk.WalletCoordinator.deleteSdkDataFlow` — DestroyManager wallet wipe
  - `co.electriccoin.zcash.preference.{EncryptedPreferenceProvider, StandardPreferenceProvider}` — DestroyManager에서 prefs 전체 wipe
  - `androidx.activity.Intent` + `android.os.Process` — DestroyManager uninstall + kill
- **Dependencies (external — Identity Regen):**
  - 새 diversified address 도출: `Synchronizer` 또는 `WalletCoordinator`의 diversified address API (구체적 호출은 `ChangeIdentityVM` 내부에서 확인 필요 — 본 dive scope에서는 IdentityManager interface만 다룸)

## 라이브러리 (Libraries)

| Package | Version | Used for |
|---------|---------|----------|
| `org.json` | API 27+ | ContactBookImpl, AddressCache (StringSet은 SDK 내장), IdentityManager 폴백 |
| `kotlinx.serialization.json` | (Kotlin 2.1.10) | IdentityManager primary serialization |
| `co.electriccoin.zcash.preference.*` | (내부) | EncryptedPreferenceProvider / StandardPreferenceProvider 추상화 |
| `cash.z.ecc.android.sdk.WalletCoordinator` | 2.4.3 | DestroyManager가 wallet DB delete |

## 워크스루 — happy path

### A. Diversified address 처리 — Bob이 새 diversified로 송신

**시나리오:** Alice → Bob 한 번 송신 후 conversation 형성. Bob이 다음 응답을 그의 *다른* diversified address(`u1bob_div2...`)에서 보낸다.

**1. Alice 디바이스에서 송신 (§1.6 doSendMessage)**

```kotlin
addressCache.addConversationPartner(peerAddress = "u1bob_div1...")
```

`AddressCacheImpl.addConversationPartner` (line 178):
- `isValidZcashAddress("u1bob_div1...")` → true (UA + length > 100)
- `conversationPartners.add(...)` returns true (신규)
- `cacheAddress(hash(Bob_div1), Bob_div1)` — unvalidated cache
- prefs `KEY_CONVERSATION_PARTNERS` StringSet 갱신

**2. Bob의 응답이 Alice 디바이스에 도착 (§1.6 convertToConversations)**

memo = `ZMSG|v4|<convId>|<hash16_of_div2>|E2E1:...`. `parseV4Message`(§1.1)이 hash16 추출 후 `addressCache.getAddress(hash16)` → null (한 번도 본 적 없는 diversified address).

`AddressCache.findConversationPartnerByHash(hash16)`:
- direct match 없음
- 16-char hash라 partners 각각의 legacy hash로 비교:
  - `ZMSGProtocol.generateLegacyAddressHash("u1bob_div1...")` 계산
  - `hash16.startsWith(legacyHash)` 확인 — 그러나 div2와 div1은 unlinkable address라 hash 가 *다르다*
- **결과: null 반환** (single-partner heuristic 제거됨, line 229)

**3. 새 conversation으로 처리**

Bob_div2가 Alice 입장에선 새 sender. convID는 같아서 *원래 conversation의 thread*에 들어가지만 sender hash가 매핑 안 됨. `ChatViewModel`은 INIT 메시지를 받기 전까지 sender address를 모르고 hash 그대로 보여주거나 "Unknown sender" 표시. 사용자가 명시적으로 contact 추가하면 그제야 정상 표시.

> **본질 한계 인정:** zchat은 diversified address 문제를 *완전히* 해결하지 못한다. 대신 *conversation ID*를 통해 "thread membership"은 유지하고, sender hash가 안 맞아도 메시지 자체는 표시한다. 우리 팀이 viewing-key 기반 sender authentication을 추가하면 이 문제 해결 가능 (§3.8).

### B. Identity Regeneration (DEC-016)

**1. 사용자가 "New Identity" 버튼 누름 (ChangeIdentityView)**

ChangeIdentityVM이:
1. 새 diversified address 도출 — SDK `Synchronizer` 또는 `WalletCoordinator` 호출 (구체적 위치는 별도 확인 필요)
2. `Identity(id = generateId(), name = "Business", address = newDivAddr, createdAt = now, isDefault = false)` 생성
3. `IdentityManager.addIdentity(identity)` → prefs JSON 갱신
4. UI에 표시

**2. Active switch**

`IdentityManager.setActiveIdentity(id)`:
- prefs `KEY_ACTIVE_ID` 갱신
- `_activeIdentityFlow.value = identity` — Compose UI reactive
- ChatViewModel의 `_currentUserAddress`가 active identity address로 전환 (구체적 wire는 ZchatComposeVM에서)

**3. 새 identity로 첫 메시지 송신**

기존 contact들은 이 새 address로부터의 메시지를 *모르는 sender*로 본다 → INIT 메시지 송신 시 그쪽 디바이스가 새 conversation 시작 또는 기존 conversation에 ADDR 메시지로 알릴 수 있음. ADDR wire format은 §1.1에서 정의되어 있고 사용자가 의도적으로 회전할 때 활용.

> **(정정 2026-05-16, 코드 verify)** `ChangeIdentityVM.sendAddressChangeNotifications` (`ChangeIdentityVM.kt:213-232`) 가 **TODO 처리 + `Log.d` 만 호출, 실제 ADDR 메시지 송신 안 함.** 함수 본체 마지막 줄: `android.util.Log.d("ChangeIdentityVM", "Would notify ${uniqueContacts.size} contacts...")`. `createV4ADDRMessage` 호출 site **0건** (Grep 결과: ChangeIdentityVM 의 *주석* 안에서 함수 이름 언급만). `parseADDRMessage` 도 production code 에서 호출 없음 (test 파일만). 즉 **ADDR migration 양방향 모두 미구현**. 우리 팀 포팅 시 *명시적 graceful migration* (기존 contact들에게 ADDR 보내고 confirmation 대기) 패턴 필요 — 거의 처음부터 구현.

### C. Remote Kill (Destroy PIN trigger)

**1. 사용자가 미리 setup**

ChatViewModel UI에서:
```kotlin
destroyManager.setupRemoteKill(phrase = "my-secret-kill-12345", amountZatoshi = 12345L)
```

`DestroyManager.setupRemoteKill` (line 238):
- phrase 길이 ≥ 12 검증
- `zchatPreferences.setRemoteKillPhrase(phrase)` → 내부적으로 **SecureHash (PBKDF2 600k)** 로 hash 후 prefs `KEY_REMOTE_KILL_PHRASE_HASH = "remote_kill_phrase_hash"` 에 저장 (plaintext NO, claude.md v2.9.1 audit fix). 코드 주석에 "SHA-256 hash" 라고 적힌 outdated 부분 있음 (line 959) — 실제 동작은 PBKDF2 — `ZchatPreferences.kt:1168-1173`, `SecureHash.kt:22-86` [정정 2026-05-16]
- `setRemoteKillAmount(12345L)`, `setRemoteKillEnabled(true)`

**2. 누군가 (또는 자기 자신) 가 kill 메시지 송신**

```
Memo: "ZCHAT_DESTROY:my-secret-kill-12345"
Amount: 12345 zatoshi
To: <자기 unified address>
```

**3. 디바이스에서 수신 시 — ChatViewModel.checkForRemoteKill (§1.6 line 1989)**

```kotlin
private fun checkForRemoteKill(amountZatoshi: Long, memo: String?, txId: String) {
    if (!zchatPreferences.isRemoteKillEnabled()) return
    if (!zchatPreferences.hasRemoteKillPhrase()) return
    if (!processedKillCheckTxIds.add(txId)) return  // dedup
    val killAmount = zchatPreferences.getRemoteKillAmount()
    if (amountZatoshi != killAmount) return
    if (memo == null) return
    val trimmedMemo = memo.trim()
    if (!trimmedMemo.startsWith(ZMSGConstants.REMOTE_KILL_PREFIX)) return
    val phraseFromMemo = trimmedMemo.removePrefix(ZMSGConstants.REMOTE_KILL_PREFIX)
    if (zchatPreferences.verifyRemoteKillPhrase(phraseFromMemo)) {
        onRemoteKillDetected?.invoke()  // DestroyManager.destroyAll() 호출
    }
}
```

**4. `destroyAll(requestUninstall = true)` 실행 (line 76)**

11단계 폭파 (위 "파일과 함수" 절 참조). 결과:
- 모든 SDK 데이터, 모든 prefs, 모든 캐시·DB·파일 삭제
- Uninstall intent (사용자 confirmation 필요)
- 즉시 `Process.killProcess(myPid())`

**중요:** uninstall intent는 사용자의 OK 클릭이 필요. 즉 진정한 "remote silent uninstall"이 아니라 **wipe + uninstall 다이얼로그**. 자동 silent uninstall은 Android 권한 모델상 불가능 (devices admin app이 아닌 한).

## 노트 / quirks / footguns

- **세 prefs 파일이 분리됨:** `zchat_contact_book` / `zchat_address_cache` / `zchat_identity_manager` + ZchatPreferences가 사용하는 메인 파일 (4 separate SharedPreferences). `DestroyManager.clearAllSharedPreferences()`가 `shared_prefs/` 디렉토리 모든 파일 일괄 삭제로 안전 처리.
- **`AddressCacheImpl`의 collision guard.** unvalidated cache는 기존 매핑을 덮어쓰지 않는다 (line 86-91). 즉 hash collision 발생 시 *먼저 본 매핑*이 우선. validated path는 *trusted source(INIT message, contact book)* 에서만 호출 — 그건 sender authentication이 있다고 (적어도 KEX 후엔) 가정. 우리 팀 포팅 시 더 강한 model: 매번 ECDSA signature로 sender authentication.
- **`single-partner heuristic`은 misrouting 위험으로 제거됨** (line 229 주석). 단일 conversation partner 라도 hash 안 맞으면 보수적으로 null 반환. 즉 *false positive 방지* 우선 — 진짜 sender가 다른 conversation에 잘못 라우팅되는 일을 막음.
- **`findConversationPartnerByHash`의 legacy fallback은 16-char hash에 대해서만.** v3 hash (12-char)에서 v4 hash (16-char) 마이그레이션 시점에 도움. 우리 팀이 v4-only로 시작하면 이 코드 path 불필요.
- **Identity는 다른 messaging identity이지 다른 wallet이 아님.** `IdentityManager.Identity`는 다른 diversified address를 사용하지만 같은 *spending key*에서 파생 — 같은 mnemonic. 즉 진정한 unlinkability를 제공하지 않음 (viewing key 가진 사람은 모든 identity의 트랜잭션을 한꺼번에 본다). UX 측면에서 "여러 페르소나"이지 cryptographic separation이 아니다.
- **`removeIdentity` 가드**: default identity / active identity 제거 금지. default = wallet 초기에 만들어진 root identity. 활성 사용 중인 것 제거 불가.
- **DestroyManager의 11단계 시퀀스는 *fail-tolerant*.** 각 단계가 try/catch 안에서 best-effort — 한 단계 실패해도 다음 단계 진행. 마지막 `Process.killProcess`는 *반드시* 실행됨. 사용자 시점에서 destroy는 *되돌릴 수 없다*.
- **`requestUninstall = true`는 사용자 클릭 필요.** silent silent uninstall은 device admin app만 가능 (root 없이). zchat은 device admin이 아니라 일반 앱이므로 *uninstall confirmation dialog* 표시 후 사용자 OK 시 OS가 uninstall. 데이터는 dialog 표시 *전에* 이미 wipe됨 → 사용자 cancel해도 wipe는 됐다 (단 앱 자체는 OS에 남음).
- **Remote kill memo는 plaintext.** `ZCHAT_DESTROY:<phrase>` prefix가 ZMSG_PROTOCOL_SPEC.md에 명시. **즉 누구나 prefix를 알면 trigger 시도 가능** — 다만 phrase는 hash로 보호되어 brute force 필요 + amount도 매치해야 함 (2-factor). 우리 팀 포팅 시 ECDSA 서명된 remote kill (특정 ECDSA 서명만 trigger) 권장. claude.md "Known Technical Debt"에 명시: "Remote kill phrase could be encrypted (currently plaintext)".
- **Destroy phrase + amount가 둘 다 정확해야 trigger.** 확률 ≈ 1/(2^128 phrase hash space × 2^64 amount). 의도치 않은 trigger 거의 불가. 다만 사용자가 phrase를 잊으면 자기도 trigger 불가 — backup mechanism 없음.
- **Identity Regeneration은 명시적 ADDR broadcast 옵션을 제공하지 않는 것으로 보임.** 코드에 `createV4ADDRMessage`(§1.1)는 정의되어 있지만 `ChangeIdentityVM`이 자동으로 호출하는 코드를 본 dive scope에서 확인하지 않음. 우리 팀 포팅 시 *명시적 graceful migration* 흐름 권장.

## 답한 open question

- **Q17** (research-plan §7): "screen/changeidentity/ (DEC-016 Identity Regeneration) — 어떤 의미에서 identity를 regenerate?"
  > **Answer:** *"Masks" 개념* — 같은 BIP-39 seed에서 파생되는 *여러 diversified addresses*를 각각 별도 Identity로 등록 + 활성 전환 가능. 새 identity로 전환 시 `_currentUserAddress`가 바뀌고, ChatViewModel 등이 새 address로 메시지 송수신. 같은 wallet 안에서 cryptographic separation이 아니라 *UX-level 페르소나* — viewing key 가진 사람은 모든 identity 다 본다. 기존 contact들과의 호환성은 *명시적 ADDR 메시지*가 자동으로 발사되는지 확인되지 않음 (caller 코드 추가 검증 필요). `createV4ADDRMessage` wire format(§1.1)은 존재하므로 우리 팀이 graceful migration을 추가하기 쉬움. — `IdentityManager.kt:17-235`, `ChangeIdentityVM.kt` (별도 분석 필요)

- **Q18** (research-plan §7): "AddressCacheImpl — hash → address 매핑 cache, hash collision 발생 시?"
  > **Answer:** **Collision guard.** `cacheAddressWithValidation(hash, address, validated = false)` 호출 시 같은 hash에 *다른 address*가 매핑되어 있으면 reject (line 86-91). validated path만 overwrite 허용. → 즉 "먼저 본 매핑 우선" 정책. 16-char hash (8 bytes, ~2^64 collision space)이라 실용적으로 충돌 거의 없음. eviction policy 없음 (unbounded growth — 그러나 unique senders 수가 collision 발생 전까지는 매우 크다). 디바이스 storage 부족 시 LRU 없음 — `clearCache()` 수동 호출만 가능. — `AddressCacheImpl.kt:77-96`

- **Q22** (research-plan §7): "Destroy PIN — trigger / wipe scope / recoverability"
  > **Answer:** Trigger는 *두 가지 path*:
  > 1. **Local**: 사용자가 직접 PIN 입력 → DestroyManager.destroyAll() 호출 (UI 흐름은 별도)
  > 2. **Remote**: `ChatViewModel.checkForRemoteKill`이 incoming tx의 `(amount, memo)` 가 매치 시 `onRemoteKillDetected` 콜백 발사 → DestroyManager.destroyAll()
  >
  > Wipe scope: 11단계 — Flexa disconnect, Synchronizer close, SDK wallet data delete (WalletCoordinator), zchat prefs clearAll, standard/encrypted prefs clear, shared_prefs/ 디렉토리 파일 삭제, cacheDir, databases/, filesDir, uninstall intent, Process.killProcess. **재복구 불가능** — mnemonic 포함 모든 비밀이 사라짐 (사용자가 외부에 backup 안 했으면). PIN/phrase 자체는 SHA-256 hash로 prefs에 저장 (plaintext NO, claude.md v2.9.1 audit fix).  — `DestroyManager.kt:76-147`

- **C8** (claims-to-verify): Local contact book with aliases
  > **Answer:** ✓. `ContactBookImpl`이 alias `name` 필드 저장. `Conversation.displayName`이 contact name 우선 표시. — `ContactBookImpl.kt`, `Contact.kt:8`, `GroupModels.kt:Conversation:310`

- **C12** (claims-to-verify): "No servers, no accounts. Zcash address is your identity"
  > **Answer:** ✓ — 본 dive에서 검증된 모든 데이터(contact, address cache, identity, destroy PIN, ratchet state, group keys)는 SharedPreferences 또는 EncryptedSharedPreferences 로컬 저장. 외부 서버 의존 0건. §1.6 Q15에서 api.zsend.xyz가 wallet/메시지 경로에 없음도 확인.

- **C66** (claims-to-verify): Remote kill — `ZCHAT_DESTROY:<phrase>` (plaintext, technical debt)
  > **Answer:** ✓ 검증됨. wire format `ZCHAT_DESTROY:<phrase>` 그대로 (`ZMSGConstants.REMOTE_KILL_PREFIX`), phrase는 SHA-256 hash로 prefs에 저장 (plaintext 저장 NO, line 51 + claude.md v2.9.1). 그러나 *memo 자체*는 plaintext — 누구나 prefix를 안다는 점에서 obfuscation 약함. claude.md "Known Technical Debt"에 명시. — `DestroyManager.kt:39-65`, `ZMSGConstants.kt:121`
