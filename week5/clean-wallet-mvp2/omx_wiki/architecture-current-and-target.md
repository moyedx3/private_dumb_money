---
title: "Architecture Current And Target"
tags: ["clean-wallet", "tee", "phala", "zcash", "proof-envelope"]
created: 2026-05-31T05:52:31.985Z
updated: 2026-05-31T05:52:31.985Z
sources: ["README.md", "clean_wallet/attestation.py", "clean_wallet/scanner.py"]
links: ["phala-dstack-attestation.md", "zcash-viewing-key-scanner-boundary.md", "session-log-2026-05-31-phala-tee-scaffold.md"]
category: architecture
confidence: high
schemaVersion: 1
---

# Architecture Current And Target

Current MVP is fixture JSON -> FixtureScanner -> exact blacklist set intersection -> MockAttestor -> clean-wallet-report-v0 JSON. Target production shape is requester verifies Phala CVM attestation/compose hash, encrypts viewing capability to attested TEE key, Phala Intel TDX CVM decrypts inside TEE, scans Zcash compact blocks with IVK/FVK/UFVK, extracts owned note commitments, compares against blacklist, creates bounded PASS/FAIL/ERROR report, and binds report_hash into dstack TDX quote reportData. Non-negotiable invariant: PASS is bounded to submitted network, pool, viewing scope, block range, and blacklist root only.

## See Also

- [[phala-dstack-attestation]]
- [[zcash-viewing-key-scanner-boundary]]
- [[session-log-2026-05-31-phala-tee-scaffold]]
