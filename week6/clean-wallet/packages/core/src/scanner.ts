/**
 * 스캐너 코어 — viewing scope로 블록 구간을 스캔해 ScanResult를 만든다.
 *
 * 이 프로젝트의 핵심. 출금 수취인을 도출하고 제재 목록과 대조한다.
 * 자세한 설명: docs/implementation/scanner.md
 */
import type {
  BlockRange,
  DerivedRecord,
  ScanResult,
  ScreeningRequest,
  ScreeningResult,
  ViewingScope,
} from "./types.ts";
import type { MockBlock, MockChain } from "./mock-chain.ts";
import { hashAddress } from "./hash.ts";

/**
 * 요청 구간 [start, end]의 블록을 빠짐없이, 순서대로 모은다.
 * 한 height라도 빠졌거나 prevHash 연결이 끊기면 throw한다 — 이것이 코드 상의
 * completeness 강제 지점이다 (docs/decisions.md D6).
 */
function collectBlocksInRange(chain: MockChain, range: BlockRange): MockBlock[] {
  if (range.startHeight > range.endHeight) {
    throw new Error(`잘못된 스캔 구간: start ${range.startHeight} > end ${range.endHeight}`);
  }
  const byHeight = new Map<number, MockBlock>(
    chain.blocks.map((b): [number, MockBlock] => [b.height, b]),
  );
  const collected: MockBlock[] = [];
  let prev: MockBlock | undefined;
  for (let height = range.startHeight; height <= range.endHeight; height++) {
    const block = byHeight.get(height);
    if (block === undefined) {
      throw new Error(`완전성 위반: height ${height} 블록이 체인에 없음`);
    }
    if (prev !== undefined && block.prevHash !== prev.blockHash) {
      throw new Error(`체인 불연속: height ${height}의 prevHash가 height ${prev.height}와 불일치`);
    }
    collected.push(block);
    prev = block;
  }
  return collected;
}

/**
 * viewing scope로 체인 구간을 스캔해 ScanResult를 만든다.
 *
 * 1. 구간 전체 블록 수집 (빠짐 있으면 throw)
 * 2. 각 블록의 각 tx의 각 output 중, 이 scope가 보낸 것을 출금 record로 도출
 * 3. 수취인 주소를 해싱
 * 4. 제재 주소 집합과 대조 → PASS / FAIL
 */
export function runScan(
  chain: MockChain,
  scope: ViewingScope,
  request: ScreeningRequest,
): ScanResult {
  const blocks = collectBlocksInRange(chain, request.scanRange);

  const derivedRecords: DerivedRecord[] = [];
  for (const block of blocks) {
    for (const tx of block.txs) {
      for (const output of tx.shieldedOutputs) {
        // 이 scope가 보낸 출금만 도출 (실제로는 OVK 복호화 성공에 해당).
        if (output.senderScopeId !== scope.scopeId) {
          continue;
        }
        derivedRecords.push({
          txid: tx.txid,
          blockHeight: block.height,
          direction: "outgoing",
          recipientAddress: output.recipientAddress,
          recipientHash: hashAddress(output.recipientAddress),
          amountZat: output.amountZat,
        });
      }
    }
  }

  const sanctionedHashes = new Set(
    request.sanctionedAddresses.map((a) => hashAddress(a.address)),
  );
  const matched = new Set<string>();
  for (const record of derivedRecords) {
    if (sanctionedHashes.has(record.recipientHash)) {
      matched.add(record.recipientHash);
    }
  }

  const result: ScreeningResult = matched.size > 0 ? "FAIL" : "PASS";
  return {
    scannedRange: request.scanRange,
    derivedRecords,
    result,
    matchedRecipientHashes: [...matched],
  };
}
