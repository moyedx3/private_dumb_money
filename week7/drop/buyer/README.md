# Lane B — Buyer Web App

구매자가 보는 화면 전부. 공개 카탈로그 → 구매당 일회용 X25519 키쌍 → ZIP-321 결제 QR(Zashi 스캔) → 버킷 폴링 + dispatch blob trial-open → `K_drop` → 콘텐츠 복호화·렌더.

스펙: [`../team/lane-B-buyer-app.md`](../team/lane-B-buyer-app.md) · 계약: [`../team/interfaces.md`](../team/interfaces.md) · 점검: [`../team/lane-B-design-check.md`](../team/lane-B-design-check.md)

## 실행

```bash
npm install
npm run dev     # 127.0.0.1:5173 — 기본 Demo(mock) 모드라 서버/체인 없이 단독 동작
npm test        # vitest (11 tests)
npm run build   # tsc + vite build
```

- **Demo(mock) 모드** *(기본)*: `MockDropApi`가 인덱서+버킷+A1을 인프로세스로 대체. 드롭 2개, "Simulate payment (mock)" 버튼으로 결제→dispatch→언락 전 과정을 서버 없이 시연.
- **Live indexer 모드**: 토글 후 인덱서 URL 입력. 실제 `GET /catalog`·`/dispatch`·`/bucket/:key`에 붙음. **단 아래 4개 요청이 통합돼야 동작**(미적용 시 mock으로 개발).

## 가정한 미적용 요청 (팀 싱크 전제)

[`../team/lane-B-requests-to-A1-A2.md`](../team/lane-B-requests-to-A1-A2.md)의 4개가 적용된다고 가정하고 짰다:

| 코드 | 가정 | 영향받는 파일 |
|---|---|---|
| R-A2-2 | 카탈로그 엔트리에 `deposit_addr` | `api.ts` `CatalogEntry` |
| R-A2-1/3 | `GET /dispatch` (dispatch 키만 반환), content와 분리 | `api.ts` `listDispatch` |
| R-A1-1 | memo 텍스트 폴백 prefix `A1B64:` (I1 문서화) | `memo.ts` `TEXT_MEMO_PREFIX` |

미적용 동안은 mock 모드로 전 기능 개발·테스트 가능.

## 바이트 계약 (상대 레인과 일치해야)

| 인터페이스 | 형식 | 파일 |
|---|---|---|
| I1 memo | `drop_id(8 BE) ‖ e_pub(32)`=40B, 또는 `A1B64:`+base64url(40B) | `memo.ts` |
| I2 dispatch blob | `crypto_box_seal(K_drop, e_pub)` = 80B (libsodium↔dryoc) | `seal.ts` |
| I4 content blob | `nonce(12) ‖ AES-256-GCM ‖ tag(16)`, key=sha256 | `content.ts` (C와 동일) |

- `memo.test.ts`가 **A1의 `memo.rs` 테스트 벡터**(`A1B64:AAAA...`)를 재현해 base64url 바이트 일치를 검증.
- `bytes.ts`·`content.ts`는 Lane C(`creator/src/`)와 **바이트 동일**(콘텐츠 복호 호환).
- **통합 시 추가 권장**: A1이 실제 만든 dispatch blob 1개를 픽스처로 받아 `trySealOpen` 교차검증(dryoc→libsodium).

## 파일 맵

```
src/
  bytes.ts      바이트/해시 헬퍼 (C와 동일)
  content.ts    AES-256-GCM I4 (C와 동일, B는 복호)
  seal.ts       libsodium: 키쌍·sealed box open·base64url
  memo.ts       I1 인코딩 (raw + A1B64: 텍스트) + 디코드
  zip321.ts     ZIP-321 URI 빌드 + 투명주소 거부 가드
  api.ts        DropApi 인터페이스 + HttpDropApi
  mockApi.ts    단독 데모/테스트용 인프로세스 백엔드 (A1 시뮬)
  purchase.ts   Purchase(e_priv↔drop_id↔h_content 매핑) + recovery 파일
  poller.ts     dispatch 폴링 + trial-open + 복호 → UnlockResult
  render.ts     콘텐츠 종류 sniff (image/text/binary)
  App.tsx       UI (catalog → QR → 폴링 → 언락)
```

## 알려진 한계 / 후속

- **e_priv 분실 = 구매 소실**: 결제 전 경고 + recovery 파일 **export/import** + **opt-in 로컬 영속(24h)** 으로 새로고침·탭 닫힘에 견딤(`persist.ts`). (spec은 IndexedDB 제안 — 작은 데이터라 localStorage로 등가 처리, 교체 가능.)
- **content_type 없음**: I3-a에 MIME가 없어 `render.ts`가 매직바이트로 sniff. → I3-a에 `content_type` 추가하면 깔끔(소소한 추가 요청거리).
- **네트워크 상관**: 폴링 IP↔지갑 IP 상관은 문서화된 out-of-scope 한계(Tor/믹스넷 필요).
- **데모 전 필수**: 데모 폰·데모 Zashi 빌드로 raw 40B 바이너리 memo가 byte-identical로 실리는지 실측 → raw/텍스트 기본형식 확정(R-A1-1).
