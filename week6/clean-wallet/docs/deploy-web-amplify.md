# 배포 가이드 — Phala 스캐너 + AWS Amplify 조회 웹

> 스캐너(TEE)는 Phala dstack 에, 결과 조회 웹은 AWS Amplify 에 배포한다.
> UFVK 는 **CLI 로만** TEE 에 들어가고, 웹·DB 에는 **비밀이 아닌 artifact 만** 흐른다.
>
> 스캐너 자체 배포 절차는 [`deploy-phala.md`](./deploy-phala.md) 에 있다. 이 문서는 그
> 위에 **DynamoDB + Amplify 조회 웹** 을 얹는 부분을 다룬다.

## 0. 구조와 보안 모델 (왜 이렇게 하나)

```
[유저 머신: CLI submit-ufvk] ──RA-TLS──►[Phala CVM: 스캐너 = TEE]
          │  artifact (PASS/FAIL + attestation, 비밀 아님)
          │  --save
          ▼
   [DynamoDB] ◄── POST /api/artifacts (Amplify)
          │
          ▼
   [Amplify 웹] ──read──► 목록·상세 조회 + 바인딩 재검증
```

핵심 원칙 — **TEE 가 필요한 건 스캐너뿐**이다. 웹·DB 는 비밀을 다루지 않으므로 일반
호스팅(Amplify/DynamoDB)으로 충분하다.

| 데이터 | 비밀? | 어디까지 가나 |
|---|---|---|
| UFVK · salt | 🔴 비밀 | 유저 머신 → (RA-TLS) → enclave. **웹·DB·Amplify 안 감** |
| raw 수취인 주소 | 🔴 비밀 | enclave 안에서만 |
| screening artifact (PASS/FAIL + attestation 바인딩) | 🟢 비밀 아님 | 스캐너 → CLI → DB → 웹 |

왜 UFVK 를 웹 폼으로 안 받나: 브라우저는 RA-TLS quote 를 검증할 수 없어(self-signed cert
+ CORS + TLS 계층 비가시) 중계 서버가 평문 UFVK 를 보게 된다. UFVK 가 어떤 서버도 거치지
않으려면 검증·전송 주체가 유저 머신(CLI)이어야 한다. 그래서 **실 UFVK 는 CLI, 웹은 결과
조회**로 분리한다.

`pickArtifact`(apps/web/lib/artifacts.ts)가 알려진 artifact 필드만 화이트리스트로 추출하므로
스캐너 응답의 `_debug` 나 실수로 섞인 값이 DB 에 저장되지 않는다.

## 1. 준비물

- **Phala Cloud 계정** + 배포된 스캐너 CVM (→ [`deploy-phala.md`](./deploy-phala.md)).
- **AWS 계정** (DynamoDB + Amplify).
- **GitHub(또는 GitLab/Bitbucket) 레포** — Amplify 가 연결해 빌드한다.
- 로컬 **AWS CLI** (테이블 생성용) + Node 22+.
- *(실 UFVK 경로)* testnet/mainnet UFVK + lightwalletd 엔드포인트.

## 2. DynamoDB 테이블 생성

artifact 1건 = 1 아이템, 파티션 키 `id`(artifact 해시). 데모 규모라 on-demand 과금.

```bash
aws dynamodb create-table \
  --table-name zcash-screening-artifacts \
  --attribute-definitions AttributeName=id,AttributeType=S \
  --key-schema AttributeName=id,KeyType=HASH \
  --billing-mode PAY_PER_REQUEST \
  --region us-east-1
```

조회는 데모 규모에서 `Scan` 으로 충분하다. 대량이 되면 `createdAt` GSI + `Query` 로 바꾼다
(apps/web/lib/dynamo.ts 의 `listArtifacts`).

## 3. Amplify 에 웹 배포

이 앱은 **Next.js SSR**(서버액션 + `/api` 라우트 + 서버사이드 DynamoDB 호출)이다 — 정적
export 불가. Amplify 의 SSR(Web Compute) 로 배포한다.

### 3.1 레포 연결

1. 코드를 GitHub 등에 push (모노레포 루트 = `clean-wallet/`).
2. Amplify 콘솔 → **Create new app** → Git 공급자 연결 → 이 레포·브랜치 선택.
3. 모노레포 안내가 나오면 **App root directory = `apps/web`** 로 지정.
4. 빌드 설정은 레포 루트의 [`amplify.yml`](../amplify.yml) 을 자동 사용한다 (workspace
   설치를 위해 루트에서 `npm ci`, 산출물은 `apps/web/.next`). Node 22 사용.

### 3.2 환경변수 (App settings → Environment variables)

| 변수 | 값 | 비고 |
|---|---|---|
| `ARTIFACTS_TABLE` | `zcash-screening-artifacts` | 미설정 시 in-memory(비영속)로 폴백 |
| `INGEST_API_KEY` | (선택) 임의 문자열 | 설정 시 `POST /api/artifacts` 에 `x-api-key` 요구 |

`AWS_REGION` 은 SSR Lambda 런타임이 배포 리전으로 자동 주입하므로 보통 설정 불필요
(Amplify 예약 변수와 충돌 가능 — 따로 넣지 말 것).

### 3.3 DynamoDB 접근 권한 (IAM)

SSR Lambda(서버 코드)가 DynamoDB 에 붙으려면 권한이 필요하다. **Amplify 의 SSR Compute
role** 에 아래 정책을 붙인다 (App settings → IAM roles → Compute role 확인 후 IAM 콘솔에서):

```json
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": ["dynamodb:PutItem", "dynamodb:GetItem", "dynamodb:Scan"],
    "Resource": "arn:aws:dynamodb:us-east-1:<ACCOUNT_ID>:table/zcash-screening-artifacts"
  }]
}
```

> 역할 설정이 번거로우면 임시로 위 권한을 가진 IAM 사용자의
> `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY` 를 Amplify 환경변수로 넣어도 된다 (권장은
> Compute role).

### 3.4 배포

저장하면 Amplify 가 빌드·배포한다. 완료 후 앱 URL(`https://<branch>.<id>.amplifyapp.com`)
확보. `/results` 가 빈 목록으로 떠야 정상.

## 4. End-to-end 테스트

### 4.1 mock 경로 (UFVK 불필요 — 먼저 이걸로 확인)

배포된 스캐너에서 mock attestation artifact 를 받아 DB 에 적재:

```bash
# 1) 스캐너에서 mock 스캔 → 응답을 artifact.json 으로 저장
#    (현재 passthrough/ratls 배포 = self-signed RA-TLS → -k 필요. 대안 gateway 종단 배포만 정식 cert)
curl -k -X POST https://<CVM>/scan \
  -H 'content-type: application/json' \
  -d '{"mode":"mock","scope":"tainted"}' -o artifact.json
```

받은 JSON 을 웹에 올리는 두 방법:

- **웹 업로드**: `/results` → "artifact 직접 업로드" → JSON 붙여넣기 → 저장.
- **API 직접**: 응답을 파일로 저장 후

  ```bash
  curl -X POST https://<app>.amplifyapp.com/api/artifacts \
    -H 'content-type: application/json' \
    -H 'x-api-key: <INGEST_API_KEY 설정 시>' \
    --data @artifact.json
  ```

`/results` 새로고침 → 항목이 보이고, 상세에서 바인딩 검증이 통과해야 한다 (attestation 은
phala-tdx 면 `⧉ 위임` 으로 표시 — 정상).

### 4.2 실 UFVK 경로 (CLI — UFVK 는 웹/DB 를 거치지 않음)

UFVK 는 CLI 로 직접 enclave 에 보내고, `--save` 로 결과 artifact 만 DB 에 적재한다:

```bash
cd clean-wallet
# INGEST_API_KEY 를 설정했다면 CLI 에도 동일 값을 노출
export INGEST_API_KEY=<같은 값>     # PowerShell: $env:INGEST_API_KEY="..."

echo 'uview1...' | node apps/scanner/tools/submit-ufvk.ts \
  --host https://<CVM> \
  --network test \
  --lwd-url https://lwd.testnet.example:443 \
  --start 2500000 --end 2500050 \
  --save https://<app>.amplifyapp.com/api/artifacts \
  --label "testnet 2.5M demo"
```

현재 passthrough(`s`) + `SCANNER_TRANSPORT=ratls` 배포라 `--no-verify` **없이** client 가 enclave
quote 직접 검증 + measurement 자동 핀. (대안 gateway 종단 배포에서만 `--no-verify` 필요 — limitations.md §3.3.)

- stdout = 스캔 결과 artifact JSON (기존과 동일).
- stderr = `artifact 저장됨 → ...` (저장 성공) 또는 저장 실패 경고 (스캔 결과는 그대로 출력).
- UFVK·salt 는 RA-TLS 로 enclave 에만 가고, `--save` 는 `_debug` 를 뺀 **artifact 만**
  전송한다.

`/results` 에서 결과 확인.

## 5. 로컬 개발 (공개 호스팅 없이)

```bash
cd clean-wallet
npm install
cp apps/web/.env.example apps/web/.env.local   # 필요 시 편집
npm run dev      # http://localhost:3000
```

- `ARTIFACTS_TABLE` 미설정 → **in-memory 저장**(프로세스 재시작 시 사라짐). `/results`
  업로드로 즉시 테스트 가능.
- 실 DynamoDB 에 붙으려면 `.env.local` 에 `ARTIFACTS_TABLE` + AWS 자격증명(프로파일 또는
  키) 설정.
- 로컬 웹 → 배포된 스캐너 호출도 동일하게 동작 (CLI `--save http://localhost:3000/api/artifacts`).

## 6. 보안·정직성 노트

- **UFVK·salt·raw 주소는 DB·웹에 저장되지 않는다.** artifact 는 설계상 거래소에 넘기는
  공개물이다 (packages/core/src/types.ts, [`architecture.md`](./architecture.md)).
- **웹의 재검증은 바인딩(정책·입금·구간·nonce·chainSource)까지만 암호학적으로 검증**한다.
  phala-tdx 의 measurement·TDX quote 검증은 Phala verifier 에 위임된다(`⧉ 위임` 표시) —
  순수 JS DCAP 가 비현실적이기 때문 ([`deploy-phala.md`](./deploy-phala.md) §6). 전체 TDX
  검증은 `submit-ufvk --expected-mrtd/--expected-rtmr3` 또는 `cloud-api.phala.com` 으로
  수행한다.
- **`/api/artifacts` 는 쓰기 엔드포인트**다. 공개 배포라면 `INGEST_API_KEY` 를 설정해 무단
  적재를 막아라 (미설정 시 공개 = 데모용).
- `/results` 는 공개 읽기(데모). artifact 가 비밀은 아니지만 목록 노출이 싫으면 Amplify
  Cognito 등으로 보호한다.

## 7. 트러블슈팅

| 증상 | 원인 / 해결 |
|---|---|
| Amplify 빌드 실패 (모듈 없음) | workspace 설치는 루트에서 — `amplify.yml` 이 `cd ../.. && npm ci` 하는지 확인 |
| 런타임 `AccessDeniedException` (DynamoDB) | SSR Compute role 에 §3.3 정책 부착, `Resource` ARN·리전 일치 확인 |
| `/results` 가 항상 비어 있음 | `ARTIFACTS_TABLE` 오타/미설정(→in-memory), 리전 불일치, 또는 아직 미저장 |
| CLI `--save` → `HTTP 401` | `INGEST_API_KEY`(서버) 와 CLI 환경변수 값 불일치 |
| 상세에서 "검증 실패" | 바인딩 불일치(변조·다른 요청)면 정상 탐지. attestation `⧉ 위임` 은 실패가 아님 |
| 스캐너 `curl` TLS 오류 | 현재 passthrough/`ratls` 배포는 self-signed RA-TLS → `curl`엔 `-k`(`--insecure`), submit-ufvk 는 `--no-verify` **없이** quote 로 검증. (대안 gateway 종단 배포만 정식 cert.) PowerShell 은 `curl.exe` |

## 8. (부록) Amplify 대신 EC2

EC2 도 동일하게 동작한다 (서버사이드 DynamoDB 호출 가능):

```bash
# Ubuntu EC2 (Node 22, IAM 인스턴스 프로파일에 §3.3 권한)
git clone <repo> && cd clean-wallet
npm ci
export ARTIFACTS_TABLE=zcash-screening-artifacts AWS_REGION=us-east-1
npm run build --workspace @clean-wallet/web
npm run start  --workspace @clean-wallet/web   # 0.0.0.0:3000, 앞단에 nginx/ALB
```

자격증명은 **EC2 인스턴스 프로파일**(IAM role)로 주는 것이 키 하드코딩보다 안전하다.
