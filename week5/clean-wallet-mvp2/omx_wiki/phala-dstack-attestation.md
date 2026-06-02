---
title: "Phala Dstack Attestation"
tags: ["phala", "dstack", "attestation", "tdx", "reportData"]
created: 2026-05-31T05:52:32.170Z
updated: 2026-06-02T10:45:44.000Z
sources: ["clean_wallet/attestation.py", "clean_wallet/enclave_key.py", "clean_wallet/service.py", "Dockerfile", "docker-compose.phala.yml", "README.md"]
links: ["architecture-current-and-target.md", "zcash-viewing-key-scanner-boundary.md", "encrypted-viewing-capability-contract.md"]
category: reference
confidence: high
schemaVersion: 1
---

# Phala Dstack Attestation

PhalaDstackAttestor expects dstack-sdk and /var/run/dstack.sock in the CVM. It calls DstackClient().get_quote(bytes.fromhex(report_hash)) and serializes quote, event_log, app_id, instance_id, measurement, and report_data. PhalaDstackVerifier runs off-CVM and posts quote hex to PHALA_ATTESTATION_VERIFY_URL or https://cloud-api.phala.com/api/v1/attestations/verify. Verification checks mode, allowlisted measurement, verified quote status, and quote reportData prefix matches expected report_hash. TDX reportData is 64 bytes; this repo passes a 32-byte SHA256 report hash, so verifier accepts hash plus zero padding.

For encrypted viewing capability setup, /attestation?purpose=enclave-key&nonce=<client_nonce> computes a SHA256 hash of the public enclave encryption-key descriptor plus requester nonce and quotes that hash through the same reportData path. This lets a client verify that the public key used to encrypt IVK/FVK/UFVK belongs to the measured CVM before submitting /proof. The current repo exposes only the descriptor and binding hash; real private-key generation/decrypt still needs a TEE-local provider.

Implemented 2026-06-02: HTTP service and Docker image now default to CLEAN_WALLET_ATTESTOR=phala rather than mock. /health, /info, /measurement, and /attestation instantiate the Phala dstack client and fail closed with 503 if dstack-sdk or /var/run/dstack.sock is unavailable. Mock attestation is now explicit opt-in via CLEAN_WALLET_ATTESTOR=mock. PhalaDstackQuote also carries vm_config when the SDK returns it, matching dstack verifier expectations.

## See Also

- [[architecture-current-and-target]]
- [[zcash-viewing-key-scanner-boundary]]
- [[encrypted-viewing-capability-contract]]
