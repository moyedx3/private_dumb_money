/**
 * CLI: verify — artifact JSON을 읽어 검증하고 결과를 출력한다.
 *
 * 사용법: node verify.ts [artifact파일]
 * 자세한 설명: docs/implementation/cli.md
 */
import { readFileSync } from "node:fs";
import type { ScreeningArtifact } from "../types.ts";
import { SimulatedAttestation } from "../attestation.ts";
import { verifyArtifact } from "../verifier.ts";
import { demoRequest, trustedMeasurements } from "../demo.ts";

const inFile = process.argv[2] ?? "artifact.json";
const artifact = JSON.parse(readFileSync(inFile, "utf8")) as ScreeningArtifact;

const attestation = new SimulatedAttestation();
const result = await verifyArtifact(artifact, demoRequest, trustedMeasurements, attestation);

console.log(`[verify] artifact: ${inFile}`);
console.log(`[verify] 검증 항목:`);
for (const check of result.checks) {
  console.log(`  ${check.ok ? "✔" : "✗"} ${check.name} — ${check.detail}`);
}
console.log(`[verify] 종합            : ${result.ok ? "검증 통과" : "검증 실패"}`);
console.log(`[verify] 신뢰 가능한 결과 : ${result.trustedResult ?? "(없음 — 검증 실패)"}`);

process.exitCode = result.ok ? 0 : 1;
