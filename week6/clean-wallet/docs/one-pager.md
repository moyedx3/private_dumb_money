# One-Pager — Zcash Private Off-Ramp Screening

## 한 문장

> Zcash 사용자가 **전체 거래내역을 공개하지 않고도**, 자신의 지갑이 제재 대상(sanctioned)
> 주소와 직접 거래한 적이 없음을 거래소에 검증 가능하게 증명하는 시스템.

## 문제

거래소는 shielded(차폐) Zcash에서 출금된 ZEC의 출처를 들여다볼 수 없다.
그래서 shielded 출처 입금을 **고위험으로 보고 거부하거나 보류**한다.
사용자 입장에서는 프라이버시를 지키려고 shielded를 썼는데, 그것이 곧 입금 거부 사유가 된다.

## 솔루션

거래소에 viewing key나 거래내역을 통째로 넘기는 대신:

1. 사용자가 **read-only viewing key**를 **attested scanner**(신뢰 가능하다고 증명된 스캐너)에 제공한다.
2. 스캐너는 지정된 블록 구간 **전체**를 스캔해, 그 viewing key로 볼 수 있는 **모든** 출금 수취인을 도출한다.
3. 스캐너는 수취인 집합을 제재 주소 집합과 대조한다.
4. 거래소에는 **raw 거래내역이 아니라 PASS/FAIL 결과(screening artifact)**만 전달된다.

스캐너는 **TEE(신뢰 실행 환경)** 안에서 실행되어 운영자가 viewing key·거래내역 평문을 볼 수 없도록
설계됐고, 거래소는 attestation으로 "약속된 코드가 실행됐다"를 검증한다.
(배포 방식별 신뢰 모델 차이 — gateway 종단 vs RA-TLS passthrough — 는 `limitations.md` §3.3.)

## 동작 흐름

```txt
거래소  → 스크리닝 요청 (정책 + 제재목록 + 입금정보)
사용자  → 스캐너 attestation 확인 → viewing key 전달
스캐너  → 블록 구간 전체 스캔 → 수취인 도출 → 제재목록 대조
스캐너  → screening artifact (PASS/FAIL + attestation) 발행
거래소  → artifact 검증 → 정책 적용
```

## 증명하는 것 / 증명하지 않는 것

| 증명하는 것 | 증명하지 않는 것 |
|---|---|
| 제출된 viewing scope 안에서 | 사용자가 가진 **모든** 지갑이 깨끗하다 |
| 지정 블록 구간 전체가 스캔되었고 | ZEC의 **전체 upstream 출처**가 깨끗하다 |
| 출금 수취인이 제재목록과 겹치지 않음 | 완전한 OFAC/AML 컴플라이언스 |

→ 이것은 **좁은 스크리닝 신호**이지, 완전한 컴플라이언스 증명이 아니다. (정직함이 설계 원칙)

## 현재 상태

- **배포됨**: attested scanner 가 Phala TDX(TEE)에 라이브. 실 Zcash **mainnet** UFVK 스캔으로
  PASS/FAIL end-to-end 검증 (샘플 artifact: `docs/examples/`).
- **추가됨**: 웹 결과 조회(`/results`) + DynamoDB + CLI `submit-ufvk --save` (`docs/deploy-web-amplify.md`).
- **범위 밖**: ZK 비-교집합 회로 (의도적 제외 — `decisions.md` D1·D2).
- 배포 config 별 신뢰 모델 차이: `limitations.md` §3.3.

## 기술 스택

TypeScript · Node.js · Next.js · Rust 사이드카(실 Zcash 스캔) · TEE는 `AttestationProvider`
추상화 — 로컬은 시뮬레이션, 배포는 **Phala/dstack (Intel TDX) 라이브**.
