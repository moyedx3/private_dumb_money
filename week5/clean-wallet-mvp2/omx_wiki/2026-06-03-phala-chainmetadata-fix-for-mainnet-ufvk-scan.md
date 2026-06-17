---
title: "2026-06-03 Phala chainMetadata fix for mainnet UFVK scan"
tags: ["phala", "zcash", "lightwalletd", "chainmetadata", "deployment"]
created: 2026-06-02T17:56:47.196Z
updated: 2026-06-02T17:56:47.196Z
sources: ["clean_wallet/proto/compact_formats.proto", "clean_wallet/lightwalletd.py", "zcash_scanner/src/main.rs", "tests/test_clean_wallet.py", "docker-compose.phala.yml"]
links: []
category: session-log
confidence: high
schemaVersion: 1
---

# 2026-06-03 Phala chainMetadata fix for mainnet UFVK scan

Fixed live mainnet UFVK scan failure `Unable to determine Sapling note commitment tree size at height 3363067` by preserving lightwalletd compact block `chainMetadata` into scanner JSON and mapping it into zcash_client_backend compact_formats::CompactBlock. Local live fetch confirmed block 3363067 returns chainMetadata `{saplingCommitmentTreeSize: 73916603, orchardCommitmentTreeSize: 50059617}`, tx_count 6, orchard action_count 16. Built/pushed amd64 image `docker.io/nogie/clean-wallet-mvp2:chainmeta-20260603-amd64` with digest `sha256:f86f20b3ad46480fa52fe2ca8d24eb1ed48fd7d3a1e10c6f602e00d39b48bc4e`; docker-compose.phala.yml pinned to that digest and Phala CVM `clean-wallet-mvp2` redeployed successfully. Live attestation after redeploy: key configured, runtime-ephemeral, scheme x25519-chacha20poly1305-v0, key_id phala-runtime-ephemeral-x25519-v1, report_data matched attestation payload hash, measurement `4bf25da42060c911f0e3bf66fe89821d4633b181015f9388e02376c471d1c28f8e3212d7e5f298ffdfbff62f97aebd26`. Live dummy encrypted UFVK proof smoke returned fail-closed ERROR (`invalid UFVK/FVK`) with quote report_data bound to report_hash. User should rerun the same hidden-prompt UFVK command for block 3363067; expected next outcome is no longer tree-size metadata error, but PASS/FAIL/ERROR based on actual key scan and blacklist.
