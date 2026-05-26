import canonicalize from "canonicalize";
import { createHash } from "node:crypto";
import { writeFileSync, mkdirSync } from "node:fs";

const OUT = "../packages/schemas/fixtures";
mkdirSync(OUT, { recursive: true });

const HEX64 = "0x" + "a".repeat(64);
const HEX96 = "0x" + "b".repeat(96);

const fixtures: { name: string; input: unknown }[] = [
  {
    name: "policy.demo",
    input: {
      policyName: "demo-v1",
      policyVersion: 1,
      network: "testnet",
      auditStartHeight: 2900000,
      auditEndHeight: 2950000,
      sanctionedAddressHashes: [HEX64, "0x" + "1".repeat(64)],
      expectedScannerCodeMeasurement: HEX96,
      createdAtUnix: 1716700000,
    },
  },
  {
    name: "policy.reordered-keys",
    input: {
      createdAtUnix: 1716700000,
      policyName: "demo-v1",
      sanctionedAddressHashes: [HEX64, "0x" + "1".repeat(64)],
      expectedScannerCodeMeasurement: HEX96,
      policyVersion: 1,
      auditEndHeight: 2950000,
      auditStartHeight: 2900000,
      network: "testnet",
    },
  },
  {
    name: "deposit-intent.demo",
    input: {
      exchangeName: "demo-exchange",
      exchangeDepositAddress: "ztestsapling1abcdef0123456789",
      depositAmountZat: "100000000",
      nonce: "0x" + "6f".repeat(16),
      expiryUnix: 1716800000,
    },
  },
  {
    name: "artifact.pass",
    input: {
      schemaVersion: 1,
      result: "PASS",
      scanRange: { network: "testnet", startHeight: 2900000, endHeight: 2950000 },
      policyHash: HEX64,
      depositIntentHash: "0x" + "47".repeat(32),
      viewingScopeCommitment: "0x" + "a0".repeat(32),
      recipientCount: 17,
      sanctionedHitCount: 0,
      scannerCodeMeasurement: HEX96,
      scanCompletedAtUnix: 1716750000,
    },
  },
  {
    name: "artifact.fail",
    input: {
      schemaVersion: 1,
      result: "FAIL",
      scanRange: { network: "testnet", startHeight: 2900000, endHeight: 2950000 },
      policyHash: HEX64,
      depositIntentHash: "0x" + "47".repeat(32),
      viewingScopeCommitment: "0x" + "a0".repeat(32),
      recipientCount: 17,
      sanctionedHitCount: 1,
      scannerCodeMeasurement: HEX96,
      scanCompletedAtUnix: 1716750000,
    },
  },
  {
    name: "artifact.zero-recipients",
    input: {
      schemaVersion: 1,
      result: "PASS",
      scanRange: { network: "testnet", startHeight: 2900000, endHeight: 2950000 },
      policyHash: HEX64,
      depositIntentHash: "0x" + "47".repeat(32),
      viewingScopeCommitment: "0x" + "a0".repeat(32),
      recipientCount: 0,
      sanctionedHitCount: 0,
      scannerCodeMeasurement: HEX96,
      scanCompletedAtUnix: 1716750000,
    },
  },
  {
    name: "nested.unicode-keys",
    input: { z: "last", a: "first", "한": "korean", "🌳": "emoji", nested: { b: 2, a: 1 } },
  },
  {
    name: "numbers.large-ints",
    input: { small: 0, big: 9007199254740991, zero: 0 },
  },
  {
    name: "arrays.empty-and-mixed",
    input: { empty: [], strings: ["b", "a"], objects: [{ k: 2 }, { k: 1 }] },
  },
  {
    name: "edge.null-and-bool",
    input: { t: true, f: false, n: null },
  },
];

for (const f of fixtures) {
  const canonical = canonicalize(f.input);
  if (canonical === undefined) throw new Error(`canonicalize returned undefined for ${f.name}`);
  const sha = createHash("sha256").update(canonical).digest("hex");
  writeFileSync(`${OUT}/${f.name}.input.json`, JSON.stringify(f.input, null, 2) + "\n");
  writeFileSync(`${OUT}/${f.name}.canonical.bin`, canonical);
  writeFileSync(`${OUT}/${f.name}.sha256.hex`, sha + "\n");
  console.log(`${f.name}: ${canonical.length} bytes, sha256=${sha.slice(0, 16)}…`);
}

console.log(`wrote ${fixtures.length} fixtures to ${OUT}`);
