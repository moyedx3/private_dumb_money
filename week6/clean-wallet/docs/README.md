# 문서 안내 (docs/)

이 폴더는 **Zcash Private Off-Ramp Screening** 프로젝트를 이해하기 위한 문서 모음이다.

프로젝트 루트의 `README.md`(영문)·`README.kr.md`(국문)는 개요·빠른 시작이고,
`docs/`는 그 아래의 상세 문서다.

## 읽는 순서

| 순서 | 문서 | 내용 |
|---|---|---|
| 0 | [../ONBOARDING.md](../ONBOARDING.md) | 빠른 시작 — 셋업·테스트·라이브 CVM e2e |
| 1 | [one-pager.md](./one-pager.md) | 한 장 요약 |
| 2 | [architecture.md](./architecture.md) | 구성요소·흐름·데이터·신뢰모델 |
| 3 | [decisions.md](./decisions.md) | 의사결정 기록 (D1~D13) |
| 4 | [limitations.md](./limitations.md) | 정직한 한계 — crypto·TEE·운영 |
| 5 | [demo-architecture-limitations.md](./demo-architecture-limitations.md) | RA-TLS 신뢰모델 + measurement 검증범위/완성 경로 |
| 참고 | [glossary.md](./glossary.md) · [implementation/](./implementation/README.md) | 용어집 · 구현 참조(모듈별 위치·역할) |
| 운영 | [demo-script.md](./demo-script.md) · [testnet-e2e.md](./testnet-e2e.md) · [deploy-phala.md](./deploy-phala.md) · [deploy-web-amplify.md](./deploy-web-amplify.md) | 시연·e2e·배포 |

처음 보는 사람은 0 → 1 → 2 순서. 용어가 막히면 용어집을.

## 문서 종류

- **이해용 문서** (1~5): 프로젝트가 무엇이고 왜 이렇게 설계됐는지.
- **구현 참조** (`implementation/README.md`): 모듈별 핵심 위치·역할 (한 파일).
- **운영 문서**: `demo-script.md`(시연 대본), `testnet-e2e.md`(로컬/testnet e2e),
  `deploy-phala.md`(스캐너 → Phala 배포), `deploy-web-amplify.md`(웹 조회 → Amplify+DynamoDB 배포).

## 루트 원본 문서와의 관계

| 파일 | 상태 |
|---|---|
| `../README.md` / `../README.kr.md` | 프로젝트 README (영문/국문) — 개요·빠른 시작·구조 |

최신 기준은 항상 `docs/`와 루트 `README.md`다.

## 상태

- 최초 작성: 2026-05-20
- 현재: Phase 1–3 완료 (코어·검증기·CLI·웹 데모), Phase 4 배포 완료 (Phala TDX 라이브,
  실 mainnet PASS/FAIL 검증), Phase 5 웹 조회(/results) + DynamoDB
- 빌드/테스트: `npm test` core 17 + scanner 19 = 36 통과, `npm run demo`·`npm run dev` 동작
