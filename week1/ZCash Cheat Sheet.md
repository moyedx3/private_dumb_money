# Zcash Cheat Sheet

## What Is Zcash?

Zcash is effectively encrypted Bitcoin. Same 21M hard cap, same halving schedule, same UTXO model, same Proof of Work consensus, but with encryption on top. Bitcoin gives you transparent money. Zcash gives you private money.

Zcash has a **transparent pool** (works exactly like Bitcoin, addresses start with `t`) and **shielded pools** (fully encrypted, addresses start with `z`). The transparent pool exists for compatibility, optionality, and auditability. The two pools are entirely independent systems that do not affect each other. Even if 99% of ZEC were transparent, the shielded 1%'s privacy is determined solely by the shielded pool.

**Shielded pool generations:**
- **Sprout (2016):** First generation. Proved private crypto was possible. Required trusted setup. Slow (40s proof time). Now deprecated.
- **Sapling (2018):** Practical on mobile. Introduced viewing keys and diversified addresses. Still required trusted setup (Powers of Tau ceremony).
- **Orchard (2022):** Built on Halo 2 proving system. No trusted setup, no toxic waste, no trust assumptions. The pool Zcash was always meant to have.

**The fundamental question Zcash answers:** How can the network verify a transaction is valid if it can't see the transaction? The sender provides a zk-SNARK (a cryptographic proof) that demonstrates validity without revealing the underlying information.

---

## Core Technical Concepts

### Notes (Encrypted UTXOs)
A **note** is an encrypted object representing a specific amount of ZEC. Created when you receive shielded ZEC. Consumed when you spend, creating new notes for the recipient (and change). Only the owner (and anyone they share a viewing key with) can see its contents.

### Commitments
- Commitment = hash of the note's fields (`addr`, `v`, `rho`, `psi`, `rcm`)
- Every commitment is added to a global **Merkle tree** containing every note commitment ever created
- When spending, you prove inside the zk-SNARK that you know a commitment and a valid Merkle path to the current root, without revealing which commitment is yours
- Commitments are never deleted (append-only tree). Spent notes stay in the tree forever
- Your anonymity set = every shielded note ever created (millions)

### Nullifiers
- Can't point to the actual commitment being spent (that would link the note to all future transactions and break privacy)
- Instead, use **nullifiers:** `nullifier = Hash(nk, rho, psi)`
  - `nk`: nullifier deriving key (secret, only you have it)
  - `rho`, `psi`: values from the note itself
- When spending, you publish the nullifier
- The network maintains a **nullifier set** of every nullifier ever published
- If a nullifier is already in the set, the transaction is rejected (double-spend prevention)
- **Deterministic:** each note produces exactly one nullifier. Spending the same note twice = same nullifier = rejected
- **Unlinkable:** no one can map nullifiers back to commitments without your private key

### Key Hierarchy
```
spending key (sk)           -- master secret, can do everything
  +-- full viewing key (fvk)    -- see all wallet activity, can't spend
  |     +-- incoming viewing key (ivk) -- detect payments to you only
  |     +-- outgoing viewing key (ovk) -- see what you sent
  |     +-- addresses (via diversifiers) -- billions of unlinkable addresses
  +-- nullifier deriving key (nk)  -- compute nullifiers when spending
```

---

## Transaction Lifecycle

1. **Wallet sync:** Scans blockchain, attempts to decrypt every shielded output using the incoming viewing key. Stores the ones that succeed.
2. **Merkle path retrieval:** Fetches Merkle paths for the notes to spend. Proves in the zk-SNARK that the commitment exists in the tree without revealing the actual commitment or path. Records the **anchor** (Merkle root at time of retrieval).
3. **Compute nullifiers:** For each note being spent, compute `nullifier = Hash(nk, rho, psi)`.
4. **Create output notes:** Generate note components (`rho`, `psi`, `rcm`), compute commitments, encrypt notes (to recipient's address for `encCiphertext`, to sender's OVK for `outCiphertext`).
5. **Generate zk-SNARK proof** that:
   - Input notes exist (valid Merkle paths)
   - Sender controls the inputs (has the spending key)
   - Nullifiers are correctly derived from the actual notes
   - Sum of inputs = sum of outputs + fee
   - Output commitments are well-formed
6. **Assemble transaction:** Bundle into Orchard "actions" (each pairs exactly one spend + one output; dummies fill gaps to hide transaction shape). Includes anchor, nullifiers, commitments, encrypted payloads, proof (~1.5 KB), and binding signature.
7. **Broadcast:** Nodes validate: proof verification, anchor check, nullifier check (not already in set), structural validity. Valid transactions enter the mempool.
8. **Block inclusion:** Miner selects transaction, mines block (PoW using Equihash). Commitment tree grows (new leaves), nullifier set expands, block reward issued. ~75 second block time.
9. **Recipient detection:** Recipient's wallet trial-decrypts every shielded output. Successful decryption reveals the note data. Wallet verifies commitment matches on-chain, stores the note as spendable.

---

## Zcash vs. Others

| | Zcash | Monero | Tornado Cash / Mixers |
|---|---|---|---|
| **Method** | Encryption (zk-SNARKs) | Obfuscation (ring signatures, 16 decoys) | Mixing (deposit/withdraw through shared pool) |
| **Anonymity set** | Every shielded note ever created (millions) | 16 possible senders per tx | Fixed denomination pools |
| **Degrades over time?** | No. Cryptographic, not probabilistic | Yes. Decoys can be eliminated via analysis | Yes. Timing/amount correlation attacks |
| **In-pool functionality** | Full monetary system (send, receive, hold, change) | Full (all tx are shielded) | None. Must withdraw to use funds |
| **Exchange availability** | Coinbase, Gemini, others | Delisted from most major exchanges | N/A (sanctioned) |

**Aztec / Private L2s:** Solve a different problem (private programmability / encrypted DeFi). Zcash is money, a private store of value. Store of value needs Lindy effect (Zcash has ~9 years), memetic strength ("encrypted Bitcoin"), and social commitment to privacy as non-negotiable.

---

## Ecosystem and Economics

**Four main organizations:** ECC -> ZODL (protocol dev, Zashi wallet), Zcash Foundation (Zebra node, grants), Shielded Labs (research, Switzerland-based), Tachyon (scaling, led by Sean Bowe of Halo 2 fame).

**Funding history:** Founders' Reward (2016-2020, 20% of block rewards to founders/investors/employees). Dev Fund (2020-2024, 20% split: 7% ECC, 5% Foundation, 8% community grants). Extended Dev Fund (2024-2025, includes lockbox for future governance).

**Turnstiles:** Since you can't count coins inside a shielded pool, each pool tracks how much ZEC entered vs. exited. Can't withdraw more than entered. Detects (not prevents) counterfeiting attempts when cashing out.

**Network Sustainability Mechanism (NSM):** Burning 1 ZEC causes 0.5 additional ZEC issued over the next 4 years (exponential decay matching halving schedule). Reduces circulating supply short-term, sustains miner incentives long-term, never exceeds 21M cap. ZIP 233 (voluntary burn), ZIP 234 (smooth issuance curve), ZIP 235 (burn 60% of tx fees).

**Zcash x Near Intents:** Pay in shielded ZEC, recipient receives coins on another chain. Bridge without leaving the shielded pool.

---

## Road Ahead

### Project Tachyon
Solves three scaling bottlenecks:
1. **Double-spend prevention:** Currently every node must store the entire nullifier set (forever). Tachyon uses "oblivious synchronization," a service constructs proofs on your behalf without seeing which nullifiers you're spending. Validators no longer need full nullifier history.
2. **Blockchain scanning:** Replaces trial-decryption of every transaction with a more efficient payment protocol. Also removes in-band secret distribution (solves "harvest now, decrypt later" quantum threat).
3. **Transaction size:** Recursive proofs bring size and verification time down to roughly Bitcoin-level.

### Quantum Resistance
- **Already protected:** On-chain anonymity. Nullifiers use symmetric crypto (quantum-safe). Commitments are perfectly hiding. Symmetric encryption has post-quantum key sizes.
- **Privacy threat:** Adversary collects encrypted tx data today, decrypts later. Tachyon removes in-band secret distribution entirely, solving this.
- **Soundness threat:** Elliptic curve cryptography could be broken. Protocol's modular design allows upgrading vulnerable primitives without overhaul. Quantum recoverability mechanisms in development (2026), letting users recover funds safely.
