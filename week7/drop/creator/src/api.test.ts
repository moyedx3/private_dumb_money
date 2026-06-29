import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchAttestation, fetchCatalog, postProvision, uploadContentBlob } from "./api";
import type { CatalogEntry } from "./api";

afterEach(() => {
  vi.unstubAllGlobals();
});

function stubFetch(handler: (input: RequestInfo | URL, init?: RequestInit) => Response | Promise<Response>): void {
  const fetchHandler: typeof fetch = async (input, init) => handler(input, init);
  vi.stubGlobal("fetch", fetchHandler);
}

function requestUrl(input: RequestInfo | URL): string {
  if (input instanceof Request) {
    return input.url;
  }
  if (input instanceof URL) {
    return input.toString();
  }
  return input;
}

describe("catalog interface", () => {
  it("uses interfaces.md I3-a public catalog shape", () => {
    const entry: CatalogEntry = {
      drop_id: 1,
      price_zec: "0.01",
      h_content: "a".repeat(64),
      title: "demo",
      deposit_addr: "u1shieldedreceiver"
    };
    expect(Object.keys(entry).sort()).toEqual(["deposit_addr", "drop_id", "h_content", "price_zec", "title"]);
  });
});

describe("attest API boundary", () => {
  it("rejects malformed attest response", async () => {
    stubFetch(async () =>
      new Response(JSON.stringify({ quote_hex: "", provisioning_pubkey_hex: "f".repeat(64) }), {
        headers: { "content-type": "application/json" },
        status: 200
      }));

    await expect(fetchAttestation("https://indexer.example")).rejects.toThrow(/attest/i);
  });

  it("names attest stage on malformed JSON", async () => {
    stubFetch(async () =>
      new Response("{", {
        headers: { "content-type": "application/json" },
        status: 200
      }));

    await expect(fetchAttestation("https://indexer.example")).rejects.toThrow(/attest.*malformed JSON/i);
  });
});

describe("catalog API boundary", () => {
  it("rejects malformed catalog response", async () => {
    stubFetch(async () =>
      new Response(
        JSON.stringify([
          {
            drop_id: 1,
            price_zec: "0.01",
            h_content: "not-hex",
            title: "demo",
            deposit_addr: "u1shieldedreceiver"
          }
        ]),
        {
          headers: { "content-type": "application/json" },
          status: 200
        }
      ));

    await expect(fetchCatalog("https://indexer.example")).rejects.toThrow(/catalog/i);
  });
});

describe("upload API boundary", () => {
  it("names upload stage on failed upload status", async () => {
    stubFetch(async () => new Response("bucket unavailable", { status: 503 }));

    await expect(uploadContentBlob("https://indexer.example", "a".repeat(64), new Uint8Array([1, 2, 3]))).rejects.toThrow(
      /upload.*503/i
    );
  });
});

describe("provision API boundary", () => {
  it("encodes title query for provision", async () => {
    let requestedUrl = "";
    stubFetch(async (input) => {
      requestedUrl = requestUrl(input);
      return new Response("", { status: 200 });
    });

    const title = "title with spaces & symbols=?";
    await postProvision("https://indexer.example", title, new Uint8Array([4, 5, 6]));

    const url = new URL(requestedUrl);
    expect(url.pathname).toBe("/provision");
    expect(url.searchParams.get("title")).toBe(title);
    expect(requestedUrl).toContain("title=");
  });
});
