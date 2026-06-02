/**
 * POST /api/artifacts — CLI(submit-ufvk --save)가 screening artifact를 DB에 적재한다.
 * GET  /api/artifacts — 저장된 artifact 요약 목록(JSON).
 *
 * 본문: artifact 객체 그대로, 또는 { artifact, label } 래퍼 둘 다 허용.
 * pickArtifact가 알려진 필드만 화이트리스트로 추출하므로 `_debug`·비밀값은 저장되지 않는다.
 *
 * 선택 보호: INGEST_API_KEY 환경변수가 설정돼 있으면 POST에 헤더
 * `x-api-key: <값>` 또는 `authorization: Bearer <값>`이 일치해야 한다 (미설정 시 공개).
 */
import { NextResponse } from "next/server";
import { pickArtifact, toStored } from "../../../lib/artifacts";
import { listArtifacts, putArtifact } from "../../../lib/dynamo";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

function authorized(req: Request): boolean {
  const key = process.env.INGEST_API_KEY;
  if (!key) return true; // 미설정 = 공개(데모).
  const header = req.headers.get("x-api-key");
  if (header && header === key) return true;
  const auth = req.headers.get("authorization");
  return auth === `Bearer ${key}`;
}

export async function POST(req: Request): Promise<NextResponse> {
  if (!authorized(req)) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  let body: unknown;
  try {
    body = await req.json();
  } catch {
    return NextResponse.json({ error: "invalid JSON body" }, { status: 400 });
  }
  // artifact 그대로 또는 { artifact, label } 래퍼.
  const wrapper = body as { artifact?: unknown; label?: unknown };
  const candidate =
    wrapper && typeof wrapper === "object" && "artifact" in wrapper
      ? wrapper.artifact
      : body;
  const label = typeof wrapper?.label === "string" ? wrapper.label : undefined;

  try {
    const artifact = pickArtifact(candidate);
    const stored = toStored(artifact, {
      savedVia: "cli",
      label: label?.trim() || undefined,
      createdAt: new Date().toISOString(),
    });
    await putArtifact(stored);
    return NextResponse.json({ ok: true, id: stored.id }, { status: 201 });
  } catch (e) {
    return NextResponse.json({ error: (e as Error).message }, { status: 400 });
  }
}

export async function GET(): Promise<NextResponse> {
  const items = await listArtifacts();
  // 목록 응답은 요약만 — artifact 본체는 상세 조회에서.
  const summary = items.map((i) => ({
    id: i.id,
    result: i.result,
    provider: i.provider,
    scanStartHeight: i.scanStartHeight,
    scanEndHeight: i.scanEndHeight,
    chainSourceKind: i.chainSourceKind,
    network: i.network,
    createdAt: i.createdAt,
    savedVia: i.savedVia,
    label: i.label,
  }));
  return NextResponse.json({ items: summary });
}
