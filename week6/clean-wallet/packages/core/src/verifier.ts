/**
 * 검증기 — 거래소 측 artifact 검증.
 *
 * 스캐너가 보낸 ScreeningArtifact가 믿을 만한지 항목별로 확인한다.
 * 이게 있어야 "거래소가 PASS/FAIL을 신뢰"하는 고리가 닫힌다.
 *
 * attestation 검증이 비동기라 verifyArtifact도 async다.
 * 자세한 설명: docs/implementation/verifier.md
 */
import type { ScreeningArtifact, ScreeningRequest, ScreeningResult } from "./types.ts";
import { chainSourceHash, depositIntentHash, policyHash } from "./hash.ts";
import { artifactBindingPayload } from "./artifact.ts";
import type { AttestationProvider } from "./attestation.ts";

/** 검증 항목 하나의 결과. */
export type VerificationCheck = {
  name: string;
  ok: boolean;
  detail: string;
};

/** artifact 검증 결과. */
export type VerificationResult = {
  /** 모든 검증 항목 통과 여부. */
  ok: boolean;
  /** ok이면 신뢰할 수 있는 PASS/FAIL, 아니면 null. */
  trustedResult: ScreeningResult | null;
  checks: VerificationCheck[];
};

/**
 * artifact를 거래소가 만든 요청 기준으로 검증한다. (architecture §6 순서)
 *
 * @param artifact 스캐너가 보낸 artifact
 * @param request 거래소가 원래 만든 스크리닝 요청 (비교 기준)
 * @param trustedMeasurements 거래소가 신뢰하는 스캐너 code measurement 목록
 * @param attestation attestation 서명 검증에 쓸 provider
 */
export async function verifyArtifact(
  artifact: ScreeningArtifact,
  request: ScreeningRequest,
  trustedMeasurements: readonly string[],
  attestation: AttestationProvider,
): Promise<VerificationResult> {
  const checks: VerificationCheck[] = [];
  const measurement = artifact.attestation.codeMeasurement;

  // 1. code measurement이 신뢰 목록에 있는가
  const measurementTrusted = trustedMeasurements.includes(measurement);
  checks.push({
    name: "code measurement 신뢰",
    ok: measurementTrusted,
    detail: measurementTrusted
      ? `신뢰 목록에 있음: ${measurement}`
      : `신뢰하지 않는 measurement: ${measurement}`,
  });

  // 2. attestation 서명이 유효한가 (변조·위조 탐지)
  const signatureValid = await attestation.verify(
    artifact.attestation,
    artifactBindingPayload(artifact),
    measurement,
  );
  checks.push({
    name: "attestation 서명",
    ok: signatureValid,
    detail: signatureValid ? "서명 유효" : "서명 무효 — artifact가 변조되었거나 위조됨",
  });

  // 3. policyHash가 요청한 정책과 일치하는가
  const policyOk = artifact.policyHash === policyHash(request.policy);
  checks.push({
    name: "정책 바인딩",
    ok: policyOk,
    detail: policyOk ? "요청 정책과 일치" : "policyHash 불일치",
  });

  // 4. depositIntentHash가 현재 입금 요청과 일치하는가
  const depositOk = artifact.depositIntentHash === depositIntentHash(request.depositIntent);
  checks.push({
    name: "입금 바인딩",
    ok: depositOk,
    detail: depositOk
      ? "현재 입금 요청과 일치"
      : "depositIntentHash 불일치 — 다른 입금 건의 artifact",
  });

  // 5. scanRange가 요청한 구간과 일치하는가
  const rangeOk =
    artifact.scanRange.startHeight === request.scanRange.startHeight &&
    artifact.scanRange.endHeight === request.scanRange.endHeight;
  checks.push({
    name: "스캔 구간",
    ok: rangeOk,
    detail: rangeOk ? "요청 구간과 일치" : "scanRange 불일치",
  });

  // 6. nonce가 이번 요청의 nonce와 일치하는가 (재생 방지)
  const nonceOk = artifact.attestation.nonce === request.nonce;
  checks.push({
    name: "nonce (재생 방지)",
    ok: nonceOk,
    detail: nonceOk ? "요청 nonce와 일치" : "nonce 불일치 — 재생된 artifact",
  });

  // 7. chainSource가 요청과 일치하고 정책 allowlist에 들어 있는가 (D9)
  const reqCsHash = chainSourceHash(request.chainSource);
  const artCsHash = chainSourceHash(artifact.chainSource);
  const sameAsRequest = artCsHash === reqCsHash;
  const approvedHashes = new Set(
    request.policy.approvedChainSources.map(chainSourceHash),
  );
  const inAllowlist = approvedHashes.has(artCsHash);
  const chainSourceOk = sameAsRequest && inAllowlist;
  checks.push({
    name: "chain source (D9)",
    ok: chainSourceOk,
    detail: !sameAsRequest
      ? "artifact.chainSource가 request.chainSource와 불일치"
      : !inAllowlist
        ? "chainSource가 policy.approvedChainSources에 없음"
        : "요청과 일치하고 정책 허용 목록에 있음",
  });

  const ok = checks.every((c) => c.ok);
  return {
    ok,
    trustedResult: ok ? artifact.result : null,
    checks,
  };
}
