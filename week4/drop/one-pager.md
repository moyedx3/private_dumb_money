# Unlockable Drop — One Pager

## TL;DR

A privacy-preserving creator-content platform on Zcash. Buyers pay shielded ZEC; encrypted content unlocks on confirmed payment. A **TEE-hosted indexer** (Intel TDX on Phala Cloud) brokers the payment → unlock step **without being able to decrypt the content itself** — content confidentiality is hardware-enforced, not trust-enforced. Buyer identity is hidden by Zcash's shielded protocol; the link between any specific buyer and any specific purchase is broken cryptographically.

---

## High-level architecture

```
   ┌────────────┐                                  ┌─────────────────┐
   │  CREATOR   │── encrypted content ───────────▶│  PUBLIC BUCKET  │
   │            │── K_drop + IVK (attested-TLS) ──┐│  (encrypted     │
   └────────────┘                                 ││   blobs +       │
                                                  ││   dispatch      │
   ┌────────────┐                                 ▼│   blobs)        │
   │   BUYER    │── shielded ZEC ─▶ ZCASH ─▶ ┌────────────────┐──────┘
   │  (browser) │                  CHAIN     │  TEE INDEXER   │
   │            │                            │  (Phala/TDX)   │
   │ ephemeral  │                            │  holds K_drop  │
   │ keypair    │                            │  inside enclave│
   └────────────┘                            └────────────────┘
        ▲                                            │
        │── wrapped key + ciphertext ────────────────┘
        │
        ▼
    decrypt locally → render content
```

**Components (one-line each):**
- **Creator** — uploads encrypted content, gives a per-drop AES key + Zcash IVK to the TEE via an attested TLS channel.
- **Buyer's browser** — generates a fresh ephemeral keypair per purchase; builds a ZIP-321 payment URI; polls for and decrypts the dispatch blob locally.
- **TEE Indexer** — Docker container on Phala Cloud / Intel TDX. Detects payments via IVK, wraps `K_drop` for the buyer's ephemeral pubkey via ECIES, publishes dispatch blob. Operator cannot read enclave memory.
- **Public bucket** — dumb storage (S3 or Blossom) for encrypted content blobs and dispatch blobs.
- **NEAR Intents** *(Phase 2 only)* — cross-chain settlement so the creator can be paid in any chain/asset they choose.

---

## End-to-end flow

1. **Creator setup:** encrypt content with `K_drop`, upload ciphertext to the bucket, hand `K_drop` + `IVK_creator` to the TEE via attested-TLS. TEE verifies its own measurement is the open-source build; only then accepts the secrets.
2. **Buyer picks a drop:** browser generates an ephemeral keypair `(e_priv, e_pub)`.
3. **Buyer pays:** browser builds a ZIP-321 URI `zcash:zs1...?amount=X&memo=drop_id||e_pub`. Buyer scans with Zashi. The shielded transaction is broadcast; the memo is encrypted by Zcash to the recipient's IVK.
4. **TEE detects:** the TEE polls the chain with `IVK_creator`, decrypts the memo, recovers `(drop_id, e_pub)`, verifies amount.
5. **TEE dispatches:** wraps `K_drop` for `e_pub` via ECIES → publishes a small blob to the public bucket. No identifier ties the blob to the buyer.
6. **Buyer unlocks:** polls the bucket, trial-decrypts each new blob with `e_priv`; the one that succeeds yields `K_drop`. Browser fetches the ciphertext, runs `AES-GCM.decrypt(K_drop, …)`, renders the content.
7. **(Phase 2 only)** TEE unshields received ZEC inside the enclave, sends transparent ZEC to NEAR Intents, solver settles into the creator's target chain (e.g. USDC on Base).

---

## Phase plan

| Phase | Scope | TEE holds | Cross-chain | Target |
|---|---|---|---|---|
| **Phase 0** | Software-only indexer (no TEE). Proves the full crypto flow end-to-end. K_drop is **not** confidential — internal team integration only. | n/a | none | ~3 days |
| **Phase 1** *(demo target)* | Move the indexer into a Docker container, deploy on Phala Cloud (Intel TDX). Creator owns own Zcash address, hands only IVK. Shielded ZEC → shielded ZEC. Satisfies the "Indexer cannot decrypt content" hard requirement. | `K_drop`s, IVK only | none | end of week 2 |
| **Phase 2** *(stretch)* | TEE generates and holds per-creator Zcash spending key inside the enclave. Unshields received ZEC in batches → NEAR Intents → creator's target chain. Adds creator-revenue-privacy tradeoff at the unshield step. | `K_drop`s, IVK, spending key, NEAR account | NEAR Intents (transparent ZEC source) | end of week 3 if Phase 1 is solid |

---

## Where to read more

- Full design: [`spec.md`](./spec.md) — architecture, sequence diagrams, security properties, tradeoffs, open questions, TEE tooling comparison.
- Background and worry we're solving: [`project-scope.md`](./project-scope.md).
