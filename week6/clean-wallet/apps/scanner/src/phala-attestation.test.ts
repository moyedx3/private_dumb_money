/**
 * PhalaAttestation 단위 테스트.
 *
 * dstack 소켓 없이도 동작 — DstackClientLike stub을 생성자에 주입해
 * SDK가 받는 인자/돌려주는 형식이 우리 코드 가정과 맞는지 검증한다.
 *
 * 검증 항목:
 * - getMeasurement() → info().tcb_info.compose_hash
 * - attest(payload, nonce) → getQuote(reportData(payload,nonce)) 호출 + 결과 wrap
 * - verify(): provider id·measurement 일치 + quote 비어있지 않음
 * - getRaTlsCredentials(altNames) → getTlsKey({usageRaTls,usageServerAuth,altNames})
 *   호출 후 certificate_chain 합쳐 반환
 */
import { test } from "node:test";
import { strict as assert } from "node:assert";
import type {
  GetQuoteResponse,
  GetTlsKeyResponse,
  InfoResponse,
  TcbInfoV05x,
  TlsKeyOptions,
} from "@phala/dstack-sdk";
import { PhalaAttestation, reportData, type DstackClientLike } from "./phala-attestation.ts";

// --- 테스트용 stub 객체 ---

type Call =
  | { kind: "info" }
  | { kind: "getQuote"; reportData: Buffer }
  | { kind: "getTlsKey"; options?: TlsKeyOptions };

function makeStubClient(opts?: {
  composeHash?: string;
  quote?: string;
  tlsKey?: { key: string; certChain: string[] };
}): DstackClientLike & { calls: Call[] } {
  const composeHash = opts?.composeHash ?? "stub-compose-hash";
  const quote = opts?.quote ?? "deadbeef";
  const tlsKey = opts?.tlsKey ?? {
    key: "-----BEGIN PRIVATE KEY-----\nSTUB\n-----END PRIVATE KEY-----",
    certChain: [
      "-----BEGIN CERTIFICATE-----\nLEAF\n-----END CERTIFICATE-----",
      "-----BEGIN CERTIFICATE-----\nINTERMEDIATE\n-----END CERTIFICATE-----",
    ],
  };
  const calls: Call[] = [];
  return {
    calls,
    async info(): Promise<InfoResponse<TcbInfoV05x>> {
      calls.push({ kind: "info" });
      return {
        app_id: "stub-app",
        instance_id: "stub-instance",
        app_cert: "",
        tcb_info: {
          mrtd: "",
          rtmr0: "",
          rtmr1: "",
          rtmr2: "",
          rtmr3: "",
          app_compose: "",
          event_log: [],
          mr_aggregated: "",
          os_image_hash: "",
          compose_hash: composeHash,
          device_id: "",
        },
        app_name: "stub",
        device_id: "",
        key_provider_info: "",
        compose_hash: composeHash,
      };
    },
    async getQuote(rd: string | Buffer | Uint8Array): Promise<GetQuoteResponse> {
      const buf = Buffer.isBuffer(rd) ? rd : Buffer.from(rd as Uint8Array);
      calls.push({ kind: "getQuote", reportData: buf });
      return {
        quote,
        event_log: "",
        replayRtmrs: () => [],
      };
    },
    async getTlsKey(options?: TlsKeyOptions): Promise<GetTlsKeyResponse> {
      calls.push({ kind: "getTlsKey", options });
      return {
        __name__: "GetTlsKeyResponse" as const,
        key: tlsKey.key,
        certificate_chain: tlsKey.certChain,
        asUint8Array: () => new Uint8Array(),
      };
    },
  };
}

test("getMeasurement은 dstack info().tcb_info.compose_hash를 반환한다", async () => {
  const stub = makeStubClient({ composeHash: "abc123" });
  const att = new PhalaAttestation(stub);
  const m = await att.getMeasurement();
  assert.equal(m, "abc123");
  assert.equal(stub.calls.length, 1);
  assert.equal(stub.calls[0]!.kind, "info");
});

test("providerId는 phala-tdx이다", () => {
  const att = new PhalaAttestation(makeStubClient());
  assert.equal(att.providerId, "phala-tdx");
});

test("attest는 sha256(payload+' '+nonce)를 reportData로 getQuote 호출하고 결과를 wrap한다", async () => {
  const stub = makeStubClient({ composeHash: "m1", quote: "qhex" });
  const att = new PhalaAttestation(stub);
  const payload = "binding-payload";
  const nonce = "n1";
  const q = await att.attest(payload, nonce);

  // reportData 일치
  const expectedReport = reportData(payload, nonce);
  const getQuoteCall = stub.calls.find((c) => c.kind === "getQuote") as
    | (Call & { kind: "getQuote" })
    | undefined;
  assert.ok(getQuoteCall, "getQuote가 호출돼야 한다");
  assert.deepEqual(getQuoteCall.reportData, expectedReport);
  assert.equal(getQuoteCall.reportData.length, 32);

  // 반환 형식
  assert.equal(q.provider, "phala-tdx");
  assert.equal(q.codeMeasurement, "m1");
  assert.equal(q.quote, "qhex");
  assert.equal(q.nonce, "n1");
  assert.ok(typeof q.timestamp === "number" && q.timestamp > 0);
});

test("verify는 provider/measurement/non-empty quote 전부 OK면 true", async () => {
  const att = new PhalaAttestation(makeStubClient());
  const ok = await att.verify(
    {
      provider: "phala-tdx",
      codeMeasurement: "m1",
      quote: "abc",
      nonce: "n",
      timestamp: 1,
    },
    "payload",
    "m1",
  );
  assert.equal(ok, true);
});

test("verify는 provider가 phala-tdx가 아니면 false", async () => {
  const att = new PhalaAttestation(makeStubClient());
  const ok = await att.verify(
    {
      provider: "simulated",
      codeMeasurement: "m1",
      quote: "abc",
      nonce: "n",
      timestamp: 1,
    },
    "payload",
    "m1",
  );
  assert.equal(ok, false);
});

test("verify는 measurement가 expected와 다르면 false", async () => {
  const att = new PhalaAttestation(makeStubClient());
  const ok = await att.verify(
    {
      provider: "phala-tdx",
      codeMeasurement: "wrong",
      quote: "abc",
      nonce: "n",
      timestamp: 1,
    },
    "payload",
    "expected",
  );
  assert.equal(ok, false);
});

test("verify는 quote가 비어있으면 false", async () => {
  const att = new PhalaAttestation(makeStubClient());
  const ok = await att.verify(
    {
      provider: "phala-tdx",
      codeMeasurement: "m1",
      quote: "",
      nonce: "n",
      timestamp: 1,
    },
    "payload",
    "m1",
  );
  assert.equal(ok, false);
});

test("getRaTlsCredentials는 getTlsKey({usageRaTls,usageServerAuth,altNames})를 호출한다", async () => {
  const stub = makeStubClient();
  const att = new PhalaAttestation(stub);
  const creds = await att.getRaTlsCredentials(["host1.example", "host2.example"]);

  const call = stub.calls.find((c) => c.kind === "getTlsKey") as
    | (Call & { kind: "getTlsKey" })
    | undefined;
  assert.ok(call, "getTlsKey가 호출돼야 한다");
  assert.equal(call.options?.usageRaTls, true);
  assert.equal(call.options?.usageServerAuth, true);
  assert.deepEqual(call.options?.altNames, ["host1.example", "host2.example"]);

  // PEM 형태 반환 + cert chain join
  assert.match(creds.key, /BEGIN PRIVATE KEY/);
  assert.match(creds.cert, /BEGIN CERTIFICATE/);
  // chain join 확인 — 두 cert가 줄바꿈으로 연결돼 들어 있어야 한다
  const certCount = creds.cert.match(/BEGIN CERTIFICATE/g)?.length ?? 0;
  assert.equal(certCount, 2);
});

test("reportData는 sha256 32바이트 결정적 결과", () => {
  const a = reportData("p", "n");
  const b = reportData("p", "n");
  const c = reportData("p", "n2");
  assert.equal(a.length, 32);
  assert.deepEqual(a, b);
  assert.notDeepEqual(a, c);
});
