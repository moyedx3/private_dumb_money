import { describe, expect, it } from "vitest";
import { demoScenarios, makeDemoArtifact } from "./guided-demo";

describe("guided demo scenarios", () => {
  it("approves a wallet with no sanctioned outgoing recipient", () => {
    const artifact = makeDemoArtifact(demoScenarios.clean);

    expect(artifact.result).toBe("PASS");
    expect(artifact.sanctionedHitCount).toBe(0);
  });

  it("rejects a wallet that sent funds to a sanctioned recipient", () => {
    const artifact = makeDemoArtifact(demoScenarios.sanctioned);

    expect(artifact.result).toBe("FAIL");
    expect(artifact.sanctionedHitCount).toBe(1);
  });
});
