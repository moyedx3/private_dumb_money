/**
 * RA-TLS 클라이언트 측 quote 풀검증 (B1, D10 후속).
 *
 * 입력: enclave가 제시한 leaf 인증서 (DER 또는 PEM)
 * 동작:
 *   1) 인증서에서 dstack의 TDX quote 확장(OID 1.3.6.1.4.1.62397.1.1)을 추출.
 *   2) Phala verifier API(또는 사용자 지정 verifier)로 quote의 암호학적 유효성을 검증.
 *   3) report_data 가 cert pubkey 와 일치하는지 확인 — sha512("ratls-cert:" || SPKI_DER).
 *      → TLS 채널이 *실제로 attest 된 enclave에 종단됨* 을 보장 (anti-substitution).
 *   4) (선택) MRTD 또는 RTMR3 가 사용자가 지정한 expected 값과 일치하는지 확인.
 *
 * dstack RA-TLS 스펙 출처:
 *   - OID: github.com/Dstack-TEE/dstack `ra-tls/src/oids.rs`
 *   - report_data 도메인 세퍼레이터: dstack-attest QuoteContentType::RaTlsCert
 *   - quote 구조 (TDX 1.0): Intel TDX architecture spec — header(48) + td_report(584)
 *
 * 한계: 이 코드는 quote 의 PCK chain·TCB 같은 암호학 검증은 Phala verifier 에 위임한다.
 * 순수 JS 풀 구현(@phala/dcap-qvl)도 가능하지만 의존성 비용 vs 사용 편의에서 verifier API 선택.
 */
import { createHash, X509Certificate } from "node:crypto";

/**
 * dstack TDX quote 확장 OID `1.3.6.1.4.1.62397.1.1` 의 DER 인코딩.
 * 62397 = 0x83 0xE7 0x3D in base-128 with continuation bit.
 */
const TDX_QUOTE_OID_DER = Buffer.from([
  0x06,
  0x0a, // OBJECT IDENTIFIER, length 10
  0x2b, // 1.3
  0x06, // 6
  0x01, // 1
  0x04, // 4
  0x01, // 1
  0x83,
  0xe7,
  0x3d, // 62397
  0x01, // 1
  0x01, // 1
]);

/** RA-TLS pubkey 바인딩 도메인 세퍼레이터 — dstack `QuoteContentType::RaTlsCert.tag()`. */
const RATLS_CERT_TAG = "ratls-cert";

/** TDX quote header 크기 (Intel TDX architecture, v4 quote). */
const TDX_QUOTE_HEADER_LEN = 48;
/** TD report 내부의 mr_td 시작 오프셋 (tee_tcb_svn 16 + mr_seam 48 + mr_signer_seam 48 + seam_attr 8 + td_attr 8 + xfam 8). */
const TD_REPORT_MR_TD_OFFSET = 16 + 48 + 48 + 8 + 8 + 8;
/** TD report 내부의 rtmr3 시작 오프셋 (mr_td..mr_owner_config 까지 48*4 = 192 + 위에서 184 = 376, + rtmr0 48 + rtmr1 48 + rtmr2 48). */
const TD_REPORT_RTMR3_OFFSET = TD_REPORT_MR_TD_OFFSET + 48 * 4 + 48 * 3;
/** TD report 내부의 report_data 시작 오프셋. */
const TD_REPORT_REPORT_DATA_OFFSET = TD_REPORT_RTMR3_OFFSET + 48;

/** Phala 공개 verifier 기본 엔드포인트. */
export const DEFAULT_PHALA_VERIFIER = "https://cloud-api.phala.com/api/v1/attestations/verify";

/** 파싱된 TDX quote 필드 — 우리 검증에 쓰는 것만. */
export type ParsedTdxQuote = {
  /** 64바이트 report_data. dstack 은 여기를 cert pubkey 에 바인딩한다. */
  reportData: Buffer;
  /** 48바이트 MRTD — enclave 이미지 해시. */
  mrTd: Buffer;
  /** 48바이트 RTMR3 — dstack 의 compose_hash 가 extend 되는 슬롯. */
  rtmr3: Buffer;
  /** 원본 quote 바이트 — verifier API 에 그대로 보낸다. */
  raw: Buffer;
};

/** verifier API 응답에서 우리가 보는 필드. 다른 필드는 무시. */
type VerifierResponse = {
  /** Phala cloud-api 의 성공 플래그 (실제 응답 키). */
  success?: boolean;
  /** 구버전/타 verifier 호환 플래그. */
  verified?: boolean;
  /** Phala cloud-api: CVM(클라우드) 증명 여부. */
  proof_of_cloud?: boolean;
  // 일부 verifier 는 status, message, advisory_ids 등을 더 돌려준다.
  [k: string]: unknown;
};

/**
 * 인증서 DER 에서 TDX quote 확장 바이트를 추출한다.
 *
 * 동작: cert 전체에서 우리 OID 의 DER 패턴을 찾고, 그 직후의 OCTET STRING content 를 돌려준다.
 * (dstack 은 critical 플래그를 안 단다 — 만약 BOOLEAN 이 있으면 건너뛴다.)
 */
export function extractTdxQuoteFromCert(certDer: Buffer): Buffer {
  const idx = certDer.indexOf(TDX_QUOTE_OID_DER);
  if (idx < 0) {
    throw new Error(
      `cert 에 TDX quote 확장(OID 1.3.6.1.4.1.62397.1.1) 이 없다 — RA-TLS cert 가 맞는지 확인하라`,
    );
  }
  let p = idx + TDX_QUOTE_OID_DER.length;

  // 선택적 critical BOOLEAN (tag 0x01) 건너뛰기.
  if (certDer[p] === 0x01) {
    const len = certDer[p + 1] ?? 0; // short form (BOOLEAN 은 항상 1)
    p += 2 + len;
  }

  // OCTET STRING (tag 0x04) 헤더 파싱.
  if (certDer[p] !== 0x04) {
    throw new Error(
      `OID 다음에 OCTET STRING 이 와야 하는데 tag=0x${certDer[p]?.toString(16)} — cert 구조 이상`,
    );
  }
  p += 1;

  // 길이: 단형(< 128) 또는 장형 (high bit = 추가 길이 바이트 개수).
  let len = certDer[p] ?? 0;
  p += 1;
  if (len & 0x80) {
    const numLenBytes = len & 0x7f;
    if (numLenBytes === 0 || numLenBytes > 4) {
      throw new Error(`비정상 ASN.1 length 인코딩 (numLenBytes=${numLenBytes})`);
    }
    len = 0;
    for (let i = 0; i < numLenBytes; i++) {
      len = (len << 8) | (certDer[p + i] ?? 0);
    }
    p += numLenBytes;
  }
  if (p + len > certDer.length) {
    throw new Error(`OCTET STRING length 가 cert 범위를 벗어남`);
  }
  // X.509 extnValue 는 OCTET STRING 이고, dstack 은 그 안에 quote 를 다시 OCTET STRING 으로
  // 한 겹 더 감싼다 (extnValue = OCTET STRING { OCTET STRING { quote } }). 위 루프는 바깥
  // 한 겹만 벗겼으므로 내부에 "정확히 채우는" OCTET STRING 이 또 있으면 한 번 더 벗긴다.
  // (raw TDX quote 도 0x04(version=4) 로 시작하므로, 길이가 정확히 일치할 때만 unwrap 해서
  //  raw quote 를 잘못 벗기는 일을 막는다.)
  return unwrapNestedOctetString(certDer.subarray(p, p + len));
}

/**
 * `buf` 가 자신을 정확히 채우는 OCTET STRING (tag 0x04, 헤더+길이 == buf.length) 이면 그
 * 내용을 돌려준다. 아니면 `buf` 그대로. dstack RA-TLS cert 의 이중 OCTET STRING 래핑 처리용.
 */
function unwrapNestedOctetString(buf: Buffer): Buffer {
  if (buf.length < 2 || buf[0] !== 0x04) return buf;
  let q = 1;
  let l = buf[q] ?? 0;
  q += 1;
  if (l & 0x80) {
    const n = l & 0x7f;
    if (n < 1 || n > 4 || q + n > buf.length) return buf;
    l = 0;
    for (let i = 0; i < n; i++) l = (l << 8) | (buf[q + i] ?? 0);
    q += n;
  }
  return q + l === buf.length ? buf.subarray(q, q + l) : buf;
}

/**
 * TDX quote 바이트에서 우리가 쓰는 필드만 뽑는다 — header skip, td_report 의 mr_td/rtmr3/report_data.
 * 풀 구조 검증(서명·QE report·PCK chain)은 verifier API 에 위임한다.
 */
export function parseTdxQuote(quote: Buffer): ParsedTdxQuote {
  if (quote.length < TDX_QUOTE_HEADER_LEN + TD_REPORT_REPORT_DATA_OFFSET + 64) {
    throw new Error(
      `quote 길이(${quote.length})가 너무 짧음 — TDX quote 가 아닐 수 있다`,
    );
  }
  const reportBodyStart = TDX_QUOTE_HEADER_LEN;
  const mrTd = quote.subarray(
    reportBodyStart + TD_REPORT_MR_TD_OFFSET,
    reportBodyStart + TD_REPORT_MR_TD_OFFSET + 48,
  );
  const rtmr3 = quote.subarray(
    reportBodyStart + TD_REPORT_RTMR3_OFFSET,
    reportBodyStart + TD_REPORT_RTMR3_OFFSET + 48,
  );
  const reportData = quote.subarray(
    reportBodyStart + TD_REPORT_REPORT_DATA_OFFSET,
    reportBodyStart + TD_REPORT_REPORT_DATA_OFFSET + 64,
  );
  return {
    reportData: Buffer.from(reportData),
    mrTd: Buffer.from(mrTd),
    rtmr3: Buffer.from(rtmr3),
    raw: quote,
  };
}

/**
 * dstack 의 RA-TLS pubkey 바인딩을 검증한다.
 *
 * report_data == sha512("ratls-cert:" || SPKI_DER) 여야 cert pubkey 가 *attest 된 enclave 에서*
 * 만들어진 것임을 안다. 다른 값이면 누군가 다른 cert 와 quote 를 짝지어 채널을 가로채는 중일 수 있음.
 */
export function verifyPubkeyBinding(
  reportData: Buffer,
  spkiDer: Buffer,
): boolean {
  if (reportData.length !== 64) return false;
  const hash = createHash("sha512");
  hash.update(`${RATLS_CERT_TAG}:`, "utf8");
  hash.update(spkiDer);
  const expected = hash.digest();
  return expected.equals(reportData);
}

/**
 * Phala verifier API (또는 호환 verifier) 에 quote 를 POST 해서 검증한다.
 *
 * 의존성: Node 의 내장 fetch (Node 22+). 별도 패키지 없음.
 */
export async function verifyQuoteViaPhalaApi(
  quote: Buffer,
  verifierUrl: string = DEFAULT_PHALA_VERIFIER,
): Promise<VerifierResponse> {
  const res = await fetch(verifierUrl, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ hex: quote.toString("hex") }),
  });
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(
      `verifier HTTP ${res.status} ${res.statusText}${text ? ` — ${text}` : ""}`,
    );
  }
  return (await res.json()) as VerifierResponse;
}

/**
 * X509Certificate 에서 SubjectPublicKeyInfo (SPKI) DER 을 꺼낸다.
 *
 * Node 의 X509Certificate.publicKey 는 KeyObject. KeyObject.export({format:'der',type:'spki'}) 로 SPKI DER.
 */
export function spkiDerFromCert(cert: X509Certificate): Buffer {
  const der = cert.publicKey.export({ format: "der", type: "spki" });
  return Buffer.isBuffer(der) ? der : Buffer.from(der);
}

/**
 * 통합 검증: 인증서 → quote 추출 → verifier API → pubkey 바인딩 → (선택) measurement 매치.
 *
 * 통과하면 (성공 정보 객체) 반환, 실패하면 throw — submit-ufvk 가 그대로 abort 하도록.
 */
export type VerifyRaTlsOptions = {
  /** verifier 엔드포인트 (기본 Phala 공개). 회사 내부 verifier 로 교체 가능. */
  verifierUrl?: string;
  /** RTMR3 가 이 값과 같아야 한다 (hex). 생략하면 RTMR3 매치는 안 한다. */
  expectedRtmr3Hex?: string;
  /** MRTD 가 이 값과 같아야 한다 (hex). 생략하면 안 한다. */
  expectedMrTdHex?: string;
};

export type VerifyRaTlsResult = {
  parsed: ParsedTdxQuote;
  verifierResponse: VerifierResponse;
};

export async function verifyRaTlsCert(
  cert: X509Certificate,
  opts: VerifyRaTlsOptions = {},
): Promise<VerifyRaTlsResult> {
  const certDer = Buffer.from(cert.raw);
  const quote = extractTdxQuoteFromCert(certDer);
  const parsed = parseTdxQuote(quote);

  // 1) verifier API — 암호학적 유효성 + TCB.
  const verifierResponse = await verifyQuoteViaPhalaApi(quote, opts.verifierUrl);
  // Phala cloud-api 는 성공을 `success:true` 로 준다 (구버전/타 verifier 는 `verified:true`).
  const verifierOk =
    verifierResponse.success === true || verifierResponse.verified === true;
  if (!verifierOk) {
    throw new Error(
      `verifier 가 quote 를 거부 — ${JSON.stringify(verifierResponse).slice(0, 400)}`,
    );
  }

  // 2) pubkey 바인딩 — channel 종단이 *진짜 그 enclave* 임을 보장.
  const spkiDer = spkiDerFromCert(cert);
  if (!verifyPubkeyBinding(parsed.reportData, spkiDer)) {
    throw new Error(
      "report_data 가 cert pubkey 와 일치하지 않음 — 다른 enclave/cert 가 짝지어졌을 수 있음 (channel substitution)",
    );
  }

  // 3) (선택) 사용자가 지정한 measurement 매치.
  if (opts.expectedMrTdHex) {
    const got = parsed.mrTd.toString("hex");
    if (got !== opts.expectedMrTdHex.toLowerCase()) {
      throw new Error(
        `MRTD mismatch — expected ${opts.expectedMrTdHex}, got ${got}`,
      );
    }
  }
  if (opts.expectedRtmr3Hex) {
    const got = parsed.rtmr3.toString("hex");
    if (got !== opts.expectedRtmr3Hex.toLowerCase()) {
      throw new Error(
        `RTMR3 mismatch — expected ${opts.expectedRtmr3Hex}, got ${got}`,
      );
    }
  }

  return { parsed, verifierResponse };
}
