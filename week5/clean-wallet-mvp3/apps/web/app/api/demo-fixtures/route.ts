import { promises as fs } from "node:fs";
import path from "node:path";
import { NextResponse } from "next/server";

export const dynamic = "force-dynamic";

type ScenarioId = "clean" | "dirty";

type FixtureText = {
  value: string;
  source: string;
  present: boolean;
  mainnetReady?: boolean;
};

type FixtureJson = {
  value: unknown;
  source: string;
  present: boolean;
};

type DemoFixturesResponse = {
  policy: FixtureJson;
  wallets: Record<ScenarioId, FixtureText>;
  depositIntents: Record<ScenarioId, FixtureJson>;
  bundles: Record<ScenarioId, FixtureJson>;
  walletMeta: FixtureJson;
};

const FALLBACK_EXPIRY_UNIX = 4102444800;

export async function GET() {
  const demoDataDir = await findDemoDataDir();
  if (!demoDataDir) {
    return NextResponse.json({ error: "demo-data directory not found" }, { status: 500 });
  }

  const response: DemoFixturesResponse = {
    policy: await readJsonFixture(demoDataDir, "policy.demo.json"),
    wallets: {
      clean: await readUfvkFixture(demoDataDir, "ufvk-clean.txt"),
      dirty: await readUfvkFixture(demoDataDir, "ufvk-dirty.txt"),
    },
    depositIntents: {
      clean: await readJsonFixture(demoDataDir, "deposit-intent-clean.json", makeFallbackIntent("clean")),
      dirty: await readJsonFixture(demoDataDir, "deposit-intent-dirty.json", makeFallbackIntent("dirty")),
    },
    bundles: {
      clean: await readJsonFixture(demoDataDir, "bundle-clean.json"),
      dirty: await readJsonFixture(demoDataDir, "bundle-dirty.json"),
    },
    walletMeta: await readJsonFixture(demoDataDir, "wallet-meta.json"),
  };

  return NextResponse.json(response);
}

async function findDemoDataDir(): Promise<string | null> {
  let current = process.cwd();
  for (let i = 0; i < 6; i += 1) {
    const candidate = path.join(current, "demo-data");
    try {
      const stat = await fs.stat(candidate);
      if (stat.isDirectory()) return candidate;
    } catch {
      // Keep walking toward the repository root.
    }
    current = path.dirname(current);
  }
  return null;
}

async function readTextFile(baseDir: string, filename: string): Promise<string | null> {
  try {
    return await fs.readFile(path.join(baseDir, filename), "utf8");
  } catch {
    return null;
  }
}

async function readUfvkFixture(baseDir: string, filename: string): Promise<FixtureText> {
  const raw = await readTextFile(baseDir, filename);
  const value = raw?.trim() ?? "";
  return {
    value,
    source: `demo-data/${filename}`,
    present: raw !== null,
    mainnetReady: value.startsWith("uview") && !value.startsWith("uviewtest"),
  };
}

async function readJsonFixture(baseDir: string, filename: string, fallback?: unknown): Promise<FixtureJson> {
  const raw = await readTextFile(baseDir, filename);
  if (raw === null) {
    return {
      value: fallback ?? null,
      source: fallback ? `generated fallback for demo-data/${filename}` : `demo-data/${filename}`,
      present: false,
    };
  }

  try {
    return {
      value: JSON.parse(raw),
      source: `demo-data/${filename}`,
      present: true,
    };
  } catch {
    return {
      value: null,
      source: `demo-data/${filename} (invalid JSON)`,
      present: true,
    };
  }
}

function makeFallbackIntent(scenario: ScenarioId) {
  return {
    exchangeName: "demo-exchange",
    exchangeDepositAddress: `public-demo-deposit-address-${scenario}`,
    depositAmountZat: "10000",
    nonce: `demo-${scenario}-replace-before-live-run`,
    expiryUnix: FALLBACK_EXPIRY_UNIX,
  };
}
