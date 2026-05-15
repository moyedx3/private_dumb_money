# §1.1 ZMSG 프로토콜 (memo transport encoding)

## 목적 (Purpose)

ZMSG 서브시스템은 Zcash shielded transaction의 memo 필드(≤ 512B) 위에 채팅 메시지를 인코딩하기 위한 wire-level 프로토콜이다. 메모 1개에 들어가지 않는 메시지를 chunk로 쪼개어 multi-output transaction으로 보내고, 받은 chunk들을 transaction 단위로 재조립한다. v4(현재)는 8자 conversation ID로 양방향 threading을 안정시키며, v3 / v2는 backward-compat 파싱만 유지된다. 또한 KEX/KEXACK/ADDR 같은 control-plane 메시지와 ZREACT/ZRCPT/ZSTAT/ZTL/ZUNLOCK/ZREQ 같은 application-level 특수 메시지의 wire format도 이 layer에서 결정된다.

## 파일과 함수 (Files & functions)

- `ui-lib/src/main/java/co/electriccoin/zcash/ui/screen/chat/model/ZMSGProtocol.kt:33` — `object ZMSGProtocol` — 프로토콜의 핵심 entrypoint
- `ZMSGProtocol.kt:106` — `generateAddressHash(address): String` — SHA-256 → 첫 8 bytes → 16 hex (v4 sender hash)
- `ZMSGProtocol.kt:125` — `generateLegacyAddressHash(address): String` — 6 bytes (12 hex) — v3 legacy 호환용
- `ZMSGProtocol.kt:139` — `generateConversationId(): String` — `SecureRandom`으로 8자 `[A-Z0-9]` 생성
- `ZMSGProtocol.kt:90` — `validateConvId(convId)` — 8자 + charset 검증, 위반 시 `IllegalArgumentException`
- `ZMSGProtocol.kt:151` — `createV4InitMessage(convId, senderAddress, message)` — `ZMSG|v4|<convID>|INIT|<address>|<msg>` 생성
- `ZMSGProtocol.kt:164` — `createV4ReplyMessage(convId, senderAddress, message)` — `ZMSG|v4|<convID>|<hash16>|<msg>` 생성
- `ZMSGProtocol.kt:190` — `createV4KEXMessage(convId, senderAddress, kexPayload)` — `ZMSG|v4|<convID>|KEX|<hash16>|<payload>` 생성
- `ZMSGProtocol.kt:203` — `createV4KEXAckMessage(convId, senderAddress, kexAckPayload)` — `KEXACK|` 변형
- `ZMSGProtocol.kt:305` — `createV4ADDRMessage(convId, oldSenderAddress, newAddress, signature)` — Identity Regeneration ADDR 메시지
- `ZMSGProtocol.kt:365` — `createChunkedV4InitMessages(convId, senderAddress, message): List<String>` — N개 청크 분할 (INIT first + CONT continuations)
- `ZMSGProtocol.kt:401` — `createChunkedV4ReplyMessages(convId, senderAddress, message): List<String>` — Reply 변형
- `ZMSGProtocol.kt:434` — `calculateV4ChunkCount(message, isInitMessage): Int` — UTF-8 byte 기준 청크 개수 계산
- `ZMSGProtocol.kt:212` / `:224` / `:319` — `isKEXMessage` / `isKEXAckMessage` / `isADDRMessage` — control-plane 판별
- `ZMSGProtocol.kt:239` / `:267` / `:334` — `parseKEXMessage` / `parseKEXAckMessage` / `parseADDRMessage`
- `ZMSGProtocol.kt:524` — `parseMemo(memo, addressCache): ParsedMessage` — **모든 incoming memo의 단일 진입점**. 파싱 순서 = GROUP → V4 → V3_INIT → V3_REF → V3_RPL → V3_HASH → V2 → PLAIN
- `ZMSGProtocol.kt:619` — `parseV4Message(memo, addressCache)` — v4 single-shot 파싱
- `ZMSGProtocol.kt:1058` — `reassembleChunks(memos, addressCache): ParsedMessage?` — **same-tx의 모든 chunked memos를 정렬·검증·연결**
- `ZMSGProtocol.kt:1162` / `:1274` — `parseChunkInfo` / `parseV4ChunkInfo` — 청크 헤더 파싱
- `ZMSGProtocol.kt:69` — `substringByBytes(str, startIndex, maxBytes)` — UTF-8 multi-byte char를 깨지 않는 byte-aware substring
- `ZMSGProtocol.kt:495` — `createRefMessage(senderAddress, message, lastReceivedTxId)` — v3 REF format (transaction-referenced reply) — diversified address 우회용 v3 patch (v4 conversation ID로 대체됨, 아직 parse만 지원)
- `ZMSGConstants.kt:9` — `object ZMSGConstants` — 모든 prefix / marker / size 상수
  - `MAX_MEMO_SIZE = 512`
  - `CONV_ID_LENGTH = 8`, `CONV_ID_CHARS = "ABCDE...0123456789"`
  - `HASH_LENGTH = 12` (legacy v3, 6 bytes), `HASH_LENGTH_NEW = 16` (v4, 8 bytes)
  - `Prefixes.V4 = "ZMSG|v4|"`, `V4C = "ZMSG|v4c|"`, `V3 = "ZMSG|v3|"`, `V3C`, `V2`, `GROUP = "ZMSG:3.0:GROUP:"`
  - `Markers.INIT = "INIT|"`, `CONT`, `REF`, `REPLY = "RPL|"`, `KEX`, `KEX_ACK`, `ADDR`
  - `ChunkSizes.V4_INIT = 330`, `V4_REPLY_FIRST = 462`, `CONTINUATION = 485`, `V3_INIT = 340`, `V3_REPLY_FIRST = 470`
  - `MAX_CHUNKS = 1000` (참고: 코드는 `ZMSGProtocol.kt:63`에 별도 정의)
  - `REMOTE_KILL_PREFIX = "ZCHAT_DESTROY:"`
  - `PLATFORM_FEE_ADDRESS = "u1pm2ju3z..."` (verbatim 178자 unified address, 모든 송신에 output 1개로 추가됨 — §1.5에서 검증)
- `ZMSGSpecialMessages.kt:17` — `object ZMSGSpecialMessages` — ZREACT/ZRCPT/ZSTAT/ZTL/ZUNLOCK/ZREQ 의 create + parse
  - `ZMSGSpecialMessages.kt:44` — `createReaction(targetTxId, emoji, senderAddress)` → `ZREACT|<txid>|<emoji>|<hash>`
  - `ZMSGSpecialMessages.kt:86` — `createReadReceipt(targetTxId, senderAddress)` → `ZRCPT|<txid>|<hash>`
  - `ZMSGSpecialMessages.kt:170` — `createScheduledMessage(message, senderAddress, unlockTimestamp)` → `ZTL|SCH|<ts>|<hash>|<msg>`
  - `ZMSGSpecialMessages.kt:185` — `createBlockLockedMessage(..., unlockHeight)` → `ZTL|BLK|<height>|<hash>|<msg>`
  - `ZMSGSpecialMessages.kt:200` — `createPaymentLockedMessage(..., requiredZatoshi)` → `ZTL|PAY|<zatoshi>|<hash>|<msg>`
  - `ZMSGSpecialMessages.kt:216` — `createConditionalMessage(..., answer, hint)` → `ZTL|CND|<answerHash>|<hint>|<hash>|<msg>` — answer 자체는 SHA-256(`generateAddressHash`)으로 hash됨
  - `ZMSGSpecialMessages.kt:382` — `verifyConditionalAnswer(answer, answerHash)` — `answer.lowercase().trim()` 후 hash 비교
  - `ZMSGSpecialMessages.kt:409` — `createPaymentRequest(amountZatoshi, senderAddress, reason)` → `ZREQ|<amount>|<hash>|<reason>` (`require(amountZatoshi > 0)` 검증)
- `ChatMessage.kt:72` — `data class ChatMessage` — 디코딩된 메시지의 메모리 모델 (txId, text, timestamp, isOutgoing, peerAddress, isPending, status, replyToId/replyToPreview, reactions, timeLock, paymentRequest, fileHash/fileZfileContent/fileBlurhash)
- `ChatMessage.kt:61` — `enum MessageStatus { SENDING, SENT, CONFIRMED, READ, FAILED }` — UI delivery indicator
- `ChatMessage.kt:283` — `data class Conversation` — peer별 메시지 묶음 + `e2eEnabled`, `e2eKeyExchangeComplete`, `isMuted`, `draft` 등 UI 상태

## 연결 (Wiring)

- **Inputs:**
  - Outgoing: `(senderAddress: String, peerAddress: String, plaintext: String, optional: lastReceivedTxId, replyToTxId)` from `ChatViewModel.doSendMessage` (§1.6)
  - Outgoing (E2E mode): plaintext가 이미 `E2EMessageProcessor.encryptOutgoing()`을 통과한 `"E2E1:<base64>"` ciphertext로 들어옴 (§1.3)
  - Incoming: `memoText: String` (Zcash 노트 복호화 후 UTF-8 텍스트) + same-tx 의 형제 memos 리스트 from SDK `Synchronizer.transactions` Flow
- **Outputs:**
  - Outgoing: `List<String>` of 1 ~ MAX_CHUNKS memo strings, 각각 ≤ 512 bytes UTF-8 (`createChunkedV4InitMessages` / `createChunkedV4ReplyMessages` / `createChunkedRefMessages` / 또는 single-shot create*Message)
  - Incoming: `ParsedMessage` data class (sender info + message body + control-plane fields like `conversationId`, `replyToTxId`, `messageType`, group fields) — `parseMemo` 또는 `reassembleChunks`로부터
- **Dependencies (internal):**
  - [§1.2 KEX + E2E 암호화](./02-kex-e2e-encryption.md) — KEX payload는 E2EEncryption이 생성, ZMSG는 wire format envelope만 담당
  - [§1.3 Double Ratchet](./03-double-ratchet.md) — message body가 ratchet 통과 후 `E2E1:` prefix ciphertext가 되어 들어옴
  - [§1.4 그룹 메시징](./04-group-messaging.md) — `parseMemo`의 GROUP branch가 `ZMSGGroupProtocol.parseGroupId / parseMessageType / parsePayload`로 위임
  - [§1.5 ZIP-321 트랜잭션 청킹](./05-zip321-tx-chunking.md) — chunked memos list가 multi-output proposal로 변환되는 곳
  - [§1.6 송신/수신 흐름](./06-send-receive-flow.md) — `ChatViewModel`이 create/parse 함수의 직접 caller
  - [§1.7 컨택트 + 주소 캐시](./07-contact-book-address-cache.md) — `AddressCacheImpl`이 sender hash → address 조회를 담당 (parseMemo의 `addressCache` 파라미터)
- **Dependencies (external):**
  - `java.security.MessageDigest` (SHA-256) — `generateAddressHash`, `generateLegacyAddressHash`, `verifyConditionalAnswer`
  - `java.security.SecureRandom` — `generateConversationId`
  - `Charsets.UTF_8` — byte-aware substring과 length 계산 (`substringByBytes`, `byteLen`)
  - `android.util.Log` — 디버그 / 경고 로깅 (`ZCHAT_PROTO`, `ZCHAT_THREADING`, `ZMSG` 태그)

## 라이브러리 (Libraries)

| Package | Version | Used for |
|---------|---------|----------|
| Kotlin stdlib | 2.1.10 (per README) | data class, `substring`, `joinToString`, `MessageDigest`, `SecureRandom` |
| `android.util.Log` | (Android SDK API 27+) | 파싱 실패 / 보안 의심 메시지 디버그 로깅 |

> ZMSG layer 자체는 **외부 의존이 거의 없다.** 순수 Kotlin + JDK crypto 만 사용한다. Zcash SDK / lightwalletd 의존은 §1.5 (ZIP-321 proposal 빌더)와 §1.6 (Synchronizer)에서 처음 등장.

## 워크스루 — happy path

아래 시나리오: Alice가 Bob에게 처음 메시지 "Hello, this is a 600-byte test message …" 를 보낸다고 가정. KEX는 별도 트랜잭션으로 사전에 완료되어 Ratchet이 활성화된 상태. UTF-8 환산 600B이므로 INIT 청크 1개 + CONT 청크 1개로 분할된다.

**1. ChatViewModel → ZMSG 진입 — `ChatViewModel.kt:2495` (§1.6)**

E2E 처리 후 ciphertext "E2E1:<base64>"를 받아서 `CreateChunkedMessageProposalUseCase`로 전달. 그 안에서 ZMSG 함수가 호출된다.

**2. convID 결정 — `ZchatPreferences.getConvId(peer) ?: ZMSGProtocol.generateConversationId()` (§1.7)**

처음 보내는 경우 `peer_convid_<peerAddress>` 키 미존재. `generateConversationId()`가 8자 `[A-Z0-9]` 무작위 ID(예: `ABC12345`)를 만들고 bidirectional 저장.

```kotlin
// ZMSGProtocol.kt:139
fun generateConversationId(): String {
    val random = java.security.SecureRandom()
    return (1..CONV_ID_LENGTH)
        .map { CONV_ID_CHARS[random.nextInt(CONV_ID_CHARS.length)] }
        .joinToString("")
}
```

`SecureRandom` 사용으로 36^8 ≈ 2.8조 조합 공간. 단일 사용자 기준 collision 실용적 0.

**3. INIT 여부 결정**

`peer_convid_<peerAddress>` 가 *방금* 생성됐으므로 첫 메시지 = INIT 형식. 그 이후는 Reply 형식.

**4. 청크 개수 계산 — `calculateV4ChunkCount(message, isInitMessage = true)` — `ZMSGProtocol.kt:434`**

```kotlin
// ZMSGProtocol.kt:434
fun calculateV4ChunkCount(message: String, isInitMessage: Boolean): Int {
    val firstChunkSize = if (isInitMessage) CHUNK_SIZE_V4_INIT else CHUNK_SIZE_V4_REPLY_FIRST
    val msgBytes = byteLen(message)
    if (msgBytes <= firstChunkSize) return 1
    var remaining = msgBytes - firstChunkSize
    var chunks = 1
    while (remaining > 0) {
        chunks++
        remaining -= CHUNK_SIZE_CONTINUATION
    }
    return chunks
}
```

600B 메시지 → first INIT chunk = 330B (`CHUNK_SIZE_V4_INIT`) 소비 → 잔여 270B → CONT chunk 1개로 충분 (485B 한도) → 총 2 청크.

**5. 청크 생성 — `createChunkedV4InitMessages(convId, senderAddress, message)` — `ZMSGProtocol.kt:365`**

```kotlin
// ZMSGProtocol.kt:382
val memo = if (i == 1) {
    "${PREFIX_V4C}$i/$totalChunks|$convId|$INIT_MARKER$senderAddress|$messagePart"
} else {
    "${PREFIX_V4C}$i/$totalChunks|$CONT_MARKER$messagePart"
}
```

각 청크의 wire:
- chunk 1: `ZMSG|v4c|1/2|ABC12345|INIT|u1alice...|E2E1:<first-330B>`
- chunk 2: `ZMSG|v4c|2/2|CONT|<remaining-bytes>`

UTF-8 인코딩 시 한국어 / 이모지 같은 multi-byte char가 chunk boundary에 걸치지 않도록 `substringByBytes`(line 69)가 char 단위 백트래킹.

```kotlin
// ZMSGProtocol.kt:69
private fun substringByBytes(str: String, startIndex: Int, maxBytes: Int): String {
    var byteCount = 0
    var endIndex = startIndex
    while (endIndex < str.length) {
        val charBytes = str[endIndex].toString().toByteArray(Charsets.UTF_8).size
        if (byteCount + charBytes > maxBytes) break
        byteCount += charBytes
        endIndex++
    }
    return str.substring(startIndex, endIndex)
}
```

**6. 결과 — `List<String>` 2개**

`CreateChunkedMessageProposalUseCase` (§1.5)가 이를 받아 ZIP-321 multi-output URI로 변환하여 `Synchronizer.proposeSend`/`proposeTransferFromUri`로 넘긴다. 그 시점부터는 ZMSG layer가 더 이상 관여하지 않는다.

---

수신 측 (Bob의 디바이스):

**7. SDK Synchronizer가 새 transaction 발견 — `ChatViewModel`이 `transactions` Flow 구독 (§1.6)**

새 tx 하나가 두 개의 output(같은 Bob 주소, 다른 memo)을 가진다. 각 output의 `memoText`가 추출되어 ZMSG layer로.

**8. 단일 메모 시도 — `parseMemo(memo, addressCache)` — `ZMSGProtocol.kt:524`**

```kotlin
// ZMSGProtocol.kt:526
when {
    memo.startsWith(ZMSGConstants.Prefixes.GROUP) -> { ... }
    memo.startsWith(PREFIX_V4) -> { branch = "V4"; parseV4Message(...) }
    memo.startsWith("$PREFIX_V3$INIT_MARKER") -> { ... }
    memo.startsWith("$PREFIX_V3$REF_MARKER") -> { ... }
    memo.startsWith("$PREFIX_V3$REPLY_MARKER") -> { ... }
    memo.startsWith(PREFIX_V3) -> { ... }
    memo.startsWith(PREFIX_V2) -> { ... }
    else -> { branch = "PLAIN"; ParsedMessage(...isUnknownSender=true, NOT_ZMSG_FORMAT) }
}
```

청크 1은 `ZMSG|v4c|1/2|...` 로 시작하므로 `PREFIX_V4` (`ZMSG|v4|`)에 prefix match가 *되지 않는다*. 즉 chunked는 단일-memo `parseMemo`로는 처리 안 되고 `reassembleChunks`로 가야 한다.

**9. 청크 재조립 — `reassembleChunks(memos, addressCache)` — `ZMSGProtocol.kt:1058`**

```kotlin
// ZMSGProtocol.kt:1062
val chunkedMemos = memos.filter { it.startsWith(PREFIX_V3C) || it.startsWith(PREFIX_V4C) }
```

같은 tx의 모든 memo를 받아 v3c / v4c chunked만 필터. 그 다음:
- `parseChunkInfo`(line 1162) 또는 `parseV4ChunkInfo`(line 1274)로 각 청크의 `(index, total, isInit, senderInfo, messagePart, convId, refTxId)` 추출
- `chunks.sortedBy { it.index }` — index 정렬
- 검증: `chunks.size != totalChunks` → null, `(1..totalChunks).toSet()` 와 실제 indices 불일치 → null (non-contiguous chunks)
- 첫 청크에서 sender 정보 추출 (INIT면 full address + `addressCache.cacheAddress(hash, address)`, reply면 hash → `addressCache.getAddress(hash)`)
- `chunks.joinToString("") { it.messagePart }` 로 본문 합침

결과는 single `ParsedMessage`. INIT 청크에서 Bob의 `AddressCacheImpl`에 Alice의 hash→address 매핑이 추가되어 이후 Reply 메시지는 hash로 바로 sender 식별 가능.

**10. ChatViewModel에서 후처리**

`ParsedMessage.message` = "E2E1:<base64>". `ChatViewModel`이 `E2EMessageProcessor.decryptIncoming("E2E1:<base64>")` (§1.3) 호출 → plaintext 추출 → `ChatMessage` 객체 생성 → state flow → `ChatDetailView` 렌더링.

## 노트 / quirks / footguns

- **1730 lines 단일 파일은 알려진 technical debt** (claude.md v2.9.1 audit). `ZMSGSpecialMessages.kt`로 일부 분리됐으나 `ZMSGProtocol.kt`는 여전히 거대하며 backward-compat parser, special types delegation, chunking, KEX/ADDR control plane이 한 object에 혼재. 우리 팀 포팅 시 v4-only + 청크 / 단일메모 분리 / control-plane 분리를 권장.
- **v3 `REF` 포맷은 deprecated되었지만 parser는 살아있다.** `createReplyMessage(senderAddress, message)`(v3 hash-only)는 `@deprecated` 주석으로 표시되지만 함수는 살아있고, parsing 우선순위(`parseMemo`)에서 여전히 V3_REF / V3_RPL / V3_HASH 분기를 갖는다. 우리 팀은 v4-only로 시작해도 안전.
- **`generateAddressHash`는 hex chars `'0'..'9' || 'a'..'f'`(lowercase)만 받는다.** `parseV4Message`(line 683-684, 1342)와 `parseV4ChunkInfo`(line 1342)의 hash 길이 검증이 lowercase 검증을 동시에 해서, 대문자 hex hash를 만들면 reply로 분류되지 않고 legacy reply format으로 fallback. (실제로는 `%02x` 포맷이 lowercase여서 정상)
- **`generateConversationId`는 송신자가 단독으로 결정한다.** Bob 쪽은 INIT 메시지를 받았을 때 비로소 같은 convID를 자신의 prefs(`conv_<convId>` → Alice 주소)에 캐싱(`addressCache.cacheAddress` 옆에서 ChatViewModel이 수행). 즉 **convID 동기화는 first INIT의 한 방향 통보로 이뤄진다** — Alice가 두 디바이스로 같은 시드를 쓰면 두 디바이스가 서로 다른 convID로 INIT을 보낼 수 있어 분기 발생. multi-device 미지원의 protocol-level 이유 중 하나 (Q5 / Q14와 매칭).
- **INIT 메시지에는 `senderAddress` 가 full plaintext로 들어간다.** Zcash 노트 암호화로 receiver만 볼 수 있지만, **E2E 본문 암호화 시점에는 sender address가 ratchet ciphertext 외부에 있다.** 즉 Bob의 receiver IVK가 leak되면, 과거 모든 ZMSG INIT 메시지에서 Alice의 unified address가 cleartext로 노출된다. Reply는 16-byte hash로만 노출 — 그러나 Alice의 address가 한 번 캐시되면 collision 외엔 reconstruct 가능.
- **Sender hash 충돌 공간:**
  - v4: 16 hex (8 bytes) = 64-bit collision resistance, birthday paradox로 ~2^32 sender 등록 시 1 collision 기대
  - v3 legacy: 12 hex (6 bytes) = 48-bit, birthday ~2^24 ≈ 16M sender — 실용적으로 위험
  - v3 hash 길이는 DEC-006으로 6→8 bytes 확장된 이력 (`ZMSGConstants.kt:28-36` 주석에 명시) — 우리 팀은 v4 (=v3+8B) 기준만 다루면 됨
- **`MAX_CHUNKS = 1000`은 DoS 보호.** 단일 메시지가 ~480KB까지 가능하지만, memory exhaustion attack 방지 상한. 우리 팀 포팅 시 이 상수의 *근거*(평균 mobile RAM, 동시 reassembly 버퍼 수)를 다시 평가 권장.
- **Conditional time-lock 답 검증의 정규화:** `verifyConditionalAnswer`는 `answer.lowercase().trim()` 후 hash 비교. 따라서 "Yes" / "yes" / " YES " 가 같게 취급되지만 영문 외 언어 normalization은 없음. 한국어 답을 쓰면 NFC/NFD 정규화 문제 가능.
- **`PLATFORM_FEE_ADDRESS` 가 protocol constants에 hardcoded.** 178자 unified address가 코드 상수. 모든 송신 transaction에 platform fee output 1개가 추가됨이 spec(ZMSG_PROTOCOL_SPEC §Transaction Structure)에 명시 — §1.5에서 코드 검증 필요. 우리 팀 포팅 시 *이 fee 정책을 그대로 수용할지* 의사결정 필요.
- **`ZCHAT_DESTROY:<phrase>` remote kill 은 plaintext.** `REMOTE_KILL_PREFIX = "ZCHAT_DESTROY:"`(`ZMSGConstants.kt:121`)가 그대로 memo에 들어가면 발동. 같은 phrase가 누구나 작성 가능 (sender authentication 없음). claude.md v2.9.1 audit "Known Technical Debt"에 "Remote kill phrase could be encrypted (currently plaintext)"로 명시됨. 우리 팀이 메시지 KEX-only 패턴으로 가면 이 기능은 NOSTR side channel 또는 별도 control 채널로 옮기는 게 자연스러움.
- **`parseMemo` 가 GROUP 분기를 가장 먼저 검사한다 (line 528).** 즉 group prefix `ZMSG:3.0:GROUP:`가 1:1 v4 prefix `ZMSG|v4|`보다 우선. `ZMSG|v4|` 자체가 group prefix와 disjoint하므로 실제 충돌은 없음.
- **`createRefInitMessage`(line 509) 같은 v3 hybrid 함수가 남아있지만 새 코드에서는 v4를 써야 한다.** parser 호환을 위해 남은 deprecated 출구.

## 답한 open question (Open questions answered for this subsystem)

- **Q1** (research-plan §7): "ZMSG v4의 conversation ID는 양쪽 당사자에게 어떻게 동기화되는가?"
  > **Answer:** 송신자가 `generateConversationId()`로 단독 생성하여 INIT 메시지의 wire format에 포함시키고(`ZMSG|v4|<convID>|INIT|...`), 수신자가 INIT을 받으면 ChatViewModel이 `ZchatPreferences`에 `conv_<convID>` → peerAddress와 `peer_convid_<peerAddress>` → convID 양방향 키로 저장한다 (§1.7). 즉 한방향 통보 모델이고, multi-device로 같은 시드를 쓰면 분기 가능. — `ZMSGProtocol.kt:139, 151`

- **Q2** (research-plan §7): "INIT 메시지에 full sender address가 들어가는데, Zcash diversified address와 어떻게 호환되는가?"
  > **Answer:** INIT은 `senderAddress`를 cleartext로 포함하지만, 이 주소는 *송신자가 결정한 하나의 diversified address*이고 그것이 곧 conversation의 identity가 된다. 같은 사용자가 매번 다른 diversified로 INIT을 보내면 convID는 같더라도 receiver의 contact book에 다른 entry로 잡힐 수 있으며, 이를 해결하려고 v4 패턴 + ADDR 메시지(`createV4ADDRMessage`, line 305)가 도입됨 — 사용자가 의도적으로 주소를 회전할 때 사용. 우연히 다른 diversified로 보내는 경우의 자동 인식 메커니즘은 없다. — `ZMSGProtocol.kt:305, 619`

- (부분) **Q11/Q12** (research-plan §7): chunking 일관성과 수신측 재조립
  > **Answer (partial):** chunked memos는 동일 tx의 outputs에 분산되고, 수신측은 `reassembleChunks(memos, addressCache)`(line 1058)가 same-tx 형제 memo 전체를 받아 정렬·검증·연결한다. 검증 항목: ① total chunk count 일치, ② index가 `(1..total)`을 정확히 채움 (gap도 중복도 허용 안 함), ③ first chunk가 isInit / hash / convID 헤더를 가짐. 단일 atomic tx 보장은 §1.5에서 결정. — `ZMSGProtocol.kt:1058-1156`

- (부분) **Q23** (research-plan §7): "Time-locked messages 실제 구현"
  > **Answer:** `ZMSGSpecialMessages.kt`에 SCH/BLK/PAY/CND 4종 모두 wire-format + create + parse 구현. CND의 answer는 `generateAddressHash(answer.lowercase().trim())`로 hash되어 hint와 함께 송신되며, 해제는 `ZUNLOCK|CND|<txid>|<answer>|<hash>` 메시지로 이뤄짐. 단, *실제 잠금 해제 시 UI 측 검증과 상태 머신*은 §1.6에서 확인 필요. — `ZMSGSpecialMessages.kt:170-377`

- (부분) **C46 / C42** (claims-to-verify): hash collision 공간
  > **Answer:** v4 = 16 hex (8 bytes, ~2^64), v3 legacy = 12 hex (6 bytes, ~2^48). 코드에 명시 (`ZMSGProtocol.kt:106-129`). 노트는 위 "footguns" 항목 참조.

- (부분) **C95** (claims-to-verify): parsing priority 9단계
  > **Answer:** 코드의 실제 우선순위 = GROUP → V4 (chunked는 별도 reassemble 경로) → V3_INIT → V3_REF → V3_RPL → V3_HASH → V2 → PLAIN. spec과 일치. ZMSG_PROTOCOL_SPEC §Parsing Priority의 8단계 "Special types"는 `parseMemo` 본체 외부 — `ChatViewModel`이 parseMemo 호출 *전* 또는 *후* 에 `ZMSGSpecialMessages.is*` 검사를 한다 (§1.6에서 확인). — `ZMSGProtocol.kt:524-588`
