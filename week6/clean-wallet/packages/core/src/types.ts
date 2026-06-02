/**
 * Zcash Private Off-Ramp Screening — 핵심 데이터 모델.
 *
 * 시스템 구성요소(사용자 / 거래소 / Attested Scanner / 검증기) 사이를 흐르는
 * 데이터와 screening artifact의 형태를 정의한다. 런타임 로직은 없다.
 *
 * 자세한 설명: docs/architecture.md §5, docs/implementation/types.md
 */

/** Zcash 네트워크. */
export type ZcashNetwork = "main" | "test";

/** 스캔 대상 블록 구간 (양 끝 높이 포함). */
export type BlockRange = {
  startHeight: number;
  endHeight: number;
};

/**
 * Zcash 데이터 소스. mock 체인 또는 외부 lightwalletd.
 *
 * 결정 D9: chainSource를 요청 파라미터 + artifact 바인딩으로 두어, 거래소가
 * `policy.approvedChainSources` allowlist로 enforce할 수 있게 한다.
 */
export type ChainSource =
  | { kind: "mock" }
  | { kind: "lightwalletd"; url: string; network: ZcashNetwork };

/**
 * 사용자가 스캐너에 제공하는 viewing scope.
 *
 * `viewingKey`는 비밀이며 enclave 안에서만 다뤄진다 — artifact에는 commitment(해시)만
 * 공개된다. 실제 Zcash에서 출금 수취인을 보려면 FVK/UFVK(=OVK 포함)가 필요하다.
 * MVP의 mock 경로는 `viewingKey`가 mock scope 식별 문자열. (docs/architecture.md §9)
 *
 * 결정 D11: commitment는 user-provided salt와 함께 해싱돼야 hiding이 보장됨
 * (`hashing.md` 참고). salt는 ViewingScope의 일부가 아니라 commitment 연산 파라미터.
 */
export type ViewingScope = {
  scopeId: string;
  network: ZcashNetwork;
  /** 비밀. enclave 밖으로 나가지 않는다. */
  viewingKey: string;
};

/** 제재 대상 ZEC 주소 1건. */
export type SanctionedAddress = {
  label: string;
  asset: "ZEC";
  address: string;
};

/**
 * 거래소가 정의하는 스크리닝 정책.
 * `policyHash`는 이 필드 전체를 해싱해 계산하며, artifact를 이 정책에 묶는다.
 */
export type ScreeningPolicy = {
  policyName: string;
  policyVersion: string;
  /** 검사 대상 블록 구간. */
  auditRange: BlockRange;
  /** 제재 주소 집합 전체에 대한 commitment(해시). */
  sanctionedAddressSetHash: string;
  /** 거래소가 신뢰하는 스캐너의 기대 code measurement. */
  scannerMeasurement: string;
  /** 이 정책이 묶이는 입금 요청의 해시. */
  depositIntentHash: string;
  /**
   * 거래소가 허용한 chain source 목록. 요청·artifact의 `chainSource`가 이 안에
   * 들어 있어야 검증기가 통과시킨다. (D9)
   */
  approvedChainSources: ChainSource[];
};

/**
 * 특정 입금 요청의 정보. artifact를 이 입금에 묶어 재사용(replay)을 막는다.
 *
 * depositIntentHash = hash(exchangeDepositAddress, depositAmountZat, nonce, expiryUnix)
 */
export type DepositIntent = {
  exchangeName: string;
  exchangeDepositAddress: string;
  depositAmountZat: string;
  nonce: string;
  expiryUnix: number;
};

/**
 * 거래소 → 스캐너로 가는 스크리닝 요청.
 * 스캐너는 이 요청대로 스캔하고 결과를 요청에 바인딩한다.
 */
export type ScreeningRequest = {
  policy: ScreeningPolicy;
  sanctionedAddresses: SanctionedAddress[];
  depositIntent: DepositIntent;
  scanRange: BlockRange;
  /**
   * 이번 스캔에 쓸 chain source. `policy.approvedChainSources` 중 하나여야 한다.
   * artifact에 그대로 바인딩되어 검증기가 정책 위반을 잡아낸다. (D9)
   */
  chainSource: ChainSource;
  /** 신선도·재생 방지를 위한 1회용 값. */
  nonce: string;
};

/** 스크리닝 결과. PASS = 제재 매칭 없음, FAIL = 제재 수취인 발견. */
export type ScreeningResult = "PASS" | "FAIL";

/**
 * 스캐너가 체인에서 도출한 출금 record 1건.
 * enclave 내부에서만 존재하며 artifact에 직접 담기지 않는다 (프라이버시 경계).
 */
export type DerivedRecord = {
  txid: string;
  blockHeight: number;
  direction: "outgoing";
  /** OVK 복호화로 복원된 수취인 주소(또는 transparent 출력의 t-addr). */
  recipientAddress: string;
  /** 정규화된 수취인 주소의 해시. */
  recipientHash: string;
  amountZat: string;
};

/**
 * 스캔 단계의 내부 산출물 (artifact로 포장되기 전).
 * `derivedRecords`는 enclave 밖으로 나가지 않는다.
 */
export type ScanResult = {
  scannedRange: BlockRange;
  derivedRecords: DerivedRecord[];
  result: ScreeningResult;
  /** FAIL인 경우 제재목록과 매칭된 수취인 해시. PASS면 빈 배열. */
  matchedRecipientHashes: string[];
};

/** attestation을 만든 실행 환경의 종류. */
export type AttestationProviderId = "simulated" | "phala-tdx" | "aws-nitro";

/** TEE attestation의 데이터 형태 (artifact에 포함된다). */
export type AttestationQuote = {
  provider: AttestationProviderId;
  /** 실행된 스캐너 코드의 해시 (measurement). */
  codeMeasurement: string;
  /** artifact 핵심 필드의 해시에 바인딩된 서명/quote. */
  quote: string;
  /** 거래소 요청의 nonce. */
  nonce: string;
  timestamp: number;
};

/**
 * 스캐너 → 거래소로 가는 유일한 산출물.
 * raw 거래내역(수취인 주소·금액·메모·txid)은 포함되지 않는다.
 */
export type ScreeningArtifact = {
  version: string;
  policyHash: string;
  depositIntentHash: string;
  scanRange: BlockRange;
  /** 실제로 사용된 chain source — request의 것과 같아야 한다. (D9) */
  chainSource: ChainSource;
  /**
   * 어떤 viewing scope가 스캔됐는지에 대한 commitment.
   * commitment = hash(viewing-key-material, salt). salt는 사용자가 보관(D11).
   */
  viewingScopeCommitment: string;
  result: ScreeningResult;
  attestation: AttestationQuote;
};
