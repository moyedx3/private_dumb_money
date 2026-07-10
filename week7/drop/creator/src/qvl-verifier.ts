import { getCollateralAndVerify } from "@phala/dcap-qvl";
import type { Report, TcbStatus } from "@phala/dcap-qvl";
import { fromHex, toHex } from "./bytes";
import type { QuoteVerification } from "./attestation";

const acceptedTcbStatuses: readonly TcbStatus[] = ["UpToDate"];

export async function verifyQuote(quoteHex: string): Promise<QuoteVerification> {
  try {
    const verified = await getCollateralAndVerify(normalizeQuoteHex(quoteHex), pccsUrl());
    if (!isAcceptedTcbStatus(verified.status)) {
      return {
        ok: false,
        error: `quote TCB status is ${verified.status}`
      };
    }
    return verifiedReportToQuoteVerification(verified.report);
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : String(error)
    };
  }
}

export const verify = verifyQuote;

function pccsUrl(): string | undefined {
  const configured = import.meta.env.VITE_DROP_PCCS_URL?.trim();
  return configured ? configured : undefined;
}

function normalizeQuoteHex(quoteHex: string): Uint8Array {
  return fromHex(quoteHex.trim().replace(/^0x/i, ""));
}

function isAcceptedTcbStatus(status: TcbStatus): boolean {
  return acceptedTcbStatuses.some((accepted) => accepted === status);
}

function verifiedReportToQuoteVerification(report: Report): QuoteVerification {
  switch (report.type) {
    case "td10": {
      const tdReport = report.asTd10();
      if (!tdReport) {
        return { ok: false, error: "QVL returned a TD 1.0 report without TD report data" };
      }
      return {
        ok: true,
        codeMeasurement: toHex(tdReport.rtMr3),
        reportData: toHex(tdReport.reportData)
      };
    }
    case "td15": {
      const tdReport = report.asTd15();
      if (!tdReport) {
        return { ok: false, error: "QVL returned a TD 1.5 report without TD report data" };
      }
      return {
        ok: true,
        codeMeasurement: toHex(tdReport.base.rtMr3),
        reportData: toHex(tdReport.base.reportData)
      };
    }
    case "sgx": {
      const sgxReport = report.asSgx();
      if (!sgxReport) {
        return { ok: false, error: "QVL returned an SGX report without enclave report data" };
      }
      return {
        ok: true,
        codeMeasurement: toHex(sgxReport.mrEnclave),
        reportData: toHex(sgxReport.reportData)
      };
    }
    default:
      return { ok: false, error: "QVL returned an unsupported quote report type" };
  }
}
