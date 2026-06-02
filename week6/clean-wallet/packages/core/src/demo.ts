/**
 * 데모/연동용 고정 스크리닝 요청. 거래소가 만든 요청을 mock한다.
 * CLI와 웹 데모가 같은 요청을 공유한다.
 *
 * 자세한 설명: docs/implementation/cli.md
 */
import type { ChainSource, DepositIntent, ScreeningRequest } from "./types.ts";
import { depositIntentHash, hashSanctionedSet } from "./hash.ts";
import { DEFAULT_SCANNER_MEASUREMENT } from "./attestation.ts";
import { mockSanctionedAddresses, mockScanRange } from "./mock-chain.ts";

const depositIntent: DepositIntent = {
  exchangeName: "MockExchange",
  exchangeDepositAddress: "t1MockExchangeDeposit000000000000000",
  depositAmountZat: "100000000",
  nonce: "deposit-nonce-001",
  expiryUnix: 1_900_000_000,
};

/** 데모 mock chain source (D9). */
const demoMockChainSource: ChainSource = { kind: "mock" };

/** 거래소가 신뢰하는 스캐너 measurement 목록. */
export const trustedMeasurements: readonly string[] = [DEFAULT_SCANNER_MEASUREMENT];

/**
 * 데모용 viewing scope commitment salt.
 *
 * D11: 실 경로에선 사용자가 random 32바이트 salt를 생성·보관해야 한다.
 * mock 데모에선 고정값으로 충분 (commitment의 hiding이 데모 목적이 아님).
 */
export const demoViewingScopeSalt = "demo-salt-do-not-use-in-production";

/** 데모 스크리닝 요청 (CLI·웹이 공유). */
export const demoRequest: ScreeningRequest = {
  policy: {
    policyName: "demo-offramp-policy",
    policyVersion: "1",
    auditRange: mockScanRange,
    sanctionedAddressSetHash: hashSanctionedSet(mockSanctionedAddresses),
    scannerMeasurement: DEFAULT_SCANNER_MEASUREMENT,
    depositIntentHash: depositIntentHash(depositIntent),
    approvedChainSources: [demoMockChainSource],
  },
  sanctionedAddresses: mockSanctionedAddresses,
  depositIntent,
  scanRange: mockScanRange,
  chainSource: demoMockChainSource,
  nonce: "request-nonce-001",
};
