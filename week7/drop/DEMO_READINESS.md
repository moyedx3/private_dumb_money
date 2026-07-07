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

- Browser-side DCAP verifier는 production hardening이 필요하다.
  - Quote, measurement, `report_data = sha256(pubkey)` 바인딩 검증 경로는 설계와 코드상 존재하지만, production 수준 검증기 배선과 배포 고정값 관리는 별도 hardening 대상이다.
- Scanner cursor persistence는 demo 수준이다.
  - 데모 흐름은 결제 감지와 dispatch 발행을 보여주지만, 재시작/장애 이후 중복 처리나 누락 방지를 production 수준으로 보장하려면 cursor 저장과 replay 정책이 필요하다.
- `lightwalletd`를 외부 public endpoint에 의존한다.
  - 기능 시연에는 충분하지만 availability와 metadata exposure 측면에서는 production risk다. production에서는 self-hosted 또는 신뢰 가능한 운영 경로가 필요하다.
- Payment UX는 아직 polished production UX가 아니다.
  - 화면은 Zashi 문구와 QR 중심이지만 실제 결제는 Zingo PC에 URI를 붙여넣어 진행했다. 기능상 문제는 아니지만 wallet UX 검증은 더 남아 있다.
- Demo 운영 편의를 위한 설정과 수동 절차가 남아 있다.
  - 재현빌드 measurement 게시, 운영 모니터링, billing stop, 장애 복구 절차는 demo hardening 항목으로 남겨야 한다.

## 발표 때 안전한 표현

> 이 데모는 실제 mainnet ZEC payment와 Phala TDX-hosted indexer를 사용합니다. Content encryption, sealed provisioning, payment memo detection, buyer unlock path는 end-to-end로 동작합니다. 다만 browser-side DCAP verifier, scan cursor persistence, production wallet UX, self-hosted lightwalletd는 demo hardening 단계로 남아 있습니다.

## 한 줄 요약

**시연 가능.** 단, **production-ready**가 아니라 **real-chain / real-TEE prototype demo**라고 말해야 한다.
