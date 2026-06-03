import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { policyHash, depositIntentHash, artifactHash, Policy, DepositIntent, ScreeningArtifact } from "./policy";

const FIXTURES = resolve(__dirname, "../../../packages/schemas/fixtures");

function load(name: string) {
  return JSON.parse(readFileSync(`${FIXTURES}/${name}.input.json`, "utf8"));
}
function loadSha(name: string) {
  return readFileSync(`${FIXTURES}/${name}.sha256.hex`, "utf8").trim();
}

describe("hashes match Rust", () => {
  it("policy.demo policy hash", () => {
    const p = load("policy.demo") as Policy;
    expect(policyHash(p)).toEqual("0x" + loadSha("policy.demo"));
  });
  it("deposit-intent.demo intent hash", () => {
    const d = load("deposit-intent.demo") as DepositIntent;
    expect(depositIntentHash(d)).toEqual("0x" + loadSha("deposit-intent.demo"));
  });
  it("artifact.pass artifact hash (no 0x prefix — used as reportData)", () => {
    const a = load("artifact.pass") as ScreeningArtifact;
    expect(artifactHash(a)).toEqual(loadSha("artifact.pass"));
  });
});
