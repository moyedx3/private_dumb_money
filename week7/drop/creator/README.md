# Drop Creator App

Lane C creator dashboard for `week7/drop`.

## Design

UI changes must follow [`DESIGN.md`](./DESIGN.md). It is the creator-local source of truth for palette, typography, spacing, components, motion, and surface strategy.

## Run

```bash
npm install
npx playwright install chromium
npm run dev
```

Environment:

- `VITE_DROP_INDEXER_URL=http://localhost:8080`
- `VITE_DROP_EXPECTED_MEASUREMENT_HEX=` must be set or entered in the UI before provisioning. It must be the expected enclave measurement hex from the deployed indexer.
- `VITE_DROP_QVL_MODULE_URL=` is optional for local route-mocked QA, but required for live browser or Node quote verification unless the page installs `window.dropQuoteVerifier`.

The app does not bundle or default to a DCAP verifier package. For live provisioning, provide an explicit high-level verifier module via `VITE_DROP_QVL_MODULE_URL` or install `window.dropQuoteVerifier` before the app runs. The verifier must expose `verifyQuote(quoteHex)` or `verify(quoteHex)` and return `{ ok, codeMeasurement, reportData }` or the documented field aliases used by the tests.

## Verify

```bash
npm test
npm run build
npm run test:browser -- --project=chromium
node scripts/check-secret-sinks.mjs
npm run qa:http-smoke
npm run qa:lane-c
npm audit
```

Browser QA uses Playwright Chromium against the real Vite page with route-mocked indexer responses. It covers the provisioning happy path, fail-closed attestation behavior, validation errors, duplicate-submit protection, and responsive screenshots under `.omo/evidence/screenshots/`.

`npm run qa:lane-c` is the complete Lane C local gate. It runs, in order, unit tests, production build, Chromium browser QA, secret-sink scanning, and HTTP smoke.

`npm run qa:http-smoke` exits 0 with a skipped-live record when `VITE_DROP_INDEXER_URL` or
`VITE_DROP_EXPECTED_MEASUREMENT_HEX` is missing. This is an explicit skipped-live result, not proof that a deployed indexer passed live smoke. The combined `qa:lane-c` command still fails if unit, build, browser, or secret-sink checks fail before the live smoke step. To smoke a deployed indexer:

```bash
VITE_DROP_INDEXER_URL="$LIVE_INDEXER_URL" VITE_DROP_EXPECTED_MEASUREMENT_HEX="$EXPECTED_MEASUREMENT" VITE_DROP_QVL_MODULE_URL="$NODE_IMPORTABLE_VERIFIER" npm run qa:http-smoke
```

Live smoke calls `/health`, `/catalog`, and `/attest`, validates the I6 response, and verifies the quote with the explicit high-level verifier module from `VITE_DROP_QVL_MODULE_URL`. If live env is supplied without a verifier, smoke fails as setup. Credential-bearing, query-bearing, or fragment-bearing indexer URLs are rejected and redacted from stdout and evidence.

The app follows `team/interfaces.md`: I4 content blobs are `nonce(12) || AES-GCM ciphertext || tag(16)`, and I5 provisioning seals JSON `{ drop_id, price_zat, k_drop, creator_ufvk, h_content, deposit_addr }`. Because JSON has no byte type, `k_drop` is the 32 raw bytes encoded as 64 hex characters inside the sealed payload. The creator form rejects transparent `t...` deposit addresses before sealing because buyer memos require a shielded address.
