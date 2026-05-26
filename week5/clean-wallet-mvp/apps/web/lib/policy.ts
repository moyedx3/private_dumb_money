// server-only — depends on canonical.ts which uses node:crypto
import { canonicalJson, sha256Hex } from "./canonical";

export type Policy = {
  policyName: string;
  policyVersion: number;
  network: "testnet";
  auditStartHeight: number;
  auditEndHeight: number;
  sanctionedAddressHashes: string[];
  expectedScannerCodeMeasurement: string;
  createdAtUnix: number;
};

export type DepositIntent = {
  exchangeName: string;
  exchangeDepositAddress: string;
  depositAmountZat: string;
  nonce: string;
  expiryUnix: number;
};

export type ScanRange = { network: "testnet"; startHeight: number; endHeight: number };

export type ScreeningArtifact = {
  schemaVersion: 1;
  result: "PASS" | "FAIL";
  scanRange: ScanRange;
  policyHash: string;
  depositIntentHash: string;
  viewingScopeCommitment: string;
  recipientCount: number;
  sanctionedHitCount: number;
  scannerCodeMeasurement: string;
  scanCompletedAtUnix: number;
};

export function policyHash(p: Policy): string {
  return "0x" + sha256Hex(canonicalJson(p));
}

export function depositIntentHash(d: DepositIntent): string {
  return "0x" + sha256Hex(canonicalJson(d));
}

export function artifactHash(a: ScreeningArtifact): string {
  return sha256Hex(canonicalJson(a));
}
