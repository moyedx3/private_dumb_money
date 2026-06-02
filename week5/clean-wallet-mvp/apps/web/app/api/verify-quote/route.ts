import { NextRequest, NextResponse } from "next/server";

// Endpoint of the dstack-verifier service. In production this is either:
//   - Phala Trust Center (t16z): https://proof.t16z.com/api/upload
//   - A self-hosted dstack-verifier binary
// The t16z upload API takes the quote as a multipart form field (`hex` = quote
// hex without a 0x prefix, or `file` = raw quote bytes) and returns
// { id, success, proof_of_cloud, quote }. `success` is the verification verdict.
// (The legacy /api/v1/verify JSON path no longer exists — it 404s.)
const VERIFIER_URL = process.env.DSTACK_VERIFIER_URL ?? "https://proof.t16z.com/api/upload";

/**
 * Parse mr_td and report_data directly from the TDX quote hex bytes.
 *
 * TDX quote layout (relevant fields):
 *   bytes  48..632  TD_REPORT (584 bytes)
 *     bytes 184..232  MR_TD (48 bytes, the code measurement)
 *     bytes 568..632  REPORT_DATA (64 bytes)
 *
 * Returns null if the quote is too short.
 */
function parseQuoteFields(quoteHex: string): { mrTd: string; reportData: string } | null {
  // Strip optional 0x prefix
  const hex = quoteHex.startsWith("0x") ? quoteHex.slice(2) : quoteHex;
  // Need at least 632 bytes (1264 hex chars) to read report_data
  if (hex.length < 1264) return null;
  const mrTd = "0x" + hex.slice(184 * 2, 232 * 2).toLowerCase();   // 96 hex chars
  const reportData = hex.slice(568 * 2, 632 * 2).toLowerCase();    // 128 hex chars, no 0x prefix
  return { mrTd, reportData };
}

export async function POST(req: NextRequest) {
  const quote = await req.json();
  // The scanner returns quotes as { quote_hex, event_log, vm_config }
  const quoteHex = (quote?.quote_hex ?? quote?.quote ?? "") as string;

  // Local parse — always attempt, regardless of upstream verifier outcome.
  const parsed = parseQuoteFields(quoteHex);

  // Trust Center upstream (signature verification).
  let upstreamOk = false;
  let upstreamError: string | undefined;
  try {
    // t16z /api/upload expects multipart form-data with `hex` = quote hex (no 0x prefix).
    const cleanHex = quoteHex.startsWith("0x") ? quoteHex.slice(2) : quoteHex;
    const form = new FormData();
    form.append("hex", cleanHex);
    const upstream = await fetch(VERIFIER_URL, { method: "POST", body: form });
    const body = await upstream.json().catch(() => ({} as Record<string, unknown>));
    // `success` is the t16z verification verdict; `proof_of_cloud` confirms a genuine
    // cloud TEE. A self-hosted dstack-verifier may instead return { verified } / { ok }.
    const b = body as {
      success?: boolean; proof_of_cloud?: boolean; verified?: boolean; ok?: boolean; error?: string;
    };
    upstreamOk = !!(b.success ?? b.verified ?? b.ok);
    upstreamError = b.error ?? (upstream.ok ? undefined : `verifier HTTP ${upstream.status}`);
  } catch (e) {
    upstreamError = `verifier unreachable: ${String(e)}`;
  }

  // Compose response. `ok` reflects signature verification (Trust Center). The locally
  // parsed mr_td and report_data are always returned when the quote is well-formed,
  // so the bundle verifier can run Check 2 (artifact-binds-quote) even when the signature
  // can't be verified (e.g., simulator quotes signed with a dev key).
  return NextResponse.json({
    ok: upstreamOk,
    codeMeasurement: parsed?.mrTd,
    reportData: parsed?.reportData,
    error: upstreamError,
  });
}
