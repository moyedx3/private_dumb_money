# A2 — 미뤄둔 후속 작업

> Lane-B 요청서(`lane-B-requests-to-A1-A2.md`) 중 이번 PR(`feat/a2-lane-b-requests`)에서
> **의도적으로 제외**한 항목. 기능 블로커가 아니므로 시간 날 때 착수한다.

## R-A2-4 — 공개 버킷을 인덱서(TEE 호스트)에서 분리 (프라이버시 정합성)

**상태:** 보류 (데모 차단 아님). 이번 PR은 R-A2-1/2/3(기능 블로커)만 처리.

**무엇.** buyer가 읽는 것(카탈로그·dispatch·content)을 인덱서(TEE 호스트)가 직접 서빙하지
말고 별도 dumb 저장소(S3/CloudFront·Blossom)에 둔다. TEE/인덱서·A1은 **put만**, buyer는 **get만**.

**왜.** 현재는 버킷=인덱서 동일 호스트라, buyer가 폴링할 때마다 TEE 호스트가 buyer IP·폴링
패턴을 본다 → "oblivious 우체부" 속성이 네트워크 레이어에서 약화 (spec §7.3, project-scope §2).
암호는 멀쩡하지만 메타데이터(누가 폴링하는지)가 새는 문제.

**착수 시 할 일.**
- A2(인프라): blob·카탈로그 저장을 외부 객체 저장소로. put 경로만 인덱서/A1 안. buyer 공개
  읽기 URL = 그 저장소. (카탈로그는 서명과 함께 두면 변조 방지까지.)
- B(config, 소): buyer 읽기 base를 인덱서가 아니라 버킷으로 — `VITE_DROP_BUCKET_URL` 도입
  (`buyer/src/api.ts`의 단일 `indexerUrl` 분리). 데모는 인덱서와 같은 값, 운용 땐 CDN.

**완료 기준.** buyer의 카탈로그·dispatch·content 읽기가 인덱서(TEE 호스트)가 아닌 별도
저장소로 가고, 인덱서는 buyer 읽기 트래픽을 보지 않는다.

## (별개 인지) 카탈로그 영속화
`catalog.rs`는 in-memory라 재시작 시 전 drop 소실 → 재-provision 필요. 데모 범위로 수용,
production은 영속 저장 필요. (이번 요청 아님; 기록용.)
