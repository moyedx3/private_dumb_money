
A practical guide for building apps on Zcash. Written for developers, not cryptographers.

---

## 1. How Zcash Works (The Big Picture)

```
The Zcash Blockchain (distributed across thousands of nodes)
                         │
                  Full Node (Zebra / zcashd)
                  validates blocks, stores chain
                         │
          ┌──────────────┴──────────────┐
          │                             │
     lightwalletd                  JSON-RPC API
     (compact block server)        (port 8232)
          │                             │
     gRPC endpoint              Your code calls
          │                     z_sendmany, etc.
          │                     (Approach A)
          │
     ┌────┴─────────────────────────────────┐
     │          librustzcash                │
     │  (core Rust crates — all light       │
     │   clients are built on this)         │
     └────┬─────────────┬──────────┬────────┘
          │             │          │
     zingolib      Android/    WebZ.js
     (Rust)        iOS SDK     (WASM)
          │             │          │
     your backend   mobile app  browser app
                  (Approach B)
```

### Layer 1: The Blockchain

Zcash is a distributed ledger forked from Bitcoin. Thousands of nodes maintain identical copies of the transaction history. Blocks are added roughly every 75 seconds.

The critical difference from Bitcoin: Zcash transactions can be **shielded**. The network verifies transactions are valid (no money created from nothing) without seeing sender, receiver, or amount — using zero-knowledge proofs.

### Layer 2: Address Pools

Zcash has two pools of money on the same chain:

- **Transparent (t-addresses)** — works exactly like Bitcoin. Balances and transactions are publicly visible. Exists for backward compatibility.
- **Shielded (z-addresses)** — the privacy layer. Three generations: Sprout (deprecated), Sapling (widely used), and Orchard (newest, Halo 2 proofs, no trusted setup). Shielded-to-shielded transactions reveal nothing to outside observers.

### Layer 3: Nodes

Node software connects to the P2P network, downloads and validates every block, stores the full chain (~300GB mainnet), and enforces consensus rules.

Two implementations exist:

|Node|Language|Status|
|---|---|---|
|**zcashd**|C++|Original, being phased out|
|**Zebra**|Rust|Built by Zcash Foundation, the future (sole node after NU7)|

A node by itself is just a validator. It doesn't know about "your" money unless you add wallet functionality.

### Layer 4: Wallet Functionality

Two completely different approaches:

**Approach A — Built-in wallet (inside zcashd):** The node has a wallet baked in. Key generation, note scanning, transaction building, proof generation — all happen inside one process. You talk to it via JSON-RPC. Convenient but tightly coupled. This is the right side of the diagram.

**Approach B — External wallet (light client):** The wallet is completely separate software that connects to a lightwalletd server. This is the left side of the diagram, and it has multiple layers:

### Layer 5: lightwalletd (The Middleware)

lightwalletd sits between full nodes and light clients. It strips full blocks down to "compact blocks" — just enough data for a light client to scan for its own transactions. It doesn't hold keys, doesn't know about wallets. It's a dumb data proxy.

### Layer 6: librustzcash (The Core Library)

[github.com/zcash/librustzcash](https://github.com/zcash/librustzcash)

All light clients are built on librustzcash — the official Rust crates that handle the actual cryptographic work. This is the single source of truth for Zcash wallet logic. You never interact with lightwalletd directly — librustzcash does that for you.

The key crates inside:

- `zcash_primitives` — transaction data structures
- `zcash_keys` — key derivation (spending keys, viewing keys)
- `zcash_client_backend` — wallet logic (scanning, note tracking, tx building)
- `zcash_client_sqlite` — SQLite storage for wallet state
- `zcash_proofs` — proof generation (Sprout)
- `pczt` — partially constructed transactions (for multi-party signing / FROST)

### Layer 7: SDKs and Wallet Libraries (Pick One)

On top of librustzcash, you choose a library based on your platform:

**zingolib** (Rust) — [github.com/zingolabs/zingolib](https://github.com/zingolabs/zingolib) Most batteries-included option. Wraps librustzcash with a high-level API:

- "create a wallet" → handles key derivation, DB init, parameter setup
- "sync" → handles the full sync loop, compact block fetching, trial decryption
- "send to address" → handles tx construction, proving, broadcasting
- Includes its own sync engine (pepper-sync) and lightwalletd gRPC client

**Android / iOS SDK** (Kotlin / Swift) — platform-native wrappers around librustzcash. Use these for mobile apps.

**WebZ.js** (JavaScript / WASM) — [github.com/ChainSafe/WebZjs](https://github.com/ChainSafe/WebZjs) Compiles librustzcash to WebAssembly for browsers. The only browser-specific Zcash SDK that exists. Still under active development (no audit). Requires a gRPC-web proxy in front of lightwalletd.

**Think of it this way:** librustzcash = React + Redux + Router. zingolib = Next.js.

These all do the same thing — sync with lightwalletd, manage keys, build shielded transactions. They just target different platforms. There is no Node.js or Python SDK — if your team doesn't write Rust, the options are Approach A (any language, just HTTP calls) or WebZ.js (browser only).

---

## 2. Approach A vs Approach B

### Approach A: JSON-RPC (Run Your Own Node)

```
Full Node (Zebra / zcashd)     →  you must run this yourself
JSON-RPC API                   →  built into the node
Your API wrapper               →  you build this
Your app logic                 →  you build this
```

- **Who holds the keys:** The node holds them internally.
- **Infrastructure:** Need to run your own full node (~10-30GB testnet, ~300GB mainnet).
- **Code complexity:** Low — just HTTP calls to JSON-RPC endpoints (`z_sendmany`, `z_getnewaddress`, etc.).
- **When to use:** Backend services that process payments, custodial wallets for agents, exchange integrations.
- **Can you use someone else's node?** No. The RPC port gives root access to the wallet. Your keys live on that machine.

### Approach B: Light Client (Use Public Infrastructure)

```
Full Node + lightwalletd       →  someone else runs this
Light client libraries         →  already exist (Rust crates, mobile SDKs, WASM)
Your wallet management layer   →  you build this
Your app logic                 →  you build this
```

- **Who holds the keys:** Your code holds them (on phone, in browser, on your backend).
- **Infrastructure:** Zero — connect to public lightwalletd servers.
- **Code complexity:** Higher — manage wallet state, sync loop, key storage through library APIs.
- **When to use:** User-facing wallets, self-custodial products, web apps where keys should never touch your server.
- **This is what most app developers use.**

### The Tradeoff

||Approach A (JSON-RPC)|Approach B (Light Client)|
|---|---|---|
|Infrastructure|Heavy (run a node)|None (use public servers)|
|Code complexity|Low (HTTP calls)|Higher (Rust libraries)|
|Key location|On the node|In your code|
|Trust model|Custodial (node holds keys)|Self-custodial possible|
|Language|Any (just HTTP)|Rust (or platform SDK)|

---

## 3. lightwalletd: Public Servers

You do NOT need to run your own node to use Approach B. There are 140+ public lightwalletd servers tracked at:

**Server health dashboard:** [hosh.zec.rocks/zec](https://hosh.zec.rocks/zec)

### Top Servers

**Testnet:**

|Server|Uptime (30D)|
|---|---|
|`testnet.zec.rocks:443`|~99.75%|
|`lightwalletd.testnet.cipherscan.app:443`|Available|
|`zcash.mysideoftheweb.com:19067`|~52% (unreliable)|

**Mainnet:**

|Server|Uptime (30D)|Ping|
|---|---|---|
|`zec.rocks:443`|99.95%|16ms|
|`na.zec.rocks:443`|99.54%|24ms|
|`lwd.zcashexplorer.app:9067`|99.83%|92ms|
|`z3.deepikaw.xyz:443`|99.97%|118ms|
|`lightwalletd.mainnet.cipherscan.app:443`|99.95%|412ms|

Full list with live status: [hosh.zec.rocks/zec](https://hosh.zec.rocks/zec)

### Want to run your own lightwalletd?

Note: lightwalletd requires a full node behind it. It's not standalone.

Docker quickstart via the zcash-stack project:

```bash
git clone https://github.com/zecrocks/zcash-stack.git
cd docker
./download-snapshot.sh          # skip days of syncing
docker compose up -d            # runs Zebra + lightwalletd
```

---

## 4. Reference Projects

### Zipher (Atmosphere Labs)

[github.com/atmospherelabs-dev/zipher-app](https://github.com/atmospherelabs-dev/zipher-app)

Privacy-first Zcash wallet for humans and AI agents. One Rust engine, three interfaces (mobile app, CLI, MCP server). Best reference for how to build a product on top of zingolib.

Architecture:

```
Flutter Mobile App ──(FFI)──→ Rust Engine ──→ zingolib ──→ librustzcash ──→ lightwalletd
CLI binary ─────────────────→ Rust Engine
MCP Server (22 tools) ──────→ Rust Engine
```

Key patterns to study: single engine / multiple consumers, spending policy engine, two-step send flow (propose then confirm), encrypted seed vault.

### CipherScan

[github.com/Kenbak/cipherscan](https://github.com/Kenbak/cipherscan)

Zcash block explorer that also provides free public lightwalletd infrastructure. Built with Next.js. Useful if you need a block explorer API for your app.

Public endpoints:

- Mainnet gRPC: `lightwalletd.mainnet.cipherscan.app:443`
- Testnet gRPC: `lightwalletd.testnet.cipherscan.app:443`
- REST API: `api.mainnet.cipherscan.app/api/*`

### Zingo Wallet (Zingo Labs)

[github.com/zingolabs/zingolib](https://github.com/zingolabs/zingolib)

The wallet that zingolib powers. Includes a CLI (`zingo-cli`) you can use to test lightwalletd connections immediately:

```bash
cargo build --release --package zingo-cli
./target/release/zingo-cli --server https://testnet.zec.rocks:443
```

### zcash-devtool (Official)

[github.com/zcash/zcash-devtool](https://github.com/zcash/zcash-devtool)

Official CLI tool for prototyping Zcash functionality. Uses `zcash_client_backend` and `zcash_client_sqlite` directly — no zingolib. Great reference code if you want to see how to wire up librustzcash without a wrapper library.

---

## 5. Key Resources

|Resource|URL|What It's For|
|---|---|---|
|ZecHub Developers|[zechub.wiki/developers](https://zechub.wiki/developers)|Best curated developer directory|
|Official Docs|[zcash.readthedocs.io](https://zcash.readthedocs.io/)|zcashd docs, integration guide, RPC reference|
|Server Health|[hosh.zec.rocks/zec](https://hosh.zec.rocks/zec)|Live lightwalletd server monitoring|
|Zcash GitHub|[github.com/zcash](https://github.com/zcash)|librustzcash, lightwalletd, SDKs, zcash-devtool|
|Zcash Foundation|[github.com/ZcashFoundation](https://github.com/ZcashFoundation)|Zebra node, FROST|
|Zingo Labs|[github.com/zingolabs](https://github.com/zingolabs)|zingolib|
|Community Forum|[forum.zcashcommunity.com](https://forum.zcashcommunity.com/)|Technical discussions, grants, announcements|
|Testnet Faucet|Search "Zcash testnet faucet"|Free test ZEC for development|

---

## 6. Quick Decision Guide

**"I'm building a backend service that manages wallets for users/agents"** → Approach A (zcashd on a VPS) for quickest path, or Approach B (zingolib on your backend) for the standard architecture.

**"I'm building a web app where users hold their own keys"** → Approach B with WebZ.js (browser) or zingolib/zcash_client_* (server-side).

**"I'm building a mobile wallet"** → Android SDK (Kotlin) or iOS SDK (Swift). Both wrap librustzcash.

**"I want to accept Zcash payments on my website"** → BTCPay Server with Zcash, or Approach A with a simple payment listener.

**"I just want to prototype and test quickly"** → Install `zingo-cli`, point at `testnet.zec.rocks:443`, start sending test transactions in minutes.

**"My team doesn't know Rust"** → Either: (a) Approach A with zcashd — any language can make HTTP calls, or (b) WebZ.js if you're building for the browser.