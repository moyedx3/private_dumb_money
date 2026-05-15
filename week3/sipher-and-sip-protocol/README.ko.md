# Sipher 와 SIP Protocol

**Repos:** `sip-protocol/sipher` @ `ded380f`, `sip-protocol/sip-protocol` @ `7afc597`  
**로컬 클론:** `/private/tmp/sipher`, `/private/tmp/sip-protocol`

## 결론

SIP Protocol은 privacy SDK / protocol layer이고, Sipher는 그 위에 얹힌 agent-facing API wrapper다.


```text
SIP Protocol = privacy machine
Sipher       = agent가 누르는 remote control
Zcash        = SIP가 zcashd에 연결되면 쓸 수 있는 backend 중 하나
```

SIP는 stealth address, commitment, viewing key, shielded intent라는 말, NEAR/Solana/EVM adapter, Zcash RPC wrapper를 하나의 SDK로 묶으려는 프로젝트다. 새로운 cryptographic breakthrough라기보다는 이미 알려진 privacy primitive들을 developer UX로 포장하는 쪽에 가깝다.

Sipher는 이 도구들을 AI agent가 부르기 쉽게 만든 REST/API surface다. `skill.md`, REST endpoint, OpenAPI SDK, agent integration이 핵심이다. Sipher 자체가 완성된 Zcash payment rail처럼 보이지는 않는다.

## 큰 그림 다이어그램

![Sipher and SIP Protocol overview](./Sipher-SIP-Protocol-overview.svg)

원본: [sipher-and-sip-protocol-big-picture.excalidraw](./sipher-and-sip-protocol-big-picture.excalidraw).

## SIP Protocol이 하는 일

SIP, 즉 Shielded Intents Protocol은 cross-chain transaction에 privacy layer를 씌우겠다는 프로젝트다. README는 이걸 "blockchain intents의 HTTPS"처럼 설명한다.

코드 기준으로 SIP는 monorepo다:

- `packages/sdk`: 핵심 TypeScript SDK.
- `packages/sdk/src/stealth`: chain family별 stealth address 생성.
- `packages/sdk/src/commitment.ts`: Pedersen commitment helper.
- `packages/sdk/src/privacy.ts`: viewing key와 encrypted disclosure helper.
- `packages/sdk/src/zcash`: Zcash RPC, shielded service, swap, bridge wrapper.
- `programs/`, `contracts/`: Solana/EVM privacy contract 실험.

제품 thesis는 대략 이렇다:

```text
normal chain intent
  -> SIP가 stealth recipient + hidden amount + optional audit key를 붙임
  -> adapter가 NEAR / Solana / EVM / Zcash-ish backend로 보냄
```

## SIP는 Zcash를 쓰나?

좁은 의미에서는 쓴다.

SIP에는 실제 Zcash 이름의 SDK 코드가 있다. `ZcashRPCClient`는 `zcashd` JSON-RPC method를 호출한다. 예를 들면 `z_validateaddress`, `z_getnewaccount`, `z_getaddressforaccount`, `z_getbalanceforaccount`, `z_sendmany`, `z_getoperationstatus`, `z_exportviewingkey` 같은 것들이다.

구조는 이렇다:

```text
SIP SDK
  -> ZcashRPCClient
  -> zcashd JSON-RPC
  -> 실제 Zcash wallet/node가 Sapling/Orchard 작업 수행
```

즉 SIP가 Zcash cryptography를 직접 구현하는 건 아니다. 실제 shielded transaction은 `zcashd`에게 위임한다.

약한 부분은 cross-chain에서 Zcash로 들어가는 route다. `ZcashSwapService`와 `ZcashBridge`에는 demo quote, mock price, mock deposit address, fallback mock txid가 있다. production에서 진짜가 되려면 실제 bridge provider와 `ZcashShieldedService`가 연결되어 있어야 한다. 그래서 "ETH/SOL/NEAR -> shielded ZEC"는 live proof가 없으면 scaffold로 보는 게 맞다.

## Sipher가 하는 일

Sipher는 AI agent-facing product surface다.

Agent가 부르기 쉬운 privacy tool을 API로 노출한다:

- stealth address 생성
- one-time payment address derive
- private/shielded transfer artifact 준비
- payment scan
- commitment 생성
- viewing key 생성 / disclosure
- privacy/compliance metadata 확인

중요한 interface는:

- `skill.md`: agent-readable contract.
- Express REST API: runtime surface.
- OpenAPI와 generated SDK.
- Eliza, LangChain 스타일 예시.

쉽게 말하면:

```text
Agent: "private payment 준비해줘"
Sipher: 간단한 API call을 받음
Sipher: SIP SDK primitive를 호출함
Sipher: address, commitment, key, unsigned tx material을 돌려줌
Wallet / chain-specific code가 실제 서명, 제출, settlement 검증을 해야 함
```

## Sipher는 Zcash를 쓰나?

강한 의미에서는 아니다.

Sipher는 `@sip-protocol/sdk`에 의존하지만, 우리가 본 Sipher repo는 주로 SIP의 stealth address, commitment, viewing key, Solana transaction, scan, API wrapper surface를 쓴다. Sipher가 Zcash wallet처럼 동작하거나, Zcash note를 scan하거나, UFVK를 다루거나, `lightwalletd`를 쓰거나, Orchard/Sapling transaction을 만들거나, ZEC settlement를 검증하는 코드는 보이지 않았다.

정리하면:

```text
SIP Protocol: Zcash RPC wrapper가 있음
Sipher: SIP primitive를 agent API로 감싼 것, Zcash payment app은 아님
```

## 둘이 합쳐서 실제로 하는 일

둘이 합쳐서 하려는 일은 privacy payment를 software agent가 쓰기 쉽게 만드는 것이다.

Stack은 이렇게 보면 된다:

```text
AI agent / app
  -> Sipher agent API
  -> SIP SDK
  -> privacy primitive or chain adapter
  -> Solana / EVM / NEAR / zcashd / bridge provider
```

가장 재사용 가치가 있는 건 역할 분리다:

- Sipher는 agent ergonomics를 담당한다: tool name, endpoint taxonomy, API key, rate limit, idempotency, OpenAPI, example.
- SIP는 privacy machinery를 담당한다: stealth generation, commitment, viewing-key encryption, Zcash RPC wrapper, adapter interface.

이 split 자체가 구체 구현보다 더 중요하다.

## Cryptographic Novelty

SIP를 cryptographic breakthrough로 보기는 어렵다.

SIP는 알려진 아이디어를 조합한다:

- stealth address
- Pedersen commitment
- viewing key
- Zcash에서 온 shielded payment language
- roadmap / integration material로서의 ZK proof system
- chain adapter와 payment intent wrapper

기여는 "새로운 수학"이 아니라 "privacy primitive를 developer와 agent가 쓰기 쉽게 만든 UX"에 가깝다.

## 우리에게 주는 의미

쓸만한 점:

- agent-readable API shape.
- 작은 privacy tool taxonomy.
- private key나 plaintext를 만지는 endpoint의 trust level 구분.
- agent wrapper와 privacy SDK의 분리.
- `zcashd` 기반 flow가 필요할 때 참고할 수 있는 Zcash RPC wrapper.

약하거나 위험한 점:

- marketing이 Zcash 완성도를 과장한다.
- Sipher는 Zcash-native settlement를 증명하지 않는다.
- cross-chain-to-ZEC route는 일부 scaffold로 보인다.
- "shielded"라는 말이 실제 Zcash shielded transaction이 아니라 다른 chain의 stealth/commitment wrapper를 뜻할 때가 있다.

우리 프로젝트에 대한 synthesis:

```text
Sipher의 agent API shape는 참고한다.
SIP의 SDK split은 참고한다.
실제 shielded payment settlement는 real Zcash tooling으로 구현한다.
"shielded"라는 말을 쓸 때는 어디서 무엇이 settle되는지 증명한다.
```
