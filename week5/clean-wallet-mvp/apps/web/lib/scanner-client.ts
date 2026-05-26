// Browser-safe: only uses fetch + plain types. No node: imports.
// Type imports below are TYPE-ONLY (erased at runtime) so this stays client-bundle safe.
import type { Policy, DepositIntent, ScreeningArtifact } from "./policy";
import type { Quote } from "./verify-quote";

export type ScreenRequest = {
  ufvk: string;
  policy: Policy;
  depositIntent: DepositIntent;
};

export type ScreenResponse = {
  artifact: ScreeningArtifact;
  quote: Quote;
};

export async function fetchAttestation(scannerUrl: string): Promise<Quote> {
  const r = await fetch(`${scannerUrl}/attestation`);
  if (!r.ok) throw new Error(`attestation: ${r.status}`);
  return await r.json();
}

export async function postScreen(scannerUrl: string, req: ScreenRequest): Promise<ScreenResponse> {
  const r = await fetch(`${scannerUrl}/screen`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(req),
  });
  if (!r.ok) {
    const body = await r.text();
    throw new Error(`screen ${r.status}: ${body}`);
  }
  return await r.json();
}
