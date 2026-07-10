import qvl from "@phala/dcap-qvl";

const acceptedTcbStatuses = new Set(["UpToDate"]);

export async function verifyQuote(quoteHex) {
  try {
    const verified = await qvl.getCollateralAndVerify(normalizeQuoteHex(quoteHex), pccsUrl());
    if (!acceptedTcbStatuses.has(verified.status)) {
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

function pccsUrl() {
  const configured = process.env.VITE_DROP_PCCS_URL?.trim() ?? process.env.PCCS_URL?.trim();
  return configured ? configured : undefined;
}

function normalizeQuoteHex(quoteHex) {
  const normalized = quoteHex.trim().replace(/^0x/i, "");
  if (normalized.length % 2 !== 0 || !/^[0-9a-fA-F]+$/.test(normalized)) {
    throw new Error("quote hex is invalid");
  }
  return Uint8Array.from(normalized.match(/.{2}/g) ?? [], (byte) => Number.parseInt(byte, 16));
}

function verifiedReportToQuoteVerification(report) {
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

function toHex(bytes) {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}
