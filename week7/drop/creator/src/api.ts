import type { AttestResponse } from "./attestation";
import { bytesToArrayBuffer } from "./bytes";

export type CatalogEntry = {
  drop_id: number;
  price_zec: string;
  h_content: string;
  title: string;
};

export async function fetchAttestation(indexerUrl: string): Promise<AttestResponse> {
  const res = await fetch(joinUrl(indexerUrl, "/attest"));
  if (!res.ok) {
    throw new Error(`/attest returned ${res.status}`);
  }
  return res.json();
}

export async function fetchCatalog(indexerUrl: string): Promise<CatalogEntry[]> {
  const res = await fetch(joinUrl(indexerUrl, "/catalog"));
  if (!res.ok) {
    throw new Error(`/catalog returned ${res.status}`);
  }
  return res.json();
}

export async function uploadContentBlob(indexerUrl: string, hContent: string, blob: Uint8Array): Promise<void> {
  const res = await fetch(joinUrl(indexerUrl, `/bucket/${hContent}`), {
    method: "PUT",
    headers: { "content-type": "application/octet-stream" },
    body: bytesToArrayBuffer(blob)
  });
  if (!res.ok) {
    throw new Error(`/bucket/${hContent} returned ${res.status}`);
  }
}

export async function postProvision(indexerUrl: string, title: string, sealed: Uint8Array): Promise<void> {
  const url = new URL(joinUrl(indexerUrl, "/provision"));
  url.searchParams.set("title", title);
  const res = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/octet-stream" },
    body: bytesToArrayBuffer(sealed)
  });
  if (!res.ok) {
    throw new Error(`/provision returned ${res.status}`);
  }
}

export function joinUrl(base: string, path: string): string {
  return `${base.replace(/\/+$/, "")}/${path.replace(/^\/+/, "")}`;
}
