# 용어집

프로젝트 문서에 나오는 용어를 쉽게 설명한다.

## Zcash 기본

- **Zcash (ZEC)** — 프라이버시 기능이 있는 암호화폐.
- **transparent address (t-addr)** — 비트코인처럼 거래가 공개되는 주소. `t`로 시작.
- **shielded address (z-addr)** — 거래 금액·수취인·메모가 암호화되는 주소. `zs`/`u` 등으로 시작.
- **shielded pool (차폐 풀)** — shielded 거래가 모여 있는 영역. 외부에서 내부를 볼 수 없다.
- **off-ramp** — 암호화폐를 법정화폐로 바꾸거나 거래소로 내보내는 출구. 여기서는 거래소 입금.

## Viewing key 관련

- **viewing key (뷰잉 키)** — 자금을 *쓸* 수는 없고 거래를 *볼* 수만 있는 읽기 전용 키.
- **IVK (Incoming Viewing Key)** — *받은* 거래만 볼 수 있는 viewing key.
- **OVK (Outgoing Viewing Key)** — *보낸* 거래의 수취인·금액을 복원할 수 있는 키.
- **FVK (Full Viewing Key)** — IVK + OVK + nullifier 키를 모두 포함한 viewing key.
  이 프로젝트는 **출금 수취인**을 봐야 하므로 FVK가 필요하다.
- **UFVK (Unified Full Viewing Key)** — transparent/Sapling/Orchard FVK를 하나로 묶은 것.
- **viewing scope** — 사용자가 검사 대상으로 제출하는 viewing key의 범위.
- **viewing scope commitment** — viewing key를 노출하지 않고 "이 scope를 검사했다"는
  지문. 사용자 제공 salt와 함께 hash해 hiding 보장 (D11).

## 체인 데이터

- **lightwalletd** — 라이트 지갑에 압축된 블록 데이터를 제공하는 Zcash 서버.
- **compact block** — light wallet용으로 축약된 블록. *받은* 노트는 찾을 수 있지만
  출금 암호문(`out_ciphertext`)이 빠져 있어, 출금 수취인 복원에는 부족.
- **full transaction** — 모든 필드를 가진 원본 거래. 출금 수취인 복원에 필요.
- **note (노트)** — shielded 자금의 단위. UTXO의 shielded 버전.
- **nullifier** — 노트가 사용(소비)됐음을 나타내는 값. 자신의 spend를 찾는 데 쓰인다.
- **block height** — 블록의 순번. 스캔 구간을 `[start, end]` 높이로 지정.
- **prev_hash chain** — 각 블록 헤더가 이전 블록 hash를 참조해 형성하는 체인 — 누락·
  치환 탐지에 쓰인다.
- **chainSource** (D9) — 블록을 어디서 받는지의 종류 (mock 또는 lightwalletd URL).
  요청·정책·artifact에 묶여 검증된다.

## 컴플라이언스

- **sanctioned address (제재 주소)** — 제재 대상으로 지정된 암호화폐 주소.
- **OFAC SDN List** — 미국 재무부 OFAC의 제재 대상 목록. Zcash 주소도 일부 포함 — 다수가
  transparent (t-addr).
- **screening** — 거래/주소가 제재 목록 등에 걸리는지 확인하는 절차.
- **KYC / AML** — 고객확인 / 자금세탁방지. 거래소의 법적 의무.

## TEE / Attestation

- **TEE (Trusted Execution Environment)** — 운영자조차 내부 메모리를 볼 수 없는,
  하드웨어로 격리된 실행 영역.
- **enclave** — TEE 안에서 격리되어 실행되는 프로그램 영역.
- **attestation** — "이 코드가 진짜 TEE 안에서 실행됐다"를 하드웨어가 증명하는 것.
- **remote attestation** — 그 증명을 원격의 제3자(거래소)가 검증하는 것.
- **RA-TLS (Remote Attested TLS)** (D10) — TLS cert에 TDX(또는 SGX) attestation quote를
  박아, 클라이언트가 cert만으로 enclave 신원을 검증할 수 있게 하는 패턴. cert 안의
  quote를 풀 검증하면 그 TLS 채널 자체가 enclave에 종단된 보장.
- **code measurement** — 실행된 코드(바이너리/이미지)의 해시. attestation에 포함되어,
  "약속된 코드"인지 대조하는 기준.
- **quote** — attestation의 서명된 산출물.
- **Intel TDX** — VM 단위로 메모리를 암호화하는 인텔의 confidential computing 기술.
- **AWS Nitro Enclaves** — AWS EC2에서 격리된 enclave를 만드는 기능.
- **Phala / dstack** — 컨테이너를 TEE(주로 Intel TDX)에 쉽게 배포하게 해주는 플랫폼.
- **DCAP** — Intel의 데이터센터용 attestation 검증 인프라. TDX quote 검증의 표준 경로.

## 보안 / 운영

- **completeness 문제** — "검사한 record 집합이 빠짐없이 전부인가"의 문제. 이 프로젝트가
  attested scanner로 푸는 핵심 문제.
- **binding payload** — artifact 핵심 필드의 정규 직렬화 문자열. attestation이 서명하는
  대상 — 한 필드라도 바뀌면 서명이 깨진다.
- **salt** (D11) — viewing scope commitment의 hiding을 위한 random 값. 사용자가 생성하고
  본인이 보관 (artifact엔 안 들어감).
- **zeroize** — Rust crate. 비밀이 든 메모리를 drop 시 0으로 채워 페이지 누수를 최소화.
- **per-request 격리** — UFVK 같은 비밀이 요청 처리 스코프 안에만 존재하다 GC/zeroize되어
  요청 간 누수를 막는 패턴.

## 이 프로젝트 고유 용어

- **attested scanner** — viewing key로 체인을 스캔하는, attestation으로 보증된 스캐너.
  이 프로젝트의 핵심 컴포넌트.
- **screening artifact** — 스캐너가 거래소에 내보내는 산출물. PASS/FAIL 결과 +
  메타데이터 + attestation. raw 거래내역·salt는 안 들어 있다.
- **screening policy** — 거래소가 정의하는 검사 기준(블록 구간, 제재 집합, 기대
  measurement, 허용 chainSource 등).
- **deposit intent** — 특정 입금 요청의 정보. artifact를 이 입금에 묶어 재사용을 막는다.
- **non-interaction / non-intersection** — 출금 수취인 집합과 제재 주소 집합이 겹치지 않음.

## ZK (MVP 제외, 참고용)

- **ZK proof** — 비밀을 공개하지 않고 어떤 명제가 참임을 증명하는 기법.
- **Circom / snarkjs** — ZK 회로를 작성·증명하는 도구.
- **Poseidon** — ZK 회로에 적합한 해시 함수.
- ZK를 MVP에서 뺀 이유는 [decisions.md](./decisions.md) D2.
