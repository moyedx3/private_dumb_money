import ky, { HTTPError } from "ky";
import { z } from "zod";
import type { AttestResponse } from "./attestation";
import { bytesToArrayBuffer } from "./bytes";

const INDEXER_TIMEOUT_MS = 15_000;
const HEX_PATTERN = /^[0-9a-fA-F]+$/;
const SHA256_HEX_PATTERN = /^[0-9a-fA-F]{64}$/;

const indexerHttp = ky.create({
  retry: 0,
  timeout: INDEXER_TIMEOUT_MS
});

const AttestResponseSchema = z.object({
  quote_hex: z.string().min(1).regex(HEX_PATTERN),
  provisioning_pubkey_hex: z.string().regex(SHA256_HEX_PATTERN)
});

const CatalogEntrySchema = z.object({
  drop_id: z.number().int().nonnegative().safe(),
  price_zec: z.string().min(1),
  h_content: z.string().regex(SHA256_HEX_PATTERN),
  title: z.string()
});

const CatalogResponseSchema = z.array(CatalogEntrySchema);

type ApiStage = "attest" | "catalog" | "upload" | "provision";

export type CatalogEntry = z.infer<typeof CatalogEntrySchema>;

export class IndexerApiError extends Error {
  readonly name = "IndexerApiError";

  constructor(
    readonly stage: ApiStage,
    message: string,
    options?: ErrorOptions
  ) {
    super(`${stage}: ${message}`, options);
  }
}

export async function fetchAttestation(indexerUrl: string): Promise<AttestResponse> {
  return fetchParsedJson("attest", joinUrl(indexerUrl, "/attest"), AttestResponseSchema);
}

export async function fetchCatalog(indexerUrl: string): Promise<CatalogEntry[]> {
  return fetchParsedJson("catalog", joinUrl(indexerUrl, "/catalog"), CatalogResponseSchema);
}

export async function uploadContentBlob(indexerUrl: string, hContent: string, blob: Uint8Array): Promise<void> {
  await sendBinary("upload", joinUrl(indexerUrl, `/bucket/${hContent}`), "put", blob);
}

export async function postProvision(indexerUrl: string, title: string, sealed: Uint8Array): Promise<void> {
  const url = new URL(joinUrl(indexerUrl, "/provision"));
  url.searchParams.set("title", title);
  await sendBinary("provision", url.toString(), "post", sealed);
}

export function joinUrl(base: string, path: string): string {
  return `${base.replace(/\/+$/, "")}/${path.replace(/^\/+/, "")}`;
}

async function fetchParsedJson<T>(stage: ApiStage, url: string, schema: z.ZodType<T>): Promise<T> {
  try {
    const payload = await indexerHttp.get(url).json<unknown>();
    return schema.parse(payload);
  } catch (error) {
    throw apiErrorFrom(stage, error);
  }
}

async function sendBinary(stage: ApiStage, url: string, method: "put" | "post", payload: Uint8Array): Promise<void> {
  try {
    await indexerHttp(url, {
      headers: { "content-type": "application/octet-stream" },
      method,
      body: bytesToArrayBuffer(payload)
    });
  } catch (error) {
    throw apiErrorFrom(stage, error);
  }
}

function apiErrorFrom(stage: ApiStage, error: unknown): IndexerApiError {
  if (error instanceof IndexerApiError) {
    return error;
  }
  if (error instanceof HTTPError) {
    return new IndexerApiError(stage, `HTTP ${error.response.status}`, { cause: error });
  }
  if (error instanceof z.ZodError) {
    return new IndexerApiError(stage, `malformed response: ${z.prettifyError(error)}`, { cause: error });
  }
  if (error instanceof SyntaxError) {
    return new IndexerApiError(stage, `malformed JSON: ${error.message}`, { cause: error });
  }
  if (error instanceof Error) {
    return new IndexerApiError(stage, error.message, { cause: error });
  }
  return new IndexerApiError(stage, "unknown failure");
}
