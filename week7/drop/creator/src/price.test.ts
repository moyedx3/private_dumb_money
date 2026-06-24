import { describe, expect, it } from "vitest";
import { priceZecToZatNumber } from "./price";

describe("priceZecToZatNumber", () => {
  it("converts ZEC strings to zatoshi exactly", () => {
    expect(priceZecToZatNumber("0")).toBe(0);
    expect(priceZecToZatNumber("0.01")).toBe(1_000_000);
    expect(priceZecToZatNumber("1.23456789")).toBe(123_456_789);
  });

  it("rejects malformed or over-precise values", () => {
    for (const bad of ["", "-1", "1.234567891", "01", "1.", "abc"]) {
      expect(() => priceZecToZatNumber(bad)).toThrow();
    }
  });

  it("rejects values above the safe-integer zatoshi limit", () => {
    expect(() => priceZecToZatNumber("90071992.54740992")).toThrow("safe integer");
  });
});
