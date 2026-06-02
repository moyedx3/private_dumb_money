/**
 * tools/submit-ufvk.ts — UFVK를 attested scanner의 RA-TLS 채널로 안전하게 보낸다.
 *
 * env에 UFVK를 두면 운영자가 평문에 접근한다(D10). 대신:
 *   1) 사용자가 자기 머신에서 이 스크립트를 실행.
 *   2) salt를 자기 머신에서 만들고 (commitment hiding — D11), 메모리에 보관.
 *   3) 먼저 TLS 핸드셰이크로 enclave cert 만 받아 *quote 풀검증* (B1).
 *      - dstack quote 확장 추출 → Phala verifier API 로 암호학적 유효성 확인.
 *      - report_data 가 cert pubkey 와 일치하는지 확인 (channel substitution 방지).
 *      - (선택) expected MRTD/RTMR3 와 매치.
 *   4) 검증 통과한 cert 를 pin 한 채로 같은 호스트에 본격 POST — UFVK·salt 본문 전송.
 *
 * 사용법:
 *   node apps/scanner/tools/submit-ufvk.ts \
 *     --host https://<cvm-host>:8080 \
 *     --network test \
 *     --lwd-url https://lwd.testnet.example:443 \
 *     --start 2500000 --end 2500050 \
 *     [--ufvk uview1...]              # 생략 시 stdin 한 줄 (권장)
 *     [--salt <hex>]                  # 생략 시 random 32바이트
 *     [--no-verify]                   # RA-TLS quote 풀검증 비활성 (디버그용)
 *     [--phala-verifier <url>]        # 검증 endpoint override
 *     [--expected-mrtd <hex>]         # 48바이트 MRTD 매치 (선택)
 *     [--expected-rtmr3 <hex>]        # 48바이트 RTMR3 매치 (선택)
 *     [--no-measurement-pin]          # 게시된 MRTD/RTMR3 자동 핀 비활성 (expected-measurements.json)
 */
import { request as httpsRequest } from "node:https";
import { request as httpRequest } from "node:http";
import { connect as tlsConnect, TLSSocket } from "node:tls";
import { X509Certificate, randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import { stdin } from "node:process";
import { verifyRaTlsCert, type VerifyRaTlsResult } from "./ra-tls-verify.ts";

type Args = {
  host: string;
  network: "main" | "test";
  lwdUrl: string;
  ufvk: string;
  startHeight: number;
  endHeight: number;
  salt: string;
  noVerify: boolean;
  noMeasurementPin: boolean;
  phalaVerifier?: string;
  expectedMrTd?: string;
  expectedRtmr3?: string;
  sanctionedAddresses?: string[];
  /** 설정 시 결과 artifact를 이 URL(웹 ingest API)에 POST해 DB에 저장한다. */
  save?: string;
  /** 저장 시 붙일 라벨 (선택). */
  label?: string;
};

function usage(msg?: string): never {
  if (msg) console.error(`error: ${msg}`);
  console.error(
    [
      "",
      "사용법:",
      "  node apps/scanner/tools/submit-ufvk.ts \\",
      "    --host <url>                # https://<cvm-host>:8080",
      "    --network <main|test>",
      "    --lwd-url <url>",
      "    --start <height> --end <height>",
      "    [--ufvk <uview...>]         # 생략 시 stdin 한 줄 (권장)",
      "    [--salt <hex>]              # 생략 시 random 32바이트",
      "    [--no-verify]               # RA-TLS quote 풀검증 비활성 (디버그용)",
      "    [--no-measurement-pin]      # 게시된 MRTD/RTMR3 자동 핀 비활성",
      "    [--phala-verifier <url>]    # verifier endpoint override",
      "    [--expected-mrtd <hex>]     # 48바이트 MRTD 매치 (선택)",
      "    [--expected-rtmr3 <hex>]    # 48바이트 RTMR3 매치 (선택)",
      "    [--save <url>]              # 결과 artifact를 웹 DB에 저장 (예: https://app/api/artifacts)",
      "    [--label <text>]            # 저장 시 붙일 라벨 (선택)",
      "",
    ].join("\n"),
  );
  process.exit(msg ? 1 : 0);
}

/**
 * stdin 에서 UFVK 를 읽는다. readline 의 "line" 은 newline 이 와야 발생해서
 * PowerShell `$x | node ...` 같이 trailing newline 안 붙이는 경우 영원히 block.
 * 대신 chunk accumulate + "end" 이벤트(EOF)에서 trim 해서 돌려준다 — newline 유무 무관.
 */
function readUfvkFromStdin(): Promise<string> {
  return new Promise((resolve, reject) => {
    let buf = "";
    stdin.setEncoding("utf8");
    stdin.on("data", (chunk) => {
      buf += chunk;
    });
    stdin.on("end", () => {
      const trimmed = buf.trim();
      if (!trimmed) {
        reject(new Error("stdin 비어있음 — UFVK 를 echo/Write-Output 으로 파이프해주세요"));
        return;
      }
      // 여러 줄이면 첫 비어있지 않은 줄.
      const firstLine = trimmed.split(/\r?\n/).find((l) => l.trim().length > 0);
      resolve((firstLine ?? trimmed).trim());
    });
    stdin.on("error", reject);
  });
}

async function parseArgs(): Promise<Args> {
  const argv = process.argv.slice(2);
  const get = (k: string): string | undefined => {
    const idx = argv.indexOf(k);
    if (idx < 0) return undefined;
    return argv[idx + 1];
  };
  const has = (k: string): boolean => argv.includes(k);

  const host = get("--host");
  const networkRaw = get("--network");
  const lwdUrl = get("--lwd-url");
  const startStr = get("--start");
  const endStr = get("--end");
  if (!host) usage("--host 필수");
  // main/mainnet, test/testnet 둘 다 받아 일관된 입력 허용.
  const networkNormalized =
    networkRaw === "main" || networkRaw === "mainnet"
      ? "main"
      : networkRaw === "test" || networkRaw === "testnet"
        ? "test"
        : undefined;
  if (!networkNormalized) usage("--network 필수 (main|mainnet|test|testnet)");
  if (!lwdUrl) usage("--lwd-url 필수");
  if (!startStr || !endStr) usage("--start --end 필수");

  // --ufvk 인자가 존재하지만 비어있는 경우 = PowerShell 변수가 empty/unset.
  // 사용자 혼란 방지 — silently stdin 으로 fallback 하지 말고 즉시 에러.
  const ufvkArgIdx = argv.indexOf("--ufvk");
  if (ufvkArgIdx >= 0) {
    const v = argv[ufvkArgIdx + 1];
    if (!v || !v.trim()) {
      usage("--ufvk 가 빈 값이다. PowerShell 변수가 set 됐는지 확인: $ufvk.Length");
    }
  }
  let ufvk = get("--ufvk");
  if (!ufvk) {
    // stdin 이 interactive TTY 면 그냥 keyboard 입력 대기 (의도된 사용).
    // stdin 이 pipe/redirect 인데 비어 있으면 5초 timeout 으로 명확히 실패.
    if (process.stdin.isTTY) {
      console.error("[submit-ufvk] UFVK를 stdin 에서 한 줄 읽는 중 (붙여넣고 Enter)...");
    } else {
      console.error("[submit-ufvk] stdin 파이프에서 UFVK 읽는 중 (5초 안에 데이터 없으면 에러)...");
    }
    ufvk = await Promise.race([
      readUfvkFromStdin(),
      new Promise<string>((_, reject) => {
        if (process.stdin.isTTY) return; // TTY 모드에서는 timeout 없음.
        setTimeout(
          () => reject(new Error("stdin pipe 가 비어 있다 — --ufvk 인자 또는 echo 로 UFVK 를 넘겨야 한다")),
          5000,
        );
      }),
    ]);
    if (!ufvk) usage("stdin에서 UFVK 읽기 실패");
  }
  const salt = get("--salt") ?? randomBytes(32).toString("hex");
  // --insecure 는 구버전 이름 — --no-verify 로 alias.
  const noVerify = has("--no-verify") || has("--insecure");
  const noMeasurementPin = has("--no-measurement-pin");

  // --sanctioned "addr1,addr2,..." (콤마 구분) — 임의의 주소들을 sanctioned 로 지정.
  // 자기 outgoing 송금 수취인을 여기 넣으면 FAIL 검출 확인 가능.
  const sancRaw = get("--sanctioned");
  const sanctionedAddresses = sancRaw
    ? sancRaw
        .split(",")
        .map((s) => s.trim())
        .filter((s) => s.length > 0)
    : undefined;

  return {
    host: host!,
    network: networkNormalized as "main" | "test",
    lwdUrl: lwdUrl!,
    ufvk,
    startHeight: Number(startStr),
    endHeight: Number(endStr),
    salt,
    noVerify,
    noMeasurementPin,
    phalaVerifier: get("--phala-verifier"),
    expectedMrTd: get("--expected-mrtd"),
    expectedRtmr3: get("--expected-rtmr3"),
    sanctionedAddresses,
    save: get("--save"),
    label: get("--label"),
  };
}

/**
 * artifact를 웹 ingest API에 POST해 DB에 저장한다 (--save).
 * 저장 URL은 일반 공개 인증서(RA-TLS 아님)이므로 표준 TLS 검증을 그대로 쓴다.
 * 응답 artifact에서 `_debug`(스캐너 진단 필드)는 제거하고 보낸다 — DB엔 artifact만.
 * 환경변수 INGEST_API_KEY가 있으면 x-api-key 헤더로 함께 보낸다.
 */
function saveArtifact(saveUrl: string, responseBody: string, label?: string): Promise<string> {
  let parsed: Record<string, unknown>;
  try {
    parsed = JSON.parse(responseBody) as Record<string, unknown>;
  } catch (e) {
    return Promise.reject(new Error(`응답 JSON 파싱 실패: ${(e as Error).message}`));
  }
  // 스캐너 응답 = artifact + _debug. _debug는 빼고 artifact만 저장.
  const { _debug, ...artifact } = parsed;
  void _debug;
  const payload = JSON.stringify({ artifact, ...(label ? { label } : {}) });
  const url = new URL(saveUrl);
  const isHttps = url.protocol === "https:";
  const apiKey = process.env.INGEST_API_KEY;
  return new Promise((resolve, reject) => {
    const reqFn = isHttps ? httpsRequest : httpRequest;
    const req = reqFn(
      {
        method: "POST",
        host: url.hostname,
        port: url.port || (isHttps ? 443 : 80),
        path: url.pathname + url.search,
        headers: {
          "content-type": "application/json",
          "content-length": Buffer.byteLength(payload),
          ...(apiKey ? { "x-api-key": apiKey } : {}),
        },
      },
      (res) => {
        let chunks = "";
        res.on("data", (c) => {
          chunks += c;
        });
        res.on("end", () => {
          if (!res.statusCode || res.statusCode >= 400) {
            reject(new Error(`HTTP ${res.statusCode}: ${chunks}`));
            return;
          }
          resolve(chunks);
        });
      },
    );
    req.on("error", reject);
    req.write(payload);
    req.end();
  });
}

/** 게시된 RA-TLS measurement 핀값 (tools/expected-measurements.json). 없거나 비면 undefined. */
type PublishedMeasurements = {
  mrtd?: string;
  rtmr3?: string;
  trust?: string;
  warning?: string;
  source?: string;
};

/** expected-measurements.json 을 읽는다. 파일 없음/깨짐/빈 값이면 undefined → 핀 skip. */
function loadPublishedMeasurements(): PublishedMeasurements | undefined {
  try {
    const raw = readFileSync(
      new URL("./expected-measurements.json", import.meta.url),
      "utf8",
    );
    const j = JSON.parse(raw) as PublishedMeasurements;
    if (!j.mrtd && !j.rtmr3) return undefined;
    return j;
  } catch {
    return undefined;
  }
}

/**
 * 호스트와 TLS 핸드셰이크만 수행해 leaf 인증서를 받아낸다 — 본문은 보내지 않는다.
 * rejectUnauthorized 는 false: self-signed (RA-TLS) 라서 표준 PKI 검증이 통과 못함.
 * 그래서 *우리가* 직접 quote 풀검증을 한다.
 */
function fetchPeerCert(host: string, port: number): Promise<X509Certificate> {
  return new Promise((resolve, reject) => {
    const socket: TLSSocket = tlsConnect(
      {
        host,
        port,
        rejectUnauthorized: false,
        // SNI — host header 와 같게.
        servername: host,
      },
      () => {
        const cert = socket.getPeerX509Certificate();
        socket.end();
        if (!cert) {
          reject(new Error("peer cert 없음"));
          return;
        }
        resolve(cert);
      },
    );
    socket.once("error", reject);
  });
}

function postScan(
  args: Args,
  pinnedCert?: X509Certificate,
): Promise<string> {
  const url = new URL("/scan", args.host);
  const body = JSON.stringify({
    mode: "real",
    ufvk: args.ufvk,
    salt: args.salt,
    chainSource: { kind: "lightwalletd", url: args.lwdUrl, network: args.network },
    scanRange: { startHeight: args.startHeight, endHeight: args.endHeight },
    ...(args.sanctionedAddresses && args.sanctionedAddresses.length > 0
      ? { sanctionedAddresses: args.sanctionedAddresses }
      : {}),
  });
  const isHttps = url.protocol === "https:";
  return new Promise((resolve, reject) => {
    const reqFn = isHttps ? httpsRequest : httpRequest;
    // RA-TLS cert 는 self-signed 체인이라 표준 PKI 검증(체인→신뢰 루트)이 통과 못 한다
    // (leaf 를 ca 로 줘도 제시되는 self-signed root 를 거부 → SELF_SIGNED_CERT_IN_CHAIN).
    // 그래서 PKI 는 끄고(rejectUnauthorized:false), 대신 이미 quote 풀검증한 그 cert 와
    // fingerprint 가 같은지로 채널을 pin 한다. UFVK 본문은 fingerprint 일치 확인 후에만 전송.
    const tlsOpts = isHttps ? { rejectUnauthorized: false } : {};
    const req = reqFn(
      {
        method: "POST",
        host: url.hostname,
        port: url.port || (isHttps ? 443 : 80),
        path: url.pathname,
        headers: {
          "content-type": "application/json",
          "content-length": Buffer.byteLength(body),
        },
        ...tlsOpts,
      },
      (res) => {
        let chunks = "";
        res.on("data", (c) => {
          chunks += c;
        });
        res.on("end", () => {
          if (!res.statusCode || res.statusCode >= 400) {
            reject(new Error(`HTTP ${res.statusCode}: ${chunks}`));
            return;
          }
          resolve(chunks);
        });
      },
    );
    req.on("error", reject);

    if (isHttps && pinnedCert) {
      // 검증된 cert 와 fingerprint 가 일치할 때만 본문(UFVK)을 쓴다. 불일치면 미전송.
      const expectedFp = pinnedCert.fingerprint256;
      req.on("socket", (socket) => {
        (socket as TLSSocket).on("secureConnect", () => {
          const peer = (socket as TLSSocket).getPeerX509Certificate();
          if (!peer || peer.fingerprint256 !== expectedFp) {
            req.destroy(
              new Error(
                "pinned cert 불일치 — postScan 채널이 RA-TLS 검증한 enclave cert 와 다르다 (UFVK 미전송)",
              ),
            );
            return;
          }
          req.write(body);
          req.end();
        });
      });
    } else {
      req.write(body);
      req.end();
    }
  });
}

async function main(): Promise<void> {
  const args = await parseArgs();
  const url = new URL(args.host);
  const port = Number(url.port) || (url.protocol === "https:" ? 443 : 80);

  console.error(
    `[submit-ufvk] host=${args.host} range=[${args.startHeight}..${args.endHeight}]`,
  );
  console.error(
    `[submit-ufvk] salt(앞 16자)=${args.salt.slice(0, 16)}... ← 본인이 보관(commitment 증명용)`,
  );

  let pinnedCert: X509Certificate | undefined;
  if (url.protocol === "https:" && !args.noVerify) {
    console.error("[submit-ufvk] RA-TLS quote 풀검증 시작...");
    // measurement 핀: CLI 인자 > 게시 파일(expected-measurements.json) > 없음(skip).
    const published = args.noMeasurementPin ? undefined : loadPublishedMeasurements();
    const expectedMrTd = args.expectedMrTd ?? published?.mrtd;
    const expectedRtmr3 = args.expectedRtmr3 ?? published?.rtmr3;
    if (!args.expectedMrTd && !args.expectedRtmr3 && published) {
      console.error(
        `[submit-ufvk] measurement 핀: 게시값 사용 (trust=${published.trust ?? "?"}) — ${published.warning ?? "expected-measurements.json"}`,
      );
    }
    const cert = await fetchPeerCert(url.hostname, port);
    let result: VerifyRaTlsResult;
    try {
      result = await verifyRaTlsCert(cert, {
        verifierUrl: args.phalaVerifier,
        expectedMrTdHex: expectedMrTd,
        expectedRtmr3Hex: expectedRtmr3,
      });
    } catch (e) {
      const msg = (e as Error).message;
      console.error(
        `[submit-ufvk] FATAL: RA-TLS 검증 실패 — UFVK 를 보내지 않고 종료. (${msg})`,
      );
      if (/mismatch/i.test(msg)) {
        console.error(
          "[submit-ufvk] ↳ measurement 불일치: 재배포로 코드/이미지가 바뀐 거면 expected-measurements.json 갱신, 아니면 다른(악성?) enclave 일 수 있다. 임시 우회: --no-measurement-pin",
        );
      }
      // process.exit() 는 열린 keep-alive 핸들과 race → Windows libuv assertion 을 낸다.
      // exitCode 만 세팅하고 return → postScan 안 타고(UFVK 미전송) 루프 드레인 후 정상 종료.
      process.exitCode = 2;
      return;
    }
    console.error(
      `[submit-ufvk] RA-TLS 검증 통과: MRTD=${result.parsed.mrTd.toString("hex").slice(0, 16)}... RTMR3=${result.parsed.rtmr3.toString("hex").slice(0, 16)}...`,
    );
    pinnedCert = cert;
  } else if (url.protocol === "https:" && args.noVerify) {
    console.error(
      "[submit-ufvk] WARNING: --no-verify — TDX quote 검증을 건너뛴다. 디버그용이 아니면 사용하지 말 것.",
    );
  }

  const responseBody = await postScan(args, pinnedCert);
  console.log(responseBody);

  if (args.save) {
    try {
      const saveResp = await saveArtifact(args.save, responseBody, args.label);
      console.error(`[submit-ufvk] artifact 저장됨 → ${args.save} : ${saveResp}`);
    } catch (e) {
      console.error(`[submit-ufvk] WARNING: artifact 저장 실패 (스캔 결과는 위에 출력됨): ${(e as Error).message}`);
    }
  }
}

await main();
