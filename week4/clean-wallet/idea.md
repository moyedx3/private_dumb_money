# Private Off-Ramp Proof for Zcash

> **Status note:** This is the initial idea brief. It describes a mock-record ZK proof flow, which is useful as a toy/demo layer but does not solve record completeness by itself. The latest direction is in `README.md` and `README.kr.md`: use a viewing scope plus an attested scanner that processes the complete requested block range, then optionally add a ZK non-interaction proof over the scanner-derived recipient set.

## 1. Project summary

Build a hackathon-grade demo that shows how a Zcash user can prove a narrow privacy-preserving screening claim:

```txt
Within my declared wallet scope and audit period,
my outgoing shielded payment recipients do not match
the provided sanctioned ZEC address set.
```

The user generates the proof locally. The exchange verifies the proof and public metadata, but does not see private wallet history.

This is not a full legal compliance system. It is a proof-of-concept for privacy-preserving source-of-funds screening.

## 2. Problem

Exchanges often treat shielded-origin ZEC as high risk because they cannot inspect the source of funds.

Current model:

```txt
User used the Zcash shielded pool
-> exchange cannot inspect history
-> exchange rejects or flags the deposit
```

Demo model:

```txt
User used the Zcash shielded pool
-> wallet locally scans private records
-> wallet generates a ZK proof of non-interaction with a sanctioned address set
-> exchange verifies the proof without seeing private transaction history
```

## 3. Exact claim

The proof claim must stay narrow:

```txt
Given the user-submitted wallet scope and audit period,
the private outgoing recipient set does not intersect with
the provided sanctioned ZEC address set.
```

In set notation:

```txt
private_outgoing_recipients intersection public_sanctioned_addresses = empty set
```

This is a direct non-interaction proof for a declared wallet scope. It is not a universal clean-funds proof.

### What this proves

- Within the declared wallet scope, the user's outgoing recipients did not match the provided sanctioned ZEC address set.
- The proof was generated for a specific screening policy.
- The proof was bound to a specific exchange deposit intent.

### What this does not prove

- It does not prove the ZEC is globally clean.
- It does not prove full OFAC compliance.
- It does not prove the entire upstream history of funds.
- It does not prove the user submitted every wallet they control.
- It does not replace exchange KYC/AML obligations.

OFAC can list digital currency addresses on the SDN List, but those listings are not exhaustive. The demo should therefore avoid broad compliance language.

## 4. MVP approach

Use mock Zcash wallet records for the first implementation. Do not attempt real Zcash wallet scanning in the MVP.

The MVP should contain:

```txt
/apps/web
  Prover UI
  Verifier UI

/circuits
  ZK circuit proving recipient non-membership

/packages/core
  Mock Zcash wallet records
  Address normalization
  Hashing helpers
  Policy generation
  Proof input generation
```

Future integrations can use:

- viewing keys
- local wallet scanning
- `zcash_client_backend`
- `lightwalletd`
- actual shielded records

## 5. Recommended stack

Use:

```txt
Next.js + TypeScript
Circom + snarkjs
circomlib Poseidon hash
Node.js scripts
```

If Circom installation is difficult during the hackathon, implement a mock proof mode first. Keep the circuit file in the repo and clearly document what remains to be wired.

## 6. User experience

### Prover flow

Page: `/prover`

```txt
1. Load mock wallet records.
2. Load mock sanctioned ZEC address list.
3. Select audit period.
4. Enter exchange deposit intent.
5. Generate proof.
6. Export proof JSON.
```

Primary copy:

```txt
Private Off-Ramp Proof for Zcash

Your wallet records stay local.
Only a proof and public policy metadata are exported.
```

Show hidden fields:

```txt
Hidden:
- recipient addresses
- full wallet history
- memos
- non-relevant amounts
```

Show public fields:

```txt
Public:
- policy hash
- sanctioned address set hash/root
- deposit intent hash
- ledger commitment
- proof
```

### Verifier flow

Page: `/verifier`

```txt
1. Paste proof JSON.
2. Verify proof.
3. Show PASS or FAIL.
```

Valid proof message:

```txt
Proof valid

Claim:
Within the declared wallet scope, the user's outgoing recipients do not match the sanctioned ZEC address set.

This does not reveal:
- recipients
- full transaction history
- memos
```

Invalid proof message:

```txt
Proof invalid

Possible reasons:
- wallet contained a sanctioned recipient
- proof was not generated for this policy
- proof was not generated for this deposit intent
- malformed proof
```

## 7. Data model

### Wallet record

```ts
type WalletRecord = {
  id: string;
  direction: "incoming" | "outgoing";
  recipientAddress?: string;
  amountZat: string;
  blockHeight: number;
  memoCommitment?: string;
  txid: string;
};
```

Only `outgoing` records with a recipient should be checked.

### Sanctioned address

```ts
type SanctionedAddress = {
  label: string;
  asset: "ZEC";
  address: string;
};
```

### Screening policy

```ts
type ScreeningPolicy = {
  policyName: string;
  auditStartHeight: number;
  auditEndHeight: number;
  sanctionedAddressHashes: string[];
  maxRecords: number;
  maxSanctioned: number;
  depositIntentHash: string;
};
```

### Deposit intent

```ts
type DepositIntent = {
  exchangeName: string;
  exchangeDepositAddress: string;
  depositAmountZat: string;
  nonce: string;
  expiryUnix: number;
};
```

Compute:

```txt
depositIntentHash = Poseidon(
  exchangeDepositAddress,
  depositAmountZat,
  nonce,
  expiryUnix
)
```

This binds the proof to a specific exchange deposit request.

## 8. Mock data

Create `mockWalletRecords.json`:

```json
[
  {
    "id": "rec_001",
    "direction": "outgoing",
    "recipientAddress": "zs1_mock_clean_recipient_001",
    "amountZat": "50000000",
    "blockHeight": 2500000,
    "memoCommitment": "memo_hash_001",
    "txid": "tx_mock_001"
  },
  {
    "id": "rec_002",
    "direction": "outgoing",
    "recipientAddress": "zs1_mock_clean_recipient_002",
    "amountZat": "25000000",
    "blockHeight": 2500020,
    "memoCommitment": "memo_hash_002",
    "txid": "tx_mock_002"
  },
  {
    "id": "rec_003",
    "direction": "incoming",
    "amountZat": "75000000",
    "blockHeight": 2500030,
    "memoCommitment": "memo_hash_003",
    "txid": "tx_mock_003"
  }
]
```

Create `mockSanctionedZecAddresses.json`:

```json
[
  {
    "label": "Mock SDN ZEC Address A",
    "asset": "ZEC",
    "address": "t1_mock_sanctioned_address_A"
  },
  {
    "label": "Mock SDN ZEC Address B",
    "asset": "ZEC",
    "address": "t1_mock_sanctioned_address_B"
  }
]
```

Also create a failing test case where one outgoing recipient equals a sanctioned address.

## 9. Circuit model

The circuit proves:

```txt
I know private outgoing recipient addresses r_1, ..., r_n such that:

1. each r_i is included in my private wallet record commitment,
2. each active r_i is within the audit period,
3. for every active r_i and every sanctioned address s_j:
   hash(r_i) != hash(s_j),
4. this proof is bound to the public depositIntentHash and policyHash.
```

MVP simplifications:

```txt
MAX_RECORDS = 8
MAX_SANCTIONED = 8
```

Private inputs:

```txt
recipientHashes[MAX_RECORDS]
activeFlags[MAX_RECORDS]
recordSalts[MAX_RECORDS]
invDiffs[MAX_RECORDS][MAX_SANCTIONED]
```

Public inputs:

```txt
sanctionedHashes[MAX_SANCTIONED]
policyHash
depositIntentHash
ledgerCommitment
```

Commitment shape:

```txt
recordCommitment_i = Poseidon(recipientHash_i, activeFlag_i, salt_i)
ledgerCommitment = Poseidon(recordCommitment_1, ..., recordCommitment_n)
```

This does not prove the user provided their entire real wallet history. It only binds the proof to the submitted wallet scope.

## 10. Non-equality constraint

For each active recipient hash and sanctioned hash, enforce:

```txt
recipientHash != sanctionedHash
```

In Circom, use a non-zero inverse witness:

```txt
diff = recipientHash - sanctionedHash
diff * invDiff === 1
```

If `diff == 0`, no valid inverse exists, so proof generation fails.

Use `activeFlag` to ignore padded records:

```txt
activeFlag * (diff * invDiff - 1) === 0
```

When `activeFlag` is `1`, inequality is enforced. When `activeFlag` is `0`, the slot is ignored.

## 11. Circuit pseudocode

Implement a circuit similar to:

```circom
template NonInteractionProof(MAX_RECORDS, MAX_SANCTIONED) {
    signal input recipientHashes[MAX_RECORDS];          // private
    signal input activeFlags[MAX_RECORDS];              // private
    signal input salts[MAX_RECORDS];                    // private
    signal input invDiffs[MAX_RECORDS][MAX_SANCTIONED]; // private

    signal input sanctionedHashes[MAX_SANCTIONED];      // public
    signal input policyHash;                            // public
    signal input depositIntentHash;                     // public
    signal input ledgerCommitment;                      // public

    // 1. activeFlags must be boolean.
    for i in 0..MAX_RECORDS-1:
        activeFlags[i] * (activeFlags[i] - 1) === 0

    // 2. Non-interaction check.
    for i in 0..MAX_RECORDS-1:
        for j in 0..MAX_SANCTIONED-1:
            diff = recipientHashes[i] - sanctionedHashes[j]
            activeFlags[i] * (diff * invDiffs[i][j] - 1) === 0

    // 3. Compute record commitments.
    // recordCommitment_i = Poseidon(recipientHash, activeFlag, salt)

    // 4. Compute ledger commitment.
    // ledgerCommitmentComputed = Poseidon(recordCommitment_1, ..., recordCommitment_n)
    // ledgerCommitmentComputed === ledgerCommitment

    // 5. Bind policyHash and depositIntentHash.
    // They are public inputs to the proof.
}
```

Use actual Circom syntax and Poseidon components from `circomlib`.

## 12. Tests

Implement tests for:

```txt
1. Clean wallet records generate a valid proof.
2. Wallet records with a sanctioned recipient fail proof generation or verification.
3. Changing policyHash after proof generation fails verification.
4. Changing depositIntentHash after proof generation fails verification.
5. Inactive padded records do not affect the result.
6. Ledger commitment changes when the private recipient set changes.
```

## 13. README structure

The README should include:

```txt
# Private Off-Ramp Proof for Zcash

## Problem
Shielded Zcash protects user privacy, but exchanges may reject shielded-origin funds because they cannot inspect source-of-funds history.

## Idea
Instead of revealing transaction history or giving a viewing key to an exchange, the user locally proves a narrow compliance predicate:
their declared outgoing recipient set does not intersect with a sanctioned ZEC address set.

## What this proves
Within the declared wallet scope and audit period, no outgoing recipient matches the provided sanctioned ZEC address set.

## What this does NOT prove
- It does not prove full OFAC compliance.
- It does not prove the entire upstream history of funds.
- It does not prove the user submitted every wallet they control.
- It does not replace exchange KYC/AML obligations.

## Demo
1. User loads wallet records locally.
2. User generates proof.
3. Exchange verifies proof.
4. Exchange sees PASS/FAIL but not the transaction history.

## Future work
- Integrate zcash_client_backend for real local wallet scanning.
- Support viewing-key based local scan.
- Use real OFAC ZEC address data.
- Replace naive blacklist comparison with Merkle set non-membership.
- Add recursive clean-source credentials passed through encrypted memos.
```

Be honest in the README: this is a wallet-scope direct non-interaction proof, not a proof of globally clean funds or full sanctions compliance.

## 14. Implementation phases

### Phase 1: Core package and docs

- TypeScript data models
- Mock wallet and sanctioned address data
- Poseidon/hash helper
- Proof input builder
- README

### Phase 2: Proof system

- Circom circuit
- `snarkjs` setup
- proof generation script
- verifier script
- tests

### Phase 3: Web app

- Next.js prover page
- Next.js verifier page
- proof export/import JSON

### Phase 4: Demo polish

- UI polish
- failing example
- demo script

## 15. Final deliverables

Create:

```txt
README.md
apps/web/prover page
apps/web/verifier page
packages/core mock data + helper functions
circuits/non_interaction.circom
scripts/generate-proof.ts
scripts/verify-proof.ts
tests/non-interaction.test.ts
```

The project should run with:

```bash
npm install
npm run test
npm run dev
```

## 16. Demo script

Use this narrative:

```txt
Problem:
Exchanges often cannot evaluate shielded-origin ZEC without asking users to reveal too much.

Current options:
1. Reject shielded-origin deposits.
2. Ask for excessive disclosure.
3. Force transparent-source deposits.

Our demo:
The wallet keeps private history local and generates a proof that the declared wallet scope did not interact with a sanctioned ZEC address set.

Result:
The exchange gets a verifiable PASS/FAIL without seeing the user's private Zcash history.
```

## 17. Reference

- [Zcash Basics](https://zcash.readthedocs.io/en/master/rtd_pages/basics.html)
