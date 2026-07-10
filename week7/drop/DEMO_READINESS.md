# 데모 시연 가능성 및 demo-only 구간

## 결론

시연은 가능하다. 이번에 운영한 경로는 실제 mainnet ZEC 결제, 실제 Phala TDX 배포, browser-side encryption/provisioning, Buyer polling까지 포함하므로 "전부 mock"인 데모가 아니다.

다만 production-ready가 아니라 **real-chain / real-TEE prototype demo**로 설명해야 정확하다. 발표에서는 실제로 동작한 부분과 demo-only 타협점을 분리해서 말해야 한다.

## 실제로 동작한 부분

- Creator 브라우저에서 content 암호화
- 암호문 blob을 Phala indexer bucket에 업로드
- `K_drop`, UFVK, deposit address를 sealed provision payload로 전송
- Phala CVM에서 catalog publish
- Buyer가 실제 Zcash payment URI와 memo 생성
- Zingo buyer wallet로 실제 mainnet shielded payment 수행
- A1 scanner가 chain/lightwalletd를 통해 payment memo 감지
- dispatch blob 발행
- Buyer가 dispatch polling 후 unlock

## Demo-only / 숨기면 안 되는 타협점

- Scanner start height는 데모용으로 수동 조정했다.
  - A1 scanner는 `A1_SCAN_START`부터 Zcash blocks를 읽으며 payment memo를 찾는다.
  - 이번 데모에서는 실시간성을 위해 이 값을 결제 직전 block 근처로 당겨서 배포했다.
  - 운영용이면 수동 block 지정 대신 `current_tip - lookback` 자동 시작 또는 UFVK/drop별 persisted cursor가 필요하다. 그렇지 않으면 너무 과거에서 시작해 unlock이 늦어지거나, 너무 최근에서 시작해 결제를 놓칠 수 있다.
- Catalog와 scanner state는 아직 in-memory다.
  - Provisioned drop catalog와 scanner cursor가 CVM 메모리에만 있다.
  - CVM restart, compose update, crash가 발생하면 등록된 drop과 scan 진행 상태가 사라질 수 있다.
  - 실제 사용자 환경에서는 결제 후 unlock 실패, 중복 dispatch, creator re-provision 요구로 이어질 수 있으므로 persistent catalog, cursor, seen-txid/replay guard가 필요하다.
- Phala deployment 변경이 Creator trust input을 바꾼다.
  - Creator는 Phala quote의 `RTMR3`를 expected measurement로 pin하고, 일치할 때만 `K_drop`을 seal한다.
  - Docker image가 같아도 compose env가 바뀌면 compose hash/RTMR3가 바뀐다. 이번에도 `A1_SCAN_START` 변경 후 새 RTMR3를 Creator에 다시 넣어야 했다.
  - 운영용이면 image digest pin, compose hash/RTMR3 publication, measurement rotation 절차가 필요하다. 사람이 수동 복사하는 방식은 misconfiguration risk가 크다.
- Provisioning key continuity는 deployment와 묶여 있다.
  - Enclave provisioning pubkey는 measurement-bound key material에서 파생된다.
  - 배포 변경으로 measurement가 바뀌면 provisioning pubkey도 바뀔 수 있고, 이전 pubkey에 seal된 payload는 새 deployment가 열 수 없다.
  - 운영용이면 redeploy 전 drain/re-provision, key/state migration, 또는 "deployment 변경 시 creator re-provision required" 정책이 명확해야 한다.
- Chain access는 public lightwalletd에 의존한다.
  - A1 scanner는 public lightwalletd endpoint로 compact blocks/full transaction data를 조회한다.
  - 기능 시연에는 충분하지만 availability, rate limit, metadata exposure 측면에서 production risk가 있다.
  - 운영용이면 self-hosted lightwalletd, trusted redundant endpoint, retry/backoff, health monitoring이 필요하다.
- Operations는 아직 runbook/manual 중심이다.
  - `A1_SCAN_START` 산정, Phala compose update, 새 RTMR3 반영, smoke check, billing stop, 장애 복구가 수동 절차다.
  - 운영용이면 deploy script/CI, smoke evidence 기록, monitoring, rollback, cleanup automation이 필요하다.

## 해결된 이전 demo-only 항목

- Browser-side DCAP verifier는 `@phala/dcap-qvl` 기반 production verifier로 교체했다.
  - 더 이상 `/tmp/drop-demo-verifier.mjs`가 기본 경로가 아니다.
  - Creator/live smoke는 실제 Phala `/attest` quote에 대해 quote verification, RTMR3 measurement match, `report_data = sha256(pubkey)` 바인딩 확인을 통과했다.
- Payment UX는 기능상 blocker가 아니다.
  - Buyer는 ZIP-321 payment URI와 memo를 만들고 QR/copy flow를 제공한다.
  - 실제 촬영/시연에서는 운영 편의를 위해 Zingo PC에 URI를 붙여넣어 결제했지만, 이는 프로토콜 우회가 아니라 wallet 입력 방식의 차이다.
  - Zashi/다른 wallet QR scan 또는 copy/paste flow로도 같은 URI/memo를 사용할 수 있다. 다만 각 wallet별 UX polish와 호환성 확인은 제품화 단계에서 계속 필요하다.

## 실제 사용자에게 배포하면 생길 수 있는 문제

- 결제 누락: scanner cursor가 사라지거나 start height를 너무 최신으로 잡으면 이미 발생한 결제를 놓칠 수 있다.
- unlock 지연: start height가 너무 과거이거나 lightwalletd가 느리면 buyer가 결제 후 오래 기다릴 수 있다.
- 중복 dispatch: 장애 후 같은 block range를 replay하면서 seen-txid 상태가 없으면 같은 결제에 dispatch를 여러 번 낼 수 있다.
- drop 소실: catalog가 in-memory라 재시작 후 buyer가 catalog를 못 보거나 creator가 다시 provision해야 할 수 있다.
- measurement mismatch: Phala compose/image 변경 후 Creator가 예전 RTMR3를 쓰면 provisioning이 막힌다.
- trust misconfiguration: 사람이 expected measurement를 잘못 복사하면 올바른 배포를 거부하거나, 더 나쁘게는 의도치 않은 배포를 신뢰할 수 있다.
- 외부 dependency 장애: public lightwalletd나 PCCS/collateral endpoint 장애가 곧 결제 감지/attestation 실패로 이어질 수 있다.
- wallet UX 실패: wallet이 memo를 기대대로 싣지 않거나 사용자가 URI를 잘못 붙여넣으면 A1이 결제를 인식하지 못한다.

## 발표 때 안전한 표현

> 이 데모는 실제 mainnet ZEC payment와 Phala TDX-hosted indexer를 사용합니다. Content encryption, browser-side DCAP quote verification, sealed provisioning, payment memo detection, buyer unlock path는 end-to-end 경로로 동작합니다. 다만 scan start/cursor persistence, catalog/state persistence, measurement publishing/rotation, production wallet UX, self-hosted lightwalletd, deployment/operations automation은 production hardening 단계로 남아 있습니다.

## 한 줄 요약

**시연 가능.** 단, **production-ready**가 아니라 **real-chain / real-TEE prototype demo**라고 말해야 한다.
