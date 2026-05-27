# Task 15 Runbook — Phala Deploy + Live Demo

This is the single source of truth for executing Task 15: deploying the scanner to
Phala Cloud, funding demo wallets, and running both demo flows live.

Work through each checkbox in order. Steps marked **USER** require your browser or
Phala account and cannot be automated.

---

## Prerequisites

- [ ] Docker daemon running (`docker info` returns no error)
- [ ] pnpm available (`pnpm --version`)
- [ ] Rust toolchain available (`~/.cargo/bin/cargo --version`)
- [ ] Phala CLI installed — if not: `npm install -g @phala/phala-cli` then `phala auth login` **(USER)**
- [ ] GHCR auth: `gh auth login` (web flow), then `echo $(gh auth token) | docker login ghcr.io -u $(gh api user --jq .login) --password-stdin`
- [ ] Confirm git remote points to your fork: `git remote get-url origin`

---

## Step A — Deploy scanner to Phala Cloud

### A1 — Export required environment variables

```bash
export TAG=$(git rev-parse --short HEAD)
export APP_NAME=clean-wallet-scanner

# Primary lightwalletd endpoint (mainnet)
export LIGHTWALLETD_URL=zec.rocks:443

# Backup endpoint — supply if you have a second node; leave blank to skip
export LIGHTWALLETD_BACKUP=

# Port the scanner HTTP server listens on inside the CVM
export SCANNER_PORT=3001
```

### A2 — Run the deploy script

```bash
cd /home/kkang/pdm/week5/clean-wallet-mvp
./scripts/deploy-cvm.sh
```

The script: builds the Docker image, pushes to GHCR, templates
`apps/scanner/docker-compose.yml`, and calls `phala cvms create`.

Expected final output:

```
==> Done. Useful follow-ups:
  phala cvms attestation clean-wallet-scanner
  phala cvms env get clean-wallet-scanner
  phala cvms logs clean-wallet-scanner
```

### A3 — Capture the CVM URL **(USER)**

```bash
phala cvms list
```

Note the public HTTPS URL for `clean-wallet-scanner`. It will look like
`https://<cvm-id>.phala.network`. Save it:

```bash
export SCANNER_URL=https://<cvm-id>.phala.network   # replace with real value
```

### A4 — Capture the code measurement (MRTD)

```bash
phala cvms attestation clean-wallet-scanner
```

The output JSON contains a `quote` or `quote_hex` field. The code measurement is
`MRTD` (bytes 128–176 of the parsed TDX quote, hex-encoded). You can also retrieve
it by pasting the `quote_hex` at https://proof.t16z.com — look for the
`mr_td` field in the parsed report body.

Save the hex string (64 hex chars / 32 bytes):

```
MRTD = 0x<64-hex-chars>
```

---

## Step B — Update policy.demo.json

### B1 — Replace the code-measurement placeholder

Open `demo-data/policy.demo.json`. Replace:

```json
"expectedScannerCodeMeasurement": "0xFILL_IN_AFTER_PHALA_DEPLOY"
```

with the MRTD captured in Step A4:

```json
"expectedScannerCodeMeasurement": "0x<your-mrtd-here>"
```

Leave `sanctionedAddressHashes` as-is for now (Step E fills it in).

### B2 — Commit the update

```bash
cd /home/kkang/pdm
git add week5/clean-wallet-mvp/demo-data/policy.demo.json
git commit -m "demo: fill in scanner code measurement from Phala deploy"
```

---

## Step C — Provision demo wallets

You need two distinct Zcash testnet wallets:
- **Wallet A (clean)** — receives testnet ZEC from the faucet but never pays a
  sanctioned address. Demo result: PASS.
- **Wallet B (dirty)** — receives testnet ZEC, then sends to the sanctioned
  address. Demo result: FAIL.

### Option 1 (recommended) — generate UFVKs via Rust helper

Create a one-off binary at `apps/scanner/src/bin/gen-ufvk.rs`:

```rust
//! Minimal UFVK generator — run with:
//!   cargo run -p clean-wallet-scanner --bin gen-ufvk -- <hex-seed>
use zcash_keys::keys::UnifiedSpendingKey;
use zcash_protocol::consensus::MainNetwork;
use zip32::AccountId;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed_hex = args.get(1).expect("usage: gen-ufvk <hex-seed>");
    let seed = hex::decode(seed_hex).expect("hex seed");
    let usk = UnifiedSpendingKey::from_seed(&MainNetwork, &seed, AccountId::ZERO)
        .expect("USK from seed");
    let ufvk = usk.to_unified_full_viewing_key();
    let encoded = ufvk.encode(&MainNetwork);
    println!("{}", encoded);
}
```

Generate two wallets (use distinct seeds — any 32+ byte hex will work):

```bash
cd /home/kkang/pdm/week5/clean-wallet-mvp

# Wallet A — clean
~/.cargo/bin/cargo run -p clean-wallet-scanner --bin gen-ufvk -- \
  deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef \
  | tee demo-data/ufvk-clean.txt

# Wallet B — dirty
~/.cargo/bin/cargo run -p clean-wallet-scanner --bin gen-ufvk -- \
  cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe \
  | tee demo-data/ufvk-dirty.txt
```

> These give you *view keys* (not spending keys). The corresponding spending key
> is derivable from the same seed — keep seeds secret. For the demo, only the UFVK
> is needed by the scanner.

### Option 2 — use the zecpages testnet faucet **(USER)**

1. Visit https://faucet.zecpages.com/
2. Request testnet ZEC to each wallet's unified address. To derive the unified
   address from a UFVK, use a Zcash light wallet (Zashi testnet build) or the
   `zcash-cli z_viewunifiedaddress` RPC.
3. Wait for confirmation (~75–150 seconds, 1 testnet block).
4. Repeat for the second wallet.

### Option 3 — skip funding entirely (PASS-only demo)

A wallet that has never received any funds will return `recipientCount: 0` and
`result: "PASS"`. This is the cheapest path to a live PASS demo when testnet
faucets are unavailable or slow. Trade-off: there is no on-chain history for the
scanner to verify, so the PASS result is trivially "empty wallet". Suitable for
demonstrating the attestation and binding checks; less convincing for the business
story.

To use this option, generate a fresh UFVK (Option 1 above) and do not fund it. The
scan will complete immediately with an empty result.

---

## Step D — Send the sanctioned-recipient transaction (FAIL demo)

This step creates the on-chain evidence the scanner will detect. You need a
zcashd node with Wallet B's spending key imported.

### D1 — Choose a sanctioned demo address

Pick any testnet address that will play the "sanctioned" role. You can derive a
fresh one (it does not need real OFAC significance for the demo). For example, use
the default zcashd testnet z-address from a throwaway node. Record it as
`SANCTIONED_ADDR`.

### D2 — Send from Wallet B to `SANCTIONED_ADDR`

**Option A — zcash-cli (if you have a synced testnet zcashd):**

```bash
zcash-cli -testnet z_sendmany \
  "$(zcash-cli -testnet z_getaddresses | jq -r '.[0]')" \
  '[{"address":"'${SANCTIONED_ADDR}'","amount":0.0001}]'
```

**Option B — Rust helper using `zcash_client_backend`:** see
`apps/scanner/README.md` for a sketch; this is out of scope for the MVP demo and
only needed if no zcashd is available.

### D3 — Note the confirmation block height

```bash
zcash-cli -testnet z_gettransactiondetails <txid>
# look for "blockheight"
```

Record: `CONFIRM_HEIGHT=<block-height>`

### D4 — Update the audit window in policy.demo.json

Set `auditStartHeight` to at least 100 blocks before `CONFIRM_HEIGHT` and
`auditEndHeight` to at least 1 block after it:

```json
"auditStartHeight": <CONFIRM_HEIGHT - 100>,
"auditEndHeight":   <CONFIRM_HEIGHT + 10>
```

Commit:

```bash
git add week5/clean-wallet-mvp/demo-data/policy.demo.json
git commit -m "demo: set audit window around sanctioned-tx confirmation block"
```

---

## Step E — Populate sanctioned-set.json

### E1 — Compute the SHA-256 hash of the sanctioned address

```bash
SANCTIONED_ADDR="ztestsapling1..."   # your actual address from Step D1
python3 -c "
import hashlib, sys
addr = sys.argv[1].encode()
h = hashlib.sha256(addr).hexdigest()
print('0x' + h)
" "${SANCTIONED_ADDR}"
```

### E2 — Update demo-data/sanctioned-set.json

Replace both FILL_IN placeholders in `demo-data/sanctioned-set.json`:

```json
{
  "entries": [
    {
      "label": "Demo Sanctioned Address (testnet)",
      "address": "<SANCTIONED_ADDR>",
      "hash": "0x<sha256-of-address>"
    }
  ]
}
```

### E3 — Update policy.demo.json sanctionedAddressHashes

```json
"sanctionedAddressHashes": ["0x<sha256-of-address>"]
```

### E4 — Commit

```bash
git add week5/clean-wallet-mvp/demo-data/
git commit -m "demo: populate sanctioned address and hash"
```

---

## Step F — Live demo runs

Open two browser tabs:
- **Tab 1 — User (Prover):** `<NEXT_JS_URL>/prover`
- **Tab 2 — Exchange (Verifier):** `<NEXT_JS_URL>/verifier`

If running locally: `cd apps/web && pnpm dev` then use `http://localhost:3000`.

### F1 — PASS flow (Wallet A)

- [ ] In the Prover tab, click **Fetch attestation** — confirm the code measurement
  shown matches `expectedScannerCodeMeasurement` in policy.demo.json.
- [ ] Paste the UFVK from `demo-data/ufvk-clean.txt`.
- [ ] Paste the contents of `demo-data/policy.demo.json` into the Policy field.
- [ ] Paste a DepositIntent JSON (example):
  ```json
  { "exchange": "demo-exchange", "depositId": "demo-001", "assetClass": "ZEC-shielded" }
  ```
- [ ] Click **Submit**. Wait ~30s for the scan to complete.
- [ ] Copy the returned bundle JSON. Switch to the Verifier tab.
- [ ] Paste the bundle + the same Policy JSON + the same DepositIntent JSON.
- [ ] Click **Verify**.
- [ ] Confirm all three checks show ✅ and **RESULT: PASS**.

### F2 — FAIL flow (Wallet B)

- [ ] Repeat the same steps with the UFVK from `demo-data/ufvk-dirty.txt`.
- [ ] Confirm all three checks show ✅ and **RESULT: FAIL**.
  (All binding checks pass — the TEE ran correctly — but the wallet touched a
  sanctioned address.)

### F3 — Tamper demo

- [ ] Copy the bundle JSON from the PASS run.
- [ ] In a text editor, find `"recipientCount"` and change its value by 1.
- [ ] Paste the modified bundle into the Verifier.
- [ ] Click **Verify**.
- [ ] Confirm check #2 fails with message: `"seal does not match this report"`.

---

## Step G — Open the quote on proof.t16z.com

- [ ] From the bundle JSON, copy the value of `quote_hex`.
- [ ] Visit https://proof.t16z.com and paste the hex string.
- [ ] Confirm the page renders **"Genuine TDX quote"**.
- [ ] Note the `mr_td` field — it should match `expectedScannerCodeMeasurement`.
- [ ] Take a screenshot for the demo deck.

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `phala cvms create` fails auth | Not logged in | `phala auth login` |
| `docker push` 403 | GHCR token expired | Re-run `gh auth login` + docker login step |
| Scanner returns 503 | lightwalletd unreachable | Check https://hosh.zec.rocks/zec/testnet.zec.rocks:443; set LIGHTWALLETD_BACKUP |
| Verifier: check #1 fails | Quote cannot be verified | CVM may not be TDX-attested; confirm `phala cvms attestation` returns a non-empty quote |
| Verifier: check #2 fails | Artifact was tampered (or bug) | Re-run the scan; compare bundle `artifact` fields |
| Verifier: check #3 fails | Policy or DepositIntent mismatch | Ensure you pasted the exact same JSON in both Prover and Verifier |
