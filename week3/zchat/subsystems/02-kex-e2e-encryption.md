# §1.2 KEX 핸드셰이크 + E2E 암호화

## 목적 (Purpose)

E2EEncryption 서브시스템은 ZMSG 위에 한 겹 더 얹는 **end-to-end 페이로드 암호화 계층**을 담당한다. secp256r1 (NIST P-256) ECDH로 양 당사자가 raw shared secret을 계산하고, HKDF(RFC 5869)로 256-bit AES 키를 도출하여 AES-256-GCM(12B nonce, 128-bit auth tag)으로 메시지를 암호화한다. KEX 핸드셰이크는 ECDSA(SHA256withECDSA) 서명으로 "이 공개키의 진짜 소유자가 이 Zcash 주소다"라는 sender authentication을 추가한다 — Zcash 노트 암호화가 제공하지 못하는 보호. 또한 ECIES(per-recipient 그룹 키 wrap, §1.4), 파일 키 wrapping(§1.8), Quantum Shield PSK(post-quantum용은 아니고 KDF input augmentation)도 같은 layer에서 노출된다.

## 파일과 함수 (Files & functions)

### `ui-lib/.../screen/chat/crypto/E2EEncryption.kt`

- `E2EEncryption.kt:24` — `enum E2EKeyVersion { V1(1), V2(2) }` — 키 도출 버전; V1은 deprecated legacy, V2가 현재
- `E2EEncryption.kt:37` — `object HKDF` — RFC 5869 HKDF (HMAC-SHA256) 구현
  - `:46` — `HKDF.extract(salt, ikm): ByteArray` — `PRK = HMAC-SHA256(salt ?: zeros32, ikm)`
  - `:58` — `HKDF.expand(prk, info, length): ByteArray` — `T(i) = HMAC-SHA256(prk, T(i-1) || info || i)`, 길이 검증 `length <= 255 * 32`
  - `:85` — `HKDF.deriveKey(ikm, salt, info, length): ByteArray` — Extract + Expand 풀 HKDF
- `E2EEncryption.kt:117` — `object E2EEncryption` — E2E 암호화의 단일 진입점
- `E2EEncryption.kt:121-126` — 상수: `KEY_CURVE = "secp256r1"`, `CIPHER_ALGORITHM = "AES/GCM/NoPadding"`, `KEY_SIZE = 256`, `GCM_TAG_LENGTH = 128`, `NONCE_SIZE = 12`, `E2E_PREFIX = "E2E:"`
- `E2EEncryption.kt:129-131` — HKDF V2 파라미터:
  - `HKDF_SALT_V2 = "ZCHAT_E2E_SALT_V2".toByteArray()`
  - `HKDF_INFO = "ZCHAT_E2E_KEY".toByteArray()`
  - `DERIVED_KEY_LENGTH = 32`
- `E2EEncryption.kt:137` — `generateKeyPair(): E2EKeyPair` — secp256r1 KeyPair 생성 (`KeyPairGenerator.getInstance("EC")` + `ECGenParameterSpec("secp256r1")`), Base64 인코딩
- `E2EEncryption.kt:159` — `deriveSharedSecret(ourPrivateKeyB64, peerPublicKeyB64, version, psk?): ByteArray` — ECDH → raw shared secret → `deriveKey()`
- `E2EEncryption.kt:208` — `deriveKeyV1(sharedSecret): ByteArray` — `SHA-256("ZCHAT_E2E_KEY_V1" || sharedSecret)` (legacy)
- `E2EEncryption.kt:226` — `deriveKeyV2(sharedSecret, psk?): ByteArray` — `HKDF(ikm = sharedSecret [|| psk], salt="ZCHAT_E2E_SALT_V2", info="ZCHAT_E2E_KEY", length=32)`
- `E2EEncryption.kt:239` — `getCurrentKeyVersion(): E2EKeyVersion = V2`
- `E2EEncryption.kt:258` — `encrypt(plaintext, sharedKey): String` — AES-256-GCM 암호화 → `"E2E:<nonce_b64>:<ct_b64>"` 형식 반환
- `E2EEncryption.kt:282` — `decrypt(encryptedMessage, sharedKey): String?` — 역방향, null on failure
- `E2EEncryption.kt:338` — `decryptWithResult(...): CryptoResult<String>` — 명시적 에러 타입 버전 (preferred)
- `E2EEncryption.kt:376` — `isE2EEncrypted(message): Boolean` — `"E2E:"` prefix 검사
- `E2EEncryption.kt:413` — `sign(privateKeyB64, message): String` — `Signature.getInstance("SHA256withECDSA")` ECDSA-P256 서명
- `E2EEncryption.kt:436` — `verify(publicKeyB64, message, signatureB64): Boolean` — 검증, 실패 시 false
- `E2EEncryption.kt:466` — `createKEXPayload(senderAddress, publicKey, privateKey): String` — `"KEX:<pubkey>:<sig>"`, 서명 메시지 = `senderAddress || publicKey`
- `E2EEncryption.kt:480` — `parseKEXPayload(payload, senderAddress): String?` — 형식 검사 + 서명 검증 후 publicKey 반환, 검증 실패 시 null
- `E2EEncryption.kt:509` — `createKEXAckPayload(...)` — KEX와 동일 패턴, prefix만 `"KEXACK:"`
- `E2EEncryption.kt:518` — `parseKEXAckPayload(...)` — 동일 검증
- `E2EEncryption.kt:544` — `isKEXPayload(payload): Boolean` — `"KEX:"` or `"KEXACK:"` prefix
- `E2EEncryption.kt:569` — `encryptECIES(recipientPublicKeyB64, plaintext): String` — ephemeral keypair + ECDH + HKDF(salt=null, info="ZCHAT_ECIES_V1") + AES-GCM. 출력 = `"ECIES:<ephemeral_pub>:<nonce>:<ct>"`. 그룹 키 분배(§1.4)에 사용
- `E2EEncryption.kt:623` — `decryptECIES(ourPrivateKeyB64, eciesBlob): ByteArray?`
- `E2EEncryption.kt:749` — `encryptGroupKeyForMember(memberPublicKey, groupKey): String` — ECIES wrapper
- `E2EEncryption.kt:760` — `decryptGroupKeyFromInvite(...)` — 역방향 (§1.4)
- `E2EEncryption.kt:778` — `generateFileKey(): ByteArray` — 32B 랜덤 AES 키 (§1.8 파일 암호화)
- `E2EEncryption.kt:792` — `encryptFile(plaintext, key, aad?): ByteArray` — `[12B IV][ct+tag]` 포맷
- `E2EEncryption.kt:834` — `wrapFileKey(fileKey, sharedSecret, psk?, aad?)` — HKDF로 wrap key 도출 (salt="ZCHAT_FILE_KEY_WRAP", info="WRAP") 후 fileKey를 AES-GCM 암호화
- `E2EEncryption.kt:859` — `unwrapFileKey(...)` — 역방향
- `E2EEncryption.kt:879` — `data class E2EKeyPair(publicKey: String, privateKey: String)` — Base64 인코딩된 keypair

### `ui-lib/.../screen/chat/crypto/QuantumShield.kt`

- `QuantumShield.kt:13` — `object QuantumShield`
- `QuantumShield.kt:15-17` — `QR_PREFIX = "ZCPSK:"`, `SECRET_LENGTH = 32`, `INFO = "zchat-quantum-shield-psk".toByteArray()`
- `QuantumShield.kt:22` — `generateRandom(): ByteArray` — `SecureRandom`으로 32B 시크릿 생성
- `QuantumShield.kt:35` — `derivePSK(secretA, secretB): ByteArray` — **order-independent** mutual PSK: 두 secret을 unsigned lexicographic sort → concat → HKDF(salt=null, info="zchat-quantum-shield-psk", length=32)
- `QuantumShield.kt:50` — `toQRPayload(secret): String` — `"ZCPSK:<base64>"`
- `QuantumShield.kt:59` — `fromQRPayload(payload): ByteArray?` — 길이 32 검증, 실패 시 null
- `QuantumShield.kt:75` — `orderSecrets(a, b): Pair<ByteArray, ByteArray>` — unsigned byte-wise lex 비교

### `ui-lib/.../screen/chat/crypto/QuantumShieldState.kt`

- `QuantumShieldState.kt:6` — `enum QuantumShieldStatus { NONE, PENDING, ACTIVE }`
- `QuantumShieldState.kt:27` — `data class QuantumShieldState(ourSecret, peerSecret, psk)` — immutable 상태 머신
  - `:40` — `generateOurSecret()`: NONE → PENDING
  - `:47` — `addPeerSecret(secret)`: PENDING → ACTIVE (peer's secret 받으면 PSK 자동 도출)
  - `:57` — `reset()`: any → NONE

### `ui-lib/.../screen/chat/datasource/ZchatPreferences.kt` (E2E 관련 부분만)

- `setE2EOurKeys(peer, publicKey, privateKey)` / `getE2EOurKeys(peer)` — peer별 long-term E2E keypair (Base64) 저장
- `setE2EPeerPublicKey(peer, peerPublicKey)` / `getE2EPeerPublicKey(peer)` — KEX 후 peer pub key
- `setQuantumShield(peer, state)` / `getQuantumShield(peer)` — peer별 PSK 상태

> **참고:** ZchatPreferences의 전체 surface는 §1.7에서 다룬다. 본 섹션은 KEX/E2E가 prefs에 *무엇*을 저장하는지만 다룬다.

### `ui-lib/.../screen/chat/viewmodel/ChatViewModel.kt` (참조만)

- `ChatViewModel.kt:1849` — `sendKEXMessage(peerAddress)` — KEX 송신 진입점 (§1.6에서 상세)
- `ChatViewModel.kt:1899` — `sendKEXAckMessage(peerAddress, peerPubKey)` — KEXACK 송신
- `ChatViewModel.kt:1641-1660` — conversation root derivation (KEX/KEXACK 두 txid 포함) (§1.3에서 상세)

## 연결 (Wiring)

- **Inputs:**
  - `senderAddress: String` — 우리 Zcash unified address
  - `peerAddress: String` — 상대방 Zcash unified address
  - `ourPrivateKey: String` (Base64) — `ZchatPreferences.getE2EOurKeys(peer)`로부터
  - `peerPublicKey: String` (Base64) — KEX 수신 후 `ZchatPreferences.getE2EPeerPublicKey(peer)`로부터
  - `psk: ByteArray?` — `QuantumShieldState.psk` (활성 시)
  - `plaintext: String` (메시지) 또는 `groupKey: ByteArray` (그룹 키 wrap) 또는 `fileKey: ByteArray` (파일 키 wrap)
- **Outputs:**
  - `sharedKey: ByteArray` (32B) — 메시지 암호화/복호화에 사용 (Ratchet 미사용 시) / Ratchet root 도출에 사용 (§1.3)
  - `"E2E:<nonce_b64>:<ct_b64>"` — 단발성 암호화 ciphertext (Ratchet 도입 전 legacy; 현재는 Ratchet `"E2E1:..."`을 더 선호)
  - `"KEX:<pubkey>:<sig>"` / `"KEXACK:<pubkey>:<sig>"` — KEX 핸드셰이크 페이로드 (ZMSG envelope로 포장됨, §1.1)
  - `"ECIES:<eph_pub>:<nonce>:<ct>"` — per-recipient 그룹 키 wrap (§1.4)
- **Dependencies (internal):**
  - [§1.1 ZMSG 프로토콜](./01-zmsg-protocol.md) — `createV4KEXMessage`, `createV4KEXAckMessage`가 본 layer의 페이로드를 wire format에 포장
  - [§1.3 Double Ratchet](./03-double-ratchet.md) — `deriveSharedSecret` 결과가 ratchet root key 도출의 IKM 중 하나
  - [§1.4 그룹 메시징](./04-group-messaging.md) — `encryptGroupKeyForMember` / `decryptGroupKeyFromInvite`가 GROUP_INVITE에서 사용
  - [§1.7 컨택트 + Identity](./07-contact-book-address-cache.md) — `ZchatPreferences`가 keypair · PSK 영구 저장
  - [§1.8 NOSTR + 파일공유](./08-nostr-side-channel.md) — `wrapFileKey` / `unwrapFileKey` / `encryptFile`이 첨부파일 암호화에 사용
- **Dependencies (external):**
  - JDK `java.security.*` — `KeyPairGenerator`, `KeyFactory`, `KeyAgreement` (ECDH), `Signature` (ECDSA), `MessageDigest` (SHA-256), `SecureRandom`
  - `java.security.spec.ECGenParameterSpec`, `PKCS8EncodedKeySpec`, `X509EncodedKeySpec`
  - `javax.crypto.*` — `Cipher`, `Mac` (HMAC-SHA256), `KeyGenerator`, `SecretKey`, `SecretKeySpec`, `GCMParameterSpec`
  - `java.util.Base64`

## 라이브러리 (Libraries)

| Package | Version | Used for |
|---------|---------|----------|
| JDK `java.security` | Android API 27+ (Conscrypt provider) | ECDH (secp256r1), ECDSA (SHA256withECDSA), SHA-256, SecureRandom |
| JDK `javax.crypto` | Android API 27+ | AES-256-GCM (`AES/GCM/NoPadding`), HMAC-SHA256, Cipher, KeyAgreement |
| Kotlin stdlib | 2.1.10 | data class, ByteArray ops |
| `androidx.annotation` | (AndroidX) | `@VisibleForTesting` annotation only |

> **Tink는 import되어 있지 않다.** README는 "Encryption: Google Tink + custom ECDH/AES-256-GCM"이라 했지만 E2EEncryption.kt 본체는 JDK 표준 라이브러리만 사용. Tink는 EncryptedSharedPreferences 내부에서만 쓰인다 (`co.electriccoin.zcash.encrypted` 마스터키 보호 — §3.6에서 검증).

## 워크스루 — happy path

### A. KEX 핸드셰이크 (한 peer에 정확히 한 번)

시나리오: Alice가 처음으로 Bob과 E2E 채팅하려 한다. Alice가 KEX를 initiate.

**1. Alice keypair 생성 — `E2EEncryption.generateKeyPair()` (line 137)**

```kotlin
val keyPairGenerator = KeyPairGenerator.getInstance("EC")
val spec = ECGenParameterSpec("secp256r1")  // NIST P-256
keyPairGenerator.initialize(spec, SecureRandom())
val keyPair = keyPairGenerator.generateKeyPair()
// → E2EKeyPair(publicKey = Base64(X.509), privateKey = Base64(PKCS#8))
```

Alice는 `(A_pub, A_priv)`를 `ZchatPreferences.setE2EOurKeys(peer = Bob, A_pub, A_priv)`에 저장. 이는 **peer별 long-term keypair** — Alice는 같은 secp256r1 키페어를 Bob 한 명에게만 사용.

**2. KEX 페이로드 작성 — `createKEXPayload(senderAddress = aliceAddr, A_pub, A_priv)` (line 466)**

```kotlin
fun createKEXPayload(senderAddress: String, publicKey: String, privateKey: String): String {
    val messageToSign = senderAddress + publicKey
    val signature = sign(privateKey, messageToSign)
    return "KEX:$publicKey:$signature"
}
```

여기서 `sign`(line 413)은 `SHA256withECDSA` 사용:

```kotlin
val signature = Signature.getInstance("SHA256withECDSA")
signature.initSign(privateKey)
signature.update(message.toByteArray(Charsets.UTF_8))
val signatureBytes = signature.sign()
```

서명 메시지 = UTF-8 인코딩된 `aliceAddr || A_pub_base64`. ECDSA-P256 raw bytes를 Base64로 인코딩하여 페이로드에 포함.

**3. ZMSG envelope 포장 — `ZMSGProtocol.createV4KEXMessage(convId, aliceAddr, kexPayload)` (§1.1)**

```
ZMSG|v4|ABC12345|KEX|<alice_hash16>|KEX:<A_pub_b64>:<sig_b64>
```

이 문자열이 Zcash transaction memo에 들어가 Bob의 unified address로 송금 (typically 최소 dust amount). Zcash 노트 암호화가 자동으로 적용되어, 네트워크 관찰자는 sender·amount·content를 볼 수 없음. Bob만 노트를 복호화할 수 있음.

**4. Bob 수신 — `ChatViewModel`이 SDK Synchronizer Flow에서 새 tx 발견 (§1.6)**

memo가 KEX prefix를 가지므로 `ZMSGProtocol.isKEXMessage(memo) = true`. Bob의 `ChatViewModel.handleIncomingMemo` 가 `ZMSGProtocol.parseKEXMessage(memo)` → `(convId, kexPayload)` 추출.

**5. 서명 검증 — `parseKEXPayload(kexPayload, senderAddress = aliceAddr)` (line 480)**

```kotlin
val parts = payload.removePrefix("KEX:").split(":", limit = 2)
val publicKey = parts[0]   // A_pub_b64
val signature = parts[1]
val messageToVerify = senderAddress + publicKey  // aliceAddr || A_pub_b64
if (!verify(publicKey, messageToVerify, signature)) return null  // 검증 실패
return publicKey  // 검증 성공 → Alice의 pubkey
```

여기 `verify`(line 436)는 `Signature.getInstance("SHA256withECDSA")`로 검증. 이 단계가 **sender authentication의 핵심**: Alice의 주소를 정말 통제하는 사람만이 그 주소에 묶인 pubkey signature를 만들 수 있음 (왜냐하면 Alice의 Zcash address가 Alice의 ECDH pubkey와 ECDSA로 묶여 있음).

> **단, ECDSA 서명은 *Alice가 E2E private key의 소유자임만* 증명한다.** "그 사람이 Alice 주소를 통제한다"는 *Zcash transaction의 송신*을 통해 implicit하게 증명된다 (왜냐하면 Bob에게 도달한 트랜잭션은 누군가 노트를 spend해야 했음 → 그 someone이 Alice 주소에 입금된 노트를 가졌음). 따라서 KEX의 보안 모델은 "Alice가 본인 주소의 노트를 spend할 수 있다 + 그 같은 사람이 이 ECDH pubkey의 priv key도 가진다"라는 두 사실의 결합이다.

**6. Bob keypair 생성 + KEXACK 송신 — line 509**

Bob도 `generateKeyPair()` → `(B_pub, B_priv)` 생성하여 prefs 저장. `createKEXAckPayload(bobAddr, B_pub, B_priv)` → `"KEXACK:<B_pub>:<sig>"` 생성하여 같은 convId의 ZMSG KEXACK 메시지로 Alice에게 송신.

**7. Alice가 KEXACK 수신 + 서명 검증 — `parseKEXAckPayload` (line 518)**

같은 검증 로직. 검증 성공 시 Alice는 `setE2EPeerPublicKey(peer = Bob, B_pub)`로 prefs에 Bob의 pubkey 저장.

**8. 양쪽이 동일하게 shared secret 도출 — `deriveSharedSecret(...)` (line 159)**

Alice: `ECDH(A_priv, B_pub) = ss`
Bob: `ECDH(B_priv, A_pub) = ss` (수학적으로 동일)

코드:
```kotlin
val keyAgreement = KeyAgreement.getInstance("ECDH")
keyAgreement.init(privateKey)
keyAgreement.doPhase(publicKey, true)
val rawSharedSecret = keyAgreement.generateSecret()
return deriveKey(rawSharedSecret, version, psk)
```

`deriveKey` (line 197)는 V2 (현재 default)을 호출하여 `deriveKeyV2`(line 226):

```kotlin
private fun deriveKeyV2(sharedSecret: ByteArray, psk: ByteArray? = null): ByteArray {
    val ikm = if (psk != null) sharedSecret + psk else sharedSecret
    return HKDF.deriveKey(
        ikm = ikm,
        salt = HKDF_SALT_V2,   // "ZCHAT_E2E_SALT_V2".toByteArray()
        info = HKDF_INFO,      // "ZCHAT_E2E_KEY".toByteArray()
        length = DERIVED_KEY_LENGTH  // 32
    )
}
```

PSK가 활성이면 (Quantum Shield ACTIVE 상태) `ikm = sharedSecret || psk` (단순 concat). 그렇지 않으면 그대로. 두 당사자가 같은 PSK를 가지면 같은 derived key 도출 — order-independent (B.4 참조).

**9. Ratchet 초기화 (§1.3)**

도출된 32B key가 Ratchet root key의 입력. 추가로 `ChatViewModel.kt:1641-1660`에서 conversation root에 **양쪽 KEX/KEXACK 두 txid를 mix**하여 같은 두 사람의 다른 대화가 다른 root을 갖게 함 (MITM에 대한 추가 binding).

### B. Quantum Shield PSK 교환

시나리오: Alice·Bob이 추가 양자 저항 layer 원함. KEX 완료 *후* 또는 *전* 어느 때라도 가능.

**1. Alice가 secret 생성 — `QuantumShieldState.generateOurSecret()` → `QuantumShield.generateRandom()`**

```kotlin
val bytes = ByteArray(32)
SecureRandom().nextBytes(bytes)
return bytes
```

상태: NONE → PENDING. Alice의 secret이 `ourSecret`에 저장.

**2. QR 코드 표시 — `QuantumShield.toQRPayload(secret)` → `"ZCPSK:<base64-32B>"`**

Alice가 QR를 화면에 띄우고, Bob이 같은 식으로 자기 secret을 생성 + QR로 표시. 두 사람이 *서로의* QR을 스캔.

**3. Alice가 Bob의 secret 받음 — `addPeerSecret(bobSecret)` (line 47)**

```kotlin
fun addPeerSecret(secret: ByteArray): QuantumShieldState {
    val newState = copy(peerSecret = secret)
    return if (newState.ourSecret != null) {
        newState.copy(psk = QuantumShield.derivePSK(newState.ourSecret, secret))
    } else newState
}
```

상태: PENDING → ACTIVE.

**4. PSK 도출 — `QuantumShield.derivePSK(aliceSecret, bobSecret)` (line 35)**

```kotlin
fun derivePSK(secretA, secretB): ByteArray {
    val (first, second) = orderSecrets(secretA, secretB)  // 정렬해서 순서 무관
    val ikm = first + second  // 64B IKM
    return HKDF.deriveKey(
        ikm = ikm,
        salt = null,
        info = "zchat-quantum-shield-psk".toByteArray(),
        length = 32
    )
}
```

`orderSecrets`(line 75)은 unsigned byte-wise lexicographic sort: 두 ByteArray의 첫 다른 byte를 unsigned로 비교, 작은 쪽을 first로. 따라서 `derivePSK(a, b) == derivePSK(b, a)` 보장 — Alice·Bob 누구의 디바이스에서도 같은 PSK 도출.

**5. 이후 모든 KEX/`deriveSharedSecret` 호출에 PSK 전달**

`deriveSharedSecret(ourPriv, peerPub, version = V2, psk = currentPSK)` → derived key가 PSK를 반영. KEX 자체는 그대로지만, **shared secret을 PSK와 mix**하여 ECDH가 깨져도 PSK가 비밀이면 키가 안전. 반대로 PSK가 leak되어도 ECDH가 안전하면 키가 안전.

### C. 단발성 메시지 암호화 (Ratchet 도입 전 legacy 경로)

`encrypt(plaintext, sharedKey)` (line 258):

```kotlin
val nonce = ByteArray(12).also { SecureRandom().nextBytes(it) }
val cipher = Cipher.getInstance("AES/GCM/NoPadding")
cipher.init(ENCRYPT_MODE, SecretKeySpec(sharedKey, "AES"), GCMParameterSpec(128, nonce))
val ciphertext = cipher.doFinal(plaintext.toByteArray(UTF_8))
return "E2E:${Base64(nonce)}:${Base64(ciphertext)}"
```

**중요:** 이 경로는 forward secrecy 없음 — 같은 sharedKey가 모든 메시지에 재사용됨. 같은 nonce가 두 번 쓰이면 AES-GCM 보안 깨짐 (catastrophic). nonce가 매번 random 12B로 생성되지만 birthday는 2^48 메시지 — 한 conversation에서 도달 어려움. 그러나 신중을 기해 **현재는 Ratchet (`E2E1:` prefix)이 우선 사용**되고 본 layer는 backward-compat / 그룹 키 wrap / 파일 wrap에만 남아있다. ZMSG_PROTOCOL_SPEC.md 5.3 prefix 표 참조.

## 노트 / quirks / footguns

- **KEX의 서명은 "주소를 통제한다"를 직접 증명하지 않는다.** 서명은 *E2E private key 소유*를 증명하고, 그 키를 *주소에 결합*하는 건 ECDSA `message = senderAddress || publicKey`라는 *binding* 일 뿐이다. 진짜 "주소 소유"는 Zcash transaction의 입금 spending에서 implicit하게 증명된다. 따라서 sender가 trusted Zcash transaction을 한 번도 보낸 적 없는 채로 KEX만 보내면 (예: KEX를 다른 트랜잭션의 memo로 piggyback) sender authentication이 약해질 수 있음 — 다만 zchat 코드 흐름에서 KEX는 항상 별도 transaction이므로 in-practice는 문제 없음.
- **`getCurrentKeyVersion() = V2`지만 V1 도출 코드도 살아있다.** legacy peer 와의 호환을 위해 `deriveSharedSecret(version = V1)` 호출이 어딘가에 남아있을 가능성. ChatViewModel 또는 prefs에서 peer별 stored version 분기가 있는지 §1.6에서 검증 필요.
- **"Quantum Shield"는 marketing name.** 실제 cryptographic property는 "symmetric KDF input augmentation" — PSK가 secret으로 유지되면 ECDH가 깨져도 derived key가 안전 (그 반대도 성립). 이건 PQ KEM이 아니다. 진짜 양자 저항은 ML-KEM-768 hybrid가 필요하며 ZMSG_PROTOCOL_SPEC.md가 "Real PQ milestone"으로 deferred 라고 명시. 우리 팀이 같은 layer를 들고 갈 거면 *이름을 정직하게* (예: "Out-of-band PSK augmentation") 변경 권장.
- **PSK 교환은 두 당사자 모두 QR을 보여주고 서로 스캔하는 4-step 의식이다.** UI 흐름이 복잡해질 수 있음 — 두 사람이 물리적으로 같이 있어야 함 (보통 첫 회의). 잃어버리면 reset → 다시 시작. 우리 팀 차별화로 이걸 더 간소화할 여지(QR 1회 + 자동 키 교환?).
- **Long-term keypair는 peer별 분리.** Alice는 Bob과 Carol에게 *다른* secp256r1 keypair를 쓴다. 이는 cross-conversation linkability를 막는 좋은 디자인이지만 prefs 저장 비용 증가. 그룹에선 ECIES wrap이 동일 long-term key를 쓰므로 다소 약화 (§1.4 참조).
- **V1 (legacy SHA-256) derivation은 보안적으로 weak.** `SHA-256("ZCHAT_E2E_KEY_V1" || sharedSecret)`는 raw ECDH output을 직접 hash — extract step 없음. 코드 주석 자체가 "weak"라고 인정. v1 peer 가 살아있다면 그 conversation은 audit 대상.
- **ECIES (그룹 키 wrap)는 HKDF salt가 `null`이다.** `encryptECIES`(line 569)는 `HKDF.deriveKey(ikm = rawSharedSecret, salt = null, info = "ZCHAT_ECIES_V1", length = 32)` — null salt는 HKDF spec상 zero-byte salt와 동등하지만 "no salt" 자체로 약점 신호. 주석 `// TODO: ECIES V2 should add a proper salt. Current V1 uses null salt for backward compat.`로 인지됨.
- **File key wrap의 salt/info는 별도.** `wrapFileKey`(line 834)는 `salt="ZCHAT_FILE_KEY_WRAP"`, `info="WRAP"` 사용. 같은 shared secret에서 *다른* 용도(메시지 / 그룹 키 / 파일 키)별로 *다른* derived key를 도출하는 정직한 KDF separation — design good.
- **`decrypt` (line 282) 는 silently null을 반환한다.** 잘못된 prefix / nonce 길이 / 키 길이 / AEAD tag 검증 실패 모두 null. 디버그 로그는 남기지만 caller가 에러 타입을 알 수 없음. `decryptWithResult`(line 338, preferred)는 `CryptoResult<String>`로 에러 enumerable. 우리 팀 포팅 시 후자만 노출 권장.
- **Base64 인코딩은 standard (`+/=`).** URL-safe (`-_`)가 아니다. ZMSG memo 안에서 `|`나 `:`가 segment 구분자라 Base64 표준은 안전하지만, URL/header context에서 재사용 시 URL-safe로 변환 필요.

## 답한 open question

- **Q3** (research-plan §7): "KEX 메시지의 ECDSA signature는 어떤 알고리즘?"
  > **Answer:** `SHA256withECDSA` over secp256r1 (NIST P-256). 서명 메시지 = UTF-8(`senderAddress || publicKeyBase64`), 즉 단순 문자열 concat. 서명 자체는 JCA 표준 ECDSA-DER encoded bytes를 Base64로 wrap. — `E2EEncryption.kt:413, 466`

- **Q4** (research-plan §7, partial): "Sender authentication"
  > **Answer (partial):** Protocol-level sender authentication은 KEX 단계의 ECDSA 서명 + Zcash 트랜잭션의 spending proof 두 가지의 결합으로 제공된다. Plain ZMSG REPLY 메시지는 hash routing만 사용 — 즉 KEX가 한 번이라도 성공한 conversation에 한해서만 sender authentication이 보장된다. KEX 전 첫 메시지(INIT plaintext)는 sender authentication 없음. — `E2EEncryption.kt:466-502` + `ZMSG_PROTOCOL_SPEC.md` Security Model

- **Q6** (research-plan §7): "Quantum Shield PSK는 어떻게 QR로 교환?"
  > **Answer:** 양쪽이 각각 `QuantumShield.generateRandom()`으로 32B 시크릿 생성 → `toQRPayload(secret)` = `"ZCPSK:<base64-32B>"` 형식 QR을 화면에 띄움 → 서로 스캔 → `addPeerSecret(scanned)` → `QuantumShieldState`가 PENDING → ACTIVE로 전이하며 PSK가 *order-independent* (unsigned lex sort 후 concat) HKDF로 자동 도출됨. UI 흐름은 §1.7에서 추가 검증. PSK를 잃어버리면 `reset()` 후 처음부터 다시 — Quantum Shield 없는 기본 ECDH로는 fallback. — `QuantumShield.kt:22-86`, `QuantumShieldState.kt:40-57`

- **Q7** (research-plan §7): "E2EKeyVersion.V1 (legacy SHA-256 only)이 코드에 살아있는가? 언제 fallback?"
  > **Answer:** 살아있다. `E2EKeyVersion.fromValue(value)`(line 29)가 알 수 없는 value 시 `V1` fallback. `deriveSharedSecret`의 `version` 파라미터가 V2를 default로 받지만 호출자(`ChatViewModel`)가 명시적으로 V1을 패스하면 SHA-256-only 도출이 됨. Peer별 stored version 분기 메커니즘이 prefs에 있는지(예: `setE2EKeyVersion(peer, version)`)는 §1.6/§1.7에서 추가 검증. claude.md / spec에는 V1이 어떤 조건에서 활성화되는지 명시 없음 → 잠재적 footgun. — `E2EEncryption.kt:24-30, 197-212`

- **C70~C73, C74** (claims-to-verify): 암호화 파라미터 검증
  > **Answer:** 모두 코드와 일치.
  > - secp256r1 (NIST P-256): `KEY_CURVE = "secp256r1"` (line 121)
  > - HKDF V2 salt/info: `"ZCHAT_E2E_SALT_V2"` / `"ZCHAT_E2E_KEY"` (line 129-130)
  > - AES-256-GCM: `KEY_SIZE = 256`, `GCM_TAG_LENGTH = 128`, `NONCE_SIZE = 12` (line 123-125)
  > - V1 legacy: `SHA-256("ZCHAT_E2E_KEY_V1" || sharedSecret)` (line 208-212)
  > - KEX payload: `"KEX:<pubkey>:<sig>"`, signed = `senderAddress || publicKey` (line 466-471)

- **C75~C77** (claims-to-verify): Quantum Shield
  > **Answer:**
  > - 32B 랜덤: `SECRET_LENGTH = 32` (`QuantumShield.kt:16`)
  > - PSK mix: `ikm = sharedSecret + psk` (코드 확인 — concat 순서는 sharedSecret 먼저, PSK 뒤; `E2EEncryption.kt:227`)
  > - PQ KEM 아님: 코드 주석에 명시되어 있지 않으나 결과적으로 KDF input augmentation일 뿐 — week2 메모의 "양자 저항 시도지만 실제 PQ는 아니다"가 정확. — `E2EEncryption.kt:226-234`, `QuantumShield.kt`

- **C100~C141** (claims-to-verify): E2E1: vs E2E: prefix 분리
  > **Answer:** `E2E:` prefix는 본 layer(legacy non-ratchet, line 126)가 생성·소비. `E2E1:` prefix는 §1.3 Ratchet wrapper(`CiphertextWireFormat`)가 생성·소비. ChatViewModel은 수신 시 *prefix 검사로 분기*하여 어느 layer로 routing 할지 결정 (§1.6에서 검증).
