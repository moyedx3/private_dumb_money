# §1.4 그룹 메시징 (ZGRP + ECIES)

## 목적 (Purpose)

그룹 메시징 서브시스템은 1:1 ZMSG와 별도의 protocol family `ZMSG:3.0:GROUP:<type>:<group_id>:<json_payload>`를 사용하며, 8종 메시지 타입(GC/GI/GA/GL/GK/GM/GY/GF)으로 그룹 생명주기를 관리한다. 그룹 키는 **AES-256 한 개를 모든 멤버가 공유**하고, 신규 멤버에게는 **per-recipient ECIES wrap**(§1.2 `encryptGroupKeyForMember`)으로 분배된다. 멤버가 KEX를 먼저 끝낸 상태가 아니라면 *plaintext group key fallback*(Zcash 노트 암호화에만 의존)이 사용된다 — 알려진 보안 trade-off. 그룹 메시지 한 통은 활성 멤버 수 N만큼 **별도 트랜잭션**으로 fan-out된다 (ZIP-321 multi-output이 아님 — 우리가 §1.5에서 본 1:1 chunking과 다른 송신 모델).

## 파일과 함수 (Files & functions)

### `ui-lib/.../screen/chat/model/ZMSGGroupProtocol.kt`

- `ZMSGGroupProtocol.kt:30` — `object ZMSGGroupProtocol`
- `ZMSGGroupProtocol.kt:35` — `GROUP_PREFIX = "ZMSG:3.0:GROUP:"` (`ZMSGConstants.Prefixes.GROUP`)
- `ZMSGGroupProtocol.kt:38-40` — AES-256-GCM 파라미터 (`AES_KEY_SIZE=256`, `GCM_NONCE_LENGTH=12`, `GCM_TAG_LENGTH=128`)
- `ZMSGGroupProtocol.kt:45` / `:52` / `:62` / `:72` — `isGroupMessage`, `parseMessageType`, `parseGroupId`, `parsePayload` — wire format 분해 (콜론 split, limit=3 — payload 안에 콜론 가능)
- Message creators (각각 `ZMSG:3.0:GROUP:<TYPE>:<groupId>:<jsonPayload>` 형식):
  - `:86` — `createGroupCreateMessage(groupId, name, creator, members, groupKey)` → `GC` — 그룹 키는 payload에 포함 *안 됨* (caller가 별도 ECIES wrap)
  - `:109` — `createGroupInviteMessage(...encryptedGroupKey: String)` → `GI` — pre-encrypted 그룹 키(`enc_key`) 사용. **선호 path**
  - `:134` — `createGroupInviteMessage(...groupKey: ByteArray)` → `GI` (overload) — **plaintext group key fallback**. base64로 그대로 `group_key` 필드에 넣음. `// TODO: Add per-recipient encryption using their public key` 주석
  - `:157` — `createGroupAcceptMessage(groupId, accepter, accepterPublicKey)` → `GA`
  - `:172` — `createGroupMsgMessage(groupId, seq, epoch, sender, plaintext, groupKey)` → `GM` — internal `encryptMessage`로 AES-GCM 후 payload에 `nonce` + `ct` + `ts`
  - `:197` — `createGroupLeaveMessage(groupId, leaver)` → `GL`
  - `:211` — `createGroupKickMessage(groupId, kicked, kicker, newEpoch, encryptedNewKey?)` → `GK` — 회전된 새 키를 함께 분배 가능
  - `:230` — `createGroupKeyMessage(groupId, newEpoch, encryptedGroupKey, reason)` → `GY` — 명시적 key rotation
  - `:248` — `createGroupInfoMessage(groupId, newName?, updater)` → `GF`
- Payload parsers (각각 `parse*Payload(jsonString)` → data class):
  - `:268` `parseGroupCreatePayload`, `:290` `parseGroupInvitePayload`, `:311` `parseGroupAcceptPayload`, `:328` `parseGroupMsgPayload`, `:349` `parseGroupLeavePayload`, `:366` `parseGroupKickPayload`
- Crypto helpers:
  - `:393` — `generateGroupKey(): ByteArray` — `KeyGenerator.getInstance("AES")` + 256-bit
  - `:402` — `encryptMessage(plaintext, groupKey): EncryptedMessage` — AES/GCM/NoPadding, 12B random nonce, 반환 `(nonce_b64, ct_b64)`
  - `:424` — `decryptMessage(nonce, ciphertext, groupKey): String?` — null on failure
  - `:446` / `:453` — `encodeGroupKey` / `decodeGroupKey` — Base64 NO_WRAP
- Storage helpers (line 462 ~ 538): `serializeGroupInfo`, `deserializeGroupInfo`, `serializeGroupMembers`, `deserializeGroupMembers` — JSON 직렬화

### `ui-lib/.../screen/chat/model/GroupModels.kt`

- `GroupModels.kt:9` — `enum AdminPolicy { CREATOR_ONLY, MULTI_ADMIN, DEMOCRATIC }` — MULTI_ADMIN/DEMOCRATIC은 "future"
- `GroupModels.kt:18` — `enum MemberStatus { INVITED, ACTIVE, LEFT, KICKED }`
- `GroupModels.kt:28` — `enum GroupMessageType { GC, GI, GA, GL, GK, GM, GY, GF }` — 코드 + 식별자
- `GroupModels.kt:47` — `data class GroupInfo(groupId, name, creatorAddress, createdAt, adminPolicy, currentEpoch, groupKey: String? (Base64), isActive)`
  - `:60` companion `generateGroupId(creator): String` — `"zgrp_" + SHA-256("$creator$timestamp$uuid8").take(12).hex`
- `GroupModels.kt:76` — `data class GroupMember(address, publicKey?, joinedAt, status, isAdmin, nickname?)`
- `GroupModels.kt:94` — `data class GroupMessage(id, groupId, txId, seq, epoch, sender, encryptedContent, decryptedContent, nonce, timestamp, blockHeight, txIndex, isPending, isFailed)`
  - `:126` `compareForOrdering` — blockHeight → txIndex → seq → senderAddress 4단계 deterministic ordering
- `GroupModels.kt:152` — `data class GroupConversation(groupInfo, members, messages, lastMessage, unreadCount, draft)`
- `GroupModels.kt:189-253` — Payload data classes (`GroupCreatePayload`, `GroupInvitePayload`, `GroupAcceptPayload`, `GroupMsgPayload`, `GroupLeavePayload`, `GroupKickPayload`)
- `GroupModels.kt:258` — `data class CreateGroupState(groupName, selectedMembers, availableContacts, isCreating, error, createdGroupId)` — Compose UI state
- `GroupModels.kt:276` / `:289` — `sealed interface GroupDetailState` / `GroupSettingsState` — UI sealed states

### `ui-lib/.../screen/chat/viewmodel/GroupViewModel.kt` (873 lines)

- `GroupViewModel.kt:45` — `class GroupViewModel(...)` — Koin에서 주입; deps = `ZchatPreferences`, `ContactBookImpl`, `CreateChunkedMessageProposalUseCase`, `Synchronizer` (간접)
- `GroupViewModel.kt:120` — `loadGroups()` — prefs `getAllGroupIds` → 각 groupId마다 `loadGroup`
- `GroupViewModel.kt:139` — `loadGroup(groupId): GroupConversation?` — prefs에서 GroupInfo / members / messages 로드
- `GroupViewModel.kt:241` — `suspend fun loadGroupMessagesFromHistory(groupId)` — 동기화된 tx history에서 GROUP_MSG 추출
- `GroupViewModel.kt:296` — `parseAndDecryptGroupMessage(memo, txId, ...)` — incoming `GM` 처리
- `GroupViewModel.kt:341` — `getGroupKeyForDecryption(groupId): ByteArray?` — epoch별 키 lookup
- `GroupViewModel.kt:419` & `:777` — `leaveGroup(...)` — 두 overload (전자: with callback, 후자: 단독)
- `GroupViewModel.kt:484` — `createGroup()` — 그룹 생성 + ECIES invite fan-out (아래 워크스루)
- `GroupViewModel.kt:560` — `E2EEncryption.encryptGroupKeyForMember(memberPublicKey, groupKey)` 호출 — per-recipient ECIES wrap
- `GroupViewModel.kt:577` — `Log.w(TAG, "No KEX with ${memberAddress.redactAddress()} - using plaintext group key")` — plaintext fallback의 보안 경고
- `GroupViewModel.kt:638` — `sendGroupMessage(groupId, message)` — 그룹 메시지 송신 + fan-out
- `GroupViewModel.kt:710` — `for (recipient in recipients) { ... createChunkedMessageProposal(...) }` — fan-out 루프 (500ms 간격)
- `GroupViewModel.kt:816` — `createGroupLeaveMessage` 호출하여 GL 메시지 fan-out
- `GroupViewModel.kt:857` — `deleteGroup(groupId)` — 로컬 prefs cleanup

### `ui-lib/.../screen/chat/view/CreateGroupView.kt` / `GroupDetailView.kt` / `GroupSettingsView.kt`

UI layer. Compose state는 `CreateGroupState` / `GroupDetailState` / `GroupSettingsState`로 driven. 본 walkthrough scope에서는 view 내부 디테일은 다루지 않는다.

## 연결 (Wiring)

- **Inputs:**
  - `creatorAddress: String`, `selectedMembers: List<String>`, `groupName: String` (createGroup)
  - `groupId, plaintext` (sendGroupMessage)
  - Incoming GROUP memo from `ChatViewModel.parseMemo` GROUP branch (§1.1) — `parseGroupId / parseMessageType / parsePayload` 결과
- **Outputs:**
  - N개 ZIP-321 메시지 transactions (createGroup 시 selectedMembers 수만큼 GI; sendGroupMessage 시 active members 수만큼 GM)
  - Local state updates: `ZchatPreferences.saveGroupInfo / saveGroupMembers / saveGroupKey / setGroupKeyEpoch / incrementGroupMessageSequence / saveGroupDraft / clearGroupDraft`
- **Dependencies (internal):**
  - [§1.1 ZMSG 프로토콜](./01-zmsg-protocol.md) — `parseMemo`의 GROUP branch가 본 layer의 parser를 dispatch
  - [§1.2 KEX + E2E](./02-kex-e2e-encryption.md) — `encryptGroupKeyForMember` / `decryptGroupKeyFromInvite` ECIES wrappers
  - [§1.5 ZIP-321 청킹](./05-zip321-tx-chunking.md) — `createChunkedMessageProposal(rawMemo = true)`로 각 멤버에게 fan-out 송신
  - [§1.6 송수신 흐름](./06-send-receive-flow.md) — `ChatViewModel`이 GROUP type 감지 시 `GroupViewModel.parseAndDecryptGroupMessage` 위임
  - [§1.7 컨택트 + Identity](./07-contact-book-address-cache.md) — `ContactBookImpl.getContact` 멤버 nickname 조회, `ZchatPreferences` 그룹 상태 storage
- **Dependencies (external):**
  - `org.json.JSONObject` / `JSONArray` — payload 직렬화
  - `javax.crypto.{Cipher, KeyGenerator, SecretKeySpec}`, `javax.crypto.spec.GCMParameterSpec`
  - `java.security.{MessageDigest, SecureRandom}`
  - `android.util.Base64` (NO_WRAP)

## 라이브러리 (Libraries)

| Package | Version | Used for |
|---------|---------|----------|
| JDK `javax.crypto` | API 27+ | AES-256-GCM 그룹 메시지 본문 암호화 |
| `org.json` | API 27+ | GROUP payload JSON 직렬화/역직렬화 |
| `android.util.Base64` | API 27+ | NO_WRAP 인코딩 (line break 없음 — memo 안에 들어가야 하므로) |
| Kotlin coroutines | (Kotlin 2.1.10) | `viewModelScope.launch` + `delay(500)` fan-out throttling |

## 워크스루 — happy path

### A. 그룹 생성 — Alice가 Bob과 Carol을 초대

**1. UI input — `CreateGroupView`**

Alice가 "Friends" 입력, Bob/Carol 체크. `CreateGroupState.isValid = true` (name + ≥1 member).

**2. `GroupViewModel.createGroup()` (line 484)**

`creatorAddress = Alice`, state.selectedMembers = [Bob, Carol].

**3. group ID + 그룹 키 생성 (line 502-506)**

```kotlin
val groupId = GroupInfo.generateGroupId(creatorAddress)
// → "zgrp_<12-hex>" — SHA-256(creator+ts+UUID8).take(12)
val groupKey = ZMSGGroupProtocol.generateGroupKey()
// → 32B AES-256 random key
val encodedGroupKey = ZMSGGroupProtocol.encodeGroupKey(groupKey)
// → Base64
```

**4. 그룹 메타 저장 — line 540-543**

```kotlin
zchatPreferences.saveGroupInfo(groupId, ZMSGGroupProtocol.serializeGroupInfo(groupInfo))
zchatPreferences.saveGroupMembers(groupId, ZMSGGroupProtocol.serializeGroupMembers(members))
zchatPreferences.saveGroupKey(groupId, 0, encodedGroupKey)  // epoch 0
zchatPreferences.setGroupKeyEpoch(groupId, 0)
```

**5. Creator E2E keypair 생성 (line 525-530)**

```kotlin
if (address == creatorAddress) {
    val keyPair = E2EEncryption.generateKeyPair()
    zchatPreferences.setE2EOurKeys(groupId, keyPair.publicKey, keyPair.privateKey)
    zchatPreferences.setE2EKeyVersion(groupId, E2EKeyVersion.V2.value)
    keyPair.publicKey
}
```

> 주목: 그룹용 E2E keypair는 *그룹별*로 따로 만든다. 즉 Alice는 *1:1 채팅* 용 secp256r1 keypair (peer별)와 *그룹* 용 keypair (그룹별)를 모두 관리. ZchatPreferences key namespace로 분리(peer = address vs peer = groupId).

**6. 각 멤버에게 GROUP_INVITE — for-loop (line 551-607)**

```kotlin
for (memberAddress in state.selectedMembers) {
    val memberPublicKey = zchatPreferences.getE2EPeerPublicKey(memberAddress)
    val inviteMemo = if (memberPublicKey != null && ourE2EPublicKey != null) {
        // 선호 path: ECIES wrap
        val encryptedGroupKey = E2EEncryption.encryptGroupKeyForMember(
            memberPublicKey = memberPublicKey,
            groupKey = groupKey
        )
        ZMSGGroupProtocol.createGroupInviteMessage(
            groupId, groupName, creatorAddress, ourE2EPublicKey, allMemberAddresses, 0, encryptedGroupKey
        )
    } else {
        // Fallback: plaintext group key
        Log.w(TAG, "No KEX with ${memberAddress.redactAddress()} - using plaintext group key")
        ZMSGGroupProtocol.createGroupInviteMessage(
            groupId, groupName, creatorAddress, memberAddress, groupKey, allMemberAddresses
        )
    }
    
    createChunkedMessageProposal(
        destinationAddress = memberAddress,
        senderAddress = creatorAddress,
        message = inviteMemo,
        isFirstMessage = false,
        amountPerOutput = Zatoshi(DEFAULT_MESSAGE_AMOUNT),
        directSubmit = true,
        skipNavigation = true,
        rawMemo = true  // GROUP message 그대로 사용 (ZMSG wrap 안 함)
    )
    delay(500)  // wallet 부하 throttle
}
```

핵심:
- **각 멤버당 별도 transaction**. ZIP-321 multi-output이 아닌 N개 atomic tx로 fan-out
- **ECIES wrap 조건**: peer와 prior KEX(§1.2) 완료되어 `getE2EPeerPublicKey(member) != null`인 경우
- **plaintext fallback**: KEX 없는 멤버에게는 그룹 키가 `group_key` 필드에 base64 plaintext로 들어감 — Zcash 노트 암호화에 의존
- **500ms 지연**: 동시 wallet operation race 방지

### B. 그룹 초대 수락 — Bob 디바이스

**7. Bob의 ChatViewModel이 새 incoming tx에서 GROUP_INVITE 발견 — §1.6**

`parseMemo`(§1.1)가 `messageType = GROUP, groupMessageType = GROUP_INVITE`로 분기.

**8. `GroupViewModel`이 invite payload 파싱 — `parseGroupInvitePayload(payload)`**

```kotlin
GroupInvitePayload(
    groupId = "zgrp_abc123def456",
    groupName = "Friends",
    inviter = aliceAddress,
    inviterPublicKey = "<alice_e2e_pub>",
    members = [alice, bob, carol],
    keyEpoch = 0,
    encryptedGroupKey = "ECIES:<eph_pub>:<nonce>:<ct>" or base64-plaintext
)
```

**9. 그룹 키 복호화 — ECIES 또는 plaintext**

ECIES path:
```kotlin
val groupKey = E2EEncryption.decryptGroupKeyFromInvite(
    ourPrivateKey = zchatPreferences.getE2EOurPrivateKey(bobAddress),
    encryptedGroupKey = payload.encryptedGroupKey
)
```

plaintext path (KEX 없는 경우):
```kotlin
val groupKey = Base64.decode(payload.group_key, Base64.NO_WRAP)
```

**10. Bob도 자기 그룹용 E2E keypair 생성 + GROUP_ACCEPT 전송**

GA payload = `{ accepter: bobAddr, accepter_pub: bobGroupPub }`. Alice에게 한 개 tx.

### C. 그룹 메시지 송신 — Alice → all

**11. `sendGroupMessage("zgrp_abc...", "Hi everyone")` (line 638)**

```kotlin
val keyEpoch = zchatPreferences.getGroupKeyEpoch(groupId)  // 0
val groupKey = ZMSGGroupProtocol.decodeGroupKey(zchatPreferences.getGroupKey(groupId, keyEpoch)!!)
val seq = zchatPreferences.incrementGroupMessageSequence(groupId)  // 1, 2, 3, …
val memo = ZMSGGroupProtocol.createGroupMsgMessage(groupId, seq, keyEpoch, senderAddress, plaintext = "Hi everyone", groupKey)
```

**12. `createGroupMsgMessage` 내부 (line 172)**

```kotlin
val encrypted = encryptMessage(plaintext, groupKey)
// AES-256-GCM, 12B random nonce, 128-bit tag, no AAD
val payload = JSONObject().apply {
    put("seq", seq); put("epoch", epoch); put("sender", senderAddress)
    put("nonce", encrypted.nonce); put("ct", encrypted.ciphertext); put("ts", currentSec)
}
return "ZMSG:3.0:GROUP:GM:$groupId:${payload}"
```

> **주의:** AAD가 *없다*. ZMSG_PROTOCOL_SPEC.md 표 §What is protected가 "Group message integrity: AES-256-GCM with AAD = `groupId || senderAddress`"라 했지만 코드 `encryptMessage`(line 402)는 `cipher.updateAAD()`를 호출하지 않음. **spec과 코드 불일치** — 우리 팀 포팅 시 spec쪽이 더 합리적이므로 AAD 추가 권장.

**13. 모든 active members에게 fan-out (line 710-729)**

```kotlin
val members = ZMSGGroupProtocol.deserializeGroupMembers(membersJson)
val recipients = members.filter { it.status == MemberStatus.ACTIVE && it.address != senderAddress }
for (recipient in recipients) {
    createChunkedMessageProposal(
        destinationAddress = recipient.address,
        message = memo,
        rawMemo = true,
        ...
    )
    delay(500)
}
```

같은 ciphertext (= same nonce, same key, same ct) 가 N개 tx의 memo에 들어감. Zcash 노트 암호화는 receiver별 다르므로 onchain shielded ciphertext는 각기 다름.

**14. Pending UI 업데이트 (line 672-691)**

Optimistic UI: 송신 시점에 `GroupMessage(isPending = true, decryptedContent = plaintext)`를 state에 즉시 추가. Tx confirmation은 §1.6 cron / Synchronizer Flow가 채움.

### D. 키 회전 (GROUP_KEY_ROTATE — GY)

`createGroupKeyMessage(groupId, newEpoch, encryptedGroupKey, reason)` 가 정의되어 있다 (line 230). 하지만 **자동 trigger 코드는 없다.**
- `leaveGroup`(line 419, 777)는 GL 메시지만 보냄 — 키 회전 *안 함*. 떠난 멤버는 여전히 epoch 0 키 보유.
- `GROUP_KICK`(line 211)은 `encryptedNewKey?` 옵션을 받지만 caller가 명시적으로 새 키 도출 후 ECIES wrap 해야 함 — 실제 호출 코드는 GroupViewModel에서 발견되지 않음 (검색 결과 GroupKickPayload만 parse).
- **자동 키 회전 미구현.** 우리 팀이 그룹 멤버 변경 시 backward secrecy 원하면 명시적 GY 호출 + per-member ECIES wrap 루프 추가 필요.

## 노트 / quirks / footguns

- **AAD가 코드에 없다.** ZMSG_PROTOCOL_SPEC.md는 GM이 `AAD = groupId || senderAddress`를 사용한다고 명시했지만 `encryptMessage`(line 402-419)는 `cipher.updateAAD()`를 호출하지 않음. 따라서 한 그룹의 ciphertext를 다른 그룹의 GM 메시지에 swap해도 (group key가 같으면) AEAD 검증 통과. spec과 코드 mismatch — *spec이 옳고 코드가 빠뜨림*. 우리 팀 포팅 시 AAD 추가 권장.
- **Fan-out = N개 별도 tx**. ZIP-321 multi-output을 *안 쓴다*. 이유:
  1. 각 멤버가 *다른 sender ECIES wrap*을 받기 때문 (initial invite). messaging 단계에서는 같은 ciphertext지만 atomicity가 필요 없음 — 각자 도착 시간이 달라도 무관.
  2. memo 512B 안에 N개 member마다 다른 ECIES wrap을 넣는 건 N=3 정도부터 불가능.
  3. 비용은 N배 (멤버 N명 = N × ~0.0001 ZEC) — group의 비용 모델이 직접 N에 비례.
- **Plaintext group key fallback의 위험성.** KEX 없는 멤버에게는 `group_key`가 base64 plaintext로 GROUP_INVITE memo에 들어감. Zcash 노트 암호화로 receiver(invitee) 외엔 안 보이지만:
  - 만약 receiver의 viewing key가 leak되면 → 과거 모든 GROUP_INVITE의 group_key 노출
  - 새 멤버를 더할 때마다 *같은 group key*가 또 plaintext로 송신될 위험 (key reuse)
  → 우리 팀 권장: KEX 미완료 멤버는 *initial 그룹에서 제외*하고 KEX 완료 후 별도로 추가.
- **`zgrp_<12-hex>` group ID는 64-bit collision space**. 12 hex = 6 bytes. ZMSG_PROTOCOL_SPEC.md (C51)는 "24-character hex"로 명시했으나 코드는 `zgrp_` prefix + 12 hex = 17자. **spec과 코드 mismatch**. 실용적으로 12 hex(48-bit)는 충분히 안전.
- **Key rotation은 protocol-level만 정의, 실제 트리거 없음.** 회사가 GL/GK로 인한 멤버 변경 시 자동 rotation을 의도했더라도 코드 호출이 없다. backward secrecy(떠난 멤버가 새 메시지 못 보게)가 보장되지 않음. ZMSG_PROTOCOL_SPEC.md "GROUP_KEY_ROTATE (GY) message type defined; epoch field per message"는 *protocol-level support* 라는 점을 정직하게 명시.
- **Group key는 영구 보관**. 한 epoch의 키가 ZchatPreferences에 영구 저장(`saveGroupKey(groupId, epoch, encodedKey)`). leave/kick 후에도 epoch 0 키가 남음 — 그 시점 이전 모든 메시지 decrypt 가능 (의도된 design — 과거 메시지 보존).
- **Sequence number (`seq`)는 sender-local**. `incrementGroupMessageSequence(groupId)`(line 659)가 prefs에서 monotonic 증가시킴. 두 device가 같은 seed로 동시 송신 시 같은 seq 발급 — `GroupMessage.compareForOrdering`(line 126)이 sender+seq combination으로 deterministic order 보장하지만 동일 sender의 두 device가 같은 seq를 쓰면 ordering 무너짐. **multi-device 미지원의 또 다른 이유**.
- **`createGroupCreateMessage`(GC)는 실제 호출 site가 없어 보임.** `createGroup`은 GROUP_INVITE만 보내고 GC는 *모든 멤버에게 그룹 자체의 존재를 알리는* broadcast인데 GroupViewModel.createGroup이 호출하지 않음. invitee는 GI로 그룹 컨텍스트 전체 받음 — GC가 redundant. 우리 팀은 GC를 제거하거나 *creator only initial state checkpoint*로 활용 검토.
- **`adminPolicy`는 enum만 정의, 정책 enforcement 없음.** `MULTI_ADMIN` / `DEMOCRATIC` "future" 주석. CREATOR_ONLY만 의미 — 그러나 protocol-level enforcement(예: GK 메시지를 verify할 때 sender가 creator인지 확인)도 코드에 없음. malicious member가 가짜 GK 보내면 다른 멤버 디바이스에서 어떻게 처리하는지 §1.6에서 확인 필요.
- **`MemberStatus`는 sender-local view**. 멤버의 INVITED/ACTIVE/LEFT/KICKED 상태는 *우리 디바이스*가 어떤 메시지를 봤느냐에 따라 결정. 다른 멤버 디바이스가 다른 view를 가질 수 있음 — group state는 eventually consistent.

## 답한 open question

- **Q8** (research-plan §7): "GROUP_INVITE는 ECIES per-member로 그룹 키를 wrap한다고 했는데, 멤버 수가 많을 때 단일 ZMSG 메시지 안에 N개 ECIES wrap을 모두 담는가, 멤버당 별도 트랜잭션을 보내는가?"
  > **Answer:** **멤버당 별도 트랜잭션**. `createGroup` 루프(line 551-607)가 각 멤버에게 `createChunkedMessageProposal`(`rawMemo = true`)로 한 번씩 GI 송신. 한 ZMSG memo 안에는 *한 멤버의* ECIES wrap만 들어감. 따라서 N명 그룹은 N개 invite tx (≈ N × 0.0001 ZEC + N × Zcash fee). — `GroupViewModel.kt:551-607`

- **Q9** (research-plan §7): "GROUP_KEY_ROTATE (GY)는 실제 코드에서 호출되는 경로가 있는가?"
  > **Answer:** **No.** `ZMSGGroupProtocol.createGroupKeyMessage`(line 230)가 정의되어 있으나 `GroupViewModel`(또는 코드 어디서도) 호출하지 않는다. `leaveGroup` / `kickMember`도 자동으로 GY를 발사하지 않음. **protocol-level만 정의되었고 실제 키 회전은 미구현**. ZMSG_PROTOCOL_SPEC.md의 표현이 정확. — `GroupViewModel.kt` 전체 (grep `createGroupKeyMessage` 결과 0건)

- **Q10** (research-plan §7): "그룹 멤버가 떠나면 그룹 키가 자동 회전되는가? 안 된다면 그 멤버가 과거 메시지를 계속 복호화할 수 있는가?"
  > **Answer:** **자동 회전 안 됨**. `leaveGroup`(line 419, 777)은 GL 메시지만 broadcast하고 자기 prefs의 그룹 키를 그대로 둔다(`deleteGroup`(line 857)이 호출되지 않는 한). **떠난 멤버는 그 이후 키 회전 전까지 모든 메시지 decrypt 가능** — backward secrecy 부재. 우리 팀 포팅 시 `leaveGroup`/`kickMember` 트리거로 자동 GY 송신 + per-active-member ECIES wrap 추가 권장. — `GroupViewModel.kt:419, 777, 857`

- **C50, C51, C52, C53, C54** (claims-to-verify):
  > **C50** Message types 8개 — ✓ GC/GI/GA/GL/GK/GM/GY/GF 모두 `GroupMessageType` enum 정의 (`GroupModels.kt:28-42`)
  > **C51** 그룹 ID 24-character hex — **✗ 부분 수정 필요**. 코드는 `zgrp_<12-hex-chars>` = 17 chars total. 12 hex = 6 bytes = 48-bit collision space. spec과 mismatch. (`GroupModels.kt:60-69`)
  > **C52** Fan-out — output 1 per member, +1 for platform fee — ✓ 코드 동작 부합 (`GroupViewModel.kt:710-729`); platform fee output 추가는 §1.5 `createChunkedMessageProposal` 내부에서 확인
  > **C53** 비용 ~0.0001 ZEC per member per message — ✓ `Zatoshi(DEFAULT_MESSAGE_AMOUNT)` 사용 (DEFAULT_MESSAGE_AMOUNT 상수 위치는 §1.5에서 확인)
  > **C54** AAD = `groupId + senderAddress` — **✗ 코드에 AAD 없음** (`encryptMessage`(line 402)가 `updateAAD` 호출 안 함). spec과 mismatch — *spec이 옳고 구현이 빠뜨림*.
