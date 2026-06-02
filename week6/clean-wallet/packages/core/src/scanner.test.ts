/**
 * 스캐너 코어 테스트. 실행: npm test
 */
import { test } from "node:test";
import { strict as assert } from "node:assert";
import type { ChainSource, ScreeningRequest } from "./types.ts";
import { runScan } from "./scanner.ts";
import { hashSanctionedSet } from "./hash.ts";
import {
  cleanScope,
  mockChain,
  mockSanctionedAddresses,
  mockScanRange,
  taintedScope,
} from "./mock-chain.ts";

const MOCK_CHAIN_SOURCE: ChainSource = { kind: "mock" };

function makeRequest(scanRange = mockScanRange): ScreeningRequest {
  return {
    policy: {
      policyName: "demo-policy",
      policyVersion: "1",
      auditRange: mockScanRange,
      sanctionedAddressSetHash: hashSanctionedSet(mockSanctionedAddresses),
      scannerMeasurement: "mock-measurement",
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
    scanRange,
    chainSource: MOCK_CHAIN_SOURCE,
    nonce: "request-nonce-1",
  };
}

test("깨끗한 scope를 스캔하면 PASS이고 출금 record를 도출한다", () => {
  const result = runScan(mockChain, cleanScope, makeRequest());
  assert.equal(result.result, "PASS");
  assert.equal(result.matchedRecipientHashes.length, 0);
  assert.equal(result.derivedRecords.length, 3); // 수취인 A, B, D
});

test("제재 수취인이 있는 scope를 스캔하면 FAIL이다", () => {
  const result = runScan(mockChain, taintedScope, makeRequest());
  assert.equal(result.result, "FAIL");
  assert.equal(result.matchedRecipientHashes.length, 1);
});

test("다른 scope의 출금은 도출되지 않는다 (scope 격리)", () => {
  const result = runScan(mockChain, cleanScope, makeRequest());
  for (const record of result.derivedRecords) {
    assert.ok(record.recipientAddress.startsWith("zs1mockCleanRecipient"));
  }
});

test("체인에 없는 구간을 스캔하면 완전성 위반으로 throw한다", () => {
  const request = makeRequest({ startHeight: 9_999_990, endHeight: 9_999_999 });
  assert.throws(() => runScan(mockChain, cleanScope, request), /완전성 위반/);
});
