# 의사결정 기록 (Decision Log)

프로젝트의 방향을 바꾼 주요 결정과 *왜* 그렇게 했는지를 기록한다.
새 결정이 생기면 아래에 추가한다.

형식: **맥락 → 결정 → 이유 → 트레이드오프**

---

## D1. mock-JSON ZK 방식을 버리고 attested scanner로 전환

- **맥락**: 최초 아이디어(초기 mock-JSON ZK 안)는 사용자가 outgoing record 목록을 직접 JSON으로
  제출하고, 그 목록이 제재 집합과 겹치지 않음을 ZK로 증명하는 것이었다.
- **결정**: 이 방식을 폐기하고, viewing key 기반 attested scanner로 전환한다.
- **이유**: **completeness 문제.** ZK 증명은 *witness에 넣은 데이터*에 대해서만 참을
  증명한다. 사용자가 제재 수취인이 든 record를 witness에서 빼면 증명은 여전히 통과한다.
  `ledgerCommitment`도 "목록이 안 바뀌었다"만 보장할 뿐 "목록이 완전하다"는 보장하지 못한다.
- **트레이드오프**: 스캐너라는 신뢰 컴포넌트가 새로 필요해진다.

## D2. ZK 비-교집합 회로를 MVP에서 제외

- **맥락**: 초기 설계는 ZK 회로를 "optional 프라이버시 레이어"로 남겨뒀다.
- **결정**: MVP에서 ZK 회로(Circom/snarkjs)를 구현하지 않는다. `non_interaction.circom`
  아이디어는 future work로만 문서에 남긴다.
- **이유**: ZK와 TEE가 푸는 문제가 다르다.
  - ZK는 **수취인 프라이버시**(witness 비공개)만 푼다. completeness는 못 푼다.
  - TEE는 **completeness**를 푼다. *그리고* enclave 안에서 대조 후 PASS/FAIL만
    내보내므로 **프라이버시도 함께** 푼다.
  - 따라서 TEE가 있으면 ZK는 사실상 중복이다. 비-교집합 산술은 enclave 안에서 그냥
    하면 된다. ZK 회로 + trusted setup의 복잡도를 더할 이유가 약하다.
- **트레이드오프**: "trustless(순수 암호학)"라는 셀링 포인트를 일부 잃는다.
  대신 신뢰 모델을 문서에 명시해 정직성을 유지한다. (pure-ZK는 completeness를
  못 풀므로 애초에 단독 선택지가 아니다.)

## D3. 로컬 우선 개발 + AttestationProvider 추상화

- **맥락**: 스캐너를 실 TEE에 올릴지, 어떻게 빠르게 만들지 논의.
- **결정**: 스캐너 바이너리를 **로컬에서 먼저** 완성·테스트한다. attestation은
  `AttestationProvider` 인터페이스로 추상화하고, MVP는 `SimulatedAttestation`을 쓴다.
- **이유**: 바이너리는 TEE 안/밖에서 코드가 동일하다. TEE는 배포 래퍼일 뿐이다.
  로컬에서 먼저 만들면 cloud 계정·키 없이 막힘없이 진행할 수 있고, enclave 안
  디버깅(셸 없음, 로그 제한)의 고통을 피한다.
- **트레이드오프**: 없음에 가깝다. 실 TEE 배포는 분리된 마지막 단계가 된다.

## D4. 실 TEE 후보는 Phala/dstack (vs AWS Nitro)

- **맥락**: 실 TEE를 쓴다면 어디서.
- **결정**: 실 TEE 배포가 필요해지면(Phase 4) **Phala dstack / Phala Cloud**를
  1순위 후보로 한다.
- **이유**: 우리 스캐너는 외부 네트워크(lightwalletd 등)를 호출해야 한다.
  - Phala dstack은 Intel TDX 기반 — VM 통째로 암호화라 컨테이너가 일반 네트워킹·
    파일시스템을 그대로 쓴다. "docker-compose 배포 → attestation" 모델.
  - AWS Nitro Enclaves는 검증되어 있지만 enclave에 네트워크가 없어, 부모 EC2가
    vsock으로 트래픽을 프록시해야 한다 — 외부 호출이 필요한 우리 케이스엔 마찰이 크다.
- **트레이드오프**: Phala는 AWS보다 신뢰 생태계가 작고 새롭다. 최종 결정은 Phase 4에서.

## D5. MVP는 mock Zcash 체인

- **맥락**: 실제 Zcash 스캔을 MVP에 넣을지.
- **결정**: MVP는 mock 체인·mock viewing scope를 쓴다. 실제 스캔은 Phase 4.
- **이유**: 실제 출금 수취인 복원은 **FVK의 OVK + full transaction**이 필요해
  일반 light wallet 스캔보다 무겁다([architecture.md](./architecture.md) 9절).
  이걸 MVP에 넣으면 핵심 흐름(스캔→artifact→검증) 데모가 늦어진다.
- **트레이드오프**: MVP는 "실제 Zcash에서 동작"을 보여주지 못한다. 대신 mock 체인을
  실제 스캐너와 같은 인터페이스로 만들어, 나중에 교체가 쉽도록 한다.

## D6. 스캔은 TEE 안에서 수행 (외부 스캔 + 데이터 주입 방식 배제)

- **맥락**: 스캔(블록 페치·trial-decryption·record 도출)을 TEE 안에서 하는데, TEE 내부
  연산 오버헤드가 크다면 스캔을 외부에서 하고 결과 데이터만 TEE에 넣는 게 낫지 않냐는 검토.
- **결정**: 스캔은 TEE 안에서 수행한다. "외부 스캔 → 결과만 TEE 주입"은 채택하지 않는다.
- **이유**:
  - 외부 스캔 결과를 받으면 TEE는 "받은 데이터를 정확히 대조했다"만 보증할 수 있고
    "그 데이터가 완전하다"는 보증하지 못한다 → D1의 completeness 문제 재발.
  - 외부 스캐너가 제재 수취인이 든 record를 누락하면 TEE는 모른 채 PASS에 서명한다.
  - 대조는 trivial하고, "빠짐없이 스캔했다"가 어렵고 핵심이다. 대조만 TEE에 두는 것은
    의미가 없다.
  - 외부 스캐너를 신뢰 가능하게 만들려면 그것도 attestation = TEE가 필요하다 (순환).
  - 오버헤드 전제도 약하다: 무거운 TEE 오버헤드는 옛 SGX(process enclave, 작은 EPC)
    이야기다. D4에서 고른 Phala dstack = Intel TDX는 VM 단위라 CPU 오버헤드가 낮고
    메모리·네트워크가 정상 동작한다.
- **허용되는 분리**: completeness가 의존하지 않는 단순 운반만 외부에 둘 수 있다.
  - 외부 OK: 블록 *페치/운반* (프록시가 raw 블록을 TEE에 전달).
  - TEE 내부 필수: 블록 진위 검증, 구간 빠짐없음 확인, trial-decryption/record 도출, 대조.
- **트레이드오프**: 스캔 부하가 TEE 안에 남는다. 성능 문제 시 신뢰 경계를 옮기지 말고
  TEE 인스턴스 확장 또는 블록 구간 병렬 스캔으로 해결한다.

## D7. attestation 인터페이스를 async로, PhalaAttestation을 실제 구현

- **맥락**: 실 TEE 데모를 위해 `PhalaAttestation`을 실제 구현해야 했다. dstack
  SDK(`@phala/dstack-sdk`)는 비동기 API다.
- **결정**:
  - `AttestationProvider` 인터페이스를 async로 전환(`Promise` 반환). `SimulatedAttestation`·
    `assembleArtifact`·`verifyArtifact`와 호출부(CLI·웹·스캐너·테스트)에 모두 await 반영.
  - `PhalaAttestation`을 골격에서 실제 구현으로. 단 core가 아니라 `apps/scanner`에 둔다 —
    `@phala/dstack-sdk` 의존성을 core(외부 런타임 의존성 0)에 들이지 않기 위해.
  - 스캐너는 `ATTESTATION_MODE` 환경변수로 provider를 고른다 (로컬=Simulated, dstack=Phala).
- **이유**: dstack SDK가 async라 sync 인터페이스로는 실 TEE provider를 구현할 수 없다.
  MVP를 sync로 시작한 건 단순성 때문이었고(D3 시기), 실 TEE 단계에서 async 전환은 불가피.
- **트레이드오프**: 기존 코드(artifact·verifier·테스트)에 await가 퍼졌다. 단 코어 스캔
  로직 자체는 그대로다. TDX quote의 암호학적 검증은 `PhalaAttestation.verify()`가 풀로
  하지 않고 Phala verifier에 위임한다 ([deploy-phala.md](./deploy-phala.md) §6).

## D8. 실 Zcash 스캔은 Rust 사이드카로

- **맥락**: 실 TEE 데모를 완성하려면 mock 체인 대신 실제 Zcash 데이터가 필요. 후보:
  - (a) **WebZjs**(브라우저용 WASM): Node 미지원 + OVK 출금 first-class API 부재 → 부적합.
  - (b) **JS로 Zcash crypto 재구현**: 사실상 `zcash_client_backend` 포팅, 비현실적.
  - (c) **Rust `zcash_client_backend` 사이드카**: 채택.
- **결정**: `apps/zcash-scanner-rs/` Rust 바이너리를 만들어 Node 스캐너가 stdin/stdout
  JSON IPC로 호출. 스캐너 컨테이너에 **멀티스테이지 Docker 빌드**로 동봉.
- **이유**:
  - `zcash_client_backend::decrypt_transaction`이 UFVK로 받는·보낸 출력을 한 번에
    복호화(OVK 포함, Sapling+Orchard). 우리에게 필요한 정확히 그 함수가 안정 API로 존재.
  - lightwalletd gRPC + 노트 암호 복호화는 Rust 생태계가 검증된 유일한 경로.
  - 사이드카 = Node 스캐너의 다른 부분(IPC·HTTP·attestation)은 그대로 유지.
- **D6와의 정합**: Rust 사이드카는 *컨테이너 안*에서 실행(같은 TEE 안). scan completeness는
  여전히 TEE 안에서 강제됨 — 외부 스캐너가 아니다.
- **트레이드오프**:
  - 새 언어 + 새 빌드 파이프라인 (멀티스테이지 Docker).
  - 실 환경 검증은 사용자 환경에서 (testnet UFVK 통합 시).

## D9. chainSource를 요청 파라미터 + artifact 바인딩 (검증자가 정책으로 enforce)

- **맥락**: 스캐너는 `lightwalletd`에서 블록을 받는다. 그런데 기존 설계에서는 *어느*
  lightwalletd인지가 attestation에 묶이지 않았다. 거래소(검증자)는 "맞는 코드·맞는
  measurement"는 확인할 수 있어도 "맞는 데이터 출처"를 확인할 방법이 없었다.
- **결정**: `chainSource: { kind: "mock" | "lightwalletd"; url; network }`를
  - `ScreeningRequest`에 필수 필드로 (거래소가 어디서 받는지 지정),
  - `ScreeningPolicy.approvedChainSources` allowlist에 (거래소가 허용한 출처 목록),
  - `ScreeningArtifact`에 (실제 사용한 출처) — **binding payload에 hash로 묶음**,
  - `verifier`의 7번째 체크에 (artifact.chainSource ∈ allowlist + request와 일치)로 둔다.
- **이유**:
  - TEE attestation은 *코드*는 묶지만 *런타임 입력*은 묶지 못 한다 — env에 어떤 URL을
    넣었는지 운영자만 안다.
  - chainSource를 요청·artifact에 묶어 binding payload에 hash로 포함하면, attestation이
    함께 서명해 사후 변조 불가. 거래소는 *자기가 신뢰하는 lightwalletd*만 허용하도록
    정책으로 enforce할 수 있다.
- **한계 (정직하게)**:
  - URL을 안다고 데이터가 검증되는 건 아니다 — lightwalletd 운영자가 거짓 블록을 줘도
    못 막는다. 완전한 해결은 **enclave 내부의 PoW 헤더 체인 검증**(또는 신뢰 체크포인트).
  - D9는 그 사이의 절충 — *불투명한 신뢰* → *정책으로 enforce 가능한 transparent 신뢰*.
- **트레이드오프**: 정책·요청·artifact·검증기·테스트 fixture에 새 필드. 마이그레이션 비용
  한 번. 그 대신 신뢰 모델이 한 단계 단단해진다.

## D10. UFVK를 env가 아니라 RA-TLS 채널로 본문 전달

- **맥락**: Phase 4 초안에서는 UFVK를 `ZCASH_UFVK` 환경변수로 받았다. 이건 **Phala
  운영자가 평문에 접근한다는 뜻**이다 (대시보드 env 입력 → 컨테이너 spawn 사이를 그들의
  인프라가 통과). 프로젝트의 핵심 narrative("그 누구도 viewing key를 보지 않는다")와
  정면 모순.
- **결정**: UFVK는 **HTTP 본문**으로만 받는다 (env에서 완전 제거). `ATTESTATION_MODE=phala`
  모드에서는 dstack `getTlsKey({usageRaTls: true})`로 enclave 안에서 생성된 TLS keypair로
  HTTPS 종단 — cert에 TDX quote가 박혀 있다 (RA-TLS). (배포 후속 D13.1: **현재 배포는 gateway
  passthrough + `SCANNER_TRANSPORT=ratls`** 라 RA-TLS end-to-end 가동. gateway-termination +
  `=http`는 passthrough 못 쓰는 환경용 대안.)
  - 사용자 측 도구 `apps/scanner/tools/submit-ufvk.ts`가 enclave로 직접 POST.
  - `/scan` body: `{ mode: "real", ufvk, salt, chainSource, scanRange }`.
  - UFVK는 요청 처리 스코프 안에서만 존재하다 GC 대상이 된다 (per-request 격리).
- **이유**: enclave는 *외부에서 들어오는 비밀*을 자기 공개키로 받아야 운영자가 못 본다.
  RA-TLS는 그 공개키 전달을 attestation에 묶는 표준 패턴.
- **한계 (정직하게)** *(D12.1·D13.x 에서 해소 — 아래 갱신)*:
  - cert 안의 TDX quote 풀 검증은 **D12.1 에서 구현됨** (`ra-tls-verify.ts`, submit-ufvk 기본
    동작; `--no-verify` 로 끔, `--insecure` 는 deprecated alias).
  - **현재 배포는 gateway passthrough** 라 이 풀검증 + measurement 자동 핀이 end-to-end 가동된다
    (단 measurement 값은 TOFU — 완성은 demo-architecture-limitations.md §4). gateway-termination
    대안 배포에서만 `--no-verify` 로 내려간다 — limitations.md §3.3.
  - 핵심 보안 향상은 여전히 **"env에서 본문으로"** (운영자 평문 노출 제거).
- **트레이드오프**: 새 클라이언트 도구 운영 필요. server.ts가 HTTPS+body parser로 커짐.
  로컬 sim 모드는 HTTP 유지(개발 편의).

## D11. viewingScopeCommitment에 사용자 제공 salt 추가

- **맥락**: 기존 `viewingScopeCommitment = hash(scope)`는 hiding이 약하다. 거래소가 어떤
  경로로든 UFVK 리스트를 갖고 있으면 commitment를 사전 매칭해 *어떤 지갑*인지 식별 가능.
- **결정**: commitment = `hash(scope || salt)`. salt는 사용자가 random 32바이트로 생성해
  요청 본문(`/scan` body의 `salt`)에 함께 넣는다. **salt는 artifact에 들어가지 않는다** —
  사용자가 보관(필요 시 나중에 본인이 자기 commitment임을 증명할 때 사용).
- **이유**: hiding 보장. 운영자·거래소는 commitment만 보고 어떤 UFVK인지 추론 불가.
- **트레이드오프**: 사용자가 salt 보관 책임. 분실 시 commitment의 "내 것" 증명 능력만 잃음
  (다른 영향 없음). mock 데모 경로는 고정 salt 사용 — hiding이 데모 목적 아님.

## D12. RA-TLS 클라이언트 측 quote 풀검증 + PoW 헤더 체인 검증 + transparent-only

직전 세션(2026-05-26)에 D10의 후속 항목 세 가지 + 확정 단위 테스트들이 합류했다. 세
항목 모두 신뢰 모델의 *남은 큰 갭들* 을 닫는 작업이라 한 결정으로 묶는다.

### D12.1 RA-TLS 클라이언트 측 quote 풀검증 (B1)

- **맥락**: D10은 UFVK 를 env 가 아니라 RA-TLS 본문으로 옮겼지만, *클라이언트가 cert 안의
  TDX quote 를 풀검증하지 않으면* "내가 진짜 그 enclave 와 말하고 있다"는 보장이 약했다.
  데모용으로 `--insecure` 옵션을 켜둔 상태였다.
- **결정**: `apps/scanner/tools/ra-tls-verify.ts` 모듈 신설.
  1) cert DER 에서 dstack OID `1.3.6.1.4.1.62397.1.1` 아래 박혀 있는 TDX quote 추출.
  2) Phala 공개 verifier API (`https://cloud-api.phala.com/api/v1/attestations/verify`) 로
     암호학적 유효성 위임 (서명·인증서 체인·TCB).
  3) `report_data == sha512("ratls-cert:" || SPKI_DER)` 검증 — channel-to-enclave
     anti-substitution.
  4) (선택) MRTD / RTMR3 매치.
  5) 검증 통과한 cert 를 pin 한 채로 본문 POST.
- **이유**: TDX quote 의 *진위* 검증 + cert pubkey 가 *그 quote 에 묶여 있는지* 확인해야
  "운영자가 enclave 인 척하고 본문을 가로채는" 시나리오를 막는다. dstack 공식 verifier 에
  암호학을 위임하면 PCK chain·QE identity 같은 복잡한 의존성 없이 핵심 보장이 성립한다.
- **트레이드오프**:
  - Phala verifier 가 살아 있어야 검증된다. **현재 passthrough 배포라 이 풀검증이 실제 가동**
    (+ measurement 자동 핀, TOFU). gateway-termination 대안 배포에선 LE cert 라 `--no-verify`
    로 내려가 가동 안 됨 (limitations.md §3.3).
  - 후속: 사내 verifier (Phala 외) 또는 local `@phala/dcap-qvl` 옵션으로 verifier 의존을 더 줄일 수 있음.

### D12.2 PoW 헤더 체인 검증 in Rust 사이드카 (C1)

- **맥락**: `architecture.md` §8 의 마지막 큰 갭 — completeness 검증이 height·prev_hash 만
  보던 시점에선 lightwalletd 가 *프로토콜에 맞춰 보이는 위조 블록* 을 통째로 만들어 주면 못
  잡았다.
- **결정**: `apps/zcash-scanner-rs/src/lib.rs` 에 `PowVerifier` 추가.
  - 각 블록의 raw header 에 대해 **Equihash(200, 9)** 솔루션 검증 + `sha256d(header) < target(bits)` 검증.
  - `CompletenessChecker` 가 옵션으로 PoW 검증기를 받아 add_block 단계에서 적용.
  - `ScanRequest.verify_pow = true` 일 때 lightwalletd 가 `CompactBlock.header` 를 함께
    보내야 한다 — 안 보내면 throw (운영 시 서버 설정 명시).
  - **신뢰 체크포인트**: `ScanRequest.start_anchor_hash_hex` 가 있으면 첫 블록의 prev_hash
    가 그 값과 일치해야 한다. 시작점 위조도 막는다.
- **이유**: enclave 안에서 PoW 까지 검증하면 lightwalletd 가 어떤 위조도 만들 수 없다 (PoW
  난도를 깨야 함). chainSource binding(D9) 위에 *진짜 PoW 검증* 이 얹혀 신뢰 모델이 한 단계
  더 단단해진다.
- **트레이드오프**:
  - PoW 검증은 CPU 비용이 있다 (블록당 수 ms). 50~100 블록 기준 큰 영향 없음.
  - lightwalletd 가 헤더를 보내도록 설정해야 한다 — 일반 mainnet 운영자 측 설정 의존.
  - Equihash 파라미터가 mainnet/testnet 외 networks 에선 다를 수 있음 (regtest 등).

### D12.3 transparent-only 송금 감지 (C2)

- **맥락**: 기존 처리는 *shielded outgoing 이 있는* tx 의 transparent vout 만 잡았다.
  사용자가 t-addr → t-addr 로만 보낸 거래는 누락 (OFAC SDN 의 ZEC 주소 다수가 t-addr 이므로
  실제 위험).
- **결정**: `OurTransparentTracker` 추가.
  - UFVK 의 transparent 컴포넌트에서 외/내부 IVK 의 index 0..20 t-addr 을 derive.
  - audit window 안에서 우리 t-addr 로 가는 vout 을 UTXO 로 추적.
  - 다음 tx 의 vin 이 우리 UTXO 를 spend 하면 그 tx 는 우리 outgoing.
  - vout 중 비-우리 t-addr 행을 수취인으로 기록.
- **한계 (정직하게)**:
  - audit window *이전* 에 받은 UTXO 를 window 안에서 쓰는 경우는 못 잡는다. 후속:
    GetAddressUtxos API 로 시작 시점 UTXO 를 미리 불러오는 보강.
  - derive 인덱스 상한(20) 을 넘는 deep wallet 은 누락 — 필요하면 ScanRequest 로 조절.
- **이유**: 좁은 스크리닝 신호의 정확성이 demo 수준에서도 의미 있게 올라간다.
- **트레이드오프**: tracker 상태 메모리 비용(블록당 수 KB). 검증 모델 변화 없음.

### D12 요약: 통합 단위 테스트 베이스라인

- `apps/scanner/src/phala-attestation.test.ts` (9 tests) — A1.
- `apps/scanner/tools/ra-tls-verify.test.ts` (10 tests) — B1.
- `apps/zcash-scanner-rs/tests/completeness.rs` (11) — A2 + C1 anchor/pow option.
- `apps/zcash-scanner-rs/tests/encoding.rs` (9) — A3.
- `apps/zcash-scanner-rs/tests/pow.rs` (9) — C1 핵심 (real mainnet block #1,000,000 헤더).
- `apps/zcash-scanner-rs/tests/transparent_tracker.rs` (7) — C2.
- 합산: TS 36 + Rust 36 = 72 tests · 모두 그린.

## D13. Phala 배포 라운드 — gateway 전송 모드 + 웹 결과 조회

scanner 를 Phala Cloud(dstack, Intel TDX)에 실제 배포하고 실 mainnet UFVK 로 PASS/FAIL
end-to-end 검증한 라운드. 배포 중 드러난 전송 모델 결정과 결과 공유용 웹/DB 계층을 묶는다.

### D13.1 SCANNER_TRANSPORT — gateway TLS 종단 대응

- **맥락**: Phala 기본 **dstack Gateway 가 공개 TLS 를 종단**(Let's Encrypt cert)하고 컨테이너로
  HTTP 를 포워딩한다. scanner 는 phala 모드에서 자체 RA-TLS HTTPS 를 8080 에 서빙해 프로토콜이
  어긋나 **빈 응답(curl exit 52)** 이 났다.
- **결정**: env `SCANNER_TRANSPORT` 도입 (`server.ts`). 기본 `ratls`(자체 RA-TLS HTTPS),
  `http` 면 평문 HTTP 서빙(TLS 는 gateway 가 담당). 배포는 `http`.
  - attestation quote 는 두 모드 모두 dstack `getQuote` 로 artifact **본문**에 들어간다 — 전송
    모드와 무관.
- **신뢰 모델 영향 (정직)**: gateway-termination + client `--no-verify` 에선 client 가 scanner
  enclave 의 attestation 을 **검증하지 않고 Phala gateway TEE(ZT-TLS)를 신뢰**한다. 데이터는
  TEE 경계 안(gateway TEE→WireGuard→scanner)에 머물러 사람 운영자 평문 노출은 없으나, D10 의
  "client 가 직접 검증" 보장은 약화된다. **end-to-end client 검증**은 gateway **TLS
  passthrough**(URL 포트 뒤 `s`) + `SCANNER_TRANSPORT=ratls` + `--no-verify` 제거로 회복
  (limitations.md §3.3).
- **트레이드오프**: 현재 데모는 termination(편의·빠름). 강한 보장은 passthrough 전환 시.

### D13.2 웹 결과 조회 + DynamoDB (Phase 5)

- **맥락**: 스캔 결과(artifact)를 팀·거래소가 조회·재검증할 표면이 필요. 단 UFVK 는 절대
  웹/DB 에 닿으면 안 된다.
- **결정**: artifact(비밀 아님)만 저장·조회하는 계층을 분리.
  - CLI `submit-ufvk --save <url>` → `apps/web/app/api/artifacts` → DynamoDB
    (`lib/dynamo.ts`, 미설정 시 in-memory). 웹 `/results` 가 조회 + 바인딩 재검증
    (`lib/artifacts.ts` `verifyStored`).
  - `pickArtifact` 화이트리스트로 `_debug`·비밀 필드 저장을 구조적으로 차단. phala-tdx
    attestation 의 quote 검증은 Phala verifier 에 위임(웹은 바인딩만 검증, ⧉ 표시).
  - 배포: AWS Amplify(SSR) + DynamoDB — `docs/deploy-web-amplify.md`.
- **이유**: TEE 와 상호작용하는 건 CLI 뿐 — 웹은 비밀 아닌 산출물만 다뤄 프라이버시 경계 유지.
- **트레이드오프**: 별도 AWS 인프라. 웹 재검증은 바인딩까지(phala-tdx attestation 은 위임).
