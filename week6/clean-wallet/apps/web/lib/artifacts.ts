/**
 * artifact 처리 — 저장 가능한 형태로 정규화하고, DB에 들어갈 요약을 만들고,
 * 거래소 관점에서 재검증한다.
 *
 * 보안 경계(중요): 여기서 다루는 ScreeningArtifact에는 UFVK·salt·raw 수취인 주소가
 * 절대 들어가지 않는다 (packages/core/src/types.ts 참고). pickArtifact는 알려진
 * artifact 필드만 화이트리스트로 추출하므로 `_debug` 같은 부가 필드나 실수로 섞인
 * 비밀값이 DB에 저장되는 것을 구조적으로 막는다.
 */
import { createHash } from "node:crypto";
import {
  demoRequest,
  SimulatedAttestation,
  trustedMeasurements,
  verifyArtifact,
  type AttestationProvider,
  type AttestationProviderId,
  type AttestationQuote,
  type ScreeningArtifact,
  type ScreeningRequest,
} from "@clean-wallet/core";
import type { WebVerification, WebVerificationCheck } from "../app/action-types";

/** DynamoDB에 저장되는 항목. 조회·정렬용 필드 + artifact 본체. */
export type StoredArtifact = {
  id: string;
  result: ScreeningArtifact["result"];
  provider: AttestationProviderId;
  codeMeasurement: string;
  scanStartHeight: number;
  scanEndHeight: number;
  chainSourceKind: ScreeningArtifact["chainSource"]["kind"];
  network?: string;
  createdAt: string; // ISO 8601 (서버 시각)
  savedVia: "cli" | "web";
  label?: string;
  artifact: ScreeningArtifact;
};

function asString(obj: Record<string, unknown>, key: string): string {
  const v = obj[key];
  if (typeof v !== "string" || v.length === 0) {
    throw new Error(`artifact.${key} 누락 또는 형식 오류 (string 필요)`);
  }
  return v;
}

/**
 * 임의의 입력에서 ScreeningArtifact 필드만 화이트리스트로 추출·검증한다.
 * 형식이 안 맞으면 throw. `_debug` 등 부가 필드는 자동으로 버려진다.
 */
export function pickArtifact(input: unknown): ScreeningArtifact {
  if (typeof input !== "object" || input === null) {
    throw new Error("artifact는 객체여야 합니다");
  }
  const o = input as Record<string, unknown>;

  const scanRange = o.scanRange as Record<string, unknown> | undefined;
  if (
    !scanRange ||
    typeof scanRange.startHeight !== "number" ||
    typeof scanRange.endHeight !== "number"
  ) {
    throw new Error("artifact.scanRange 누락 또는 형식 오류 ({startHeight,endHeight})");
  }

  const cs = o.chainSource as Record<string, unknown> | undefined;
  if (!cs || (cs.kind !== "mock" && cs.kind !== "lightwalletd")) {
    throw new Error("artifact.chainSource 누락 또는 형식 오류");
  }
  const chainSource: ScreeningArtifact["chainSource"] =
    cs.kind === "lightwalletd"
      ? {
          kind: "lightwalletd",
          url: asString(cs, "url"),
          network: cs.network === "main" ? "main" : "test",
        }
      : { kind: "mock" };

  const result = o.result;
  if (result !== "PASS" && result !== "FAIL") {
    throw new Error('artifact.result는 "PASS" 또는 "FAIL"이어야 합니다');
  }

  const at = o.attestation as Record<string, unknown> | undefined;
  if (!at) throw new Error("artifact.attestation 누락");
  const provider = at.provider;
  if (provider !== "simulated" && provider !== "phala-tdx" && provider !== "aws-nitro") {
    throw new Error(`artifact.attestation.provider 형식 오류: ${String(provider)}`);
  }
  if (typeof at.timestamp !== "number") {
    throw new Error("artifact.attestation.timestamp 형식 오류 (number)");
  }
  const attestation: AttestationQuote = {
    provider,
    codeMeasurement: asString(at, "codeMeasurement"),
    quote: asString(at, "quote"),
    nonce: asString(at, "nonce"),
    timestamp: at.timestamp,
  };

  return {
    version: asString(o, "version"),
    policyHash: asString(o, "policyHash"),
    depositIntentHash: asString(o, "depositIntentHash"),
    scanRange: {
      startHeight: scanRange.startHeight,
      endHeight: scanRange.endHeight,
    },
    chainSource,
    viewingScopeCommitment: asString(o, "viewingScopeCommitment"),
    result,
    attestation,
  };
}

/** artifact의 정규 직렬화 해시 — 동일 artifact 재저장 시 같은 id(멱등). */
export function artifactId(a: ScreeningArtifact): string {
  return createHash("sha256").update(JSON.stringify(a)).digest("hex").slice(0, 32);
}

/** artifact를 DB 저장 항목으로 요약한다. */
export function toStored(
  a: ScreeningArtifact,
  opts: { savedVia: "cli" | "web"; label?: string; createdAt: string },
): StoredArtifact {
  return {
    id: artifactId(a),
    result: a.result,
    provider: a.attestation.provider,
    codeMeasurement: a.attestation.codeMeasurement,
    scanStartHeight: a.scanRange.startHeight,
    scanEndHeight: a.scanRange.endHeight,
    chainSourceKind: a.chainSource.kind,
    network: a.chainSource.kind === "lightwalletd" ? a.chainSource.network : undefined,
    createdAt: opts.createdAt,
    savedVia: opts.savedVia,
    label: opts.label,
    artifact: a,
  };
}

/**
 * artifact를 검증할 때 비교 기준이 되는 요청을 재구성한다.
 *
 * 스캐너(apps/scanner/src/server.ts)는 mock·real 모두 demoRequest를 베이스로 쓰고,
 * real일 때만 policy.approvedChainSources / scanRange / chainSource를 덮어쓴다.
 * 따라서 검증 요청은 demoRequest + artifact의 chainSource·scanRange로 정확히 복원된다.
 * (assembleArtifact가 policyHash·depositIntentHash·nonce를 이 요청에서 파생하므로 일치.)
 */
export function requestForArtifact(a: ScreeningArtifact): ScreeningRequest {
  const real = a.chainSource.kind === "lightwalletd";
  return {
    ...demoRequest,
    policy: real
      ? { ...demoRequest.policy, approvedChainSources: [a.chainSource] }
      : demoRequest.policy,
    scanRange: a.scanRange,
    chainSource: a.chainSource,
  };
}

/**
 * phala-tdx 등 TEE attestation을 위한 위임 provider.
 * 순수 JS로 TDX quote 전체(DCAP)를 검증하는 것은 비현실적이라(deploy-phala.md §6),
 * 구조·measurement 일치만 확인하고 quote의 암호학 검증은 Phala verifier에 위임한다.
 */
class DelegatedTdxAttestation implements AttestationProvider {
  readonly providerId: AttestationProviderId;
  constructor(id: AttestationProviderId) {
    this.providerId = id;
  }
  async getMeasurement(): Promise<string> {
    return "";
  }
  async attest(): Promise<AttestationQuote> {
    throw new Error("위임 provider는 attest를 수행하지 않습니다");
  }
  async verify(
    quote: AttestationQuote,
    _payload: string,
    expectedMeasurement: string,
  ): Promise<boolean> {
    return (
      quote.provider === this.providerId &&
      quote.codeMeasurement === expectedMeasurement &&
      typeof quote.quote === "string" &&
      quote.quote.length > 0
    );
  }
}

const DELEGATED_CHECK_NAMES = new Set(["code measurement 신뢰", "attestation 서명"]);

/**
 * artifact를 거래소 관점에서 재검증한다.
 * simulated artifact는 attestation 서명까지 전부 검증하고,
 * phala-tdx 등은 measurement·quote 검증을 Phala verifier에 위임(delegated)한다.
 */
export async function verifyStored(a: ScreeningArtifact): Promise<WebVerification> {
  const measurement = a.attestation.codeMeasurement;
  const isSim = a.attestation.provider === "simulated";
  const provider: AttestationProvider = isSim
    ? new SimulatedAttestation(measurement)
    : new DelegatedTdxAttestation(a.attestation.provider);
  // simulated는 거래소가 신뢰하는 measurement 목록으로 검증.
  // delegated는 measurement 신뢰 판단 자체를 Phala verifier에 위임하므로 아래에서 표시만 바꾼다.
  const trusted = isSim ? trustedMeasurements : [measurement];

  const base = await verifyArtifact(a, requestForArtifact(a), trusted, provider);

  const checks: WebVerificationCheck[] = base.checks.map((c) =>
    !isSim && DELEGATED_CHECK_NAMES.has(c.name)
      ? {
          name: c.name,
          ok: c.ok,
          delegated: true,
          detail:
            "Phala verifier 외부 검증 위임 — 이 페이지는 TDX quote를 암호학적으로 검증하지 않습니다 (docs/deploy-phala.md §6)",
        }
      : { name: c.name, ok: c.ok, detail: c.detail },
  );

  const ok = checks.filter((c) => !c.delegated).every((c) => c.ok);

  return {
    ok,
    trustedResult: ok ? a.result : null,
    attestationMode: isSim ? "verified" : "delegated",
    checks,
    note: isSim
      ? undefined
      : "measurement·quote(attestation)는 Phala verifier 외부 검증 대상입니다. submit-ufvk의 --expected-mrtd/--expected-rtmr3 또는 cloud-api.phala.com으로 확인하세요.",
  };
}
