import { describe, it, expect } from "vitest";
import { reportDataBindsArtifact } from "./verify-quote";

describe("reportDataBindsArtifact", () => {
  it("matches when prefix equals artifact hash", () => {
    const artifactHash = "a".repeat(64);
    const reportData = artifactHash + "0".repeat(64);
    expect(reportDataBindsArtifact(reportData, artifactHash)).toBe(true);
  });
  it("rejects when prefix differs", () => {
    const artifactHash = "a".repeat(64);
    const reportData = "b".repeat(64) + "0".repeat(64);
    expect(reportDataBindsArtifact(reportData, artifactHash)).toBe(false);
  });
  it("rejects truncated reportData", () => {
    expect(reportDataBindsArtifact("ab", "a".repeat(64))).toBe(false);
  });
  it("is case-insensitive", () => {
    const lower = "a".repeat(64);
    const upper = "A".repeat(64);
    expect(reportDataBindsArtifact(upper + "0".repeat(64), lower)).toBe(true);
  });
});
