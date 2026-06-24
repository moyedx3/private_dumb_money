import { redactConfiguredSecrets, SmokeError, summarizeText } from "./http-smoke-core.mjs";

const REQUEST_TIMEOUT_MS = 15_000;

export async function requestText(stage, url) {
  const response = await request(stage, url);
  return {
    status: response.status,
    body: await response.text()
  };
}

export async function requestJson(stage, url) {
  const response = await request(stage, url);
  try {
    return {
      status: response.status,
      body: await response.json()
    };
  } catch (error) {
    throw new SmokeError(stage, `malformed JSON body: ${redactConfiguredSecrets(error.message)}`, { cause: error });
  }
}

async function request(stage, url) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  try {
    const response = await fetch(url, {
      headers: { accept: "application/json, text/plain;q=0.9" },
      signal: controller.signal
    });
    if (!response.ok) {
      const body = await response.text().catch(() => "");
      throw new SmokeError(stage, `HTTP ${response.status} ${summarizeText(body)}`);
    }
    return response;
  } catch (error) {
    if (error instanceof SmokeError) {
      throw error;
    }
    if (error?.name === "AbortError") {
      throw new SmokeError(stage, `request timed out after ${REQUEST_TIMEOUT_MS}ms`, { cause: error });
    }
    throw new SmokeError(stage, error instanceof Error ? error.message : String(error), { cause: error });
  } finally {
    clearTimeout(timeout);
  }
}
