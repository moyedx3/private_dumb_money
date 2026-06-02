/**
 * mock Zcash 체인 — 실제 체인을 단순화한 가짜 데이터.
 *
 * 실제 Zcash 스캔(FVK/OVK + full transaction)은 무거우므로 MVP는 mock으로 대체한다.
 * (docs/decisions.md D5)
 *
 * 자세한 설명: docs/implementation/mock-chain.md
 */
import type {
  BlockRange,
  SanctionedAddress,
  ViewingScope,
  ZcashNetwork,
} from "./types.ts";

/** mock shielded output — 실제 Zcash 출력의 단순화 모델. */
export type MockShieldedOutput = {
  /**
   * 이 output을 만든(보낸) viewing scope의 id.
   * 실제 Zcash의 out_ciphertext에 대응 — 이 scope의 OVK로만 도출된다.
   */
  senderScopeId: string;
  /** OVK 복호화로 복원되는 수취인 주소 (mock). */
  recipientAddress: string;
  amountZat: string;
};

export type MockTransaction = {
  txid: string;
  shieldedOutputs: MockShieldedOutput[];
};

export type MockBlock = {
  height: number;
  blockHash: string;
  prevHash: string;
  txs: MockTransaction[];
};

/** mock Zcash 체인. blocks는 height 오름차순이며 구간 내 빠짐이 없다. */
export type MockChain = {
  network: ZcashNetwork;
  blocks: MockBlock[];
};

// --- mock viewing scopes ---

/** 깨끗한 지갑 — 제재 주소로 보낸 적 없음. */
export const cleanScope: ViewingScope = {
  scopeId: "scope-clean",
  network: "main",
  viewingKey: "uview-mock-clean-93f1a0",
};

/** 오염된 지갑 — 제재 주소로 보낸 출금이 있음. */
export const taintedScope: ViewingScope = {
  scopeId: "scope-tainted",
  network: "main",
  viewingKey: "uview-mock-tainted-2b77ce",
};

// --- mock 제재 주소 ---

const SANCTIONED_ALPHA = "t1MockSanctionedAlpha000000000000000";

export const mockSanctionedAddresses: SanctionedAddress[] = [
  { label: "Mock SDN ZEC Alpha", asset: "ZEC", address: SANCTIONED_ALPHA },
  { label: "Mock SDN ZEC Beta", asset: "ZEC", address: "t1MockSanctionedBeta0000000000000000" },
];

// --- mock 체인 ---

function out(
  senderScopeId: string,
  recipientAddress: string,
  amountZat: string,
): MockShieldedOutput {
  return { senderScopeId, recipientAddress, amountZat };
}

function block(height: number, txs: MockTransaction[]): MockBlock {
  return {
    height,
    blockHash: `blockhash-${height}`,
    prevHash: `blockhash-${height - 1}`,
    txs,
  };
}

/**
 * 높이 2,500,000 ~ 2,500,005 의 6블록 mock 체인.
 * scope-clean 스캔 → PASS, scope-tainted 스캔 → FAIL 이 나오도록 구성했다.
 *
 * - 2,500,002 는 shielded 출력 없는 빈 블록 (빈 블록 처리 확인용).
 * - scope-other 출력은 우리 scope가 아니므로 스캔 시 무시된다 (noise).
 */
export const mockChain: MockChain = {
  network: "main",
  blocks: [
    block(2_500_000, [
      { txid: "tx-a1", shieldedOutputs: [out("scope-clean", "zs1mockCleanRecipientA00000000000000", "50000000")] },
    ]),
    block(2_500_001, [
      { txid: "tx-b1", shieldedOutputs: [out("scope-clean", "zs1mockCleanRecipientB00000000000000", "12000000")] },
      { txid: "tx-b2", shieldedOutputs: [out("scope-other", "zs1mockOtherRecipient0000000000000000", "9900000")] },
    ]),
    block(2_500_002, []),
    block(2_500_003, [
      { txid: "tx-d1", shieldedOutputs: [out("scope-tainted", SANCTIONED_ALPHA, "30000000")] },
    ]),
    block(2_500_004, [
      { txid: "tx-e1", shieldedOutputs: [out("scope-tainted", "zs1mockCleanRecipientC00000000000000", "7000000")] },
    ]),
    block(2_500_005, [
      { txid: "tx-f1", shieldedOutputs: [out("scope-clean", "zs1mockCleanRecipientD00000000000000", "25000000")] },
    ]),
  ],
};

/** mockChain 전체를 덮는 스캔 구간. */
export const mockScanRange: BlockRange = { startHeight: 2_500_000, endHeight: 2_500_005 };
