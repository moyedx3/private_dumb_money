# Technical Appendix - ZK Non-Interaction Layer

This appendix explains the ZK part of the idea. It is no longer the complete system design by itself.

The current project direction is in `README.md` and `README.kr.md`:

```txt
record completeness comes from an attested scanner,
not from a user-supplied JSON list.
```

The ZK circuit described here can still be useful as an optional privacy layer over the scanner output, but it does not solve scan completeness on its own.

## The Core Limitation

A zero-knowledge proof only proves a statement about the witness that the prover supplies.

If the statement is:

```txt
The private recipient set does not intersect with the sanctioned set.
```

then the circuit can prove that for the submitted private set. It cannot automatically prove:

```txt
The private recipient set contains every relevant wallet record.
```

That distinction matters. A malicious user could omit the record that contains a sanctioned recipient. The non-interaction circuit would still pass for the incomplete set.

This is why the main design moved to:

```txt
viewing scope + complete block-range scan + attested scanner
```

## What the ZK Layer Can Prove

Given:

```txt
R = private recipient hash set
S = public sanctioned address hash set
```

the ZK circuit can prove:

```txt
R intersection S = empty set
```

without revealing the members of `R`.

This is useful if `R` was produced by a trusted or attested process. It is weak if `R` was hand-picked by the user.

## Inputs

Private witness:

```txt
recipientHashes[MAX_RECORDS]
activeFlags[MAX_RECORDS]
salts[MAX_RECORDS]
invDiffs[MAX_RECORDS][MAX_SANCTIONED]
```

Public inputs:

```txt
sanctionedHashes[MAX_SANCTIONED]
policyHash
depositIntentHash
recipientSetCommitment
```

`recipientSetCommitment` replaces the older emphasis on `ledgerCommitment`. The important point is the same: it commits to the recipient set used by the proof. It does not prove completeness.

## Non-Equality Constraint

For every active recipient hash and every sanctioned hash:

```txt
diff = recipientHash - sanctionedHash
activeFlag * (diff * invDiff - 1) = 0
```

If `activeFlag = 1`, the circuit requires:

```txt
diff * invDiff = 1
```

That is only possible when `diff != 0`. If the recipient hash equals the sanctioned hash, proof generation fails because `0` has no inverse.

If `activeFlag = 0`, the slot is ignored:

```txt
0 * (diff * invDiff - 1) = 0
```

## Commitment

The circuit can commit to the recipient set:

```txt
recordCommitment_i = Poseidon(recipientHash_i, activeFlag_i, salt_i)
recipientSetCommitment = Poseidon(recordCommitment_1, ..., recordCommitment_n)
```

The verifier sees the commitment, not the recipients.

This prevents the prover from changing the private set after proof generation. It does not prove that the set contains every relevant Zcash record.

## Policy Binding

The proof should be bound to a screening policy:

```txt
policyName
policyVersion
auditStartHeight
auditEndHeight
sanctionedAddressSetHash
scannerMeasurement
depositIntentHash
```

The verifier should reject the proof if `policyHash` differs from the requested policy.

## Deposit Binding

The proof should also be bound to a deposit request:

```txt
depositIntentHash = Hash(
  exchangeDepositAddress,
  depositAmountZat,
  nonce,
  expiryUnix
)
```

The verifier should reject the proof if this hash does not match the current deposit request.

## How This Fits the New Architecture

Recommended split:

```txt
Attested scanner:
  - verifies its own code measurement
  - receives read-only viewing scope
  - scans complete block range
  - derives all relevant recipient records visible under that scope
  - emits recipientSetCommitment and screening artifact

Optional ZK layer:
  - proves the committed recipient set does not intersect with the sanctioned set
  - hides the recipient hashes from the exchange
```

In other words:

```txt
completeness is an attestation/scanning problem
non-intersection privacy is a ZK problem
```

## MVP Test Cases

For the ZK layer:

```txt
1. Clean recipient set generates a valid proof.
2. Recipient set containing a sanctioned hash fails proof generation or verification.
3. Changing policyHash after proof generation fails verification.
4. Changing depositIntentHash after proof generation fails verification.
5. Inactive padded records do not affect the result.
6. recipientSetCommitment changes when the private recipient set changes.
```

For the scanner/artifact layer:

```txt
1. Scanner processes every mock block in [start, end].
2. Scanner result changes to FAIL when any visible outgoing recipient is sanctioned.
3. Scanner artifact is rejected when scanRange changes.
4. Scanner artifact is rejected when scannerMeasurement is not trusted.
5. Scanner artifact is rejected when policyHash or depositIntentHash changes.
```

## Bottom Line

The ZK circuit is feasible, but it is not enough.

The system becomes meaningful only when the private set checked by the circuit is produced by a complete scan over a specific viewing scope. For the MVP, that completeness should be represented by an attested scanner and a screening artifact.

