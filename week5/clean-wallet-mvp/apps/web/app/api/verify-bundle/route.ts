import { NextRequest, NextResponse } from "next/server";
import {
  Policy,
  DepositIntent,
  ScreeningArtifact,
  artifactHash,
  policyHash,
  depositIntentHash,
} from "@/lib/policy";
import { reportDataBindsArtifact, Quote } from "@/lib/verify-quote";

type VerifyBundleRequest = {
  artifact: ScreeningArtifact;
  quote: Quote;
  policy: Policy;
  depositIntent: DepositIntent;
  // Optional: pass-through verification result from /api/verify-quote
  quoteVerification?: {
    ok: boolean;
    codeMeasurement?: string;
    reportData?: string;
    error?: string;
  };
};

type CheckResult = { ok: boolean; detail: string };

type VerifyBundleResponse = {
  check1_quoteGenuine: CheckResult;
  check2_quoteBindsArtifact: CheckResult;
  check3_artifactBindsContext: CheckResult;
  finalResult?: "PASS" | "FAIL";
};

export async function POST(req: NextRequest) {
  const { artifact, quote, policy, depositIntent, quoteVerification }:
    VerifyBundleRequest = await req.json();

  // Check 1: quote authenticity + code measurement matches policy
  let check1: CheckResult;
  if (!quoteVerification) {
    check1 = { ok: false, detail: "Quote verification not yet performed (call /api/verify-quote first)." };
  } else if (!quoteVerification.ok) {
    check1 = { ok: false, detail: "Attestation is not genuine." };
  } else if (
    quoteVerification.codeMeasurement &&
    policy.expectedScannerCodeMeasurement &&
    quoteVerification.codeMeasurement.toLowerCase() !==
      policy.expectedScannerCodeMeasurement.toLowerCase()
  ) {
    check1 = {
      ok: false,
      detail: "Scanner code does not match the policy's expected version.",
    };
  } else {
    check1 = { ok: true, detail: "Quote is genuine; code measurement matches policy." };
  }

  // Check 2: quote.reportData binds artifact
  const aHash = artifactHash(artifact);
  let check2: CheckResult;
  if (!quoteVerification?.reportData) {
    check2 = { ok: false, detail: "Cannot verify binding: reportData missing from quote." };
  } else if (!reportDataBindsArtifact(quoteVerification.reportData, aHash)) {
    check2 = {
      ok: false,
      detail: "Attestation seal does not match this report.",
    };
  } else {
    check2 = { ok: true, detail: "Attestation seal matches this report." };
  }

  // Check 3: artifact binds deposit, policy, scanRange, not-expired
  const pHash = policyHash(policy);
  const dHash = depositIntentHash(depositIntent);
  const problems: string[] = [];
  if (artifact.policyHash.toLowerCase() !== pHash.toLowerCase()) {
    problems.push("Report is for a different screening policy.");
  }
  if (artifact.depositIntentHash.toLowerCase() !== dHash.toLowerCase()) {
    problems.push("Report is not for this deposit.");
  }
  if (
    artifact.scanRange.network !== policy.network ||
    artifact.scanRange.startHeight !== policy.auditStartHeight ||
    artifact.scanRange.endHeight !== policy.auditEndHeight
  ) {
    problems.push("Report covers a different scan range than the policy requires.");
  }
  const now = Math.floor(Date.now() / 1000);
  if (now > depositIntent.expiryUnix) {
    problems.push("Deposit intent expired before verification.");
  }
  const check3: CheckResult = problems.length
    ? { ok: false, detail: problems.join("\n  ") }
    : { ok: true, detail: "Artifact binds this deposit, policy, and scan range." };

  const allOk = check1.ok && check2.ok && check3.ok;
  const response: VerifyBundleResponse = {
    check1_quoteGenuine: check1,
    check2_quoteBindsArtifact: check2,
    check3_artifactBindsContext: check3,
    finalResult: allOk ? (artifact.result as "PASS" | "FAIL") : undefined,
  };
  return NextResponse.json(response);
}
