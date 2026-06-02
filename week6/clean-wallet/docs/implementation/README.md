# 구현 참조 (implementation/)

모듈별 핵심 위치·역할 요약. 상세는 코드 + [architecture.md](../architecture.md) §3,
신뢰모델은 [demo-architecture-limitations.md](../demo-architecture-limitations.md).

상태: Phase 1–5 완료. 스캐너 Phala TDX 라이브, mainnet UFVK PASS/FAIL 검증. 배포 = RA-TLS passthrough(단계1).

## packages/core (순수 TS)

| 모듈 | 위치 | 역할 |
|---|---|---|
| 타입 | `src/types.ts` | ChainSource·ViewingScope·ScreeningArtifact·BlockRange 등 |
| 해시 | `src/hash.ts` | sha256 헬퍼 — `hashAddress`·`policyHash`·`chainSourceHash`·`viewingScopeCommitment = hash(scope‖salt)` (D11) |
| mock 체인 | `src/mock-chain.ts` | 데모용 mock Zcash 체인 + clean/tainted scope |
| 스캐너 코어 | `src/scanner.ts` | `runScan` — 구간 전체 스캔 → 출금 수취인 도출 → 제재 대조 (PASS/FAIL) |
| attestation(sim) | `src/attestation.ts` | `AttestationProvider` 인터페이스 + `SimulatedAttestation` (ed25519 고정시드) |
| artifact | `src/artifact.ts` | `assembleArtifact` — 결과+메타 서명, raw record 제외 |
| 검증기 | `src/verifier.ts` | artifact 7항목 검증 (서명·measurement·바인딩·chainSource allowlist D9) |
| CLI | `src/cli/{scan,verify}.ts` | `npm run demo:scan / demo:verify` |

## apps/scanner (배포 서비스)

| 모듈 | 위치 | 역할 |
|---|---|---|
| 서버 | `src/server.ts` | HTTP/HTTPS(RA-TLS) `/scan`·`/health`. `SCANNER_TRANSPORT`(ratls 기본/http) 분기, Rust 사이드카 spawn |
| Phala attestation | `src/phala-attestation.ts` | 실 TDX(dstack SDK): `getQuote`·`getMeasurement`(=`compose_hash`)·`getRaTlsCredentials` |
| UFVK 제출 | `tools/submit-ufvk.ts` | UFVK를 RA-TLS 본문으로 전송. RA-TLS 풀검증 + measurement 자동핀 + 채널 fingerprint pin |
| RA-TLS 검증 | `tools/ra-tls-verify.ts` | cert→TDX quote 추출(이중 OCTET STRING unwrap)→Phala verifier(`success`)→pubkey 바인딩→MRTD/RTMR3 핀 |
| measurement 게시 | `tools/expected-measurements.json` | MRTD/RTMR3 핀 게시값 (현재 `trust: tofu-snapshot`; 완성=demo-architecture-limitations.md §4) |
| 지갑 키 도구 | `apps/zcash-scanner-rs` bin `gen-testnet-wallet` | mnemonic/seed → UFVK·주소 derive ([ONBOARDING §0.1](../../ONBOARDING.md)) |

## apps/zcash-scanner-rs (Rust 사이드카)

`src/main.rs` + `src/lib.rs`. 실 lightwalletd gRPC 스캔: sapling/orchard **OVK outgoing** + 해당 tx의 transparent vout.
블록 구간 **완전성**(height·prev_hash) + **PoW 헤더 체인**(Equihash, D12.2) 검증. UFVK `zeroize`.
stdin JSON ↔ stdout JSON (server.ts가 spawn). bins: `gen-testnet-wallet`·`scan-incoming`·`check-taddr`.

## apps/web (Next.js)

`/prover`·`/verifier`(sim 데모), `/results`·`/results/[id]`(DB 조회·재검증), `/api/artifacts`(ingest, `INGEST_API_KEY` 보호).
`lib/dynamo.ts`(DynamoDB + in-memory 폴백)·`lib/artifacts.ts`(정규화·화이트리스트 재검증)·`app/actions.ts`(server actions).
배포: [deploy-web-amplify.md](../deploy-web-amplify.md).
