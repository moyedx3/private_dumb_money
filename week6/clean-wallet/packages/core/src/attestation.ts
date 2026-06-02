/**
 * 시뮬레이션 attestation.
 *
 * 실제 TEE attestation을 흉내 낸다. 진짜 하드웨어 보증이 아니다 — 키가 소스/메모리에
 * 있다. 실제 TEE(Phala/TDX) 구현은 apps/scanner의 PhalaAttestation. (docs/decisions.md D3·D4)
 *
 * 인터페이스 메서드는 async다 — 실제 TEE provider(dstack SDK)가 비동기라서.
 *
 * 자세한 설명: docs/implementation/attestation.md
 */
import {
  createPrivateKey,
  createPublicKey,
  sign as edSign,
  verify as edVerify,
} from "node:crypto";
import type { AttestationProviderId, AttestationQuote } from "./types.ts";

/**
 * attestation을 만들고 검증하는 컴포넌트의 공통 인터페이스. (architecture §4.3)
 * MVP는 SimulatedAttestation, 실 TEE 배포는 PhalaAttestation(apps/scanner).
 * 메서드는 async — 실제 TEE provider가 비동기 SDK를 쓰기 때문.
 */
export interface AttestationProvider {
  readonly providerId: AttestationProviderId;
  /** 실행 환경의 code measurement. */
  getMeasurement(): Promise<string>;
  /** payload(artifact 바인딩 페이로드)와 nonce에 대한 quote 생성. */
  attest(payload: string, nonce: string): Promise<AttestationQuote>;
  /** quote가 payload·measurement에 대해 유효한지 검증. */
  verify(
    quote: AttestationQuote,
    payload: string,
    expectedMeasurement: string,
  ): Promise<boolean>;
}

// --- 시뮬레이션 enclave 키 (고정 시드 ed25519) ---
// 고정 시드라 프로세스·실행 간 동일하다 → 다른 프로세스에서 만든 artifact도 검증 가능.
// 실제 TEE는 이 키가 하드웨어에 봉인된다. 여기서는 시뮬레이션이라 시드가 소스에 있다.
const SIMULATED_ENCLAVE_SEED = Buffer.alloc(32, 0x5a);
const PKCS8_ED25519_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const enclavePrivateKey = createPrivateKey({
  key: Buffer.concat([PKCS8_ED25519_PREFIX, SIMULATED_ENCLAVE_SEED]),
  format: "der",
  type: "pkcs8",
});
const enclavePublicKey = createPublicKey(enclavePrivateKey);

/** MVP 스캐너의 기본 code measurement (mock 값). */
export const DEFAULT_SCANNER_MEASUREMENT = "sim-scanner-measurement-v1";

/** 서명 대상 메시지를 정규 직렬화한다. */
function signedMessage(
  payload: string,
  measurement: string,
  nonce: string,
  timestamp: number,
): Buffer {
  return Buffer.from(
    JSON.stringify(["attestation", payload, measurement, nonce, timestamp]),
    "utf8",
  );
}

/** 시뮬레이션 attestation provider. */
export class SimulatedAttestation implements AttestationProvider {
  readonly providerId: AttestationProviderId = "simulated";
  readonly #measurement: string;

  constructor(measurement: string = DEFAULT_SCANNER_MEASUREMENT) {
    this.#measurement = measurement;
  }

  async getMeasurement(): Promise<string> {
    return this.#measurement;
  }

  async attest(payload: string, nonce: string): Promise<AttestationQuote> {
    const timestamp = Date.now();
    const msg = signedMessage(payload, this.#measurement, nonce, timestamp);
    const quote = edSign(null, msg, enclavePrivateKey).toString("hex");
    return {
      provider: this.providerId,
      codeMeasurement: this.#measurement,
      quote,
      nonce,
      timestamp,
    };
  }

  async verify(
    quote: AttestationQuote,
    payload: string,
    expectedMeasurement: string,
  ): Promise<boolean> {
    if (quote.provider !== "simulated") {
      return false;
    }
    if (quote.codeMeasurement !== expectedMeasurement) {
      return false;
    }
    const msg = signedMessage(payload, quote.codeMeasurement, quote.nonce, quote.timestamp);
    try {
      return edVerify(null, msg, enclavePublicKey, Buffer.from(quote.quote, "hex"));
    } catch {
      return false;
    }
  }
}
