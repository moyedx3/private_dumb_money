/**
 * screening artifact 조립.
 *
 * ScanResult를 정책·입금·viewing scope·chain source에 바인딩하고 attestation으로
 * 서명해 거래소로 나갈 ScreeningArtifact를 만든다.
 *
 * 자세한 설명: docs/implementation/artifact.md
 */
import type {
  BlockRange,
  ChainSource,
  ScanResult,
  ScreeningArtifact,
  ScreeningRequest,
  ScreeningResult,
  ViewingScope,
} from "./types.ts";
import {
  chainSourceHash,
  depositIntentHash,
  policyHash,
  viewingScopeCommitment,
} from "./hash.ts";
import type { AttestationProvider } from "./attestation.ts";

/**
 * 현재 artifact 포맷 버전.
 * 1.1: binding payload에 chainSource hash 포함 (D9).
 */
export const ARTIFACT_VERSION = "1.1";

/**
 * attestation이 서명할 binding payload. artifact 핵심 필드(attestation 제외)
 * 전체를 정규 직렬화한다. 한 필드라도 바뀌면 서명 검증이 깨진다.
 */
function computeBindingPayload(
  version: string,
  policyHashValue: string,
  depositIntentHashValue: string,
  scanRange: BlockRange,
  chainSource: ChainSource,
  viewingScopeCommitmentValue: string,
  result: ScreeningResult,
): string {
  return JSON.stringify([
    "screeningArtifact",
    version,
    policyHashValue,
    depositIntentHashValue,
    scanRange.startHeight,
    scanRange.endHeight,
    chainSourceHash(chainSource),
    viewingScopeCommitmentValue,
    result,
  ]);
}

/** 완성된 artifact로부터 binding payload를 다시 계산한다 (검증기용). */
export function artifactBindingPayload(artifact: ScreeningArtifact): string {
  return computeBindingPayload(
    artifact.version,
    artifact.policyHash,
    artifact.depositIntentHash,
    artifact.scanRange,
    artifact.chainSource,
    artifact.viewingScopeCommitment,
    artifact.result,
  );
}

/**
 * ScanResult를 ScreeningArtifact로 조립한다.
 * - 정책/입금/viewing scope/chain source를 해시로 바인딩
 * - binding payload를 attestation으로 서명 (attestation은 비동기 — await)
 *
 * ScanResult.derivedRecords(수취인·금액)는 artifact에 담기지 않는다 (프라이버시 경계).
 * salt는 viewing scope commitment의 hiding을 위한 사용자 제공 random 값(D11) —
 * artifact에는 들어가지 않는다.
 */
export async function assembleArtifact(
  scanResult: ScanResult,
  scope: ViewingScope,
  salt: string,
  request: ScreeningRequest,
  attestation: AttestationProvider,
): Promise<ScreeningArtifact> {
  const version = ARTIFACT_VERSION;
  const policyHashValue = policyHash(request.policy);
  const depositIntentHashValue = depositIntentHash(request.depositIntent);
  const scanRange = scanResult.scannedRange;
  const chainSource = request.chainSource;
  const viewingScopeCommitmentValue = viewingScopeCommitment(scope, salt);
  const result = scanResult.result;

  const payload = computeBindingPayload(
    version,
    policyHashValue,
    depositIntentHashValue,
    scanRange,
    chainSource,
    viewingScopeCommitmentValue,
    result,
  );
  const quote = await attestation.attest(payload, request.nonce);

  return {
    version,
    policyHash: policyHashValue,
    depositIntentHash: depositIntentHashValue,
    scanRange,
    chainSource,
    viewingScopeCommitment: viewingScopeCommitmentValue,
    result,
    attestation: quote,
  };
}
