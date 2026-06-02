/**
 * artifact 저장소 — DynamoDB.
 *
 * 환경변수:
 * - ARTIFACTS_TABLE : DynamoDB 테이블 이름. 미설정 시 in-memory(개발용, 비영속)로 폴백.
 * - AWS_REGION      : 리전 (Amplify가 자동 주입, 기본 us-east-1).
 * 자격증명은 AWS SDK 기본 체인(Amplify 컴퓨트 IAM 역할 / 환경변수 / 프로파일)에서 온다.
 *
 * 저장되는 데이터는 ScreeningArtifact(비밀 아님)뿐 — UFVK·salt·raw 주소는 들어가지 않는다.
 */
import {
  DynamoDBClient,
} from "@aws-sdk/client-dynamodb";
import {
  DynamoDBDocumentClient,
  GetCommand,
  PutCommand,
  ScanCommand,
} from "@aws-sdk/lib-dynamodb";
import type { StoredArtifact } from "./artifacts";

const TABLE = process.env.ARTIFACTS_TABLE;
const REGION = process.env.AWS_REGION ?? "us-east-1";

const doc = TABLE
  ? DynamoDBDocumentClient.from(new DynamoDBClient({ region: REGION }), {
      marshallOptions: { removeUndefinedValues: true },
    })
  : null;

// ARTIFACTS_TABLE 미설정 시 개발용 in-memory 저장. 프로세스 재시작 시 사라진다.
const memStore = new Map<string, StoredArtifact>();
let warnedMem = false;
function warnMemOnce(): void {
  if (!warnedMem) {
    console.warn(
      "[dynamo] ARTIFACTS_TABLE 미설정 — in-memory 저장(개발용, 비영속)을 사용합니다.",
    );
    warnedMem = true;
  }
}

const byNewest = (a: StoredArtifact, b: StoredArtifact): number =>
  b.createdAt.localeCompare(a.createdAt);

export async function putArtifact(item: StoredArtifact): Promise<void> {
  if (!doc) {
    warnMemOnce();
    memStore.set(item.id, item);
    return;
  }
  await doc.send(new PutCommand({ TableName: TABLE, Item: item }));
}

export async function getArtifact(id: string): Promise<StoredArtifact | null> {
  if (!doc) {
    warnMemOnce();
    return memStore.get(id) ?? null;
  }
  const res = await doc.send(new GetCommand({ TableName: TABLE, Key: { id } }));
  return (res.Item as StoredArtifact | undefined) ?? null;
}

export async function listArtifacts(): Promise<StoredArtifact[]> {
  if (!doc) {
    warnMemOnce();
    return [...memStore.values()].sort(byNewest);
  }
  // 데모 규모(저용량)에서는 Scan으로 충분. 대량이면 GSI + Query로 전환.
  const res = await doc.send(new ScanCommand({ TableName: TABLE }));
  return ((res.Items as StoredArtifact[] | undefined) ?? []).sort(byNewest);
}
