---
title: "Phala Enclave IVK Next Step Handoff"
tags: ["phala", "ivk", "tee", "scanner", "handoff", "next-step"]
created: 2026-06-02T10:16:13.071Z
updated: 2026-06-02T10:36:36.000Z
sources: ["omx_wiki/architecture-current-and-target.md", "omx_wiki/zcash-viewing-key-scanner-boundary.md", "omx_wiki/phala-dstack-attestation.md", "omx_wiki/encrypted-viewing-capability-contract.md", "clean_wallet/service.py", "clean_wallet/scanner.py", "clean_wallet/enclave_key.py", "tests/test_clean_wallet.py"]
links: ["architecture-current-and-target.md", "zcash-viewing-key-scanner-boundary.md", "phala-dstack-attestation.md", "encrypted-viewing-capability-contract.md", "phala-live-poc-deployment-followup.md"]
category: session-log
confidence: high
schemaVersion: 1
---

# Phala Enclave IVK Next Step Handoff

As of 2026-06-02, the repo has a working fixture/mock proof MVP plus Phala/dstack attestation scaffold. The desired target flow is: requester verifies Phala CVM attestation and compose/measurement, sends an encrypted viewing capability (IVK/FVK/UFVK), enclave decrypts it, scanner trial-decrypts Zcash compact blocks, extracts only owned note commitments, compares them with the signed blacklist, then emits a bounded PASS/FAIL/ERROR report whose report_hash is bound into Phala TDX quote reportData.

Current implementation boundary: clean_wallet/service.py exposes /health, /measurement, /attestation, and /proof. /proof now accepts both legacy fixture-shaped scan payloads and a validated encrypted viewing capability contract. /measurement exposes a public enclave encryption-key descriptor, and /attestation?purpose=enclave-key&nonce=<client_nonce> quotes a hash of that descriptor plus nonce so clients can bind IVK encryption to the attested CVM. clean_wallet/scanner.py defines ZcashViewingKeyScanner as a production seam, but it intentionally returns ERROR until a real decrypt/scanner path is wired. clean_wallet/attestation.py contains PhalaDstackAttestor and PhalaDstackVerifier for dstack quote generation/verification, including reportData binding.

Needed user/project inputs before real IVK scanner implementation: choose viewing capability scope (Orchard IVK only vs FVK/UFVK), choose chain data source (lightwalletd endpoint, full node/RPC, or verifier-provided compact-block bundle), provide Phala deployment constraints (CVM app/compose hash or image digest allowlist, expected measurement policy), define secret provisioning method (dstack-derived app key/KMS, enclave public key from attestation, or TLS terminated inside CVM), and define demo network/block range/blacklist manifest source.

Completed 2026-06-02 implementation slices: added a non-secret API contract and request schema for encrypted_viewing_capability + chain_source + blacklist_manifest + scope, kept FixtureScanner backward compatibility, rejected plaintext viewing capability field names, required key_id for encrypted capability submissions, added an enclave key descriptor + nonce-bound attestation payload hash, and kept the production path ERROR-only so it cannot mint PASS without real Zcash scanning.

Recommended next implementation slice for production scanning: plug in the actual TEE-local key provider/decryptor behind `CLEAN_WALLET_ENCLAVE_PUBLIC_KEY` and matching private-key material, still without logging key material. Immediate live PoC deployment tracking is captured in [[phala-live-poc-deployment-followup]]: build/push a digest-pinned image, deploy to Phala Cloud, and verify live dstack quote/reportData binding before claiming production scanner readiness. After that, implement or integrate the concrete lightwalletd compact-block scanner for Orchard IVK, then run an end-to-end Phala CVM test where both enclave-key payload_hash and final report_hash are verified against TDX quote reportData.

Non-negotiable privacy invariant: never log or return raw viewing keys, z-addresses, decrypted notes, amounts, tx metadata, or raw owned commitment lists. PASS remains bounded only to submitted network, pool, viewing scope, block range, and blacklist root.

## See Also

- [[architecture-current-and-target]]
- [[zcash-viewing-key-scanner-boundary]]
- [[phala-dstack-attestation]]
- [[encrypted-viewing-capability-contract]]
- [[phala-live-poc-deployment-followup]]
