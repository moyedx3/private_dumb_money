# week7/drop — Unlockable Drop, reality-checked

This folder is the **feasibility-corrected** continuation of `week4/drop/`. We pressure-tested the v0 spec against the code we actually shipped in `week5/clean-wallet-mvp/` and the live Zcash/NEAR/Phala specs, fixed five wrong/understated claims, and defined the spikes that decide whether to build.

## Contents

| File | What it is |
|---|---|
| [`spec.md`](./spec.md) | **Spec v1** — the corrected spec. Supersedes `week4/drop/spec.md` (v0). Has a `Changelog v0→v1` table; corrections are tagged `[C1]`–`[C5]` inline. |
| [`feasibility-review.md`](./feasibility-review.md) | The teardown that produced the corrections — verdict by component, with evidence (clean-wallet `file:line`, ZIPs, NEAR/Phala docs). |
| [`spikes.md`](./spikes.md) | How to test the 3 legs that, if any fails, change the design. **Do these before planning.** |

## The 30-second version

- **Phase 1 is feasible.** The scariest piece (an attested TEE scanning shielded Zcash) is what `clean-wallet` already is — we reuse its lightwalletd client, attestation wrapper, and Phala deploy pipeline.
- **No Phase-1 landmine.** The only *impossible* things (on-chain atomicity; shielded-through-NEAR-Intents) are already designed around.
- **Phase 2 is the danger zone** — an in-enclave spending key whose derivation is bound to the code measurement, so a rebuild can strand funds (`[C4]`).
- **Three spikes gate the plan:** Zashi memo-from-QR (`#1`), IVK incoming detection + memo recovery (`#2`), secret-IN provisioning to the enclave (`#3`).

## Relationship to other weeks

- `week4/drop/` — original v0 spec + `project-scope.md` (the "why", still valid).
- `week5/clean-wallet-mvp/` — the shipped Rust scanner + attestation we reuse.
- `week3/` — the "no script → need an honest-but-curious intermediary" decision that birthed this design.
