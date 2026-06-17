---
title: "Encrypted Viewing Capability Contract"
tags: ["phala", "ivk", "encryption", "api-contract", "tee", "zcash"]
created: 2026-06-02T10:16:22.764Z
updated: 2026-06-02T10:36:36.000Z
sources: ["clean_wallet/service.py", "clean_wallet/scanner.py", "clean_wallet/enclave_key.py", "tests/test_clean_wallet.py", "README.md", "omx_wiki/phala-enclave-ivk-next-step-handoff.md"]
links: ["phala-enclave-ivk-next-step-handoff.md", "zcash-viewing-key-scanner-boundary.md", "phala-dstack-attestation.md"]
category: architecture
confidence: medium
schemaVersion: 1
---

# Encrypted Viewing Capability Contract

Planned production contract for the Phala TEE path: requester must verify the CVM attestation/measurement before sending any viewing capability. The request should carry encrypted_viewing_capability, viewing_scope_id or its commitment, network, pool, block_range, blacklist_manifest reference/body, and a chain_source declaration. The enclave decrypts inside the CVM and passes plaintext only to the scanner boundary.

Implemented on 2026-06-02: clean_wallet/service.py now validates a non-secret `/proof` envelope containing `request.encrypted_viewing_capability` with scheme, capability_type (`ivk`, `fvk`, `ufvk`), ciphertext, required key_id, plus `request.chain_source` (`lightwalletd`, `full_node_rpc`, or `compact_block_bundle`). Existing fixture payloads remain backward compatible. Plaintext key field names such as `viewing_key`, `ivk`, `fvk`, `ufvk`, `seed_phrase`, and `mnemonic` are rejected without echoing secret values. The production path instantiates ZcashViewingKeyScanner but intentionally returns ERROR until decrypt/scanner dependencies are selected and wired.

Implemented key attestation slice on 2026-06-02: clean_wallet/enclave_key.py defines a public enclave encryption-key descriptor and a deterministic `clean-wallet-enclave-key-v0` attestation payload hash. `/measurement` includes the current descriptor, and `/attestation?purpose=enclave-key&nonce=<client_nonce>` returns the payload, payload_hash, and quote where reportData binds the payload hash. If `CLEAN_WALLET_ENCLAVE_PUBLIC_KEY` is absent, descriptor status is explicitly `unconfigured`; this avoids pretending the scaffold can decrypt real IVKs.

Open contract decisions: IVK-only is narrower and may be pool-specific; FVK/UFVK is more complete for account-level scanning but increases sensitivity and parsing complexity. Chain source can be lightwalletd/full-node RPC (simpler online scan) or verifier-provided compact-block bundle (better reproducibility but larger payload and validation burden). Secret provisioning can use a dstack-derived key/KMS, an enclave public key exposed in an attested measurement response, or TLS terminated inside the CVM; the current repo has not selected one.

Implementation guidance: keep schema validation before cryptography, reject plaintext viewing capability fields by default, redact request bodies from logs, keep FixtureScanner as demo mode, and make production scanner return ERROR rather than PASS when decrypt/scan dependencies are unavailable.

## See Also

- [[phala-enclave-ivk-next-step-handoff]]
- [[zcash-viewing-key-scanner-boundary]]
- [[phala-dstack-attestation]]
