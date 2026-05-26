import { NextRequest, NextResponse } from "next/server";

// Endpoint of the dstack-verifier service. In production this is either:
//   - Phala Trust Center: https://proof.t16z.com/api/v1/verify
//   - A self-hosted dstack-verifier binary
// For MVP, we proxy to the public Trust Center.
const VERIFIER_URL = process.env.DSTACK_VERIFIER_URL ?? "https://proof.t16z.com/api/v1/verify";

export async function POST(req: NextRequest) {
  const quote = await req.json();
  try {
    const upstream = await fetch(VERIFIER_URL, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(quote),
    });
    const body = await upstream.json();
    // Normalise the verifier's response into our QuoteVerification shape:
    //   { ok, codeMeasurement, reportData, error? }
    // Field names differ across verifier versions; adjust here if needed.
    return NextResponse.json({
      ok: !!body.verified || !!body.ok,
      codeMeasurement: body.code_measurement ?? body.mrtd ?? body.measurement,
      reportData: body.report_data ?? body.reportData,
      error: body.error,
    });
  } catch (e) {
    return NextResponse.json({ ok: false, error: String(e) }, { status: 502 });
  }
}
