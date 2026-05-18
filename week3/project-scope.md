# Project Scope — "Unlockable Drop" on Zcash

- **Status**: Scope decision (post-week-3 sync)
- **Builds on**: [`회의록_week3.md`](./회의록_week3.md), [`pay-anyone-legend/`](./pay-anyone-legend/), [`zchat/`](./zchat/)
- **Decision summary**: We keep Zcash. We narrow the original "private creator platform" to a **one-time encrypted content drop** with an indexer service designed so that **even a fully-logged, fully-compromised server learns nothing that breaks user privacy**.

---

## 1. The worry we ran into in the week 3 sync

The original idea was a Zcash-native creator platform: creators publish encrypted content, subscribers pay shielded ZEC, content unlocks. During the week 3 discussion this hit two blocking concerns:

### 1.1 No on-chain atomicity (the "no script" problem)

Quoting the meeting (paraphrased from the transcript):

> "정확하게는 [Zcash]가 사는 게 의미가 있으려면 shielded pool에서 결제한 거랑 크리에이터의 키가 교환되는 시스템이 **동기적으로** 작동하면 충분히 설득력 있는 시스템이라 생각하는데 그걸 동기적으로 할 수가 없으니까." — 발화자 4
> "스크립트가 없어서 안 되는 거죠. … 강제성이 없는 거죠." — 발화자 1 / 발화자 4
> "아 그러니까 결국에 어떤 다른 주체가 대신 해 줘야 되는." — 발화자 2

In plain terms: Zcash's shielded protocol has no smart-contract layer, so we cannot atomically bind *"payment lands"* to *"decryption key is released."* Some off-chain party has to bridge the two events.

### 1.2 That off-chain party becomes a privacy attack surface

We explicitly flagged this as the anti-pattern from the Aztec Bridge research:

> "무조건적으로 운영자 신뢰해야 하는 구조입니다." — 발화자 3 (about Aztec Bridge)
> "프라이버시가 … 완전 바이너리니까 여기서 조금만 뭔가 있어도 이게 확 그냥 어택 벡터가 생겨버리는." — 발화자 1

If we naively add a server that holds the decryption keys and watches who pays for what, we've rebuilt a centralized Patreon with extra steps — and any subpoena, breach, or rogue admin reveals the entire buyer×content matrix. That ruins the reason for using Zcash at all.

These two worries combined are why we stalled: **we need an intermediary, but every intermediary we sketched compromised the privacy story.**

---

## 2. The solution direction — "honest-but-curious" intermediary

Reading [Sean Bowe's Tachyon post](https://seanbowe.com/blog/tachyon-scaling-zcash-oblivious-synchronization/) reframes this problem. Bowe is proposing exactly the same pattern at the Zcash protocol level: an **oblivious syncing service** — a server that helps wallets while staying *honest-but-curious*. The protocol is designed so that even if the server logs everything it sees forever, it learns essentially nothing about the wallet.

The general principle: **don't try to remove the intermediary — engineer its inputs so that they are unlinkable to identity, content, and to each other.**

For our use case, that translates to an indexer service with these properties:

| Server sees | Server does NOT see | Why it's safe |
|---|---|---|
| AES-encrypted content blobs uploaded by creators | Plaintext content | Standard E2E. Server is dumb storage (Blossom/NIP-96-style). |
| Incoming shielded payments via the creator's **IVK** (Incoming Viewing Key) | Buyer wallet identity, balances, other transactions | IVK reveals *only* payments addressed to it — Zcash's whole design point. |
| A buyer-supplied **ephemeral pubkey** (carried in the payment memo, encrypted to the server's IVK) | Buyer long-term identity | Ephemeral key is fresh per purchase. Server can't link two purchases. |
| Dispatched decryption blobs published to a **public bucket** | Which buyer fetched which blob | Anyone can fetch all blobs; only the holder of the matching ephemeral privkey can decrypt. Batching + jitter mitigates timing correlation. |

The server's role collapses from **"custodian"** to **"crypto-mailman."** Worst case if the server is fully compromised: it can refuse to dispatch (denial of service) — it cannot steal funds, cannot reveal who bought what, cannot decrypt content. That is a *qualitatively* different trust model from the Aztec-bridge-style "trust the operator absolutely" structure we wanted to avoid.

Three lines of defense layered:

- **Cryptographic** — server never holds plaintext content or buyer identity.
- **Economic** — server never custodies funds; payment is a direct shielded ZEC transaction to the creator.
- **Network** — same Tor/mixnet caveat Bowe ships with. We acknowledge it explicitly and treat it as a separate layer (out of scope for the demo).

---

## 3. Option 1 — "Unlockable Drop"

The concrete project scope this enables:

### 3.1 What it is

A creator publishes encrypted "drops" (image, post, short file). Anyone can browse a public catalog of encrypted titles + price. A buyer scans a ZIP-321 QR code, pays shielded ZEC via Zashi (or any Zcash wallet), and within seconds their phone/browser unlocks and displays the content. **No accounts, no logins, no subscription state.**

### 3.2 Components

1. **Creator dashboard** — upload, encrypt locally with a per-drop AES key, push ciphertext to storage, register the drop (price + metadata) with the indexer.
2. **Indexer service** — the honest-but-curious server described in §2. Watches creator IVKs, decrypts ephemeral pubkeys out of payment memos, dispatches per-purchase decryption blobs.
3. **Buyer UI (web)** — generates an ephemeral keypair per purchase, builds a ZIP-321 URI / QR with the encrypted memo, polls the public dispatch bucket, decrypts content client-side.
4. **Storage layer** — start with Blossom or a stub; not the interesting part.

### 3.3 What this design buys us narratively

- A direct, **demoable** answer to the worry in §1: yes there's a server, no it can't betray you.
- A presentation hook: *"We Tachyon-shaped the architecture before Tachyon ships."* The pattern we adopt is the same pattern the Zcash core team is publicly pushing as the long-term direction.
- A clean live demo at the final presentation: 3 teammates as creators, audience members scan QR → pay → image unlocks on their phones.

### 3.4 Explicit non-goals

- **Subscription / recurring access** — requires continuous trust over time. Dropped from scope. We acknowledge it as future work.
- **DRM / anti-piracy** — a paying buyer can leak their decryption key. Same problem every DRM-free platform has. Out of scope.
- **Full network-layer privacy** — Tor/mixnet documented but not implemented.
- **On-chain atomicity** — we explicitly concede this; the server is honest-but-curious, not trust-free.

### 3.5 Risk we accept

The indexer can be *uncooperative* (refuse to dispatch). This is denial of service, not privacy loss. Mitigations if time permits: k-of-n threshold dispatch (Shamir-split the dispatch key across multiple cooperating servers).

---

## 4. Next step

Move from this scope document to an implementation plan. Open questions to resolve there:

- Exact memo format for buyer ephemeral pubkey (within ZIP-321 memo size limits)
- IVK polling cadence and how the indexer maps "payment seen on creator C" → "which drop X"
- Public dispatch bucket design (just a CDN with hash-addressed blobs?)
- Whether to host the indexer ourselves or write it as a self-hostable binary creators can run
- Demo logistics: pre-loaded Zashi wallets vs. live testnet onboarding for the audience
