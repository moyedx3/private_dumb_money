# §2 Category-E (x402 + Zcash) reference extraction

## 2.1 What "x402 + Zcash" means in this codebase vs. Secure Legion / NLx402

(filled at Task 11)

## 2.2 The exact 402 → quote → MPC sign → execute call sequence

(filled at Task 11)

## 2.3 Lift-and-use vs redo

(filled at Task 11)

## 2.4 Differentiation room for our team

(filled at Task 11)

## Background reading: x402 facilitator landscape

### x402 프로토콜 한 단락 정의

x402는 HTTP 402 "Payment Required" 상태 코드를 이용해 서버가 클라이언트에게 결제를 자동으로 요구하고 클라이언트(또는 AI 에이전트)가 서명된 결제 proof를 포함하여 재요청하는 **인터넷 네이티브 결제 표준**이다. 원래 Coinbase가 제안했으나 현재는 [x402 Foundation](https://github.com/x402-foundation/x402)으로 이관된 오픈 스탠다드로, "결제를 완전히 HTTP 계층 위에서 처리하여 account, session, API key 없이 stablecoin 결제를 가능하게 한다"는 것이 핵심 목표다. 프로토콜 명세는 [x402-specification-v2.md](https://github.com/x402-foundation/x402/blob/main/specs/x402-specification-v2.md)에서 관리되며, v1(2025-08 출시)과 v2(2025-12 개정) 두 버전이 존재하고 PAL의 코드는 v1 필드명(`maxAmountRequired`, `X-PAYMENT` 헤더)을 사용한다.

---

### 메시지 포맷

#### `PaymentRequired` (서버 → 클라이언트, HTTP 402 응답)

서버는 HTTP 402 응답과 함께 `PAYMENT-REQUIRED` 헤더에 아래 JSON을 **base64 인코딩**하여 전달한다 (v2 기준; v1에서는 동일 JSON이 `X-PAYMENT-REQUIRED` 헤더로 전달됨).

```json
{
  "x402Version": 2,
  "error": "PAYMENT-SIGNATURE header is required",
  "resource": {
    "url": "https://api.example.com/premium-data",
    "description": "Access to premium market data",
    "mimeType": "application/json"
  },
  "accepts": [
    {
      "scheme": "exact",
      "network": "eip155:84532",
      "amount": "10000",
      "asset": "0x036CbD53842c5426634e7929541eC2318f3dCF7e",
      "payTo": "0x209693Bc6afc0C5328bA36FaF03C514EF312287C",
      "maxTimeoutSeconds": 60,
      "extra": {
        "name": "USDC",
        "version": "2"
      }
    }
  ]
}
```

`accepts` 배열 안의 `PaymentRequirements` 오브젝트 주요 필드 (v2 명칭 기준):

| 필드 | 타입 | 설명 |
|------|------|------|
| `scheme` | string | 결제 scheme (`"exact"`, `"upto"` 등) |
| `network` | string | CAIP-2 체인 식별자 (예: `"eip155:8453"` = Base mainnet) |
| `amount` | string | 결제 금액 (atomic unit; v1에서는 `maxAmountRequired`) |
| `asset` | string | ERC-20 컨트랙트 주소 또는 ISO 4217 통화 코드 |
| `payTo` | string | 수신자 지갑 주소 |
| `maxTimeoutSeconds` | number | 결제 완료 허용 최대 시간 (초) |
| `extra` | object | scheme별 추가 정보 (EVM `exact`의 경우 `name`, `version`) |
| `resource` | string | (v1에서는 `PaymentRequirements` 내부 필드였으나 v2에서는 상위 `resource` 오브젝트로 분리) |

**v1 vs v2 필드명 차이:** v1은 `network`가 문자열 이름(`"base-sepolia"`)이고 `PaymentRequirements` 내부에 `resource`, `description`, `mimeType`이 포함됐다. v2는 `network`가 CAIP-2 형식(`"eip155:84532"`)이고 이 필드들이 상위 `resource` 오브젝트로 분리됐다. PAL 코드(`lib/chainSig.ts:80-88`)는 v1 필드명(`maxAmountRequired`)을 사용한다.

---

#### `PAYMENT-SIGNATURE` 헤더 (클라이언트 → 서버, `PaymentPayload`)

클라이언트는 `PAYMENT-SIGNATURE` 헤더에 아래 JSON을 **base64 인코딩**하여 전달한다 (v2 기준; v1에서는 `X-PAYMENT` 헤더 사용).

```json
{
  "x402Version": 2,
  "resource": {
    "url": "https://api.example.com/premium-data",
    "description": "Access to premium market data",
    "mimeType": "application/json"
  },
  "accepted": {
    "scheme": "exact",
    "network": "eip155:84532",
    "amount": "10000",
    "asset": "0x036CbD53842c5426634e7929541eC2318f3dCF7e",
    "payTo": "0x209693Bc6afc0C5328bA36FaF03C514EF312287C",
    "maxTimeoutSeconds": 60,
    "extra": { "name": "USDC", "version": "2" }
  },
  "payload": {
    "signature": "0x2d6a7588...571c",
    "authorization": {
      "from": "0x857b06519E91e3A54538791bDbb0E22373e36b66",
      "to": "0x209693Bc6afc0C5328bA36FaF03C514EF312287C",
      "value": "10000",
      "validAfter": "1740672089",
      "validBefore": "1740672154",
      "nonce": "0xf3746613c2d920b5fdabc0856f2aeb2d4f88ee6037b8cc5d04a71a4462f13480"
    }
  }
}
```

**헤더 이름 버전 차이:**
- v1: `X-PAYMENT` (PAL 코드 `app/content/page.tsx:144`가 사용하는 이름)
- v2: `PAYMENT-SIGNATURE`

---

#### `exact` scheme on EVM — EIP-3009 typed data 구조

`payload.authorization`은 EIP-3009 `transferWithAuthorization`의 EIP-712 typed data이다. 명세([x402-specification-v2.md §6.1.1](https://github.com/x402-foundation/x402/blob/main/specs/x402-specification-v2.md))에서 직접 인용:

```javascript
const authorizationTypes = {
  TransferWithAuthorization: [
    { name: "from",        type: "address" },
    { name: "to",          type: "address" },
    { name: "value",       type: "uint256" },
    { name: "validAfter",  type: "uint256" },
    { name: "validBefore", type: "uint256" },
    { name: "nonce",       type: "bytes32" },
  ],
};
```

- `from`: 결제자 지갑 (서명자)
- `to`: 수신자 지갑 (`payTo`와 동일해야 함)
- `value`: 결제 금액 (atomic unit)
- `validAfter` / `validBefore`: authorization 유효 시간 창 (Unix timestamp)
- `nonce`: 32-byte 랜덤값 — EIP-3009 컨트랙트 수준에서 사용 후 소각, replay 방어

Settlement는 facilitator가 `transferWithAuthorization(from, to, value, validAfter, validBefore, nonce, v, r, s)`를 ERC-20 컨트랙트에 직접 호출하여 실행한다. Facilitator는 금액이나 수신자를 변경할 수 없다 — 명세 인용: *"The Facilitator cannot modify the amount or destination. They serve only as the transaction broadcaster."*

또한 EVM에서는 `exact` scheme이 EIP-3009 외에 **Permit2** (Uniswap, proxy 컨트랙트 `0x402085c248EeA27D92E8b30b2C58ed07f9E20001`)와 **ERC-7710** (delegation 기반)도 지원한다.

---

#### `PAYMENT-RESPONSE` 헤더 (서버 → 클라이언트, `SettlementResponse`)

서버는 settlement 완료 후 `PAYMENT-RESPONSE` 헤더에 아래 JSON을 **base64 인코딩**하여 반환한다:

```json
{
  "success": true,
  "transaction": "0x1234567890abcdef...",
  "network": "eip155:84532",
  "payer": "0x857b06519E91e3A54538791bDbb0E22373e36b66"
}
```

| 필드 | 타입 | 설명 |
|------|------|------|
| `success` | boolean | settlement 성공 여부 |
| `transaction` | string | on-chain tx hash |
| `network` | string | CAIP-2 체인 식별자 |
| `payer` | string | 결제자 지갑 주소 |
| `errorReason` | string | 실패 시 오류 이유 |

출처: [x402 HTTP transport spec](https://github.com/x402-foundation/x402/blob/main/specs/transports-v2/http.md)

---

### 알려진 facilitator 구현

| 이름 | 운영자 | 지원 체인 | 지원 asset | 출처 / URL |
|------|--------|-----------|------------|------------|
| Coinbase x402 facilitator | Coinbase / x402 Foundation | Base mainnet, Base Sepolia, Polygon, Arbitrum, World, Solana, Avalanche (+ 추가 예정) | USDC (EIP-3009 & Permit2), EURC, SPL tokens | [https://x402.org/facilitator](https://x402.org/facilitator) — Cloudflare 공식 예제에서 사용 확인 ([Cloudflare x402 docs](https://developers.cloudflare.com/agents/x402/)) |
| PayAI x402 Facilitator | PayAI, Inc. | Solana, Base, Polygon, Avalanche, Sei, SKALE, XLayer, Peaq, IoTeX, KiteAI | Stablecoins + 커스텀 토큰 | [https://facilitator.payai.network](https://facilitator.payai.network) |
| second-state x402-facilitator | Second-State (자체 호스팅 템플릿) | Base, Avalanche, Polygon, Sei, Solana (RPC URL 설정으로 추가 가능) | USDC, USDT | [github.com/second-state/x402-facilitator](https://github.com/second-state/x402-facilitator) (self-hosted, Docker 배포) |
| OpenZeppelin x402 facilitator | OpenZeppelin (Stellar 특화) | Stellar | USDC (Stellar) | [docs.openzeppelin.com/relayer/guides/stellar-x402-facilitator-guide](https://docs.openzeppelin.com/relayer/guides/stellar-x402-facilitator-guide) |
| NLx402 | PCEF (Perkins Coie Entrepreneur Fund, 501(c)(3) nonprofit) | Solana (추정; 공식 문서 없음) | USDC (추정) | 주간 2 참조; [Secure Legion GitHub](https://github.com/Secure-Legion) acknowledgments에서 "NLx402 payment protocol core logic, attributed to PCEF" 확인 — 독립 문서 미공개 |

**NLx402에 대한 별도 설명:** NLx402는 Secure Legion의 messaging app ([Secure-Legion/android](https://github.com/Secure-Legion/android))의 acknowledgments 섹션에서 "PCEF (Perkins Coie Entrepreneur Fund, 501(c)(3))가 개발한 payment protocol core logic"으로 언급된다. 공식 spec 문서나 독립 GitHub repo는 공개되지 않았으며, Solana mainnet 기반 facilitator로 추정된다. Secure Legion의 구조에서 NLx402는 `NLx402:<quote_hash>` 형태의 memo를 Zcash shielded transaction의 encrypted memo field에 삽입함으로써 Zcash shielded tx가 x402 결제 proof를 동시에 carry하는 방식으로 사용됐다 (week2 §#26 참조). **권위 있는 1차 출처 없음 — week2 reference와 Secure Legion acknowledgment 외 공식 문서 미발견.**

---

### facilitator API 표면

x402 compliant facilitator는 다음 HTTP 엔드포인트를 반드시 노출해야 한다 (출처: [x402-specification-v2.md §7](https://github.com/x402-foundation/x402/blob/main/specs/x402-specification-v2.md)):

| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/verify` | POST | `PaymentPayload` + `PaymentRequirements`를 받아 signature 검증, balance 확인, tx simulation을 수행하고 `{ isValid, payer }` 반환. **blockchain 상태는 변경하지 않음.** |
| `/settle` | POST | `/verify`와 동일한 body를 받아 `transferWithAuthorization`(또는 Permit2/ERC-7710 등 scheme별 함수)을 실제로 blockchain에 broadcast하고 tx hash를 반환. **가스비는 facilitator가 부담.** |
| `/supported` | GET | facilitator가 지원하는 `(scheme, network)` 쌍 목록과 signer 주소를 반환. |

**`/verify` 요청 body 예시:**
```json
{
  "x402Version": 2,
  "paymentPayload": { /* PaymentPayload 전체 */ },
  "paymentRequirements": { /* PaymentRequirements 한 항목 */ }
}
```

**`/verify` 성공 응답:**
```json
{ "isValid": true, "payer": "0x857b..." }
```

**`/settle` 성공 응답:**
```json
{
  "success": true,
  "payer": "0x857b...",
  "transaction": "0x1234...",
  "network": "eip155:84532"
}
```

Resource server는 `/verify` 만 호출하고 자체적으로 settlement를 처리할 수도 있고, 또는 `/verify` 없이 `/settle`만 호출할 수도 있다 — 두 경우 모두 명세가 허용한다.

---

### Privacy implications: settlement asset이 privacy-preserving이면 어떻게 변하는가

x402의 현행 EVM `exact` scheme은 **투명한 ERC-20 토큰** (USDC on Base)을 전제로 설계되어 있다. Settlement asset을 shielded ZEC처럼 privacy-preserving 자산으로 바꾸면 프로토콜의 여러 전제가 근본적으로 달라진다.

#### `payTo` 필드 — shielded address가 되는가?

현행 명세에서 `payTo`는 EVM checksum address(예: `0x209693Bc...`) 또는 Solana public key다. Zcash shielded 결제로 전환하면 `payTo`는 `u1...` 또는 `zs1...` 형식의 Unified Address 또는 Sapling address가 되어야 한다. 이 주소는 EVM 주소와 형식이 완전히 다르므로 기존 facilitator의 주소 파싱·검증 로직이 호환되지 않는다. 또한 shielded address는 수신자가 자발적으로 disclosure key를 공개하지 않는 한 on-chain에서 잔액 조회 자체가 불가능하다.

#### facilitator의 `/verify` 능력 — shielded balance를 검증할 수 있는가?

현행 EVM `exact` scheme에서 `/verify`는 (1) signature ecrecover, (2) `balanceOf(from)` 조회, (3) `transferWithAuthorization` simulation 세 단계를 수행한다. Zcash shielded pool에서는 이 세 단계 모두 작동하지 않는다:

- **Signature ecrecover 불가:** Zcash shielded tx의 서명은 EIP-712 구조가 아니라 Sapling/Orchard spending key에서 파생된 재편증명(spend proof)이다.
- **`balanceOf` 조회 불가:** shielded note는 암호화되어 있어 facilitator가 특정 address의 잔액을 외부에서 조회할 방법이 없다. 잔액 검증은 spending key(또는 incoming viewing key)를 가진 주체만 수행할 수 있다.
- **Simulation 불가:** Zcash 결제는 EVM smart contract call이 아니라 zk-SNARK proof(Sapling/Orchard) 생성과 UTXO 소비를 수반하며, facilitator가 이를 사전 시뮬레이션하는 표준 인터페이스가 없다.

결론적으로, 표준 x402 `/verify` 인터페이스는 shielded asset에 적용되지 않는다 — **facilitator가 검증 역할을 수행하려면 새로운 scheme 정의가 필요하다.**

#### Replay 방어 메커니즘 — nonce vs Zcash nullifier vs memo carry

현행 EVM `exact` scheme의 replay 방어는 EIP-3009 nonce에 의존한다: nonce는 ERC-20 컨트랙트에 사용 후 기록되며, 동일 nonce로 두 번 `transferWithAuthorization`을 호출하면 컨트랙트 수준에서 revert된다 (명세 §10.1 인용: *"EIP-3009 contracts inherently prevent nonce reuse at the smart contract level"*).

Zcash shielded tx에서의 replay 방어 메커니즘은 구조적으로 다르다:

- **Nullifier:** Zcash는 각 shielded note 소비 시 전 세계적으로 유일한 nullifier를 블록체인에 공개적으로 기록하고, 동일 note를 두 번 소비하면 nullifier 중복으로 거부된다. 이는 EIP-3009 nonce와 유사한 역할이지만, 검증 주체가 "ERC-20 컨트랙트"가 아니라 "Zcash 풀 전체"이며 facilitator가 직접 nullifier 집합에 접근하려면 lightwalletd 또는 full node가 필요하다.

- **Memo field carry (NLx402 패턴):** Secure Legion이 제안한 방식은 `NLx402:<quote_hash>`를 Zcash shielded tx의 encrypted memo field에 삽입하는 것이다. 이 경우 Zcash tx 자체가 x402 결제 proof의 carrier가 되며, facilitator는 수신자의 incoming viewing key로 memo를 복호화해 `quote_hash`를 확인함으로써 replay를 방어한다. 그러나 이 방식은 facilitator가 수신자의 incoming viewing key에 접근 가능해야 한다는 전제를 포함하며, 수신자의 개인정보 trade-off를 수반한다.

- **결제 후 proof 제출 (PAL 방식):** PAL은 반대 방향으로 접근한다 — shielded ZEC를 먼저 1Click으로 USDC로 swap한 뒤, USDC transfer의 tx hash를 `X-PAYMENT` 헤더의 bearer로 사용한다. 이 경우 replay 방어는 Base의 USDC 컨트랙트(EIP-3009 nonce)가 담당하고, Zcash privacy는 funding 단계에서만 존재한다. x402 protocol flow와 Zcash privacy가 완전히 분리된 구조다.

세 접근 방식 모두 "Zcash를 x402 settlement asset으로 직접 쓰는" 경우의 문제를 서로 다른 방식으로 회피하고 있다. Zcash를 x402 settlement rail로 직접 통합하는 facilitator — nullifier를 `/verify`의 replay check로 사용하고, shielded tx의 viewing key 기반 검증을 `/settle` 흐름에 통합하는 — 는 현재 공개된 구현 중에서 발견되지 않는다. 이 공백이 우리 팀의 Category-E 차별화 포인트로 남겨진 영역이다 (Task 11에서 구체화).
