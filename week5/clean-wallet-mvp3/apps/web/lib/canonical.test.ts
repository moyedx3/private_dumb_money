import { describe, it, expect } from "vitest";
import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { canonicalJson, sha256Hex } from "./canonical";

const FIXTURES = resolve(__dirname, "../../../packages/schemas/fixtures");

const names = readdirSync(FIXTURES)
  .filter((n) => n.endsWith(".input.json"))
  .map((n) => n.replace(".input.json", ""))
  .sort();

describe("canonical JSON", () => {
  for (const name of names) {
    it(`matches fixture: ${name}`, () => {
      const input = JSON.parse(readFileSync(`${FIXTURES}/${name}.input.json`, "utf8"));
      const expectedCanonical = readFileSync(`${FIXTURES}/${name}.canonical.bin`, "utf8");
      const expectedSha = readFileSync(`${FIXTURES}/${name}.sha256.hex`, "utf8").trim();
      const actualCanonical = canonicalJson(input);
      expect(actualCanonical).toEqual(expectedCanonical);
      expect(sha256Hex(actualCanonical)).toEqual(expectedSha);
    });
  }
});
