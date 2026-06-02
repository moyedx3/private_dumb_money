/**
 * 검증기 테스트. 실행: npm test
 */
import { test } from "node:test";
import { strict as assert } from "node:assert";
import type { ChainSource, ScreeningArtifact, ScreeningRequest } from "./types.ts";
import { runScan } from "./scanner.ts";
import { assembleArtifact } from "./artifact.ts";
import { DEFAULT_SCANNER_MEASUREMENT, SimulatedAttestation } from "./attestation.ts";
import { verifyArtifact } from "./verifier.ts";
import { hashSanctionedSet } from "./hash.ts";
import {
  cleanScope,
  mockChain,
  mockSanctionedAddresses,
  mockScanRange,
  taintedScope,
} from "./mock-chain.ts";

const TRUSTED = [DEFAULT_SCANNER_MEASUREMENT];
const MOCK_CHAIN_SOURCE: ChainSource = { kind: "mock" };
const TEST_SALT = "test-salt-verifier";

function makeRequest(): ScreeningRequest {
  return {
    policy: {
      policyName: "demo-policy",
      policyVersion: "1",
      auditRange: mockScanRange,
      sanctionedAddressSetHash: hashSanctionedSet(mockSanctionedAddresses),
      scannerMeasurement: DEFAULT_SCANNER_MEASUREMENT,
      depositIntentHash: "mock-deposit-intent-hash",
      approvedChainSources: [MOCK_CHAIN_SOURCE],
    },
    sanctionedAddresses: mockSanctionedAddresses,
    depositIntent: {
      exchangeName: "MockExchange",
      exchangeDepositAddress: "t1MockExchangeDeposit000000000000000",
      depositAmountZat: "100000000",
      nonce: "deposit-nonce-1",
      expiryUnix: 1_900_000_000,
    },
    scanRange: mockScanRange,
    chainSource: MOCK_CHAIN_SOURCE,
    nonce: "request-nonce-1",
  };
}

test("정상 artifact는 검증을 통과하고 trustedResult를 돌려준다", async () => {
  const att = new SimulatedAttestation();
  const request = makeRequest();
  const scan = runScan(mockChain, cleanScope, request);
  const artifact = await assembleArtifact(scan, cleanScope, TEST_SALT, request, att);

  const v = await verifyArtifact(artifact, request, TRUSTED, att);
  assert.equal(v.ok, true);
  assert.equal(v.trustedResult, "PASS");
  assert.ok(v.checks.every((c) => c.ok));
});

test("FAIL 결과의 artifact도 검증은 통과하고 trustedResult는 FAIL", async () => {
  const att = new SimulatedAttestation();
  const request = makeRequest();
  const scan = runScan(mockChain, taintedScope, request);
  const artifact = await assembleArtifact(scan, taintedScope, TEST_SALT, request, att);

  const v = await verifyArtifact(artifact, request, TRUSTED, att);
  assert.equal(v.ok, true);
  assert.equal(v.trustedResult, "FAIL");
});

test("변조된 artifact는 서명 검증에서 실패한다", async () => {
  const att = new SimulatedAttestation();
  const request = makeRequest();
  const scan = runScan(mockChain, cleanScope, request);
  const artifact = await assembleArtifact(scan, cleanScope, TEST_SALT, request, att);

  // PASS → FAIL 위조 시도
  const tampered: ScreeningArtifact = { ...artifact, result: "FAIL" };
  const v = await verifyArtifact(tampered, request, TRUSTED, att);
  assert.equal(v.ok, false);
  assert.equal(v.trustedResult, null);
  assert.equal(v.checks.find((c) => c.name === "attestation 서명")?.ok, false);
});

test("신뢰 목록에 없는 measurement는 거부된다", async () => {
  const att = new SimulatedAttestation();
  const request = makeRequest();
  const scan = runScan(mockChain, cleanScope, request);
  const artifact = await assembleArtifact(scan, cleanScope, TEST_SALT, request, att);

  const v = await verifyArtifact(artifact, request, ["other-measurement"], att);
  assert.equal(v.ok, false);
  assert.equal(v.checks.find((c) => c.name === "code measurement 신뢰")?.ok, false);
});

test("다른 입금 요청에 재사용하면 검증에 실패한다", async () => {
  const att = new SimulatedAttestation();
  const request = makeRequest();
  const scan = runScan(mockChain, cleanScope, request);
  const artifact = await assembleArtifact(scan, cleanScope, TEST_SALT, request, att);

  const otherRequest: ScreeningRequest = {
    ...request,
    nonce: "different-request-nonce",
    depositIntent: { ...request.depositIntent, nonce: "different-deposit-nonce" },
  };
  const v = await verifyArtifact(artifact, otherRequest, TRUSTED, att);
  assert.equal(v.ok, false);
  assert.equal(v.trustedResult, null);
});

test("policy.approvedChainSources에 없는 chainSource면 검증 실패 (D9)", async () => {
  const att = new SimulatedAttestation();
  // 요청은 lightwalletd로 가지만 정책은 mock만 허용 → 정책 위반
  const lwdChainSource: ChainSource = {
    kind: "lightwalletd",
    url: "https://lwd.example.com",
    network: "main",
  };
  const base = makeRequest();
  const request: ScreeningRequest = {
    ...base,
    chainSource: lwdChainSource,
    // policy의 approvedChainSources에는 mock만 → request.chainSource는 허용 안 됨
    policy: { ...base.policy, approvedChainSources: [{ kind: "mock" }] },
  };
  const scan = runScan(mockChain, cleanScope, request);
  const artifact = await assembleArtifact(scan, cleanScope, TEST_SALT, request, att);

  const v = await verifyArtifact(artifact, request, TRUSTED, att);
  assert.equal(v.ok, false);
  assert.equal(v.checks.find((c) => c.name === "chain source (D9)")?.ok, false);
});

test("artifact의 chainSource를 변조하면 binding 서명에서 실패한다 (D9)", async () => {
  const att = new SimulatedAttestation();
  const request = makeRequest();
  const scan = runScan(mockChain, cleanScope, request);
  const artifact = await assembleArtifact(scan, cleanScope, TEST_SALT, request, att);

  // chainSource만 살짝 바꾼다 — binding payload 달라져 서명 검증이 깨진다
  const tampered: ScreeningArtifact = {
    ...artifact,
    chainSource: { kind: "lightwalletd", url: "https://evil.example", network: "main" },
  };
  const v = await verifyArtifact(tampered, request, TRUSTED, att);
  assert.equal(v.ok, false);
  assert.equal(v.checks.find((c) => c.name === "attestation 서명")?.ok, false);
});
