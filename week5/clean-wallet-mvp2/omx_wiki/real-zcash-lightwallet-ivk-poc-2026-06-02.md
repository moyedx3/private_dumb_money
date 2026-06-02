---
title: "real-zcash-lightwallet-ivk-poc-2026-06-02"
tags: ["zcash", "lightwalletd", "tee", "phala", "ivk", "ralph", "ufvk"]
created: 2026-06-02T12:27:01.910Z
updated: 2026-06-02T12:38:14.482Z
sources: [".omx/reports/architect-audit-real-zcash-lightwallet-poc-20260602.md", "README.md", "tests/test_clean_wallet.py", "zcash_scanner/src/main.rs"]
links: []
category: session-log
confidence: high
schemaVersion: 1
---

# real-zcash-lightwallet-ivk-poc-2026-06-02

# Real Zcash lightwallet IVK enclave PoC handoff — 2026-06-02

User clarified the target is not a fake anti-cheat commitment demo. The project goal is: inside the enclave/CVM, receive an encrypted Zcash viewing capability (IVK/FVK/UFVK), fetch real Zcash compact blocks from lightwalletd, perform owned-note commitment extraction inside the enclave, compare against blacklist commitments, then generate an attested PASS/FAIL/ERROR certificate/report.

Implemented direction:
- Phala/default `/proof` rejects fixture proofs and prover-submitted owned commitments.
- Plaintext viewing key fields (`ivk`, `fvk`, `ufvk`, etc.) are rejected without echoing secrets.
- `clean_wallet/enclave_key.py` decrypts X25519 + ChaCha20-Poly1305 encrypted viewing capability using enclave env private key.
- `clean_wallet/lightwalletd.py` vendors lightwalletd protos, generates gRPC bindings, and fetches compact blocks via `CompactTxStreamer.GetBlockRange`.
- `ZcashViewingKeyScanner` decrypts the capability, fetches lightwalletd compact blocks, invokes `CLEAN_WALLET_ZCASH_SCANNER_CMD`, normalizes returned owned commitments, and fail-closes to ERROR on any scanner/fetch/decrypt failure.
- Existing proof/report generation now accepts generic `Scanner` + `Attestor`, so scanner output can produce attested reports.

Verification evidence:
- `python3 -m unittest discover -s tests` -> 24 tests OK.
- `python3 -m compileall -q clean_wallet tests` -> OK.
- Public lightwalletd smoke: `lightwalletd.mainnet.cipherscan.app:443` latest mainnet compact block fetched successfully.
- Fresh HTTP `/proof` smoke: fixture disabled, encrypted UFVK envelope, real lightwalletd latest block, scanner command returning blacklisted commitment -> attested FAIL report.
- Docker amd64 build/import smoke: `clean-wallet-mvp2:real-lightwalletd-poc-amd64` built and imports generated lightwalletd proto bindings.

Important blocker:
- Actual Sapling/Orchard trial-decryption scanner binary is not implemented yet. Tried `zecscope-scanner 0.1.0` and direct `zcash_client_backend`/`zcash_keys`; fresh cargo resolution failed because transitive `core2 ^0.3` is yanked. Current repo proves the enclave/lightwallet/report boundary and fail-closed behavior, but not real note decryption.

Next step:
- Implement or vendor a buildable librustzcash-based scanner command satisfying `CLEAN_WALLET_ZCASH_SCANNER_CMD` JSON stdin/stdout contract. It must parse UFVK/FVK/IVK, trial-decrypt Sapling/Orchard outputs from compact blocks, and return real owned note commitments. Do not re-enable fixture proofs in Phala/default mode.

---

## Update (2026-06-02T12:38:14.482Z)

# Real Zcash lightwallet viewing-capability enclave PoC handoff — 2026-06-02 update

The repo is now aligned with the clarified direction: not prover-submitted fake commitments. The service/CVM path receives an encrypted viewing capability, decrypts it inside the enclave/container, fetches real compact blocks from lightwalletd, runs an enclave-local Zcash scanner command, compares scanner-produced owned commitments against the blacklist, and produces an attested report.

Implemented:
- Phala/default `/proof` rejects fixture proofs and prover-submitted owned commitments.
- Plaintext viewing key fields are rejected without echoing secrets.
- `clean_wallet/enclave_key.py` decrypts X25519 + ChaCha20-Poly1305 encrypted viewing capability using enclave env private key.
- `clean_wallet/lightwalletd.py` vendors lightwalletd protos, generates gRPC bindings, and fetches compact blocks via `CompactTxStreamer.GetBlockRange`.
- `ZcashViewingKeyScanner` decrypts the capability, fetches lightwalletd compact blocks, invokes `CLEAN_WALLET_ZCASH_SCANNER_CMD`, normalizes returned owned commitments, and fail-closes to ERROR on any scanner/fetch/decrypt failure.
- `zcash_scanner/` adds a Rust `clean-wallet-zcash-scanner` binary using `zcash_client_backend 0.22` + `zcash_keys 0.13`; it supports encrypted UFVK/FVK plaintext, scans compact blocks, and maps decrypted wallet outputs back to on-chain `cmu`/`cmx` commitments.
- Dockerfile is multi-stage and wires `/app/bin/clean-wallet-zcash-scanner` as the default `CLEAN_WALLET_ZCASH_SCANNER_CMD`.

Verification evidence:
- `cargo fmt --manifest-path zcash_scanner/Cargo.toml -- --check` -> PASS.
- `cargo check --manifest-path zcash_scanner/Cargo.toml -q` -> PASS.
- `cargo build --release --manifest-path zcash_scanner/Cargo.toml -q` -> PASS.
- Raw IVK/UIVK invocation smoke fails closed with: `raw IVK/UIVK scanning is not implemented; submit an encrypted UFVK/FVK`.
- `python3 -m unittest discover -s tests` -> 24 tests OK.
- `python3 -m compileall -q clean_wallet tests` -> OK.
- Public lightwalletd smoke fetched latest mainnet compact block from `lightwalletd.mainnet.cipherscan.app:443`.
- Fresh HTTP `/proof` smoke: fixture disabled, encrypted UFVK envelope, real lightwalletd latest block, scanner command returning blacklisted commitment -> attested FAIL report.
- Docker amd64 final build and container smoke: default scanner cmd is `/app/bin/clean-wallet-zcash-scanner`, binary executable, Python lightwalletd proto import OK.

Remaining caveat:
- Raw IVK/UIVK-only support is still not implemented. The working production scanner accepts encrypted UFVK/FVK and derives incoming keys inside the enclave. Raw IVK requests fail closed until a lower-level `ScanningKeys::new` implementation is added.
