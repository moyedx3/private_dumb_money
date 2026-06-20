import { describe, expect, it } from "vitest";
import type { CatalogEntry } from "./api";

describe("catalog interface", () => {
  it("uses interfaces.md I3-a public catalog shape", () => {
    const entry: CatalogEntry = {
      drop_id: 1,
      price_zec: "0.01",
      h_content: "a".repeat(64),
      title: "demo"
    };
    expect(Object.keys(entry).sort()).toEqual(["drop_id", "h_content", "price_zec", "title"]);
  });
});
