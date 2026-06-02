/**
 * RA-TLS 클라이언트 검증 모듈 단위 테스트 (B1).
 *
 * 검증 대상: extractTdxQuoteFromCert · parseTdxQuote · verifyPubkeyBinding.
 * verifyQuoteViaPhalaApi 는 네트워크를 타므로 여기선 테스트하지 않음 (e2e 단계).
 * verifyRaTlsCert (통합) 도 네트워크 의존 — 컴포넌트만 검증.
 */
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { createHash, generateKeyPairSync } from "node:crypto";
import {
  extractTdxQuoteFromCert,
  parseTdxQuote,
  verifyPubkeyBinding,
} from "./ra-tls-verify.ts";

// dstack quote 확장 OID 의 DER 인코딩 — verify 모듈 내부와 동일해야 검색됨.
const OID_BYTES = Buffer.from([
  0x06, 0x0a, 0x2b, 0x06, 0x01, 0x04, 0x01, 0x83, 0xe7, 0x3d, 0x01, 0x01,
]);

// 가짜 cert DER: 노이즈 + OID + OCTET STRING(quote) + 노이즈. 우리 추출기는
// indexOf 로 OID 패턴을 잡으므로 풀 X.509 구조 없이도 작동.
function makeFakeCertDer(quote: Buffer): Buffer {
  // OCTET STRING 헤더: 짧으면 0x04 [len], 128 이상이면 long form.
  const octetHeader =
    quote.length < 0x80
      ? Buffer.from([0x04, quote.length])
      : (() => {
          // long-form length: 2바이트면 충분 (TDX quote 는 4~10KB)
          const lenHi = (quote.length >> 8) & 0xff;
          const lenLo = quote.length & 0xff;
          return Buffer.from([0x04, 0x82, lenHi, lenLo]);
        })();
  const noise = Buffer.from([0xde, 0xad, 0xbe, 0xef, 0x00, 0x01]);
  return Buffer.concat([noise, OID_BYTES, octetHeader, quote, noise]);
}

test("extractTdxQuoteFromCert 는 OID 다음 OCTET STRING 내용을 돌려준다 (short length)", () => {
  const quote = Buffer.from("0123456789abcdef".repeat(4), "hex"); // 32 bytes
  const cert = makeFakeCertDer(quote);
  const got = extractTdxQuoteFromCert(cert);
  assert.deepEqual(got, quote);
});

test("extractTdxQuoteFromCert 는 OCTET STRING long-form 길이도 처리한다", () => {
  const quote = Buffer.alloc(500, 0xab); // > 127 → long-form length
  const cert = makeFakeCertDer(quote);
  const got = extractTdxQuoteFromCert(cert);
  assert.equal(got.length, 500);
  assert.deepEqual(got, quote);
});

test("extractTdxQuoteFromCert 는 OID 가 없으면 throw", () => {
  const cert = Buffer.from("00112233445566778899aabbccddeeff", "hex");
  assert.throws(() => extractTdxQuoteFromCert(cert), /TDX quote 확장.*없다/);
});

test("extractTdxQuoteFromCert 는 OID 다음에 다른 tag 가 오면 throw", () => {
  // OID + 잘못된 tag (0x02 INTEGER)
  const bad = Buffer.concat([
    Buffer.from([0x30, 0x10]), // SEQUENCE wrapper noise
    OID_BYTES,
    Buffer.from([0x02, 0x01, 0x00]), // INTEGER 0 대신 OCTET STRING 와야 함
  ]);
  assert.throws(() => extractTdxQuoteFromCert(bad), /OCTET STRING/);
});

test("extractTdxQuoteFromCert 는 dstack 이중 OCTET STRING 래핑을 한 겹 더 벗긴다", () => {
  // 실 dstack cert: extnValue = OCTET STRING { OCTET STRING { quote } }.
  const quote = Buffer.concat([
    Buffer.from([0x04, 0x00, 0x02, 0x00]), // raw TDX v4 quote 시작 (version=4 LE)
    Buffer.alloc(700, 0x11),
  ]);
  const innerHeader = Buffer.from([0x04, 0x82, (quote.length >> 8) & 0xff, quote.length & 0xff]);
  const wrapped = Buffer.concat([innerHeader, quote]); // 안쪽 OCTET STRING { quote }
  const cert = makeFakeCertDer(wrapped); // 바깥이 또 OCTET STRING 으로 감쌈
  const got = extractTdxQuoteFromCert(cert);
  assert.deepEqual(got, quote);
});

test("extractTdxQuoteFromCert 는 raw quote(0x04 version) 를 과도하게 벗기지 않는다", () => {
  // 단일 래핑: 내용이 0x04 로 시작하지만 OCTET STRING 으로 정확히 채우지 않음 → 그대로 둬야 함.
  const quote = Buffer.concat([
    Buffer.from([0x04, 0x00, 0x02, 0x00, 0x81, 0x00, 0x00, 0x00]),
    Buffer.alloc(700, 0x22),
  ]);
  const cert = makeFakeCertDer(quote);
  const got = extractTdxQuoteFromCert(cert);
  assert.deepEqual(got, quote);
});

// --- parseTdxQuote ---

// 합성 TDX quote: 48 헤더 + 584 TD report. mr_td/rtmr3/report_data 위치에 알 수 있는 마커.
function makeFakeTdxQuote(opts: {
  mrTd: Buffer;
  rtmr3: Buffer;
  reportData: Buffer;
}): Buffer {
  assert.equal(opts.mrTd.length, 48);
  assert.equal(opts.rtmr3.length, 48);
  assert.equal(opts.reportData.length, 64);
  const quote = Buffer.alloc(48 + 584);
  // header (48): version=4, ak_type=2, tee_type=0x81 등 — 우리는 안 읽음.
  quote.writeUInt16LE(4, 0);
  quote.writeUInt16LE(2, 2);
  quote.writeUInt32LE(0x81, 4);
  // td_report 안의 offset 들 (lib.ts 와 동일해야 함):
  const tdStart = 48;
  // mr_td @ tdStart + 16+48+48+8+8+8 = tdStart + 136
  opts.mrTd.copy(quote, tdStart + 136);
  // rtmr3 @ tdStart + 136 + 48*4 + 48*3 = tdStart + 472
  opts.rtmr3.copy(quote, tdStart + 472);
  // report_data @ tdStart + 472 + 48 = tdStart + 520
  opts.reportData.copy(quote, tdStart + 520);
  return quote;
}

test("parseTdxQuote 는 mr_td / rtmr3 / report_data 를 정확한 오프셋에서 읽는다", () => {
  const mrTd = Buffer.from("aa".repeat(48), "hex");
  const rtmr3 = Buffer.from("bb".repeat(48), "hex");
  const reportData = Buffer.from("cc".repeat(64), "hex");
  const quote = makeFakeTdxQuote({ mrTd, rtmr3, reportData });
  const parsed = parseTdxQuote(quote);
  assert.deepEqual(parsed.mrTd, mrTd);
  assert.deepEqual(parsed.rtmr3, rtmr3);
  assert.deepEqual(parsed.reportData, reportData);
  assert.equal(parsed.raw.length, quote.length);
});

test("parseTdxQuote 는 quote 가 너무 짧으면 throw", () => {
  const short = Buffer.alloc(100);
  assert.throws(() => parseTdxQuote(short), /너무 짧음/);
});

// --- verifyPubkeyBinding ---

test("verifyPubkeyBinding 는 sha512('ratls-cert:' || SPKI_DER) 와 같으면 true", () => {
  // 실제 키페어의 SPKI DER 로 테스트.
  const { publicKey } = generateKeyPairSync("ed25519");
  const spki = publicKey.export({ format: "der", type: "spki" });
  const spkiBuf = Buffer.isBuffer(spki) ? spki : Buffer.from(spki);
  const h = createHash("sha512");
  h.update("ratls-cert:", "utf8");
  h.update(spkiBuf);
  const reportData = h.digest();
  assert.equal(verifyPubkeyBinding(reportData, spkiBuf), true);
});

test("verifyPubkeyBinding 는 다른 pubkey 면 false", () => {
  const { publicKey: pk1 } = generateKeyPairSync("ed25519");
  const { publicKey: pk2 } = generateKeyPairSync("ed25519");
  const spki1 = Buffer.from(pk1.export({ format: "der", type: "spki" }) as Buffer);
  const spki2 = Buffer.from(pk2.export({ format: "der", type: "spki" }) as Buffer);
  const h = createHash("sha512");
  h.update("ratls-cert:", "utf8");
  h.update(spki1);
  const reportData = h.digest();
  // 같은 reportData 인데 cert 는 다른 키 → false.
  assert.equal(verifyPubkeyBinding(reportData, spki2), false);
});

test("verifyPubkeyBinding 는 잘못된 도메인(누락) 인 reportData 면 false", () => {
  const { publicKey } = generateKeyPairSync("ed25519");
  const spki = Buffer.from(publicKey.export({ format: "der", type: "spki" }) as Buffer);
  // 도메인 세퍼레이터 없이 단순 sha512(spki) → 일치하지 않아야 함.
  const reportData = createHash("sha512").update(spki).digest();
  assert.equal(verifyPubkeyBinding(reportData, spki), false);
});

test("verifyPubkeyBinding 는 reportData 길이가 64 가 아니면 false", () => {
  const spki = Buffer.alloc(32);
  assert.equal(verifyPubkeyBinding(Buffer.alloc(63), spki), false);
  assert.equal(verifyPubkeyBinding(Buffer.alloc(65), spki), false);
  assert.equal(verifyPubkeyBinding(Buffer.alloc(32), spki), false);
});
