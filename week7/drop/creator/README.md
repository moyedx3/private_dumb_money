# Drop Creator App

Lane C creator dashboard for `week7/drop`.

## Run

```bash
npm install
npm run dev
```

Defaults:

- `VITE_DROP_INDEXER_URL=http://localhost:8080`
- `VITE_DROP_EXPECTED_MEASUREMENT_HEX=` must be set or entered in the UI before provisioning.
- `VITE_DROP_QVL_MODULE_URL=@phala/dcap-qvl-web` is the browser-side quote verifier module specifier.

If the QVL package is served outside the bundle, expose a compatible verifier as
`window.dropQuoteVerifier.verifyQuote(quoteHex)` or set `VITE_DROP_QVL_MODULE_URL` to an absolute browser-importable module URL.

## Verify

```bash
npm test
npm run build
npm audit
```

The app follows `team/interfaces.md`: I4 content blobs are `nonce(12) || AES-GCM ciphertext || tag(16)`, and I5 provisioning seals JSON `{ drop_id, price_zat, k_drop, creator_ufvk, h_content }`. Because JSON has no byte type, `k_drop` is the 32 raw bytes encoded as 64 hex characters inside the sealed payload.
