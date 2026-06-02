"use server";

import {
  assembleArtifact,
  cleanScope,
  demoRequest,
  demoViewingScopeSalt,
  mockChain,
  runScan,
  SimulatedAttestation,
  taintedScope,
} from "@clean-wallet/core";
import type {
  SaveActionResult,
  ScanActionResult,
  VerifyActionResult,
} from "./action-types";
import { pickArtifact, toStored, verifyStored } from "../lib/artifacts";
import { getArtifact, putArtifact } from "../lib/dynamo";

/** viewing scope를 스캔해 screening artifact를 만든다 (로컬 sim 데모). */
export async function runScanAction(
  scopeName: "clean" | "tainted",
): Promise<ScanActionResult> {
  const scope = scopeName === "tainted" ? taintedScope : cleanScope;
  const attestation = new SimulatedAttestation();
  const scan = runScan(mockChain, scope, demoRequest);
  const artifact = await assembleArtifact(scan, scope, demoViewingScopeSalt, demoRequest, attestation);
  return { artifact, recordCount: scan.derivedRecords.length, result: scan.result };
}

/**
 * artifact JSON 문자열을 검증한다.
 * mock·real(phala-tdx) artifact 모두 지원 — 요청을 artifact로부터 복원해 비교한다.
 */
export async function verifyArtifactAction(
  artifactJson: string,
): Promise<VerifyActionResult> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(artifactJson);
  } catch {
    return { ok: false, error: "JSON 파싱 실패 — artifact 형식을 확인하세요." };
  }
  try {
    const artifact = pickArtifact(parsed);
    return { ok: true, verification: await verifyStored(artifact) };
  } catch (e) {
    return { ok: false, error: (e as Error).message };
  }
}

/** artifact JSON을 검증·정규화해 DB에 저장한다 (웹 업로드 경로). */
export async function saveArtifactAction(
  artifactJson: string,
  label?: string,
): Promise<SaveActionResult> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(artifactJson);
  } catch {
    return { ok: false, error: "JSON 파싱 실패 — artifact 형식을 확인하세요." };
  }
  try {
    const artifact = pickArtifact(parsed);
    const stored = toStored(artifact, {
      savedVia: "web",
      label: label?.trim() || undefined,
      createdAt: new Date().toISOString(),
    });
    await putArtifact(stored);
    return { ok: true, id: stored.id };
  } catch (e) {
    return { ok: false, error: (e as Error).message };
  }
}

/** 저장된 artifact를 id로 재검증한다 (상세 페이지). */
export async function verifyStoredArtifactAction(
  id: string,
): Promise<VerifyActionResult> {
  const item = await getArtifact(id);
  if (!item) return { ok: false, error: "해당 id의 artifact가 없습니다." };
  return { ok: true, verification: await verifyStored(item.artifact) };
}
