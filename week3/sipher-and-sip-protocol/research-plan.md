# Week 3 Research Plan — Sipher And SIP Protocol

**Date:** 2026-05-15  
**Primary repos:** `sip-protocol/sipher`, `sip-protocol/sip-protocol`  
**Local clones:** `/private/tmp/sipher`, `/private/tmp/sip-protocol`

## Goal

Explain the two-product architecture clearly:

```text
Sipher = agent-facing API wrapper
SIP Protocol = privacy SDK / protocol layer
```

The research should answer whether either project gives us a useful reference for private AI-agent payments, and whether the Zcash claims map to real Zcash usage.

## Deliverables

Directory:

```text
week3/sipher-and-sip-protocol/
  README.md
  README.ko.md
  research-plan.md
  sipher-and-sip-protocol-big-picture.excalidraw
  Sipher-SIP-Protocol-overview.svg
```

Keep both README files concise. They should combine the Sipher and SIP findings instead of splitting into many files.

## SIP Protocol Research Track

1. **Read product claims**
   - Root `README.md`
   - `packages/sdk/README.md`
   - `docs/ARCHITECTURE.md`
   - `SDK-ROADMAP.md`

2. **Map SDK primitives**
   - stealth addresses
   - commitments
   - viewing keys
   - intent builder
   - privacy levels
   - chain adapters

3. **Verify Zcash implementation**
   - `packages/sdk/src/zcash/rpc-client.ts`
   - `packages/sdk/src/zcash/shielded-service.ts`
   - `packages/sdk/src/zcash/swap-service.ts`
   - `packages/sdk/src/zcash/bridge.ts`
   - examples and tests under `examples/zcash-connection` and `packages/sdk/tests/zcash`

4. **Separate real implementation from scaffolding**
   - direct `zcashd` RPC calls
   - demo prices / mock txids
   - required external bridge providers
   - roadmap language around proof composition

5. **Assess novelty**
   - Is there a new cryptographic primitive?
   - Or is this packaging of known primitives?
   - Which pieces are production-relevant to us?

## Sipher Research Track

1. **Map agent surface**
   - `skill.md`
   - OpenAPI
   - REST endpoints
   - Eliza and LangChain examples

2. **Trace private payment API flow**
   - stealth generation
   - transfer preparation
   - Solana transaction artifacts
   - scanning
   - claim endpoint trust boundary

3. **Identify what Sipher owns**
   - API key auth
   - tiers and rate limits
   - idempotency
   - audit redaction
   - request/response schemas
   - generated SDKs

4. **Identify what SIP owns**
   - cryptographic primitive calls
   - stealth address logic
   - commitments
   - viewing-key helpers
   - chain-specific SDK functions

5. **Check Zcash claims**
   - Does Sipher call SIP's Zcash RPC path?
   - Does it scan Zcash notes?
   - Does it verify ZEC settlement?
   - Does it only use Zcash-inspired language?

## Final Questions

- What should we reuse from Sipher?
- What should we reuse from SIP?
- Where would x402 attach?
- Where would real Zcash shielded settlement attach?
- What terms should we avoid because they imply stronger privacy than the code proves?

## Expected Conclusion To Validate

Sipher is valuable as an agent API reference. SIP Protocol is valuable as a privacy SDK reference. Neither should be treated as proof that Sipher itself is a complete Zcash-native payment product.
