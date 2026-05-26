# Demo Script (5 minutes)

## Setup before demo (T-1h)
- [ ] Same-day reachability: visit https://hosh.zec.rocks/zec/testnet.zec.rocks:443 — confirm green.
- [ ] Backup lightwalletd configured in CVM env? (`phala cvms env list <name>`)
- [ ] Both UFVK files have expected on-chain history
- [ ] Both browser tabs ready: User UI and Verifier UI
- [ ] Screen recording running as fallback

## Narrative

### Slide 1 (15s) — the problem
Exchanges treat shielded-origin ZEC as high-risk because they can't see source of funds.
Naive answer: "user lists recipients and proves no intersection with sanctions" — but the user can omit the bad one.

### Slide 2 (30s) — the trust pattern
Sealed forensics lab analogy. The user's UFVK goes into an attested TEE; the TEE
scans the chain itself and emits a sealed report. Exchange checks the seal, not the wallet.

### Live demo (3 min)

**Step A: PASS flow** [60s]
1. Navigate to /prover, click "Fetch attestation" — show the code measurement.
2. Paste clean UFVK + policy + intent. Click Submit.
3. Wait ~30s for scan to complete. Show the artifact JSON.
4. Switch to /verifier, paste bundle + policy + intent. Click Verify.
5. Show all 3 ✅ + RESULT: PASS.

**Step B: FAIL flow** [60s]
Same steps with dirty UFVK. Same three ✅ on the binding checks; RESULT: FAIL.
*Key point: the trust pipeline is identical for PASS and FAIL — the answer is just different.*

**Step C: tamper demo** [30s]
In the bundle JSON, flip one byte of `artifact.recipientCount`. Re-verify. Show check #2 fails with "seal does not match this report."

### Slide 3 (45s) — limits
- Doesn't prove user gave us every wallet they own
- Doesn't prove ZEC's upstream history is clean
- Relies on Intel TDX trust assumption
- ZK privacy layer over the recipient set is v2

### Slide 4 (15s) — open
Code: github.com/moyedx3/private_dumb_money/tree/master/week5/clean-wallet-mvp
Spec: docs/superpowers/specs/2026-05-26-clean-wallet-mvp-design.md

## Fallback if live demo fails
- Pre-recorded video at `docs/demo-recording.mp4` (TODO: record after T-2h check)
