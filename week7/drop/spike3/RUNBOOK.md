# Spike #3 on real Phala — secret-IN, end to end

Proves the thing the dstack **simulator can't**: a secret sealed into the enclave is readable **only by the measured code**, the quote is **genuinely Intel-signed**, and the **operator never sees plaintext**. The secret-IN itself is a built-in CLI feature (`phala envs`), so there's no crypto to write for the spike.

> You're already logged in (`phala status` → `samoyedali`). `alpine` is public, so no GHCR visibility step (that only applied to the custom scanner image).

## ✅ VERIFIED on real Phala (2026-06-18)

Ran this end to end on a real `tdx.small` CVM (`dropspike3`):

- Sealed `K_DROP_TEST=hello-from-creator` via `phala envs update` → the enclave logged `DECRYPTED-INSIDE-ENCLAVE len=18 sha256=6befcca1…`, and that sha256 **matches** `printf %s hello-from-creator | sha256sum` exactly → the measured enclave got the real plaintext; the Phala host only ever held ciphertext.
- `phala cvms attestation` returned a **genuine TDX quote** — `Mrtd f06dfda6…`, `Rtmr0–3`, 30-entry event log (not the simulator's dev-signed quote). (`Mrtd` equals clean-wallet's because it's the shared dstack base image; the *app* is measured into `Rtmr3` via the compose hash.)

**Conclusion: spike #3 feasibility is confirmed on real hardware.** Secret-IN works via built-in primitives; only the creator-driven runtime endpoint remains to *build* (Lane A2).

### Gotcha that cost us a deploy
dstack decrypts the sealed env and exposes it for **compose `${VAR}` interpolation** — the container does **not** get it unless the compose maps it through `environment:`. The first run logged `NO-SECRET-VISIBLE` until `docker-compose.yml` added `environment: { K_DROP_TEST: ${K_DROP_TEST:-} }`. Compose changes update in place: `phala deploy --cvm-id <name> --compose <file> --wait` (no on-chain ceremony needed).

## Run it

```bash
# 1. Deploy a tiny proof CVM on real Intel TDX (tdx.small = cheapest).
phala deploy --name dropspike3 \
  --compose week7/drop/spike3/docker-compose.yml \
  --instance-type tdx.small --wait

# 2. Seal a secret INTO it. The CLI encrypts to the CVM's KMS-derived key;
#    only the measured enclave can decrypt. This is the secret-IN primitive.
phala envs update dropspike3 -e K_DROP_TEST=hello-from-creator

# 3. Confirm the enclave decrypted it (you see the sha256, never the plaintext).
phala cvms logs --cvm-id dropspike3 | grep DECRYPTED-INSIDE-ENCLAVE
#    → DECRYPTED-INSIDE-ENCLAVE len=18 sha256=<...>

# 4. Pull the REAL Intel-signed quote and verify it.
phala cvms attestation --cvm-id dropspike3
#    → verify at https://proof.t16z.com  (Check 1 "signature genuine" PASSES here —
#      unlike the simulator, which always fails Check 1 with its dev key)

# 5. Stop billing when done.
phala cvms stop dropspike3        # or: phala cvms delete dropspike3
```

## What each step proves (vs the spike #3 criteria)

| Step | Proves |
|---|---|
| 2 → 3 | **secret-IN works on real hardware**: a sealed secret is decryptable only inside the enclave |
| 3 (sha256 in logs) | the **measured code** actually got the plaintext |
| 2 (over the wire) | **operator can't read it** — Phala only ever received ciphertext |
| 4 (attestation) | the quote is a **genuine Intel-signed TDX quote** (Check 1 passes) — the part the simulator can't show |
| redeploy w/ changed compose | bonus: measurement changes → KMS re-derives → the **`[C4]` rebuild footgun**, live |

## The one nuance (what this does NOT cover)

`phala envs` seals env vars **at config time, set by you (the deployer)**. The drop's real flow is the **creator (a third party) sealing `K_drop` at runtime, after verifying the quote themselves**. But:

- The offline-encrypt half already exists too: `phala envs encrypt` produces a hex ciphertext bound to the CVM's key — i.e. a creator *could* encrypt `K_drop` without pushing it.
- So the only thing left to **build** (not prove) is a runtime endpoint that accepts the creator's pre-encrypted blob after they've verified attestation + the `report_data` binding (which `attest.rs` already does).

**Conclusion:** this spike confirms secret-IN is *feasible on real hardware* using built-in primitives. The creator-driven runtime wiring is then **Lane A2's build task**, not a feasibility unknown.
