export type DemoScenarioId = "clean" | "sanctioned";

export type DemoScenario = {
  id: DemoScenarioId;
  label: string;
  shortLabel: string;
  eyebrow: string;
  description: string;
  walletLabel: string;
  outgoingTransfers: Array<{
    label: string;
    amount: string;
    sanctioned: boolean;
  }>;
  result: "PASS" | "FAIL";
  decision: string;
  recipientCount: number;
  sanctionedHitCount: number;
};

export const demoScenarios: Record<DemoScenarioId, DemoScenario> = {
  clean: {
    id: "clean",
    label: "Clean wallet",
    shortLabel: "Clean",
    eyebrow: "Expected: approved",
    description: "This wallet has outgoing transfers, but none were sent to an address in the sanctioned set.",
    walletLabel: "Wallet A",
    outgoingTransfers: [
      { label: "Coffee merchant", amount: "0.15 ZEC", sanctioned: false },
      { label: "Hardware wallet shop", amount: "0.42 ZEC", sanctioned: false },
    ],
    result: "PASS",
    decision: "No sanctioned outgoing recipient was found. The exchange can accept this deposit.",
    recipientCount: 2,
    sanctionedHitCount: 0,
  },
  sanctioned: {
    id: "sanctioned",
    label: "Wallet with sanctioned transfer",
    shortLabel: "Sanctioned transfer",
    eyebrow: "Expected: rejected",
    description: "This wallet sent funds to one address in the sanctioned set during the requested audit range.",
    walletLabel: "Wallet B",
    outgoingTransfers: [
      { label: "Sanctioned recipient", amount: "0.125 ZEC", sanctioned: true },
      { label: "Bookstore", amount: "0.08 ZEC", sanctioned: false },
    ],
    result: "FAIL",
    decision: "A sanctioned outgoing recipient was found. The exchange must reject this deposit.",
    recipientCount: 2,
    sanctionedHitCount: 1,
  },
};

export const demoScenarioOrder: DemoScenarioId[] = ["clean", "sanctioned"];

export function makeDemoArtifact(scenario: DemoScenario) {
  return {
    mode: "guided-demo-preview",
    wallet: scenario.walletLabel,
    result: scenario.result,
    scanRange: { network: "mainnet", startHeight: 2700000, endHeight: 2700020 },
    recipientCount: scenario.recipientCount,
    sanctionedHitCount: scenario.sanctionedHitCount,
    privacy: "Only the attested screening result is shared with the exchange.",
  };
}
