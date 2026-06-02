/**
 * 해시 헬퍼 — SHA-256 기반.
 *
 * 데이터 바인딩(policyHash, depositIntentHash, chainSourceHash),
 * 비공개 비교(수취인/제재 주소 해시),
 * commitment(viewingScopeCommitment with salt — D11)에 쓰인다.
 *
 * 자세한 설명: docs/implementation/hashing.md
 */
import { createHash } from "node:crypto";
import type {
  ChainSource,
  DepositIntent,
  SanctionedAddress,
  ScreeningPolicy,
  ViewingScope,
} from "./types.ts";

/** UTF-8 문자열의 SHA-256 16진 해시. */
function sha256Hex(input: string): string {
  return createHash("sha256").update(input, "utf8").digest("hex");
}

/**
 * 도메인 태그가 붙은 필드 목록을 정규 직렬화해 해싱한다.
 * 태그는 종류가 다른 데이터끼리의 해시 충돌(domain collision)을 막는다.
 * 배열 JSON.stringify는 구분자 모호성이 없어 안전하다.
 */
function hashFields(domain: string, fields: readonly (string | number)[]): string {
  return sha256Hex(JSON.stringify([domain, ...fields]));
}

/**
 * 주소 정규화. 현재는 앞뒤 공백만 제거한다.
 * 주의: 소문자화하지 않는다 — Zcash transparent 주소(t1...)는 Base58이라
 * 대소문자를 구분한다. 소문자화하면 주소가 깨진다.
 */
export function normalizeAddress(address: string): string {
  return address.trim();
}

/** 정규화된 주소의 해시. 수취인·제재 주소에 동일하게 적용된다. */
export function hashAddress(address: string): string {
  return hashFields("address", [normalizeAddress(address)]);
}

/** 제재 주소 집합 전체의 해시 (입력 순서와 무관). */
export function hashSanctionedSet(addresses: readonly SanctionedAddress[]): string {
  const memberHashes = addresses.map((a) => hashAddress(a.address)).sort();
  return hashFields("sanctionedSet", memberHashes);
}

/** 입금 요청의 해시. artifact를 특정 입금 건에 묶는다. */
export function depositIntentHash(intent: DepositIntent): string {
  return hashFields("depositIntent", [
    intent.exchangeDepositAddress,
    intent.depositAmountZat,
    intent.nonce,
    intent.expiryUnix,
  ]);
}

/**
 * 단일 chain source의 해시. 종류별로 다른 필드를 갖는 discriminated union을
 * 안정적으로 직렬화한다.
 */
export function chainSourceHash(cs: ChainSource): string {
  if (cs.kind === "mock") {
    return hashFields("chainSource", ["mock"]);
  }
  return hashFields("chainSource", ["lightwalletd", cs.url, cs.network]);
}

/** 허용된 chain source 집합 전체의 해시 (입력 순서와 무관). */
export function approvedChainSourcesHash(list: readonly ChainSource[]): string {
  const memberHashes = list.map(chainSourceHash).sort();
  return hashFields("approvedChainSources", memberHashes);
}

/** 스크리닝 정책의 해시. artifact를 특정 정책에 묶는다. (D9: approvedChainSources 포함) */
export function policyHash(policy: ScreeningPolicy): string {
  return hashFields("policy", [
    policy.policyName,
    policy.policyVersion,
    policy.auditRange.startHeight,
    policy.auditRange.endHeight,
    policy.sanctionedAddressSetHash,
    policy.scannerMeasurement,
    policy.depositIntentHash,
    approvedChainSourcesHash(policy.approvedChainSources),
  ]);
}

/**
 * viewing scope의 commitment — `hash(scope || salt)`.
 *
 * D11: salt는 사용자가 생성·보관(요청 시 함께 제출)하는 random 값. artifact에는
 * commitment만 들어가고 salt는 들어가지 않아 — 거래소가 알려진 UFVK 리스트로
 * 사전 매칭하는 걸 막는다(hiding). 사용자는 salt를 보관해 두면 나중에 자기 자신에게
 * "이 commitment는 내 scope다"를 증명할 수 있다.
 *
 * 테스트·mock 경로는 salt = "" 사용해도 무방(완전한 hiding은 실 경로에서만 필요).
 */
export function viewingScopeCommitment(scope: ViewingScope, salt: string): string {
  return hashFields("viewingScope", [
    scope.scopeId,
    scope.network,
    scope.viewingKey,
    salt,
  ]);
}
