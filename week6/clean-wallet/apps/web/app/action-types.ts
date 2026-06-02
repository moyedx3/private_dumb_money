import type { ScreeningArtifact, ScreeningResult } from "@clean-wallet/core";

/** runScanAction의 결과. */
export type ScanActionResult = {
  artifact: ScreeningArtifact;
  recordCount: number;
  result: ScreeningResult;
};

/**
 * 검증 항목 1건. core의 VerificationCheck에 `delegated` 플래그를 더한 것.
 * delegated=true면 이 페이지가 직접 암호학 검증을 하지 않고 Phala verifier에 위임한 항목.
 */
export type WebVerificationCheck = {
  name: string;
  ok: boolean;
  detail: string;
  delegated?: boolean;
};

/**
 * 웹에서 artifact를 재검증한 결과.
 * - attestationMode "verified": simulated artifact — ed25519 서명까지 전부 검증.
 * - attestationMode "delegated": phala-tdx 등 — measurement·quote는 Phala verifier에 위임,
 *   바인딩(정책·입금·구간·nonce·chainSource)만 이 페이지에서 검증.
 */
export type WebVerification = {
  /** 위임 항목을 제외한, 로컬에서 검증 가능한 모든 항목 통과 여부. */
  ok: boolean;
  trustedResult: ScreeningResult | null;
  attestationMode: "verified" | "delegated";
  checks: WebVerificationCheck[];
  /** delegated 모드일 때 사용자에게 보여줄 안내. */
  note?: string;
};

/** verifyArtifactAction / verifyStoredArtifactAction의 결과. */
export type VerifyActionResult =
  | { ok: true; verification: WebVerification }
  | { ok: false; error: string };

/** artifact 저장(saveArtifactAction / 업로드) 결과. */
export type SaveActionResult =
  | { ok: true; id: string }
  | { ok: false; error: string };
