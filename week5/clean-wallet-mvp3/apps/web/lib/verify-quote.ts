export type Quote = {
  quote_hex: string;
  event_log: unknown;
  vm_config: unknown;
};

export type QuoteVerification = {
  ok: boolean;
  codeMeasurement?: string;
  reportData?: string;  // 64 bytes hex
  error?: string;
};

/**
 * Calls our local /api/verify-quote route, which proxies to dstack-verifier.
 * Reason: dstack-verifier is a service users self-host or call via Trust Center;
 * we abstract that behind a single endpoint here.
 */
export async function verifyQuote(quote: Quote): Promise<QuoteVerification> {
  const resp = await fetch("/api/verify-quote", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(quote),
  });
  if (!resp.ok) {
    return { ok: false, error: `verifier returned ${resp.status}` };
  }
  return await resp.json();
}

/** Compare `quote.reportData[0..32]` against the expected `artifactHash` (hex without 0x). */
export function reportDataBindsArtifact(reportDataHex: string, artifactHashHex: string): boolean {
  if (reportDataHex.length < 64) return false;
  return reportDataHex.slice(0, 64).toLowerCase() === artifactHashHex.toLowerCase();
}
