import Link from "next/link";
import { notFound } from "next/navigation";
import { getArtifact } from "../../../lib/dynamo";
import { verifyStored } from "../../../lib/artifacts";

export const dynamic = "force-dynamic";

export default async function ResultDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const item = await getArtifact(id);
  if (!item) notFound();

  const v = await verifyStored(item.artifact);

  return (
    <section className="stack">
      <p>
        <Link href="/results">← 결과 목록</Link>
      </p>
      <h1>Artifact {id.slice(0, 12)}…</h1>

      <div className="row">
        <span className={item.result === "PASS" ? "badge pass" : "badge fail"}>
          스캔 결과: {item.result}
        </span>
        <span className={v.ok ? "badge pass" : "badge fail"}>
          {v.ok ? "검증 통과" : "검증 실패"}
        </span>
        <span className="badge muted">attestation: {v.attestationMode}</span>
      </div>

      {item.label && <p className="muted">라벨: {item.label}</p>}

      {v.note && <div className="note">{v.note}</div>}

      <h2>검증 항목</h2>
      <ul className="checks">
        {v.checks.map((c) => (
          <li key={c.name} className={c.delegated ? "delegated" : c.ok ? "ok" : "no"}>
            <span className="mark">{c.delegated ? "⧉" : c.ok ? "✔" : "✗"}</span>
            <span className="cname">{c.name}</span>
            <span className="cdetail">{c.detail}</span>
          </li>
        ))}
      </ul>
      <p className="muted">
        신뢰 가능한 결과:{" "}
        <strong>{v.trustedResult ?? "(없음 — 검증 실패 또는 위임 미완료)"}</strong>
      </p>

      <h2>screening artifact</h2>
      <pre className="code">{JSON.stringify(item.artifact, null, 2)}</pre>
    </section>
  );
}
