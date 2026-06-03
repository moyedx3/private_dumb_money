# Trust Model

## Who has to trust what

| Party | Must trust | Defense if compromised |
|---|---|---|
| User | Intel TDX root + the published scanner code; the policy file in the repo | Verifies attestation *before* uploading UFVK; can read scanner source on GitHub |
| Exchange | Intel TDX root + the published scanner code; its own DepositIntent | Re-runs all 3 binding checks client-side; never trusts the user |
| Both | The Phala Cloud operator runs the *measured* image, not a modified one | Code measurement check in the policy fails if operator ran a modified image |

## The three checks (recap)

1. **Quote is genuine** — `dstack-verifier` traces signature to Intel TDX root.
   *Without this*: attacker prints a fake quote, forges any PASS.
2. **Quote binds this artifact** — `sha256(JCS(artifact)) == quote.reportData[0..32]`.
   *Without this*: attacker pairs a real quote with a forged artifact.
3. **Artifact binds this deposit + policy** — `depositIntentHash` and `policyHash`
   re-derived locally must match what's in the artifact; `scanRange` must match the policy.
   *Without this*: attacker reuses an old PASS for a different deposit.

## What's deliberately out of scope (acknowledged future work)

- **lightwalletd content honesty** — a malicious lightwalletd could feed forged blocks. Defense: header-chain verification against trusted checkpoints. Future work.
- **Side channels** — no timing obfuscation; lightwalletd sees which range we queried. Future work (PIR/Rime-style traffic shaping).
- **Multiple viewing scopes per user** — only one UFVK per request. Future work.
- **ZK non-intersection over the recipient set** — would hide `recipientCount`. Future work.

## How a user trusts the policy

For MVP, `demo-data/policy.demo.json` is the canonical policy. A real exchange would publish its own signed policy. For demos, the policy is in the repo and anyone can inspect it before participating.
