---
title: "Real Zcash lightwallet IVK/UIVK enclave PoC status 2026-06-02"
tags: ["zcash", "lightwalletd", "phala", "enclave", "ivk", "uivk", "commitment", "ralph"]
created: 2026-06-02T12:48:35.040Z
updated: 2026-06-02T12:48:35.040Z
sources: [".omx/reports/architect-audit-real-zcash-lightwallet-poc-20260602.md", "clean_wallet/service.py", "clean_wallet/lightwalletd.py", "clean_wallet/enclave_key.py", "zcash_scanner/src/main.rs", "tests/test_clean_wallet.py", "Dockerfile"]
links: []
category: session-log
confidence: high
schemaVersion: 1
---

# Real Zcash lightwallet IVK/UIVK enclave PoC status 2026-06-02

# Real Zcash lightwallet IVK/UIVK enclave PoC status — 2026-06-02

Direction clarified and implemented toward the real target: no prover-submitted owned commitments in production. The Phala/default `/proof` path accepts encrypted viewing capability plus a lightwalletd source; inside the enclave/container it decrypts the capability, fetches compact blocks from lightwalletd, runs the Rust Zcash scanner, extracts on-chain `cmu`/`cmx` commitments for decryptable wallet outputs, compares them with the blacklist manifest, and emits an attested report.

Implemented boundaries:
- Fixture proofs are rejected unless mock/local fixture mode is explicitly enabled.
- Plaintext key fields such as `ivk`, `uivk`, `fvk`, `ufvk`, seed phrase, and mnemonic are rejected before parsing production proof payloads.
- Encrypted viewing capability uses X25519 + ChaCha20-Poly1305 and is decrypted from `CLEAN_WALLET_ENCLAVE_PRIVATE_KEY_B64`.
- `LightwalletdClient` supports real `GetLatestBlock` and `GetBlockRange` compact-block fetches.
- Rust scanner supports ZIP-316 UFVK/FVK and UIVK/IVK strings using `zcash_client_backend 0.22` + `zcash_keys 0.13`; UIVK/IVK mode is nullifier-less but can still discover decryptable outputs and return their commitments.
- Docker amd64 image `clean-wallet-mvp2:real-lightwalletd-poc-amd64` includes `/app/bin/clean-wallet-zcash-scanner` and wires it as `CLEAN_WALLET_ZCASH_SCANNER_CMD`.

Final evidence:
- `cargo fmt --manifest-path zcash_scanner/Cargo.toml -- --check` PASS.
- `cargo check --manifest-path zcash_scanner/Cargo.toml -q` PASS.
- `cargo build --release --manifest-path zcash_scanner/Cargo.toml -q` PASS.
- `python3 -m unittest discover -s tests` PASS, 25 tests.
- `python3 -m compileall -q clean_wallet tests` PASS.
- Public lightwalletd smoke against `lightwalletd.mainnet.cipherscan.app:443` fetched latest block; final observed height `3363864`.
- Docker amd64 image build PASS; image id `sha256:5fe947aeddae332937b0bbb0396aa0e074a0c3137a02568a0e663a35728c28b1`.
- Container smoke PASS: scanner executable present, invalid UIVK fails closed, Python service imports and accepts `uivk` contract.

Remaining risks:
- Pool-specific raw IVK byte formats are not implemented; practical supported capabilities are ZIP-316 UFVK/FVK/UIVK strings encrypted to the enclave.
- No known positive real-wallet owned-note fixture was available, so current evidence does not prove a live wallet-owned note match.
- Locally rebuilt image was not pushed/deployed to Phala in this iteration; previous live deployment remains stale until a new push/deploy is performed.
