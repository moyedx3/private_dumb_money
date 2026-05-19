# Latch — System Specification

- **Status**: V1 specification.
- **Hard requirements** (any violation invalidates the design):
  1. The Buyer cannot extract `D` without paying.
  2. The Seller cannot substitute `D` after listing time.
  3. No protocol step requires either party's long-term Zcash spending key — only per-trade Orchard keys.
  4. The escrow contract never releases funds outside a documented verdict-resolution path.

---

## 1. Goals and non-goals

### Goals

1. **The Buyer pays in shielded ZEC.** Sender privacy is provided by Zcash's Orchard pool.
2. **The Seller commits to `D` at listing time.** The on-chain `H_D` and `metadata_hash` are immutable post-listing; AAD-bound AEAD detects any post-hoc substitution.
3. **Per-trade Orchard keys for both parties.** Long-term wallets are never touched. A challenge reveals only the per-trade `IVK_b` of the Buyer for the disputed trade; nothing else.
4. **A federated verifier set with bonded slashing** resolves disputes. A permissionless fraud-proof window lets anyone overturn a corrupted verdict and slash the verifier's bond.
5. **Permissionless settlement** after the verdict-and-window has elapsed. Anyone can invoke `settle_trade`; the code path is deterministic given inputs.

### Non-goals

- **Subscriptions or recurring access.** One-time digital-good purchases only.
- **DRM or piracy prevention.** A paying Buyer can republish `K` and `E_K(D)`.
- **Network-layer privacy** (IP correlation between the Zcash broadcast and the NEAR escrow call). Out of protocol scope.
- **Custody-free cross-chain payment.** ZEC ↔ NEAR custody is a federated PoA bridge; § 7 details what the bridge holds.
- **Catalog moderation, KYC, or dispute appeals.** Not enforced at the protocol layer.

---

## 2. High-level architecture

```
   ┌──────────────────────────────────────────────────────────────────────┐
   │                              SELLER                                  │
   │  - generates per-listing K (AES-256, OS CSPRNG)                      │
   │  - computes H_D = BLAKE2b-256(D); metadata_hash = BLAKE2b-256(JSON)  │
   │  - encrypts E_K(D) with AAD = metadata_hash; uploads to IPFS         │
   │  - generates per-trade OVK_s when a buyer arrives                    │
   │  - delivers K inside a Zcash z2z note memo (ZIP-302 V1 layout)       │
   └────────┬───────────────────────────────────────────────────┬─────────┘
            │ create_listing / register_delivery / submit_delivery │
            │ (Zcash side: z2z to UA_b)                            │
            ▼                                                       ▼
   ┌──────────────────────────────────────┐               ┌──────────────┐
   │      NEAR ESCROW CONTRACT             │               │  ZCASH       │
   │  - listings, reservations, trades     │               │  ORCHARD     │
   │  - challenges, verifier set, bonds    │               │  POOL        │
   │  - emits one event per state change   │               │              │
   │  - 11 external entry points           │               │  - delivery  │
   │  - all state transitions enforced     │               │    note + cm │
   │    on-chain                           │               │  - 512-byte  │
   └──┬───────────────────────┬───────────┘               │    memo with │
      ▲                       ▲                            │    K(32 B)   │
      │ finalize_payment      │ submit_attestation         └────┬─────────┘
      │ (bridge attestor)     │ (registered verifier)            │
      │                       │                                   │
   ┌──┴────────────────────┐  │  ┌────────────────────────────────┴──────┐
   │  PoA BRIDGE            │  │  │             BUYER                     │
   │  - 5–7 validators      │  │  │  - generates per-trade Orchard keys   │
   │  - MPC threshold key   │  │  │    (UA_b, IVK_b, buyer_ivk_commit)    │
   │  - custodies the Zcash │  │  │  - reserves listing, pays via z2t to  │
   │    transparent address │  │  │    the bridge transparent address     │
   │    receiving the buyer'│  │  │  - decrypts memo with IVK_b → K       │
   │    s shielded payment  │  │  │  - fetches E_K(D), decrypts with K,   │
   │  - mints a wrapped     │  │  │    checks H(D') = H_D                 │
   │    token on NEAR equal │  │  │  - on mismatch: files a challenge     │
   │    to the deposit      │  │  │    (reveals IVK_b on-chain)           │
   └────────────────────────┘  │  └───────────────────────────────────────┘
                               │
                          ┌────┴────────────────────┐
                          │  VERIFIER SET            │
                          │  - 3–5 federated daemons │
                          │  - re-runs the predicate │
                          │    on chain inputs only  │
                          │  - bonded; bond slashed  │
                          │    on a successful fraud │
                          │    proof against the     │
                          │    verifier's verdict    │
                          └─────────────────────────┘

                              ┌──────────────────────────────┐
                              │   IPFS                       │
                              │  - hosts E_K(D) by CID       │
                              │  - the ciphertext is public; │
                              │    useless without K         │
                              └──────────────────────────────┘
```

---

## 3. Components

| # | Component | Hosting | What it holds |
|---|---|---|---|
| 1 | **Seller's local tooling** | Seller's machine | `D`, fresh `K` per listing, per-trade `OVK_s`, full Zcash wallet |
| 2 | **Buyer's local tooling** | Buyer's machine | per-trade Orchard keypair (`SK_b`, `UFVK_b`, `IVK_b`, `UA_b`), in-memory only |
| 3 | **Escrow contract** | NEAR (one account per deployment) | listings, reservations, trades, challenges, verifier set, verifier bonds |
| 4 | **IPFS** | Operator's Kubo node (public read) | `E_K(D)` blobs only |
| 5 | **PoA Bridge** | 5–7-validator infrastructure with MPC threshold key | the Buyer's locked ZEC at a transparent Zcash address; the spending key for that address; the wrapped-token mint authority on NEAR |
| 6 | **Verifier daemon** | 3–5 identified operators | per-verifier signing key; verifier bond locked in the escrow contract; transient access to revealed `IVK_b`, the fetched delivery memo, and the IPFS blob |

The escrow contract exposes the following external entry points. Caller authentication is enforced on-chain.

| Entry point | Caller | Purpose |
|---|---|---|
| `create_listing` | seller | publishes a listing, attaches `seller_collateral` |
| `cancel_listing` | seller | takes an unreserved listing offline |
| `reserve_listing` | buyer | binds `UA_b` and `buyer_ivk_commit` to a specific listing |
| `finalize_payment` | bridge attestor | converts a reserved listing into a `PaymentLocked` trade once the bridge confirms payment |
| `register_delivery` | seller | commits to the Seller's per-trade `OVK_s` |
| `submit_delivery` | seller | publishes `delivery_cm` (the Zcash note commitment); starts `T_challenge` |
| `file_challenge` | buyer | opens a dispute, reveals `IVK_b`, attaches `buyer_challenge_collateral` |
| `submit_attestation` | registered verifier | records a verdict (`Honest` / `Fraud` / `Inconclusive`) |
| `submit_fraud_proof` | anyone | clears a resolved verdict during `T_fraud_proof`, slashing the corrupt verifier's bond |
| `settle_trade` | anyone | distributes funds per the final verdict; permissionless |
| `register_verifier_bond` | anyone | attaches a bond to qualify as a verifier |

---

## 4. End-to-end flow

### 4.1 Listing

The Seller, working entirely offline relative to the chain:

1. Generates `K` from the operating system CSPRNG (32 bytes).
2. Computes `H_D = BLAKE2b-256(D)` and `metadata_hash = BLAKE2b-256(metadata_json)`.
3. Encrypts `E_K(D) = AES-256-GCM.encrypt(K, D, AAD = metadata_hash)`. The blob is `nonce(12) ‖ ciphertext(n) ‖ tag(16)`.
4. Uploads `E_K(D)` to IPFS and receives `CID`.
5. Calls `create_listing { H_D, CID, metadata_hash, price, lifetime_ns }`, attaching `seller_collateral = price`.

After this, `Listing.status = Active`. The on-chain record exposes `(H_D, CID, metadata_hash, price, lifetime_ns)` and nothing about `D` or `K`.

### 4.2 Reserve and payment

```
 BUYER (local)         BRIDGE                  NEAR ESCROW
       │                  │                          │
       │ generate per-trade Orchard keys             │
       │ → SK_b, UFVK_b, IVK_b, UA_b                 │
       │ → buyer_ivk_commit = H(IVK_b)               │
       │                                             │
       │ reserve_listing { UA_b, buyer_ivk_commit,   │
       │                   expected_payment }        │
       │ ────────────────────────────────────────────▶
       │                                             │
       │                              Listing → Reserved
       │                              Reservation pending (T_reservation)
       │                                             │
       │ z2t shielded payment to bridge transparent  │
       │ address (amount = price)                    │
       │ ───────────────────────▶                    │
       │                  │                          │
       │     validator quorum observes deposit,      │
       │     MPC-threshold-signs an attestation       │
       │                  │                          │
       │                  │ finalize_payment {       │
       │                  │   reservation_id,         │
       │                  │   bridge_attestation }    │
       │                  ────────────────────────────▶
       │                                             │
       │                              wrapped token minted
       │                              Trade created
       │                              status = PaymentLocked
       │                              T_key_delivery starts (24h)
```

`UA_b` is a fresh per-trade Unified Address. It is the destination for the Seller's z2z delivery note in 4.3. Zcash's diversifier design makes `UA_b` unlinkable to the Buyer's long-term wallet.

### 4.3 Delivery

```
 SELLER                ZCASH CHAIN              NEAR ESCROW
       │                   │                          │
       │ register_delivery { OVK_s commitment }       │
       │ ─────────────────────────────────────────────▶
       │                                              │
       │ build 512-byte memo:                         │
       │   byte 0       = 0xF5         (ZIP-302 binary marker)
       │   bytes 1..33  = K            (32-byte AES key)
       │   byte 33      = 0x01         (Latch V1 version)
       │   bytes 34..512 = 0x00 …      (zero padding, strictly checked)
       │                                              │
       │ z2z transaction:                             │
       │   from: address derived from OVK_s           │
       │   to:   UA_b                                 │
       │   memo: the 512 bytes above                  │
       │                                              │
       │ ── shielded note ──▶│                        │
       │                     │ note encrypted to UA_b's key
       │                     │ delivery_cm materialises on Zcash
       │                                              │
       │ submit_delivery { delivery_cm,               │
       │                   seller_key_commit }        │
       │ ─────────────────────────────────────────────▶
       │                                              │
       │                              Trade → Delivered
       │                              T_challenge starts (48h)
```

`delivery_cm` is the cross-chain binding evidence: it exists on Zcash as the note commitment of the delivery note, and is registered on NEAR via `submit_delivery`. The verifier later checks that both refer to the same note.

### 4.4 Buyer verification (offline)

Without touching the chain:

1. The Buyer scans the Zcash chain with their per-trade `IVK_b`, finds the delivery note, decrypts it.
2. Memo bytes `1..33` yield `K`. The marker `0xF5`, version `0x01`, and zero padding past byte 33 are all checked strictly; any deviation is treated as fraud.
3. The Buyer fetches `E_K(D)` from IPFS using `CID`.
4. Decrypt: `D' = AES-256-GCM.decrypt(K, E_K(D), AAD = metadata_hash)`. A wrong `K`, tampered ciphertext, or wrong AAD fails the GCM tag check.
5. Compare `BLAKE2b-256(D')` to the listing's `H_D`. Equality means honest delivery.

If any step fails, the Buyer's option is § 4.6 (challenge).

### 4.5 Happy settlement

Once `T_challenge` (48h) has elapsed without a `file_challenge`, anyone may call `settle_trade`:

- The Seller receives `price + seller_collateral`.
- The trade moves to `Settled`; the listing moves to `Completed`.

The settlement caller is permissionless. Once the window has elapsed, no party can block it.

### 4.6 Challenge and resolution

```
 BUYER                 NEAR ESCROW         VERIFIER           NEAR ESCROW
       │                   │                  │                       │
       │ file_challenge { revealed_ivk_b, reason }                    │
       │ + buyer_challenge_collateral (50% of price)                  │
       │ ─────────────────────────────────────────────────────────────▶
       │                                                              │
       │ contract checks: H(revealed_ivk_b) == buyer_ivk_commit       │
       │ ──▶ Trade → Challenged                                       │
       │     T_verification starts (24h)                              │
       │                                                              │
       │                       fetches off-chain:                     │
       │                         - revealed_ivk_b from chain          │
       │                         - delivery_cm from chain             │
       │                         - decrypts the Zcash note → memo     │
       │                         - extracts K from memo[1..33]        │
       │                         - fetches E_K(D) from IPFS           │
       │                         - re-runs § 4.4 steps 2–5            │
       │                       → Verdict ∈ { Honest, Fraud,           │
       │                                     Inconclusive }           │
       │                                                              │
       │                 submit_attestation { verdict, signature }    │
       │                 ───────────────────────────────────────────────▶
       │                                                              │
       │                                    quorum reached →         │
       │                                    Trade → Resolved          │
       │                                    T_fraud_proof starts (12h)│
       │                                                              │
       │ (optional, anyone) submit_fraud_proof { evidence }           │
       │ ──▶ verdict cleared                                          │
       │     attesting verifier's bond slashed                        │
       │     T_verification re-opens for another verifier to attest    │
       │                                                              │
       │ settle_trade   (after T_fraud_proof, anyone)                 │
       │ ──▶ funds flow per the final verdict (§ 5)                    │
```

A verdict of `Inconclusive` indicates the verifier could not reach the inputs (IPFS unreachable, malformed payload). It is distinct from `Honest` and `Fraud` and produces a different fund flow.

---

## 5. Fund flow by verdict

`price = P`. `seller_collateral = P` (100% of price). `buyer_challenge_collateral = P/2` (50% of price). Values shown are net deltas at `settle_trade`.

| Outcome | Buyer net | Seller net | Verifier bond | Resolution trigger |
|---|---|---|---|---|
| Happy path (no challenge) | `−P` | `+P` | 0 | `T_challenge` expired without `file_challenge` |
| Verdict = `Fraud` | `+P` | `−P` | 0 | Buyer's payment + Seller's collateral + Buyer's challenge collateral all return to Buyer; Seller forfeits collateral |
| Verdict = `Honest` | `−(P + P/2)` | `+(P + P/2)` | 0 | Buyer's price + Buyer's challenge collateral both go to Seller |
| Verdict = `Inconclusive` | 0 | 0 | 0 | Both parties refunded their own deposits |
| Fraud proof against verdict | per re-attestation | per re-attestation | `−bond` for the slashed verifier | `submit_fraud_proof` clears verdict before `T_fraud_proof`; a different verifier then attests; the corrupt verifier's bond is slashed |

---

## 6. Cryptographic primitives

| Primitive | Use | Construction |
|---|---|---|
| **BLAKE2b-256** | `H_D`, `metadata_hash`, `buyer_ivk_commit` | Plain BLAKE2b with 256-bit output; constant-time comparison via `subtle::ConstantTimeEq` |
| **AES-256-GCM** | `E_K(D)` | Standard NIST construction. Nonce is 96 bits, generated fresh by `OsRng` per encryption and prepended to the ciphertext. AAD is bound to `metadata_hash` — decryption with a different AAD fails the tag check. Tag is 128 bits |
| **Orchard note encryption** | Delivery memo carrying `K` | The Zcash Orchard note encryption scheme. The note is encrypted to `UA_b`'s diversified transmission key; the buyer recovers `(note, address, memo)` with `IVK_b` |
| **Orchard note commitment** | `delivery_cm` cross-chain binding | The standard Orchard cm: a Pedersen-style commitment over the note's fields. Materialises on Zcash automatically when the note is included in a block |
| **ZIP-302 V1 memo layout** | The 512-byte payload of the delivery note | `byte 0 = 0xF5` (ZIP-302 binary-payload marker), `bytes 1..33 = K`, `byte 33 = 0x01` (Latch V1 version), `bytes 34..512 = 0x00` (zero padding, strictly checked on decode) |
| **Per-trade Orchard keys** | Buyer-side privacy + selective IVK reveal | The Buyer's `SK_b` is sampled fresh per trade. `IVK_b` is derived from `UFVK_b`. `UA_b` is a unified address with a single Orchard diversifier. `buyer_ivk_commit = BLAKE2b-256(IVK_b)` is the on-chain commitment; the contract checks `H(revealed_ivk_b) == buyer_ivk_commit` at `file_challenge` time |

The symmetric key `K` is `Zeroize` + `ZeroizeOnDrop` in memory, has no `Debug` impl, and compares in constant time. Constant-time equality is used wherever a comparison touches a secret or a hash output.

---

## 7. Security properties

### Guaranteed

| Property | Mechanism |
|---|---|
| **Seller cannot substitute `D` after listing** | `H_D` is committed on-chain at listing time; the verifier re-hashes the decrypted plaintext and compares |
| **Seller cannot swap metadata after listing** | `metadata_hash` is bound into the AES-GCM tag as AAD; decryption with different metadata fails the tag check |
| **Buyer cannot extract `D` without paying** | `K` lives only inside a Zcash note addressed to `UA_b`; `UA_b` only receives that note after `finalize_payment` confirms the bridge attestation of a shielded payment |
| **Per-trade key isolation** | The Buyer's `SK_b` is sampled per trade; a challenge reveals only `IVK_b`, which decrypts only that single trade's note |
| **Verifier collusion is detectable and slashable** | Anyone can submit a fraud proof during `T_fraud_proof`; the corrupt verifier's bond is slashed and a different verifier must re-attest |
| **Settlement is deterministic given a verdict** | The `settle_trade` code path is total: verdict and timing windows uniquely determine the fund flow |
| **The Seller does not need to be online to claim a happy-path payout** | `settle_trade` is permissionless; any account can trigger it once `T_challenge` elapses |

### Not guaranteed

- **Custody-free funds.** The PoA bridge holds the actual ZEC at a transparent Zcash address. § 9.1.
- **Network-layer anonymity.** IP correlation between the Buyer's Zcash broadcast and the Buyer's NEAR `reserve_listing` is not addressed. § 9.5.
- **IPFS persistence.** A Seller can pin `E_K(D)` to an unreliable host. Mitigation is operational: the Buyer should fetch `E_K(D)` before locking payment.
- **DRM or anti-piracy.** A paying Buyer can republish `K` and `E_K(D)`.
- **Recovery from a lost per-trade `SK_b`.** The per-trade key is single-shot; losing it before settlement forfeits the purchase.

---

## 8. Trust model

| Entity | Trust required | Reason |
|---|---|---|
| **Escrow contract code + NEAR consensus** | High — for state machine correctness and no deep reorg of the escrow chain | The contract is the rule of law; a chain reorg deeper than confirmation could revert recorded state |
| **Zcash protocol** (Halo 2 proofs, note commitments, IVK / OVK decryption) | High — soundness of the shielded payment and the cross-chain `cm` evidence | Standard Zcash assumptions |
| **Cryptographic primitives** (BLAKE2b-256, AES-256-GCM, Orchard note encryption) | High — collision resistance + AEAD tag soundness + correct note decryption | Off-the-shelf, well-vetted constructions |
| **PoA Bridge** | High — the bridge's validator quorum holds the spending key for the Zcash transparent address that receives the Buyer's payment, and signs the attestations that mint the wrapped token on NEAR | This is the largest single trust point. Bounded by `MAX_LISTING_PRICE_YOCTO` |
| **Verifier set** | Medium per-challenge — the majority of attesting verifiers must run the predicate honestly | A successful fraud proof slashes a corrupt verifier's bond; a permissionless audit window allows anyone to dispute a verdict |
| **IPFS operator** | Low — sees only ciphertext | Could DoS by deleting blobs, cannot read content |
| **Each party's own client software** | High for their own funds | Standard wallet hygiene; per-trade key isolation limits the blast radius of a single compromised key to one trade |

The protocol explicitly does NOT trust:

- Any single party with custody of the Buyer's payment on the escrow chain. The escrow contract holds it; no human or service can move funds outside the documented paths.
- Any party to retroactively determine what was sold. `H_D` is committed at listing time.
- Any party to adjudicate outside the verifier set + fraud-proof window. There is no appeals process to a private operator.
- Any party to know the buyer's identity. Per-trade Orchard keys mean that a successful challenge burns only the per-trade key, not the buyer's long-term wallet.

---

## 9. Tradeoffs and known drawbacks

Every design choice has a cost. Listed in rough order of severity.

### 9.1 The bridge holds the actual ZEC

The Buyer's payment is a z2t Zcash transaction into a transparent address whose spending key is controlled by 5–7 validators jointly via an MPC threshold scheme. The NEAR escrow contract mints a wrapped token equal to the deposit; the ZEC itself never crosses to NEAR. The bridge can:

- Fabricate a `finalize_payment` for a deposit that did not happen (drains the wrapped-token issuance from the contract).
- Refuse to attest a real deposit (Buyer's ZEC is stuck at the transparent address).
- Be compromised at the MPC layer, exposing all in-transit funds.

**Mitigation:** the `MAX_LISTING_PRICE_YOCTO` parameter caps the worst-case loss per listing. The validator set is itself collateralised and identified, so a fraudulent attestation has reputational and economic cost. The escrow contract's `T_reservation` window prevents a buyer's funds from being held in reservation indefinitely without bridge action.

**What this does NOT mitigate:** a coordinated compromise of the bridge's MPC quorum during a high-value batch. Raising listing caps requires re-evaluating this trust assumption.

### 9.2 `Inconclusive` is a soft outcome

When the verifier cannot fetch `E_K(D)` or finds it malformed, it returns `Inconclusive`. Both parties get their own deposits refunded and the trade closes without punishing either side. A Seller who refuses to keep their IPFS blob reachable produces the same on-chain trace as a Seller who never lied.

**Mitigation:** Buyer-side discipline — fetch `E_K(D)` from IPFS before locking payment. A Seller who pins unreliably loses Buyers, not just disputes.

**What this does NOT mitigate:** a Seller who pins reliably until just before challenge time, then unpins.

### 9.3 The fraud proof is permissive

`submit_fraud_proof` accepts any non-empty `evidence` blob and re-opens the challenge. The intent is procedural: a second verifier must then attest, and the corrupt attester's bond is slashed by the fact of their attestation alone. There is no on-chain cryptographic check that the supplied evidence is sound.

**Mitigation:** verifier slashing makes "attest falsely, then have it overturned" economically losing — a single bad attestation always costs the attester their bond.

**What this does NOT mitigate:** a verifier with bond margin willing to absorb a slash to grief one specific trade.

### 9.4 No DRM, no anti-piracy

A paying Buyer can republish `K` and `E_K(D)` to anyone. This is the same problem every digital-good marketplace has. The protocol does not attempt to solve it.

### 9.5 Network-layer correlation

The Buyer's Zcash broadcast and their `reserve_listing` NEAR call both originate from IP addresses. A network observer who sees both can correlate the Buyer to the trade. The protocol provides cryptographic privacy; network-layer privacy requires Tor or a mixnet at the operational layer.

### 9.6 Per-trade key lifecycle is single-shot

The Buyer's per-trade `SK_b` lives only on their machine. Closing the wallet between `reserve_listing` and `settle_trade` forfeits the purchase. The per-trade keypair must be persisted across the trade's full duration.

### 9.7 Catalog spam is not blocked at the protocol layer

Anyone with `seller_collateral` can list anything. The economic floor on spam is the cost of the collateral; an adversary willing to forfeit collateral can pollute the catalog.

---

## 10. Protocol parameters (V1)

All parameters are constants in the deployed contract.

| Symbol | Value | Meaning |
|---|---|---|
| `T_RESERVATION` | 1 hour | Maximum time a reservation may sit without `finalize_payment` |
| `T_KEY_DELIVERY` | 24 hours | Time the Seller has to call `submit_delivery` after `finalize_payment` |
| `T_CHALLENGE` | 48 hours | Window during which the Buyer may file a challenge after `submit_delivery` |
| `T_VERIFICATION` | 24 hours | Time the verifier set has to attest after `file_challenge` |
| `T_FRAUD_PROOF` | 12 hours | Window during which a verdict can be challenged with a fraud proof |
| `SELLER_COLLATERAL` | 100% of `price` | Forfeited on `Fraud`; returned otherwise |
| `BUYER_CHALLENGE_COLLATERAL` | 50% of `price` | Forfeited on `Honest`; returned otherwise |
| `MIN_LISTING_LIFETIME` | 1 hour | Floor on `lifetime_ns` at `create_listing` |
| `MAX_LISTING_LIFETIME` | 365 days | Cap on `lifetime_ns` at `create_listing` |
| `MAX_LISTING_PRICE` | 100 NEAR (testnet) | Cap on `price` at `create_listing` |
| `VERIFIER_QUORUM` | configured at deployment | Number of agreeing attestations required to resolve a challenge |
| `MEMO_SIZE` | 512 bytes | Zcash memo field size (protocol-imposed) |
| `MEMO_LEAD_BINARY` | `0xF5` | ZIP-302 marker byte |
| `MEMO_PROTOCOL_VERSION` | `0x01` | Latch V1 version byte |

---

## 11. Glossary

- **`D`** — the cleartext digital good (file, dataset, recipe — anything serialisable).
- **`K`** — the per-listing 32-byte AES-256 key.
- **`E_K(D)`** — `AES-256-GCM(K, D, AAD = metadata_hash)`, transported as `nonce(12) ‖ ciphertext(n) ‖ tag(16)`.
- **`H_D`** — `BLAKE2b-256(D)`, the public commitment to the cleartext good.
- **`metadata_hash`** — `BLAKE2b-256(metadata_json)`; bound into the AEAD tag as AAD.
- **`CID`** — IPFS content identifier of `E_K(D)`.
- **`UA_b`** — the Buyer's per-trade Zcash Unified Address.
- **`IVK_b`** — the Buyer's per-trade Incoming Viewing Key; revealed on-chain only when the Buyer files a challenge.
- **`UFVK_b`** — the Buyer's per-trade Unified Full Viewing Key; private.
- **`SK_b`** — the Buyer's per-trade spending key; never on-chain.
- **`buyer_ivk_commit`** — `BLAKE2b-256(IVK_b)`; the on-chain commitment to the per-trade IVK that the Buyer must reveal to challenge.
- **`OVK_s`** — the Seller's per-trade Outgoing Viewing Key.
- **memo** — the 512-byte ZIP-302 V1 binary memo of the delivery Zcash note, carrying `K`.
- **`delivery_cm`** — the Orchard note commitment of the Seller's delivery note; cross-chain binding evidence.
- **attestation** — a verifier's signed verdict (`Honest`, `Fraud`, `Inconclusive`).
- **fraud proof** — bytes supplied during `T_fraud_proof` to overturn a resolved verdict and slash the attesting verifier's bond.
- **seller collateral** — funds posted by the Seller at `create_listing`; forfeited on `Fraud`.
- **buyer challenge collateral** — funds posted by the Buyer at `file_challenge`; forfeited on `Honest`.
