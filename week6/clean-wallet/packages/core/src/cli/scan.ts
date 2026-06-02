/**
 * CLI: scan — viewing scope를 스캔해 screening artifact를 JSON으로 출력한다.
 *
 * 사용법: node scan.ts [clean|tainted] [출력파일]
 * 자세한 설명: docs/implementation/cli.md
 */
import { writeFileSync } from "node:fs";
import { runScan } from "../scanner.ts";
import { assembleArtifact } from "../artifact.ts";
import { SimulatedAttestation } from "../attestation.ts";
import { cleanScope, mockChain, taintedScope } from "../mock-chain.ts";
import { demoRequest, demoViewingScopeSalt } from "../demo.ts";

const scopeArg = process.argv[2] ?? "clean";
const outFile = process.argv[3] ?? "artifact.json";

if (scopeArg !== "clean" && scopeArg !== "tainted") {
  console.error(`사용법: scan [clean|tainted] [출력파일]  (받은 값: ${scopeArg})`);
  process.exit(1);
}

const scope = scopeArg === "tainted" ? taintedScope : cleanScope;
const attestation = new SimulatedAttestation();

const scan = runScan(mockChain, scope, demoRequest);
const artifact = await assembleArtifact(scan, scope, demoViewingScopeSalt, demoRequest, attestation);

writeFileSync(outFile, JSON.stringify(artifact, null, 2), "utf8");

console.log(`[scan] viewing scope : ${scope.scopeId}`);
console.log(
  `[scan] 스캔 구간     : ${demoRequest.scanRange.startHeight} ~ ${demoRequest.scanRange.endHeight}`,
);
console.log(
  `[scan] 출금 record   : ${scan.derivedRecords.length}건 (enclave 내부, 외부 비공개)`,
);
console.log(`[scan] 결과          : ${scan.result}`);
console.log(`[scan] artifact 저장 : ${outFile}`);
