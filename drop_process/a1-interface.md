# A1 Interface Index

A1 interface 문서는 분량이 길어져 아래 파일들로 분리한다.

## 전체 구조

| 파일 | 내용 |
| --- | --- |
| `a1-interface/00-overview.md` | 현재 A1 구현 상태와 파일 맵 |
| `a1-interface/01-wire-format.md` | memo, dispatch blob, bucket key wire format |
| `a1-interface/02-engine-and-scanner.md` | scanner, payment engine, catalog/bucket boundary |
| `a1-interface/03-state-security.md` | encrypted scan state, TEE/enclave 보안 경계 |
| `a1-interface/04-api-service-vector.md` | creator 등록, buyer dispatch 조회 service vector |
| `a1-interface/05-operations.md` | live smoke CLI, 환경 변수, 검증 상태 |
| `a1-interface/06-next-gaps.md` | 남은 구현 후보와 운영화 gap |

## 파트별 협업 문서

| 파일 | 대상 |
| --- | --- |
| `a1-interface/lanes/lane-b-buyer-app.md` | Lane B / 구매자 앱 |
| `a1-interface/lanes/lane-a2-enclave.md` | Lane A2 / enclave, sealed provisioning |
| `a1-interface/lanes/lane-c-creator-content.md` | Lane C / creator, content 등록 |
| `a1-interface/lanes/lane-d-storage-bucket.md` | Lane D / storage, bucket |

## A1 한 줄 요약

A1은 Zcash shielded 결제를 감지하고, memo를 파싱해 결제를 검증한 뒤, 구매자만 복호화할 수 있는 콘텐츠 키 dispatch blob을 생성·저장·조회 가능하게 만드는 결제 레이어다.
