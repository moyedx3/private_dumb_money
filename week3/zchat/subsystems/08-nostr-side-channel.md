# §1.8 NOSTR 보조채널 + 파일공유 (Blossom / NIP-96)

## 목적 (Purpose)

NOSTR 통합은 **메시지 transport가 아니라 file upload 인증 채널**이다. 큰 첨부파일(이미지/PDF/ZIP)은 512B Zcash memo로 chunking 가능하지만 비용·지연 문제로 비현실적이므로, zchat은 *파일 본체*를 NIP-96 또는 Blossom 서버(외부, 다중 fallback)에 AES-256-GCM 암호화하여 업로드하고, *URL + SHA-256 hash + ECIES wrapped file key*만 `ZFILE|` wire format으로 ZMSG memo에 담는다. NOSTR identity는 BIP-44 path `m/44'/1237'/0'/0/0` (secp256k1 + Schnorr BIP-340)에서 파생되어 Zcash UA와 *같은 BIP-39 seed*에서 함께 도출된다 — week2 메모의 dual derivation claim과 일치. 따라서 NOSTR이 "보조"라는 표현은 정확하며, *메시지 자체*는 100% Zcash memo만 사용한다.

## 파일과 함수 (Files & functions)

### `ui-lib/.../nostr/NOSTRIdentity.kt` (333 lines)

- `:21` — `class NOSTRIdentity private constructor(privateKey: ByteArray, publicKey: ByteArray, npub: String)`
- `:32` — `signNIP98Event(url, method): String` — NIP-98 HTTP Auth (kind 27235), Schnorr 서명, Base64 wrap된 JSON event 반환 → `Authorization: Nostr <eventBase64>` 헤더
- `:70` — `signBlossomAuthEvent(sha256Hex, sizeBytes): String` — Blossom auth (kind 24242), expiration = +600초, tags `[t=upload, x=sha256, size, expiration]`
- `:101-122` companion 상수:
  - `NIP98_KIND = 27235`, `BLOSSOM_AUTH_KIND = 24242`
  - `BLOSSOM_AUTH_EXPIRY_SECONDS = 600L`
  - `DERIVATION_PATH = m/44'/1237'/0'/0/0` (purpose'/coin_type'/account'/change/index, hardened 표시)
- `:127` — `fromSeed(seed: ByteArray): NOSTRIdentity` — BIP-32 derivation + x-only pubkey (32B, BIP-340) + Bech32 `npub` encoding
- `:146` — `deriveKey(seed)` — BIP-32 HMAC-SHA512 master key + path segment 5단계 derivation (hardened/normal 분기). `Secp256k1.privKeyTweakAdd` 로 child key 계산
- `:186` — `compressPublicKey(uncompressed): ByteArray` — 65B uncompressed → 33B compressed (BIP-340)
- `:234` — `bech32Encode(hrp, data): String` — BIP-173 Bech32 (`npub1...`)
- `:299` — `convertBits(data, fromBits, toBits, pad)` — 8-bit → 5-bit conversion for Bech32

### `ui-lib/.../nostr/FileUploadClient.kt` (21 lines)

```kotlin
sealed class UploadOutcome {
    data class Success(val url: String, val sha256: String) : UploadOutcome()
    data class Failure(val error: String, val serverUrl: String) : UploadOutcome()
}
interface FileUploadClient {
    suspend fun upload(data: ByteArray, mimeType: String, identity: NOSTRIdentity): UploadOutcome
}
```

### `ui-lib/.../nostr/BlossomClient.kt` (78 lines)

- `:23` — `class BlossomClient(serverUrl, httpClientProvider) : FileUploadClient`
- `:29` — `upload(data, mimeType, identity): UploadOutcome` — PUT `${serverUrl}/upload`, body = raw bytes, `Authorization: Nostr <blossomAuthBase64>`, `Content-Type: <mimeType>`
- `:57` — `parseBlossomResponse(json, sha256Hex)` — JSON `{url, sha256, size, type}` 파싱; URL 없으면 `$serverUrl/$sha256Hex` fallback

### `ui-lib/.../nostr/NIP96Client.kt` (101 lines)

- `:22` — `class NIP96Client(serverUrl, httpClientProvider) : FileUploadClient`
- `:28` — `upload(data, mimeType, identity): UploadOutcome` — POST `${serverUrl}/api/v2/media`, multipart `formData(file)`, `Authorization: Nostr <nip98AuthBase64>`
- `:66` — `parseNip96Response(json, data)` — `nip94Event.tags` 안에서 `["url", "..."]` 추출. URL 없으면 Failure

### `ui-lib/.../nostr/FileUploadManager.kt` (59 lines)

- `:12` — `class FileUploadManager(identity, httpClientProvider)`
- `:16-24` 하드코딩된 서버 목록:
  - `nip96Servers = ["https://nostr.build", "https://void.cat"]`
  - `blossomServers = ["https://blossom.band", "https://blossom.nostr.build"]`
- `:33` — `upload(data, mimeType): UploadOutcome` — **다단 fallback**: NIP-96 servers 우선, 실패 시 Blossom servers
- `:50` companion `sha256Hex(data): String`

### `ui-lib/.../screen/chat/model/ZFILEMessage.kt` (65 lines)

- `:3` — `enum ZFILEType(code, mimeType)`:
  - JPEG=`j`, PNG=`p`, GIF=`g`, WEBP=`w`, PDF=`d`, ZIP=`z`, TXT=`t`
- `:19` — `data class ZFILEMessage(hash, type, size, url, wrappedKey, blurhash)`
- `:27` — `serialize(): String = "ZFILE|$hash|$type.code|$size|$url|$wrappedKey|$blurhash"`
- `:49` — `parse(raw): ZFILEMessage?` — `ZFILE|` prefix 검증 + 7개 part split
- `:47` — companion `isFileMessage(content): Boolean` — `"ZFILE|"` prefix 검사

### `ui-lib/.../screen/chat/filesharing/` (7 파일)

- `SecureHash.kt:20` — `object SecureHash` — **PBKDF2WithHmacSHA256, 600,000 iterations** (OWASP 2023), 16B random salt, 256-bit output. Format: `"pbkdf2:<iter>:<salt_hex>:<hash_hex>"` + legacy plain SHA-256 backward-compat. Constant-time compare. **filesharing 디렉토리에 있지만 Destroy PIN / Remote kill phrase가 이걸로 hash 됨** (claude.md v2.9.1 audit이 plain SHA-256 → PBKDF2 업그레이드)
- `FileIntegrityCheck.kt:1` — file SHA-256 검증 (다운로드 시 hash matching)
- `FileDownloadCache.kt` — 다운로드된 파일 로컬 캐시 (path → bytes 또는 file URI)
- `BitmapSampling.kt` — 이미지 미리보기 thumbnail down-sampling (메모리 효율)
- `BlurhashDecoder.kt` — Blurhash → low-res placeholder bitmap (파일 download 진행 중 표시)
- `QuantumShieldScanBridge.kt` — Quantum Shield system 과 파일 scanning bridge (정확한 사용처는 추가 확인 필요)
- `UploadProgressTracker.kt:1` — `progress: StateFlow<Float?>` — UI에서 업로드 진행률 표시

## 연결 (Wiring)

- **Inputs:**
  - File bytes + mimeType (사용자가 첨부파일 선택)
  - `NOSTRIdentity` (BIP-39 seed에서 derive, 앱 초기화 시 1회)
  - ECDH shared secret + optional PSK (peer 와의 §1.2 KEX 완료 후) — wrap file key 도출용
- **Outputs:**
  - `UploadOutcome.Success(url, sha256)` — Blossom/NIP-96 서버의 HTTP URL
  - `ZFILEMessage` serialized → ZMSG memo로 송신 (§1.5)
  - 수신 시: 다운로드 + 복호화된 파일 bytes → `FileDownloadCache` → Compose UI 표시
- **Dependencies (internal):**
  - [§1.2 KEX + E2E](./02-kex-e2e-encryption.md) — `E2EEncryption.generateFileKey`, `encryptFile`, `wrapFileKey`, `unwrapFileKey`, `decryptFile`
  - [§1.1 ZMSG](./01-zmsg-protocol.md) — ZFILE은 special message type (parsing priority에서 plain 직전)
  - [§1.6 송수신 흐름](./06-send-receive-flow.md) — `ChatViewModel`이 ZFILE 메시지 감지 시 별도 파일 다운로드 + 표시 흐름 진입
  - [§1.7 컨택트 + Identity](./07-contact-book-address-cache.md) — Destroy PIN / Remote kill phrase가 `SecureHash`로 hash됨 (cross-cut)
- **Dependencies (external):**
  - `fr.acinq.secp256k1.Secp256k1` — KMP secp256k1 wrapper (Bitcoin/NOSTR 표준)
  - `io.ktor.client.*` — HTTP client (PUT/POST + multipart + auth header)
  - `kotlinx.serialization.json` — Blossom/NIP-96 응답 파싱
  - `javax.crypto.{Mac, SecretKeyFactory, PBEKeySpec}` — HMAC-SHA512, PBKDF2
  - 외부 서버: nostr.build, void.cat, blossom.band, blossom.nostr.build (HTTP 외부 dependencies)

## 라이브러리 (Libraries)

| Package | Version | Used for |
|---------|---------|----------|
| `fr.acinq.secp256k1` | (acinq KMP) | secp256k1 + Schnorr BIP-340 + privKeyTweakAdd (BIP-32 derivation) |
| `io.ktor:ktor-client-*` | (Ktor 2.x) | HTTP client + multipart form + auth header |
| `kotlinx.serialization.json` | (Kotlin 2.1.10) | Blossom/NIP-96 JSON response 파싱 |
| `javax.crypto` | API 27+ | PBKDF2WithHmacSHA256, HMAC-SHA512, AES-GCM (file encrypt — §1.2 위임) |

## 워크스루 — happy path

### A. 파일 송신 — Alice가 이미지 첨부 (1MB JPEG)

**1. 사용자 파일 선택 + ChatViewModel 호출**

(UI 코드는 본 dive scope 외 — Compose attachment picker)

**2. File key 생성 — `E2EEncryption.generateFileKey()` (§1.2)**

```kotlin
val fileKey = E2EEncryption.generateFileKey()  // 32B random AES-256
```

**3. 파일 암호화 — `E2EEncryption.encryptFile(plaintext, fileKey)` (§1.2)**

```kotlin
val encryptedBytes = E2EEncryption.encryptFile(fileBytes, fileKey)
// Output: [12B IV][AES-256-GCM(plaintext) + 16B tag]
```

**4. SHA-256 hash 계산 — `FileUploadManager.sha256Hex(encryptedBytes)`**

```kotlin
val sha256Hex = FileUploadManager.sha256Hex(encryptedBytes)  // 64 hex chars
```

**5. NIP-96 server 우선 시도 — `FileUploadManager.upload(encryptedBytes, "image/jpeg")` (line 33)**

```kotlin
for (serverUrl in nip96Servers) {  // nostr.build, void.cat
    val result = NIP96Client(serverUrl, httpClientProvider).upload(data, mimeType, identity)
    if (result is UploadOutcome.Success) return result
}
for (serverUrl in blossomServers) {  // blossom.band, blossom.nostr.build
    val result = BlossomClient(serverUrl, httpClientProvider).upload(data, mimeType, identity)
    if (result is UploadOutcome.Success) return result
}
return UploadOutcome.Failure(...)
```

**6. NIP-96 upload — `NIP96Client.upload(data, mimeType, identity)` (line 28)**

```kotlin
val uploadUrl = "$serverUrl/api/v2/media"
val authHeader = identity.signNIP98Event(uploadUrl, "POST")  // kind 27235 Schnorr base64
val response = client.submitFormWithBinaryData(
    url = uploadUrl,
    formData = formData { append("file", data, ContentType + ContentDisposition) }
) {
    header(HttpHeaders.Authorization, "Nostr $authHeader")
}.body()
val url = response.nip94Event?.tags?.firstOrNull { it[0] == "url" }?.get(1)
```

→ 서버가 NIP-98 auth 검증(Schnorr signature, kind 27235, url+method 매치) 후 파일 저장. response의 NIP-94 file metadata event에서 URL 추출.

**7. File key wrap — `E2EEncryption.wrapFileKey(fileKey, sharedSecret, psk?)` (§1.2)**

```kotlin
val wrappedKey = E2EEncryption.wrapFileKey(
    fileKey = fileKey,
    sharedSecret = E2EEncryption.deriveSharedSecret(ourPriv, peerPub, V2, psk),
    aad = null
)
// Internal: HKDF(ikm=ss+psk, salt="ZCHAT_FILE_KEY_WRAP", info="WRAP", 32) → wrapKey
//          encryptFile(fileKey, wrapKey) → [12B IV][ct+tag]
```

> **주의:** wrap key는 *long-term* ECDH shared secret에서 도출 — Ratchet message key가 아님. 즉 파일 키 wrap에는 forward secrecy 없음. peer의 long-term E2E priv key가 leak되면 과거 모든 첨부파일 복호화 가능. 메시지 본문(Ratchet)보다 약한 보안.

**8. ZFILE wire format 생성 + ZMSG 송신**

```kotlin
val blurhash = computeBlurhash(thumbnail)  // 이미지 미리보기용
val zfileMsg = ZFILEMessage(
    hash = sha256Hex,
    type = ZFILEType.JPEG,
    size = encryptedBytes.size.toLong(),
    url = serverReturnedUrl,
    wrappedKey = Base64(wrappedKey),
    blurhash = blurhash
).serialize()
// "ZFILE|<hash>|j|<size>|<url>|<wrappedKey>|<blurhash>"
```

이 문자열이 § 1.5 `createChunkedMessageProposal(rawMemo = true)`로 들어가 Zcash transaction memo로 송신. memo 안에 *파일 자체*는 없음 — URL + hash + key + blurhash 만.

### B. 파일 수신 — Bob 디바이스

**1. ChatViewModel이 ZMSG 메시지 분기에서 `ZFILEMessage.isFileMessage(decryptedContent)` 검출**

**2. `ZFILEMessage.parse(raw)` (line 49) → `ZFILEMessage` 객체**

**3. UI에 *Blurhash placeholder* 표시 — `BlurhashDecoder.decode(blurhash)` → 저해상도 이미지**

**4. 백그라운드 다운로드:**
```kotlin
val encryptedBytes = HttpClient.get(zfileMsg.url).body()
val computed = FileUploadManager.sha256Hex(encryptedBytes)
if (computed != zfileMsg.hash) throw IntegrityException()  // FileIntegrityCheck
```

**5. File key unwrap — `E2EEncryption.unwrapFileKey(wrappedKey, sharedSecret, psk?)`**

```kotlin
val fileKey = E2EEncryption.unwrapFileKey(
    wrapped = Base64.decode(zfileMsg.wrappedKey),
    sharedSecret = E2EEncryption.deriveSharedSecret(ourPriv, peerPub, V2, psk)
)
```

**6. 파일 복호화 — `E2EEncryption.decryptFile(encryptedBytes, fileKey)`**

**7. 캐시 + UI 표시 — `FileDownloadCache.put(hash, bytes)` → Compose Image / ViewerActivity**

### C. NOSTRIdentity 도출 (앱 초기화)

**1. PersistableWallet에서 mnemonic 로드 → `Mnemonics.toSeed()` → 64B seed**

**2. `NOSTRIdentity.fromSeed(seed)` (line 127)**

```kotlin
fun fromSeed(seed: ByteArray): NOSTRIdentity {
    val (privateKey, _) = deriveKey(seed)  // BIP-32 path m/44'/1237'/0'/0/0
    val pubkey65 = Secp256k1.pubkeyCreate(privateKey)
    val xOnlyPubkey = pubkey65.copyOfRange(1, 33)  // BIP-340 x-only (32B)
    val npub = bech32Encode("npub", xOnlyPubkey)
    return NOSTRIdentity(privateKey, xOnlyPubkey, npub)
}
```

**3. BIP-32 derivation 단계 (line 146):**

```
master_hmac = HMAC-SHA512("Bitcoin seed", seed)
key0 = master_hmac[0..32], chain0 = master_hmac[32..64]

for index in [44|HARDENED, 1237|HARDENED, 0|HARDENED, 0, 0]:
    if hardened:
        data = 0x00 || key || index_big_endian
    else:
        data = compressedPubkey(key) || index_big_endian
    hmac = HMAC-SHA512(chain, data)
    key = privKeyTweakAdd(key, hmac[0..32])
    chain = hmac[32..64]
```

**4. Bech32 encode** — `bech32Encode("npub", xOnlyPubkey)` → `npub1...` 56자 문자열

## 노트 / quirks / footguns

- **NOSTR은 메시지 transport가 아니다.** week2 메모의 "NOSTR을 보조에 둔다"가 정확. NOSTR 의 역할 = (1) BIP-340 Schnorr signing identity (BIP-44 path 1237'), (2) NIP-98 / Blossom auth header로 파일 업로드 서버 인증. 메시지 자체는 Zcash memo only.
- **dual derivation 검증됨** (week2 메모와 일치): m/44'/133' (Zcash UA) + m/44'/1237' (NOSTR secp256k1) — 둘 다 *같은 BIP-39 seed*에서 도출. seed restore 시 두 identity 모두 동일 복원.
- **secp256k1 vs secp256r1 두 곡선 공존.** 1:1 KEX는 secp256r1 (NIST P-256, ECDH + ECDSA SHA256), NOSTR identity는 secp256k1 (Bitcoin/NOSTR 표준, Schnorr BIP-340). E2EEncryption.kt와 NOSTRIdentity.kt가 *다른* JCA provider 사용 — Conscrypt vs Secp256k1-KMP. 우리 팀 포팅 시 cryptographic agility 측면에서 일관성 검토.
- **외부 server 의존.** 파일은 4개 서버(nostr.build, void.cat, blossom.band, blossom.nostr.build) 중 하나에 저장. 사용자가 자체 서버를 등록할 UI 옵션 없음 (코드 하드코딩). server 운영자는 ciphertext만 볼 수 있지만 *URL을 publish*하므로 access pattern (누가 어떤 파일 다운로드 하는지) 관찰 가능.
- **파일 키 wrap은 long-term ECDH secret 사용.** Ratchet message key 가 아님. 즉 파일 첨부는 forward secrecy *없음*. long-term E2E priv key leak 시 모든 과거 첨부파일 복호화 가능. 우리 팀 차별화 후보: 파일 키를 ratchet message key로 wrap.
- **PBKDF2 600,000 iterations**. OWASP 2023 권장치 그대로. Android 디바이스에서 ~300ms per verify — UX 측면에서 PIN 확인 시 약간 지연. brute force GPU cracking 대비. SecureHash가 `filesharing/` 하위에 있는 게 namespace 측면에서 어색 (실제로는 prefs / Destroy PIN / Remote kill phrase에 쓰임).
- **Bech32 + secp256k1-KMP는 NOSTR이 표준이지만 Zcash 측에서 안 쓰는 도구.** Zcash UA는 ZIP-32 + Sapling/Orchard PRF 기반이라 다른 path. 우리 팀이 NOSTR을 메시지 transport로 끌어올린다면 (week2 가설), 이 BIP-44 derivation은 그대로 가져가서 NIP-04 / NIP-17 (Sealed Sender)로 확장 가능.
- **Blossom server 캐시 만료.** NOSTR auth event의 `expiration` = 600초. 서버는 600초 이내 upload만 허용. 일부 server는 시간 지난 파일을 자동 GC — 미디어 영구성 보장 없음. 우리 팀이 *영구성*이 필요한 첨부파일 (예: 채팅 history backup)에 NOSTR을 쓰면 broken link 위험.
- **`isFileMessage` 검사가 ZMSG parsing priority의 어디에 들어가는지 확인 필요.** `ZFILEMessage.isFileMessage` 가 ChatViewModel에서 호출되지만 ZMSGSpecialMessages.kt에는 ZFILE이 *없다*. 즉 ZFILE은 parsing priority의 standalone special types 외에 별도 path로 처리될 가능성 — code 추가 확인 필요 (§1.6 ChatViewModel 분기).

## 답한 open question

- **Q19** (research-plan §7): "NOSTR이 정말 보조채널인가? 메인 메시지 transport에는 영향을 주지 않는가? BlossomClient.kt, NIP96Client.kt 역할?"
  > **Answer:** **✓ 정확히 보조채널.** NOSTR은 *file upload 인증*에만 사용된다. BlossomClient는 BUD-02 PUT 업로드, NIP96Client는 NIP-96 multipart POST, 모두 NOSTRIdentity로 서명한 NIP-98 / Blossom auth header (kind 27235 / 24242) 와 Schnorr 서명을 동반. 파일 본체 외 *메시지·KEX·그룹 등 control plane*은 Zcash memo only. week2 메모와 정확히 일치. — `NOSTRIdentity.kt:32-99`, `BlossomClient.kt:29-55`, `NIP96Client.kt:28-64`, `FileUploadManager.kt:33-48`

- **Q20** (research-plan §7): "NOSTR Identity와 Zcash identity는 같은 BIP-39 seed에서 파생되는가?"
  > **Answer:** **✓ Yes.** `NOSTRIdentity.fromSeed(seed)`(line 127)가 같은 BIP-39 seed의 64 bytes를 받아 BIP-32 derivation으로 `m/44'/1237'/0'/0/0` 도출. Zcash UA는 별도로 ZIP-32 path m/32'/<coin>'/<account>' (§3.2)에서 도출. **같은 seed, 다른 curve, 다른 derivation 표준** — week2 메모의 "m/44'/133' Zcash UA + m/44'/1237' NOSTR secp256k1 동시 파생" 정확. 다만 본 dive에서 검증된 path는 NOSTR만 — Zcash 측 path 정확 검증은 §3.2 (Layer B `ZashiSpendingKeyDataSource`에서). — `NOSTRIdentity.kt:115-181`

- **C25** (claims-to-verify): BIP44 dual derivation
  > **Answer:** ✓ NOSTR 측 검증됨. m/44'/1237'/0'/0/0 정확. Zcash 측은 §3.2에서 추가 검증.
