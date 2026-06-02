---
title: "Phala runtime enclave key deployment 2026-06-03"
tags: ["phala", "deployment", "enclave-key", "fvk", "attestation", "lightwalletd"]
created: 2026-06-02T17:37:54.473Z
updated: 2026-06-02T17:37:54.473Z
sources: ["docker-compose.phala.yml", "clean_wallet/enclave_key.py", "clean_wallet/client_encrypt.py", "scripts/encrypt_viewing_capability.py"]
links: []
category: session-log
confidence: high
schemaVersion: 1
---

# Phala runtime enclave key deployment 2026-06-03

# Phala runtime enclave key deployment — 2026-06-03

Updated the Clean Wallet Phala deployment so FVK/UFVK/UIVK can be encrypted to an attested enclave public key without placing an enclave private key in compose/env.

Implemented:
- `CLEAN_WALLET_AUTO_GENERATE_ENCLAVE_KEY=1` support in `clean_wallet/enclave_key.py`.
- When enabled, the service generates a process-local ephemeral X25519 private key and exposes only the public key in `/info` and `/attestation?purpose=enclave-key`.
- `key_origin=runtime-ephemeral` is included in the public descriptor.
- `docker-compose.phala.yml` now pins `docker.io/nogie/clean-wallet-mvp2@sha256:e6a06667a74808820f435233b1a387f29266609d8d10eed65a9b95cf76cc0da5` and sets the runtime key env.

Deployment:
- Existing CVM `clean-wallet-mvp2` updated successfully with `phala deploy --cvm-id clean-wallet-mvp2 -c docker-compose.phala.yml --wait --json`.
- App URL: `https://1cc48311ccb81c6982687095b840021bce576eb9-8080.dstack-pha-prod5.phala.network`.
- New compose hash observed: `6618c4154426125474675ce73c4c72de397a9b67db2105e13589e3f3004eb29e`.

Verification:
- `/health` returned ok true with measurement `a14e8e68eeca7946ccb39aabf9c159c8e3cd7d4d0e916e41f04e878f365046b518f692ba8cacbc705035f120d7732a05`.
- `/attestation?purpose=enclave-key&nonce=runtime-key-check` returned `status=configured`, `scheme=x25519-chacha20poly1305-v0`, `key_id=phala-runtime-ephemeral-x25519-v1`, `key_origin=runtime-ephemeral`, and a public key; quote `report_data` matched `attestation_payload_hash`.
- Local helper generated a live encrypted proof payload using dummy input; plaintext was absent from output.
- Submitting the dummy payload to `/proof` returned HTTP 200 with report `result=ERROR` and error prefix `zcash scanner failed: scanner command failed: Error: invalid UFVK/FVK: Address is not Bech32m encoded`; this proves fail-closed scanner boundary for invalid viewing capability and quote `report_data` matched the report hash.

Operational note:
- Because the key is runtime-ephemeral, encrypted FVK payloads expire on CVM restart. Always fetch fresh `/attestation?purpose=enclave-key` immediately before encrypting a real FVK.
