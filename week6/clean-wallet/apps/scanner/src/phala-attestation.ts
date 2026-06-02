/**
 * PhalaAttestation — 실제 TEE(Phala dstack / Intel TDX) attestation provider.
 *
 * dstack CVM 안에서 실행될 때 @phala/dstack-sdk로 진짜 TDX quote를 생성한다.
 * dstack 환경 밖(소켓 없음)에서는 동작하지 않는다 — 로컬은 SimulatedAttestation을 쓴다.
 *
 * D10 (RA-TLS): `getRaTlsCredentials()`는 enclave 안에서 만들어진 TLS keypair를 준다.
 * cert에는 TDX quote가 박혀 있어 — 클라이언트가 cert만으로 enclave 신원을 검증할 수 있다.
 * 비밀(UFVK 등)을 이 TLS 채널로만 흘려보내면 운영자가 평문에 접근하지 못한다.
 *
 * 주의: 이 코드는 TDX 하드웨어가 있어야 실제로 검증된다. 배포·확인 절차는
 * docs/deploy-phala.md 참고.
 */
import { createHash } from "node:crypto";
import { DstackClient } from "@phala/dstack-sdk";
import type {
  GetQuoteResponse,
  GetTlsKeyResponse,
  InfoResponse,
  TcbInfoV05x,
  TlsKeyOptions,
} from "@phala/dstack-sdk";
import type {
  AttestationProvider,
  AttestationProviderId,
  AttestationQuote,
} from "@clean-wallet/core";

/** payload+nonce를 32바이트 reportData로 압축한다 (TDX reportData는 최대 64바이트). */
export function reportData(payload: string, nonce: string): Buffer {
  return createHash("sha256").update(`${payload} ${nonce}`, "utf8").digest();
}

/** RA-TLS 자격증명 — node:https에 그대로 넘길 수 있는 PEM 형태. */
export type RaTlsCredentials = {
  /** PEM private key. */
  key: string;
  /** PEM cert chain (여러 인증서를 줄바꿈으로 이어 붙임). */
  cert: string;
};

/**
 * PhalaAttestation이 실제로 호출하는 dstack 클라이언트 메서드만 모은 좁은 인터페이스.
 * 단위 테스트에서 stub을 주입하기 쉽도록 분리. 실 코드는 @phala/dstack-sdk의 DstackClient.
 */
export interface DstackClientLike {
  info(): Promise<InfoResponse<TcbInfoV05x>>;
  getQuote(reportData: string | Buffer | Uint8Array): Promise<GetQuoteResponse>;
  getTlsKey(options?: TlsKeyOptions): Promise<GetTlsKeyResponse>;
}

/**
 * Phala dstack / Intel TDX 기반 attestation provider.
 *
 * - `getMeasurement()` — dstack `info()`의 `compose_hash` (배포된 앱 구성의 해시).
 * - `attest()` — dstack `getQuote()`로 TDX quote 생성, reportData에 payload+nonce 바인딩.
 * - `verify()` — provider·measurement 구조 검증. TDX quote의 암호학적 검증(Intel DCAP)은
 *   Phala verifier / dstack-verifier로 위임한다 (docs/deploy-phala.md §6).
 * - `getRaTlsCredentials()` — RA-TLS용 cert+key (D10).
 */
export class PhalaAttestation implements AttestationProvider {
  readonly providerId: AttestationProviderId = "phala-tdx";
  readonly #client: DstackClientLike;

  /**
   * @param init endpoint 문자열 (기본은 /var/run/dstack.sock의 DstackClient를 만든다) 또는
   *             stub/실 client 객체. 후자는 단위 테스트용.
   */
  constructor(init?: string | DstackClientLike) {
    if (init && typeof init !== "string") {
      this.#client = init;
    } else {
      this.#client = new DstackClient(init);
    }
  }

  async getMeasurement(): Promise<string> {
    const info = await this.#client.info();
    return info.tcb_info.compose_hash;
  }

  async attest(payload: string, nonce: string): Promise<AttestationQuote> {
    const timestamp = Date.now();
    const result = await this.#client.getQuote(reportData(payload, nonce));
    const measurement = await this.getMeasurement();
    return {
      provider: this.providerId,
      codeMeasurement: measurement,
      quote: result.quote,
      nonce,
      timestamp,
    };
  }

  async verify(
    quote: AttestationQuote,
    _payload: string,
    expectedMeasurement: string,
  ): Promise<boolean> {
    if (quote.provider !== "phala-tdx") {
      return false;
    }
    if (quote.codeMeasurement !== expectedMeasurement) {
      return false;
    }
    // TDX quote의 암호학적 검증(Intel DCAP: 서명·인증서 체인·TCB)은 여기서 하지 않는다.
    // 순수 JS 풀 구현이 비현실적이라, quote.quote(hex TDX quote)를 Phala verifier /
    // dstack-verifier로 독립 검증한다. 이 메서드는 구조·measurement 일치만 확인한다.
    // (docs/deploy-phala.md §6)
    return quote.quote.length > 0;
  }

  /**
   * RA-TLS 자격증명을 가져온다. dstack이 enclave 안에서 keypair를 만들고 cert에
   * TDX quote를 박아 돌려준다. 사용 흐름:
   *   1) scanner 시작 시 한 번 호출.
   *   2) node:https 서버를 이 cert+key로 시작.
   *   3) 클라이언트는 cert를 받아 TDX quote 추출·검증 후 본 채널로 UFVK 같은 비밀 전송.
   *
   * altNames는 cert SAN에 들어갈 호스트 — 클라이언트가 어떤 호스트로 접속하는지에 맞춰야 한다.
   */
  async getRaTlsCredentials(altNames?: string[]): Promise<RaTlsCredentials> {
    const r = await this.#client.getTlsKey({
      usageRaTls: true,
      usageServerAuth: true,
      altNames,
    });
    return {
      key: r.key,
      cert: r.certificate_chain.join("\n"),
    };
  }
}
