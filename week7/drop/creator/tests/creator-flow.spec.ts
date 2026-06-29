import { createHash } from "node:crypto";
import { mkdir } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import type { Page, Request } from "@playwright/test";

const indexerUrl = "http://mock-indexer.test";
const measurementHex = "f".repeat(64);
const pubkeyBytes = Uint8Array.from({ length: 32 }, () => 7);
const pubkeyHex = Array.from(pubkeyBytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
const pubkeyHashHex = createHash("sha256").update(pubkeyBytes).digest("hex");
const boundReportDataHex = `${pubkeyHashHex}${"0".repeat(64)}`;
const mismatchedReportDataHex = `${"0".repeat(128)}`;
const responsiveViewportWidths = [375, 768, 1280] as const;

type RecordedRequest = {
  readonly url: string;
  readonly method: string;
  readonly body: Uint8Array;
};

type MockIndexerLog = {
  readonly bucket: RecordedRequest[];
  readonly attest: RecordedRequest[];
  readonly provision: RecordedRequest[];
  readonly catalog: RecordedRequest[];
};

type MockAttestation = {
  readonly quote_hex: string;
  readonly provisioning_pubkey_hex: string;
};

type MockIndexerOptions = {
  readonly attestation?: MockAttestation;
};

test("mock indexer fixture records catalog requests", async ({ page }) => {
  const requests = await setupMockIndexer(page);
  await page.goto("/");

  const entries = await page.evaluate(async (mockIndexerUrl) => {
    const response = await fetch(`${mockIndexerUrl}/catalog`);
    return response.json();
  }, indexerUrl);

  expect(entries).toEqual([]);
  expect(requests.catalog).toHaveLength(1);
  expect(requests.catalog[0]?.method).toBe("GET");
  expect(requests.bucket).toHaveLength(0);
  expect(requests.provision).toHaveLength(0);
});

test("provisions a drop through the complete creator form", async ({ page }) => {
  const requests = await setupMockIndexer(page);
  await installQuoteVerifier(page);
  await page.goto("/");

  await fillCreatorForm(page);
  const bucketRequest = page.waitForRequest(
    (request) => request.url().startsWith(`${indexerUrl}/bucket/`) && request.method() === "PUT"
  );
  const provisionRequest = page.waitForRequest(
    (request) => request.url().startsWith(`${indexerUrl}/provision`) && request.method() === "POST"
  );
  await page.getByRole("button", { name: "Encrypt + Provision" }).click();

  await Promise.all([bucketRequest, provisionRequest]);
  await expect(page.locator("p.success")).toContainText("Drop provisioned");
  const hContentValue = resultValue(page, "h_content");
  await expect(hContentValue).toHaveText(/^[0-9a-f]{64}$/);
  const sealedBytesValue = resultValue(page, "sealed bytes");
  await expect(sealedBytesValue).toHaveText(/^[1-9]\d*$/);
  expect(Number(await sealedBytesValue.innerText())).toBeGreaterThan(48);
  expect(requests.bucket).toHaveLength(1);
  expect(requests.bucket[0]?.method).toBe("PUT");
  expect(requests.provision).toHaveLength(1);
  expect(requests.provision[0]?.method).toBe("POST");

  await page.screenshot({ path: ".omo/evidence/screenshots/task-8-happy.png", fullPage: true });
});

test("blocks provisioning when report_data mismatches", async ({ page }) => {
  const requests = await setupMockIndexer(page);
  await installQuoteVerifier(page, mismatchedReportDataHex);
  await page.goto("/");

  await fillCreatorForm(page);
  const attestationRequest = page.waitForRequest(`${indexerUrl}/attest`);
  await page.getByRole("button", { name: "Encrypt + Provision" }).click();

  await attestationRequest;
  await expect(page.locator("p.error")).toContainText(/measurement|report_data/);
  expect(requests.bucket).toHaveLength(1);
  expect(requests.bucket[0]?.method).toBe("PUT");
  expect(requests.attest).toHaveLength(1);
  expect(requests.attest[0]?.method).toBe("GET");
  expect(requests.provision).toHaveLength(0);

  await page.screenshot({ path: ".omo/evidence/screenshots/task-8-failure.png", fullPage: true });
});

test("rejects invalid drop id before upload", async ({ page }) => {
  const requests = await setupMockIndexer(page);
  await installQuoteVerifier(page);
  await page.goto("/");

  await fillCreatorForm(page);
  await page.getByLabel("Drop ID").fill("1.2");
  await page.getByRole("button", { name: "Encrypt + Provision" }).click();

  await expect(page.locator("p.error")).toContainText(/validation: drop_id/);
  expect(requests.bucket).toHaveLength(0);
  expect(requests.provision).toHaveLength(0);
});

test("resets completed steps when validation fails after a prior success", async ({ page }) => {
  const requests = await setupMockIndexer(page);
  await installQuoteVerifier(page);
  await page.goto("/");

  await fillCreatorForm(page);
  await page.getByRole("button", { name: "Encrypt + Provision" }).click();
  await expect(page.locator("p.success")).toContainText("Drop provisioned");

  await page.getByLabel("Drop ID").fill("1.2");
  await page.getByRole("button", { name: "Encrypt + Provision" }).click();

  await expect(page.locator("p.error")).toContainText(/validation: drop_id/);
  await expect(page.getByLabel("Encrypt + upload I4 blob: idle")).toBeVisible();
  await expect(page.getByLabel("Verify TDX attestation: idle")).toBeVisible();
  await expect(page.getByLabel("Seal I5 payload + provision: idle")).toBeVisible();
  expect(requests.bucket).toHaveLength(1);
  expect(requests.provision).toHaveLength(1);
  await page.screenshot({ path: ".omo/evidence/screenshots/global-stale-validation-reset.png", fullPage: true });
});

test("prevents duplicate submit", async ({ page }) => {
  const requests = await setupMockIndexer(page);
  await installQuoteVerifier(page);
  await page.goto("/");

  await fillCreatorForm(page);
  const submit = page.getByRole("button", { name: "Encrypt + Provision" });
  await submit.dblclick();

  await expect(page.locator("p.success")).toContainText("Drop provisioned");
  await expect(submit).toBeEnabled();
  expect(requests.bucket).toHaveLength(1);
  expect(requests.provision).toHaveLength(1);
});

test("responsive visual states", async ({ page }) => {
  await mkdir(".omo/evidence/screenshots", { recursive: true });

  for (const width of responsiveViewportWidths) {
    await page.setViewportSize({ width, height: 900 });
    await page.unrouteAll({ behavior: "ignoreErrors" });
    await setupMockIndexer(page);
    await installQuoteVerifier(page, mismatchedReportDataHex);
    await page.goto("/");

    await fillCreatorForm(page);
    const attestationRequest = page.waitForRequest(`${indexerUrl}/attest`);
    await page.getByRole("button", { name: "Encrypt + Provision" }).click();
    await attestationRequest;
    await expect(page.locator(".message.error")).toContainText(/measurement|report_data/);

    const submit = page.getByRole("button", { name: "Encrypt + Provision" });
    if (width === 375) {
      await page.getByLabel("Title").fill("");
      await expect(submit).toBeDisabled();
      await page.getByLabel("Expected measurement hex").focus();
      await expect(page.getByLabel("Expected measurement hex")).toHaveCSS("border-color", "rgb(15, 107, 80)");
    } else if (width === 768) {
      await submit.hover();
      await expect(submit).toHaveCSS("filter", "brightness(0.85)");
    } else {
      await page.getByLabel("Expected measurement hex").focus();
      await expect(page.getByLabel("Expected measurement hex")).toHaveCSS("outline-color", "rgb(153, 199, 184)");
    }

    await expectNoHorizontalOverflow(page);
    await page.screenshot({ path: `.omo/evidence/screenshots/task-10-responsive-${width}.png`, fullPage: true });
  }
});

async function setupMockIndexer(page: Page, options: MockIndexerOptions = {}): Promise<MockIndexerLog> {
  const requests: MockIndexerLog = {
    bucket: [],
    attest: [],
    provision: [],
    catalog: []
  };
  const attestation = options.attestation ?? {
    quote_hex: "abcd",
    provisioning_pubkey_hex: pubkeyHex
  };

  await page.route(`${indexerUrl}/bucket/**`, async (route, request) => {
    if (request.method() !== "PUT") {
      await route.fulfill({ status: 405, body: "expected PUT" });
      return;
    }
    requests.bucket.push(await recordedRequest(request));
    await route.fulfill({ status: 204, body: "" });
  });
  await page.route(`${indexerUrl}/attest`, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fulfill({ status: 405, body: "expected GET" });
      return;
    }
    requests.attest.push(await recordedRequest(request));
    await route.fulfill({ status: 200, json: attestation });
  });
  await page.route(`${indexerUrl}/provision?**`, async (route, request) => {
    if (request.method() !== "POST") {
      await route.fulfill({ status: 405, body: "expected POST" });
      return;
    }
    requests.provision.push(await recordedRequest(request));
    await route.fulfill({ status: 204, body: "" });
  });
  await page.route(`${indexerUrl}/provision`, async (route, request) => {
    if (request.method() !== "POST") {
      await route.fulfill({ status: 405, body: "expected POST" });
      return;
    }
    requests.provision.push(await recordedRequest(request));
    await route.fulfill({ status: 204, body: "" });
  });
  await page.route(`${indexerUrl}/catalog`, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fulfill({ status: 405, body: "expected GET" });
      return;
    }
    requests.catalog.push(await recordedRequest(request));
    await route.fulfill({ status: 200, json: [] });
  });

  return requests;
}

async function installQuoteVerifier(page: Page, reportDataHex: string = boundReportDataHex): Promise<void> {
  await page.addInitScript(
    ({ codeMeasurement, reportData }) => {
      window.dropQuoteVerifier = {
        verifyQuote: () => ({
          ok: true,
          codeMeasurement,
          reportData
        })
      };
    },
    { codeMeasurement: measurementHex, reportData: reportDataHex }
  );
}

async function fillCreatorForm(page: Page): Promise<void> {
  await page.getByLabel("Indexer URL").fill(indexerUrl);
  await page.getByLabel("Expected measurement hex").fill(measurementHex);
  await page.getByLabel("Drop ID").fill("1");
  await page.getByLabel("Price ZEC").fill("0.01");
  await page.getByLabel("Title").fill("Route mocked drop");
  await page.getByLabel("Creator UFVK").fill("uview1routefixture");
  await page.getByLabel("Shielded deposit address").fill("u1shieldedroutefixture");
  await page.getByLabel("Text fallback").fill("browser route mock content");
}

async function recordedRequest(request: Request): Promise<RecordedRequest> {
  return {
    url: request.url(),
    method: request.method(),
    body: request.postDataBuffer() ?? new Uint8Array()
  };
}

function resultValue(page: Page, label: string) {
  return page.locator(".result div").filter({ has: page.locator("dt", { hasText: label }) }).locator("dd");
}

async function expectNoHorizontalOverflow(page: Page): Promise<void> {
  const hasOverflow = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth);
  expect(hasOverflow).toBe(false);
}
