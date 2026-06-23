// Indexer/bucket client. The buyer only ever READS: the public catalog, the dispatch-blob list,
// individual dispatch blobs, and content blobs. It never talks to the chain or the TEE directly.
//
// Assumes the lane-B → A1/A2 requests are applied:
//   - I3-a catalog carries `deposit_addr`            (R-A2-2)
//   - GET /dispatch returns dispatch-only keys       (R-A2-1 + R-A2-3)
// Until they land, use `MockDropApi` (mockApi.ts) to develop standalone.

export type CatalogEntry = {
  drop_id: number;
  price_zec: string;
  h_content: string;
  title: string;
  deposit_addr: string;
};

export interface DropApi {
  fetchCatalog(): Promise<CatalogEntry[]>;
  /** Dispatch-blob keys only (not content blobs). Buyer trial-opens each. */
  listDispatch(): Promise<string[]>;
  getDispatch(key: string): Promise<Uint8Array>;
  getContent(hContent: string): Promise<Uint8Array>;
}

export function joinUrl(base: string, path: string): string {
  return `${base.replace(/\/+$/, "")}/${path.replace(/^\/+/, "")}`;
}

export class HttpDropApi implements DropApi {
  constructor(private readonly indexerUrl: string) {}

  async fetchCatalog(): Promise<CatalogEntry[]> {
    const res = await fetch(joinUrl(this.indexerUrl, "/catalog"));
    if (!res.ok) throw new Error(`/catalog returned ${res.status}`);
    return (await res.json()) as CatalogEntry[];
  }

  async listDispatch(): Promise<string[]> {
    const res = await fetch(joinUrl(this.indexerUrl, "/dispatch"));
    if (!res.ok) throw new Error(`/dispatch returned ${res.status}`);
    return (await res.json()) as string[];
  }

  async getDispatch(key: string): Promise<Uint8Array> {
    return this.getBytes(joinUrl(this.indexerUrl, `/dispatch/${key}`));
  }

  async getContent(hContent: string): Promise<Uint8Array> {
    return this.getBytes(joinUrl(this.indexerUrl, `/bucket/${hContent}`));
  }

  private async getBytes(url: string): Promise<Uint8Array> {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`${url} returned ${res.status}`);
    return new Uint8Array(await res.arrayBuffer());
  }
}
