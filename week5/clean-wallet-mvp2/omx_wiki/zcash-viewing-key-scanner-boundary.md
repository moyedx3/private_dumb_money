---
title: "Zcash Viewing Key Scanner Boundary"
tags: ["zcash", "scanner", "ivk", "ufvk", "privacy", "tee"]
created: 2026-05-31T05:52:32.354Z
updated: 2026-05-31T05:52:32.354Z
sources: ["clean_wallet/scanner.py"]
links: ["architecture-current-and-target.md", "phala-dstack-attestation.md"]
category: architecture
confidence: high
schemaVersion: 1
---

# Zcash Viewing Key Scanner Boundary

FixtureScanner is the only operational scanner. ZcashViewingKeyScanner is a safe production seam that returns ERROR until a concrete scanner is implemented. Target TEE flow: decrypt encrypted viewing capability inside attested TEE, fetch or receive compact blocks for declared range, trial-decrypt Orchard/Sapling outputs with IVK/FVK/UFVK, extract owned note commitments, and return only commitments to proof layer. Never log or report raw viewing keys, decrypted notes, addresses, tx metadata, or raw owned commitment lists.

## See Also

- [[architecture-current-and-target]]
- [[phala-dstack-attestation]]
