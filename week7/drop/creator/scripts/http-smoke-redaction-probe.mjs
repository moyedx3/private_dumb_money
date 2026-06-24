#!/usr/bin/env node
import http from "node:http";
import { readFile } from "node:fs/promises";
import { spawn } from "node:child_process";
import { EVIDENCE_DIR, writeJsonEvidence } from "./http-smoke-core.mjs";

const secretValue = ["fixture", "secret", "value"].join("-");
const secretModuleUrl = new URL("/module.mjs", "https://verifier.example.test");
secretModuleUrl.username = ["mod", "user"].join("");
secretModuleUrl.password = ["mod", "pass"].join("");
const secretModuleUrlText = secretModuleUrl.toString();
const secrets = [secretValue, secretModuleUrl.username, secretModuleUrl.password, secretModuleUrlText];

const server = http.createServer((request, response) => {
  if (request.url === "/health") {
    response.writeHead(200, { "content-type": "text/plain" });
    response.end(`healthy echo ${secretValue} ${secretModuleUrlText}`);
    return;
  }
  if (request.url === "/catalog") {
    response.writeHead(500, { "content-type": "text/plain" });
    response.end(`catalog failed with ${secretValue} ${secretModuleUrlText}`);
    return;
  }
  response.writeHead(404, { "content-type": "text/plain" });
  response.end("not found");
});

await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const { port } = server.address();
const indexerUrl = `http://127.0.0.1:${port}`;
let child;
let exitCode;
let stdout = "";
let stderr = "";

try {
  child = spawn(process.execPath, ["scripts/http-smoke.mjs"], {
    cwd: process.cwd(),
    env: {
      ...process.env,
      VITE_DROP_INDEXER_URL: indexerUrl,
      VITE_DROP_EXPECTED_MEASUREMENT_HEX: "f".repeat(64),
      VITE_DROP_QVL_MODULE_URL: secretModuleUrlText,
      VITE_DROP_PCCS_URL: secretValue
    },
    stdio: ["ignore", "pipe", "pipe"]
  });

  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    stdout += chunk;
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });

  exitCode = await new Promise((resolve) => {
    child.on("close", resolve);
  });
} finally {
  await new Promise((resolve) => server.close(resolve));
}

let failedEvidence = "";
try {
  failedEvidence = await readFile(`${EVIDENCE_DIR}/http-smoke-live-failed.json`, "utf8");
} catch (error) {
  if (error instanceof Error && "code" in error && error.code === "ENOENT") {
    failedEvidence = "";
  } else {
    throw error;
  }
}

const combined = `${stdout}\n${stderr}\n${failedEvidence}`;
const leaked = secrets.filter((secret) => combined.includes(secret));
const expectedFailure =
  exitCode === 1 &&
  stdout.includes("/health HTTP 200: healthy echo [redacted-env] [redacted-env]") &&
  stderr.includes("LIVE SMOKE FAILED: catalog: HTTP 500 catalog failed with [redacted-env] [redacted-env]");
await writeJsonEvidence("http-smoke-redaction-probe.json", {
  mode: "redaction-probe",
  childExitCode: exitCode,
  expectedFailure,
  stdout,
  stderr,
  failedEvidence,
  leaked
});

if (!expectedFailure) {
  console.error("redaction probe did not observe the expected child failure mode");
  process.exit(1);
}

if (leaked.length > 0) {
  console.error(`redaction probe leaked secret markers: ${leaked.join(", ")}`);
  process.exit(1);
}

console.log("redaction probe passed: response body summaries were sanitized");
