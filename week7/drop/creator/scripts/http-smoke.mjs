#!/usr/bin/env node
import {
  ensureEvidenceDir,
  fromHex,
  joinUrl,
  normalizeBaseUrl,
  normalizeExpectedMeasurement,
  redactConfiguredSecrets,
  safeUrlLabel,
  sha256Hex,
  summarizeText,
  writeJsonEvidence
} from "./http-smoke-core.mjs";
import { requestJson, requestText } from "./http-smoke-request.mjs";
import { AttestResponseSchema, CatalogResponseSchema, parseWithSchema } from "./http-smoke-schemas.mjs";
import { verifyAttestation } from "./http-smoke-verifier.mjs";

async function main() {
  await ensureEvidenceDir();

  const indexerUrl = process.env.VITE_DROP_INDEXER_URL?.trim();
  const expectedMeasurementHex = process.env.VITE_DROP_EXPECTED_MEASUREMENT_HEX?.trim();
  const missing = missingEnvVars(indexerUrl, expectedMeasurementHex);

  if (missing.length > 0) {
    await recordSkippedLive(missing);
    return;
  }

  const baseUrl = normalizeBaseUrl(indexerUrl);
  const expectedMeasurement = normalizeExpectedMeasurement(expectedMeasurementHex);
  const summary = liveSummary(baseUrl);

  console.log(`LIVE SMOKE: ${safeUrlLabel(baseUrl)}`);
  await smokeHealth(baseUrl, summary);
  await smokeCatalog(baseUrl, summary);
  const attestation = await smokeAttest(baseUrl, summary);
  const verification = await verifyAttestation(attestation, expectedMeasurement);

  summary.attestation = {
    verified: true,
    measurementPrefix: verification.codeMeasurement.slice(0, 16),
    reportDataPrefix: verification.reportData.slice(0, 16)
  };
  summary.browserLiveQaCanRun = true;

  await writeJsonEvidence("http-smoke-live.json", summary);
  console.log("attestation schema: ok");
  console.log("quote verification: ok");
  console.log("browser live QA can run: yes");
}

function missingEnvVars(indexerUrl, expectedMeasurementHex) {
  return [
    ["VITE_DROP_INDEXER_URL", indexerUrl],
    ["VITE_DROP_EXPECTED_MEASUREMENT_HEX", expectedMeasurementHex]
  ]
    .filter(([, value]) => !value)
    .map(([name]) => name);
}

async function recordSkippedLive(missing) {
  await writeJsonEvidence("http-smoke-live-skipped.json", {
    mode: "skipped-live",
    reason: `missing ${missing.join(", ")}`,
    browserLiveQaCanRun: false,
    checkedAt: new Date().toISOString()
  });
  console.log(`LIVE SMOKE SKIPPED: missing ${missing.join(", ")}. No deployed indexer was contacted.`);
  console.log("browser live QA can run: no");
}

function liveSummary(baseUrl) {
  return {
    mode: "live",
    indexerEndpoint: safeUrlLabel(baseUrl),
    checkedAt: new Date().toISOString(),
    endpoints: {},
    browserLiveQaCanRun: false
  };
}

async function smokeHealth(baseUrl, summary) {
  const health = await requestText("health", joinUrl(baseUrl, "/health"));
  summary.endpoints.health = {
    status: health.status,
    bytes: health.body.length,
    bodySummary: summarizeText(health.body)
  };
  console.log(`/health HTTP ${health.status}: ${summary.endpoints.health.bodySummary}`);
}

async function smokeCatalog(baseUrl, summary) {
  const catalog = await requestJson("catalog", joinUrl(baseUrl, "/catalog"));
  const catalogRows = parseWithSchema("catalog", CatalogResponseSchema, catalog.body);
  summary.endpoints.catalog = {
    status: catalog.status,
    entries: catalogRows.length,
    firstEntryKeys: catalogRows[0] ? Object.keys(catalogRows[0]).sort() : []
  };
  console.log(`/catalog HTTP ${catalog.status}: ${catalogRows.length} public entries`);
}

async function smokeAttest(baseUrl, summary) {
  const attest = await requestJson("attest", joinUrl(baseUrl, "/attest"));
  const attestation = parseWithSchema("attest", AttestResponseSchema, attest.body);
  const pubkeyHashPrefix = sha256Hex(fromHex(attestation.provisioning_pubkey_hex)).slice(0, 16);
  summary.endpoints.attest = {
    status: attest.status,
    quoteHexChars: attestation.quote_hex.length,
    provisioningPubkeySha256Prefix: pubkeyHashPrefix
  };
  console.log(
    `/attest HTTP ${attest.status}: quote_hex ${attestation.quote_hex.length} hex chars, provisioning_pubkey sha256 ${pubkeyHashPrefix}...`
  );
  return attestation;
}

main().catch(async (error) => {
  const message = redactConfiguredSecrets(error instanceof Error ? error.message : String(error));
  await ensureEvidenceDir().catch(() => {});
  await writeJsonEvidence("http-smoke-live-failed.json", {
    mode: "failed",
    checkedAt: new Date().toISOString(),
    error: message
  }).catch(() => {});
  console.error(`LIVE SMOKE FAILED: ${message}`);
  process.exit(1);
});
