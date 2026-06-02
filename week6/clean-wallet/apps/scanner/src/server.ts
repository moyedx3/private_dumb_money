/**
 * Attested Scanner 서비스.
 *
 * 동작 모드:
 * - `ATTESTATION_MODE`  "phala" → PhalaAttestation + HTTPS(RA-TLS), 그 외 → SimulatedAttestation + HTTP.
 *
 * /scan 본문 스키마 (D10·D11 — UFVK는 env가 아니라 본문으로):
 *   { "mode": "mock", "scope"?: "clean"|"tainted" }
 *   { "mode": "real", "ufvk": "uview1...", "salt": "<hex>",
 *     "chainSource": { kind: "lightwalletd", url, network },
 *     "scanRange": { startHeight, endHeight } }
 *
 * UFVK는 본문으로만 받으며 처리 후 함수 스코프를 벗어나면 GC 대상이 된다 (per-request 격리).
 * RA-TLS(phala) 모드일 때 본문 자체가 enclave 안에서만 평문이 된다 — 운영자도 못 본다.
 *
 * `ZCASH_SCANNER_BIN`: Rust 사이드카 바이너리 경로 (기본은
 * apps/zcash-scanner-rs/target/release/zcash-scanner-rs[.exe]).
 */
import { spawn } from "node:child_process";
import { createServer as createHttpServer, type IncomingMessage } from "node:http";
import { createServer as createHttpsServer } from "node:https";
import type { ServerResponse } from "node:http";
import {
  assembleArtifact,
  cleanScope,
  demoRequest,
  demoViewingScopeSalt,
  hashAddress,
  mockChain,
  runScan,
  SimulatedAttestation,
  taintedScope,
  type AttestationProvider,
  type BlockRange,
  type ChainSource,
  type DerivedRecord,
  type ScanResult,
  type ScreeningArtifact,
  type ScreeningRequest,
  type ScreeningResult,
  type ViewingScope,
  type ZcashNetwork,
} from "@clean-wallet/core";
import { PhalaAttestation } from "./phala-attestation.ts";

const PORT = Number(process.env.PORT ?? 8080);
const ATTESTATION_MODE = process.env.ATTESTATION_MODE ?? "simulated";
const DEFAULT_SCANNER_BIN =
  process.platform === "win32"
    ? "./apps/zcash-scanner-rs/target/release/zcash-scanner-rs.exe"
    : "./apps/zcash-scanner-rs/target/release/zcash-scanner-rs";
const ZCASH_SCANNER_BIN = process.env.ZCASH_SCANNER_BIN ?? DEFAULT_SCANNER_BIN;

// DSTACK_ENDPOINT (또는 DSTACK_SIMULATOR_ENDPOINT) 가 설정되면 그 socket/HTTP 로 dstack 클라이언트 연결.
// 안 설정되면 default `/var/run/dstack.sock` (실 Phala CVM).
// 로컬 simulator 사용 예: DSTACK_ENDPOINT=/tmp/dstack-sim/dstack.sock 또는 http://localhost:8090
const DSTACK_ENDPOINT =
  process.env.DSTACK_ENDPOINT ?? process.env.DSTACK_SIMULATOR_ENDPOINT;
const phalaAttestation =
  ATTESTATION_MODE === "phala" ? new PhalaAttestation(DSTACK_ENDPOINT) : null;

// 전송 모드. phala 모드는 기본 RA-TLS(앱이 자체 HTTPS 종단). 단 dstack Gateway가 TLS를
// 종단하는 배포(공개 도메인)에선 앱이 HTTP를 서빙해야 한다 — 그땐 SCANNER_TRANSPORT=http.
// HTTP 모드여도 attestation quote는 dstack getQuote로 생성돼 artifact 본문에 그대로 들어간다
// (잃는 것은 RA-TLS 채널 바인딩뿐 — 그건 app-managed TLS=dstack-ingress 경로에서 복구). (D10)
const RATLS_ENABLED =
  phalaAttestation !== null &&
  (process.env.SCANNER_TRANSPORT ?? "ratls").toLowerCase() !== "http";

function selectAttestation(): AttestationProvider {
  return phalaAttestation ?? new SimulatedAttestation();
}

function readBody(req: IncomingMessage): Promise<string> {
  return new Promise((resolve) => {
    let body = "";
    req.on("data", (chunk) => {
      body += chunk;
    });
    req.on("end", () => resolve(body));
  });
}

// ====== Rust 사이드카 IPC ======

type RustScanInput = {
  network: string;
  lightwalletd_url: string;
  ufvk: string;
  start_height: number;
  end_height: number;
};

type RustOutgoingRecord = {
  txid: string;
  block_height: number;
  recipient_address: string;
  amount_zat: string;
  pool: string;
};

type RustScanResponse = {
  ok: boolean;
  scanned_range?: { start: number; end: number };
  lightwalletd_tip?: number;
  outgoing_records: RustOutgoingRecord[];
  error?: string;
  notes?: string[];
};

/** Rust 사이드카 바이너리를 spawn → stdin JSON 요청 → stdout JSON 응답. */
function runRustScanner(input: RustScanInput): Promise<RustScanResponse> {
  return new Promise((resolve, reject) => {
    const child = spawn(ZCASH_SCANNER_BIN, [], {
      stdio: ["pipe", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", (err) => {
      reject(new Error(`spawn ${ZCASH_SCANNER_BIN} failed: ${err.message}`));
    });
    child.on("close", (code) => {
      if (stderr.trim()) {
        // 주의: Rust 사이드카는 UFVK/recipient/amount를 stderr에 찍지 않음 (zcash-scanner-rs.md 참고).
        console.error(`[rust-scanner] ${stderr.trim()}`);
      }
      let parsed: RustScanResponse;
      try {
        parsed = JSON.parse(stdout) as RustScanResponse;
      } catch (e) {
        reject(
          new Error(
            `failed to parse rust scanner output (exit ${code}): ${
              (e as Error).message
            }\nstdout (head): ${stdout.slice(0, 500)}`,
          ),
        );
        return;
      }
      if (!parsed.ok) {
        reject(new Error(parsed.error ?? `rust scanner exited ${code}`));
        return;
      }
      resolve(parsed);
    });
    child.stdin.write(JSON.stringify(input));
    child.stdin.end();
  });
}

/** Rust records → ScanResult (sanctioned 비교까지). */
function buildScanResultFromRustRecords(
  rustRecords: RustOutgoingRecord[],
  request: ScreeningRequest,
): ScanResult {
  const sanctionedHashes = new Set(
    request.sanctionedAddresses.map((a) => hashAddress(a.address)),
  );
  const matched = new Set<string>();
  const derivedRecords: DerivedRecord[] = rustRecords.map((r) => {
    const recipientHash = hashAddress(r.recipient_address);
    if (sanctionedHashes.has(recipientHash)) {
      matched.add(recipientHash);
    }
    return {
      txid: r.txid,
      blockHeight: r.block_height,
      direction: "outgoing",
      recipientAddress: r.recipient_address,
      recipientHash,
      amountZat: r.amount_zat,
    };
  });
  const result: ScreeningResult = matched.size > 0 ? "FAIL" : "PASS";
  return {
    scannedRange: request.scanRange,
    derivedRecords,
    result,
    matchedRecipientHashes: [...matched],
  };
}

// ====== /scan 본문 파싱 + 분기 ======

type ScanContext = {
  scan: ScanResult;
  scope: ViewingScope;
  salt: string;
  request: ScreeningRequest;
};

type ScanRequestBody =
  | { mode: "mock"; scope?: "clean" | "tainted" }
  | {
      mode: "real";
      ufvk: string;
      salt: string;
      chainSource: ChainSource;
      scanRange: BlockRange;
      /** 선택 — 본문에 sanctioned 주소를 넘기면 demoRequest 기본값을 override 한다. */
      sanctionedAddresses?: string[];
    };

function parseScanBody(bodyText: string): ScanRequestBody {
  if (!bodyText.trim()) {
    return { mode: "mock", scope: "clean" };
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(bodyText);
  } catch (e) {
    throw new Error(`invalid JSON body: ${(e as Error).message}`);
  }
  if (typeof parsed !== "object" || parsed === null) {
    throw new Error("body must be an object");
  }
  const obj = parsed as Record<string, unknown>;
  const mode = obj.mode;
  if (mode === "mock") {
    const scope = obj.scope === "tainted" ? "tainted" : "clean";
    return { mode: "mock", scope };
  }
  if (mode === "real") {
    const ufvk = obj.ufvk;
    const salt = obj.salt;
    const chainSource = obj.chainSource;
    const scanRange = obj.scanRange;
    if (typeof ufvk !== "string" || !ufvk.trim()) {
      throw new Error("body.ufvk required (string) in real mode");
    }
    if (typeof salt !== "string" || !salt.trim()) {
      throw new Error("body.salt required (hex string, ≥ 16 chars) in real mode");
    }
    if (
      typeof chainSource !== "object" ||
      chainSource === null ||
      typeof (chainSource as { kind?: unknown }).kind !== "string"
    ) {
      throw new Error("body.chainSource required (ChainSource) in real mode");
    }
    if (
      typeof scanRange !== "object" ||
      scanRange === null ||
      typeof (scanRange as { startHeight?: unknown }).startHeight !== "number" ||
      typeof (scanRange as { endHeight?: unknown }).endHeight !== "number"
    ) {
      throw new Error("body.scanRange required ({startHeight, endHeight}) in real mode");
    }
    // 선택: sanctionedAddresses — string[] 형태로 임의 주소 override.
    let sanctionedAddresses: string[] | undefined;
    const rawSanc = obj.sanctionedAddresses;
    if (rawSanc !== undefined) {
      if (!Array.isArray(rawSanc) || !rawSanc.every((x) => typeof x === "string")) {
        throw new Error("body.sanctionedAddresses must be an array of strings");
      }
      sanctionedAddresses = rawSanc as string[];
    }
    return {
      mode: "real",
      ufvk,
      salt,
      chainSource: chainSource as ChainSource,
      scanRange: scanRange as BlockRange,
      sanctionedAddresses,
    };
  }
  throw new Error(`unknown mode: ${String(mode)} (expected "mock" or "real")`);
}

function runMockScan(scopeName: "clean" | "tainted"): ScanContext {
  const scope = scopeName === "tainted" ? taintedScope : cleanScope;
  const request = demoRequest;
  const scan = runScan(mockChain, scope, request);
  return { scan, scope, salt: demoViewingScopeSalt, request };
}

async function runRealScan(body: {
  ufvk: string;
  salt: string;
  chainSource: ChainSource;
  scanRange: BlockRange;
  sanctionedAddresses?: string[];
}): Promise<ScanContext> {
  if (body.chainSource.kind !== "lightwalletd") {
    throw new Error(
      `real scan은 lightwalletd chainSource만 지원 — 받은 kind: ${body.chainSource.kind}`,
    );
  }
  const { url, network } = body.chainSource;
  // 사용자가 본문에 sanctionedAddresses 를 줬으면 그걸로 demoRequest 의 기본값 override.
  // demo 의 mock 주소만 비교해선 실 거래내역과 매칭될 일이 없어 항상 PASS — 진짜 PASS/FAIL
  // 차이를 보려면 사용자가 (자기 송금 수취인) 또는 (임의의 알려진 주소) 를 넣어야 한다.
  const sanctionedList =
    body.sanctionedAddresses && body.sanctionedAddresses.length > 0
      ? body.sanctionedAddresses.map((address) => ({
          label: "user-provided",
          asset: "ZEC" as const,
          address,
        }))
      : demoRequest.sanctionedAddresses;
  const realRequest: ScreeningRequest = {
    ...demoRequest,
    policy: {
      ...demoRequest.policy,
      approvedChainSources: [body.chainSource],
    },
    sanctionedAddresses: sanctionedList,
    scanRange: body.scanRange,
    chainSource: body.chainSource,
  };
  const realScope: ViewingScope = {
    scopeId: "real-zcash-ufvk",
    network: network as ZcashNetwork,
    viewingKey: body.ufvk,
  };
  const rustResp = await runRustScanner({
    network: network,
    lightwalletd_url: url,
    ufvk: body.ufvk,
    start_height: body.scanRange.startHeight,
    end_height: body.scanRange.endHeight,
  });
  const scan = buildScanResultFromRustRecords(rustResp.outgoing_records, realRequest);
  return { scan, scope: realScope, salt: body.salt, request: realRequest };
}

// ====== HTTP 핸들러 ======

async function handle(req: IncomingMessage, res: ServerResponse): Promise<void> {
  if (req.method === "GET" && req.url === "/health") {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(
      JSON.stringify({
        status: "ok",
        attestationMode: ATTESTATION_MODE,
        transport: RATLS_ENABLED ? "https-ra-tls" : "http",
      }),
    );
    return;
  }

  if (req.method === "POST" && req.url === "/scan") {
    try {
      const bodyText = await readBody(req);
      const body = parseScanBody(bodyText);
      const ctx: ScanContext =
        body.mode === "mock" ? runMockScan(body.scope ?? "clean") : await runRealScan(body);

      const attestation = selectAttestation();
      const artifact: ScreeningArtifact = await assembleArtifact(
        ctx.scan,
        ctx.scope,
        ctx.salt,
        ctx.request,
        attestation,
      );
      // 진단용 정보 — artifact 본체에는 raw 거래내역이 안 들어가지만 (D6, 보안 경계),
      // 디버그 시 outgoing 추출 자체가 됐는지 알아야 PASS 이유 (no records vs no match) 가
      // 구분된다. _debug.derivedRecordsCount + matchedRecipientHashes 항상 노출 (count·hash
      // 만 — 실 주소 X). DEBUG_DERIVED=true 환경변수면 raw derivedRecords 도 같이.
      const responseObj: Record<string, unknown> = {
        ...artifact,
        _debug: {
          derivedRecordsCount: ctx.scan.derivedRecords.length,
          matchedRecipientHashes: ctx.scan.matchedRecipientHashes,
          ...(process.env.DEBUG_DERIVED === "true"
            ? { derivedRecords: ctx.scan.derivedRecords }
            : {}),
        },
      };
      console.error(
        `[scanner] /scan result=${ctx.scan.result} outgoing_records=${ctx.scan.derivedRecords.length} matched=${ctx.scan.matchedRecipientHashes.length}`,
      );
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify(responseObj, null, 2));
    } catch (e) {
      const msg = (e as Error).message;
      console.error(`[scanner] /scan error: ${msg}`);
      res.writeHead(500, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: msg }));
    }
    return;
  }

  res.writeHead(404, { "content-type": "application/json" });
  res.end(JSON.stringify({ error: "not found — GET /health 또는 POST /scan" }));
}

// ====== 서버 부트 ======

async function startServer(): Promise<void> {
  const requestHandler = (req: IncomingMessage, res: ServerResponse): void => {
    void handle(req, res);
  };

  if (RATLS_ENABLED && phalaAttestation) {
    // D10: dstack RA-TLS — enclave 안에서 만들어진 cert로 HTTPS 종단.
    const altNames = process.env.RATLS_ALT_NAMES
      ? process.env.RATLS_ALT_NAMES.split(",").map((s) => s.trim())
      : undefined;
    const creds = await phalaAttestation.getRaTlsCredentials(altNames);
    const server = createHttpsServer({ key: creds.key, cert: creds.cert }, requestHandler);
    server.listen(PORT, () => {
      console.log(`[scanner] attested scanner — https://0.0.0.0:${PORT} (RA-TLS via dstack)`);
      console.log(`[scanner] attestation mode: phala · transport: https-ra-tls`);
      console.log(`[scanner] GET /health  ·  POST /scan`);
    });
  } else {
    // 평문 HTTP. simulated 모드, 또는 phala+SCANNER_TRANSPORT=http(gateway가 TLS 종단).
    const server = createHttpServer(requestHandler);
    server.listen(PORT, () => {
      console.log(`[scanner] attested scanner — http://0.0.0.0:${PORT}`);
      console.log(
        `[scanner] attestation mode: ${ATTESTATION_MODE} · transport: http${
          phalaAttestation ? " (TLS at gateway)" : ""
        }`,
      );
      console.log(`[scanner] GET /health  ·  POST /scan`);
    });
  }
}

await startServer();
