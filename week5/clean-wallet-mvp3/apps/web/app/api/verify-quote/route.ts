import { NextRequest, NextResponse } from "next/server";

// Phala Cloud remote-attestation API. It uploads the quote and validates the
// Intel TDX signature. A self-hosted verifier can replace this URL if it
// implements the same { hex } request shape.
const VERIFIER_URL =
  process.env.PHALA_ATTESTATION_VERIFY_URL ??
  "https://cloud-api.phala.com/api/v1/attestations/verify";

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

  // Phala remote-attestation API (signature verification).
  let upstreamOk = false;
  let upstreamError: string | undefined;
  try {
    const upstream = await fetch(VERIFIER_URL, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ hex: quoteHex }),
    });
    const body = await upstream.json().catch(() => ({} as {
      quote?: { verified?: boolean };
      verified?: boolean;
      ok?: boolean;
      error?: string;
      message?: string;
    }));
    upstreamOk = !!body.quote?.verified || !!body.verified || !!body.ok;
    upstreamError = body.error ?? (!upstream.ok ? body.message ?? `verifier returned ${upstream.status}` : undefined);
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
