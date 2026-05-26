# Clean-Wallet MVP

Attested Zcash testnet scanner for private off-ramp screening. See:
- Spec: ../../docs/superpowers/specs/2026-05-26-clean-wallet-mvp-design.md
- Plan: ../../docs/superpowers/plans/2026-05-26-clean-wallet-mvp.md

## Quick start

```bash
# Rust scanner tests
cd apps/scanner && cargo test

# Web app
cd apps/web && pnpm install && pnpm dev

# Generate golden vectors
pnpm run gen:vectors
```
