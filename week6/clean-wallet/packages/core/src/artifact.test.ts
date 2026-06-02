/**
 * artifact 조립 + attestation 테스트. 실행: npm test
 */
import { test } from "node:test";
import { strict as assert } from "node:assert";
import type { ChainSource, ScreeningArtifact, ScreeningRequest } from "./types.ts";
import { runScan } from "./scanner.ts";
import { assembleArtifact, artifactBindingPayload } from "./artifact.ts";
import { DEFAULT_SCANNER_MEASUREMENT, SimulatedAttestation } from "./attestation.ts";
import { hashSanctionedSet } from "./hash.ts";
import {
  cleanScope,
  mockChain,
  mockSanctionedAddresses,
  mockScanRange,
  taintedScope,
} from "./mock-chain.ts";

const MOCK_CHAIN_SOURCE: ChainSource = { kind: "mock" };
const TEST_SALT = "test-salt-deterministic";

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

test("깨끗한 scope → PASS artifact, attestation 검증 통과", async () => {
  const att = new SimulatedAttestation();
  const request = makeRequest();
  const scan = runScan(mockChain, cleanScope, request);
  const artifact = await assembleArtifact(scan, cleanScope, TEST_SALT, request, att);

  assert.equal(artifact.result, "PASS");
  assert.equal(
    await att.verify(
      artifact.attestation,
      artifactBindingPayload(artifact),
      await att.getMeasurement(),
    ),
    true,
  );
});

test("제재 수취인 scope → FAIL artifact", async () => {
  const att = new SimulatedAttestation();
  const request = makeRequest();
  const scan = runScan(mockChain, taintedScope, request);
  const artifact = await assembleArtifact(scan, taintedScope, TEST_SALT, request, att);
  assert.equal(artifact.result, "FAIL");
});

test("artifact는 raw 거래내역을 담지 않는다", async () => {
  const att = new SimulatedAttestation();
  const request = makeRequest();
  const scan = runScan(mockChain, cleanScope, request);
  const artifact = await assembleArtifact(scan, cleanScope, TEST_SALT, request, att);

  assert.ok(!Object.keys(artifact).includes("derivedRecords"));
  assert.ok(!JSON.stringify(artifact).includes("zs1mockCleanRecipient"));
});

test("core 필드를 변조하면 attestation 검증이 실패한다", async () => {
  const att = new SimulatedAttestation();
  const request = makeRequest();
  const scan = runScan(mockChain, cleanScope, request);
  const artifact = await assembleArtifact(scan, cleanScope, TEST_SALT, request, att);

  const tampered: ScreeningArtifact = { ...artifact, result: "FAIL" };
  assert.equal(
    await att.verify(
      tampered.attestation,
      artifactBindingPayload(tampered),
      await att.getMeasurement(),
    ),
    false,
  );
});

test("신뢰하지 않는 measurement면 검증이 실패한다", async () => {
  const att = new SimulatedAttestation();
  const request = makeRequest();
  const scan = runScan(mockChain, cleanScope, request);
  const artifact = await assembleArtifact(scan, cleanScope, TEST_SALT, request, att);

  assert.equal(
    await att.verify(
      artifact.attestation,
      artifactBindingPayload(artifact),
      "untrusted-measurement",
    ),
    false,
  );
});

test("artifact에 chainSource가 포함되고 binding payload에 반영된다 (D9)", async () => {
  const att = new SimulatedAttestation();
  const request = makeRequest();
  const scan = runScan(mockChain, cleanScope, request);
  const artifact = await assembleArtifact(scan, cleanScope, TEST_SALT, request, att);

  assert.deepEqual(artifact.chainSource, MOCK_CHAIN_SOURCE);
  // chainSource 변조 → binding payload 달라짐 → 서명 무효
  const tampered: ScreeningArtifact = {
    ...artifact,
    chainSource: { kind: "lightwalletd", url: "https://evil.example", network: "main" },
  };
  assert.equal(
    await att.verify(
      tampered.attestation,
      artifactBindingPayload(tampered),
      await att.getMeasurement(),
    ),
    false,
  );
});
