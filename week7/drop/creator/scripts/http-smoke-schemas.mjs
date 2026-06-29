import { z } from "zod";
import { HEX_PATTERN, SmokeError } from "./http-smoke-core.mjs";

const SHA256_HEX_PATTERN = /^[0-9a-fA-F]{64}$/;

export const AttestResponseSchema = z.object({
  quote_hex: z.string().min(1).regex(HEX_PATTERN),
  provisioning_pubkey_hex: z.string().regex(SHA256_HEX_PATTERN)
});

export const CatalogResponseSchema = z.array(
  z.object({
    drop_id: z.number().int().nonnegative().safe(),
    price_zec: z.string().min(1),
    h_content: z.string().regex(SHA256_HEX_PATTERN),
    title: z.string(),
    deposit_addr: z.string().min(1)
  })
);

export function parseWithSchema(stage, schema, value) {
  const parsed = schema.safeParse(value);
  if (!parsed.success) {
    throw new SmokeError(stage, `malformed response: ${z.prettifyError(parsed.error)}`);
  }
  return parsed.data;
}
