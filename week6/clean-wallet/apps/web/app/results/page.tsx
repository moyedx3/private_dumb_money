import Link from "next/link";
import { listArtifacts } from "../../lib/dynamo";
import { UploadForm } from "./upload-form";

// 항상 최신 상태를 읽는다 (캐시 안 함).
export const dynamic = "force-dynamic";

export default async function ResultsPage() {
  const items = await listArtifacts();

  return (
    <section className="stack">
      <h1>Screening 결과 (DB 조회)</h1>
      <p className="lead">
        CLI <code>submit-ufvk --save</code> 또는 아래 업로드로 저장된 screening artifact
        목록입니다. UFVK·salt·거래내역은 저장되지 않습니다 — artifact(PASS/FAIL + attestation
        바인딩)만 보관·조회합니다.
      </p>

      <UploadForm />

      {items.length === 0 ? (
        <div className="note">
          아직 저장된 artifact가 없습니다. CLI로 스캔 후 <code>--save &lt;이 사이트&gt;/api/artifacts</code>
          를 붙이거나, 위에 artifact JSON을 붙여넣어 업로드하세요.
        </div>
      ) : (
        <table className="results">
          <thead>
            <tr>
              <th>결과</th>
              <th>provider</th>
              <th>scan 구간</th>
              <th>chain source</th>
              <th>저장</th>
              <th>시각 (UTC)</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {items.map((i) => (
              <tr key={i.id}>
                <td>
                  <span className={i.result === "PASS" ? "badge pass" : "badge fail"}>
                    {i.result}
                  </span>
                </td>
                <td className="mono">{i.provider}</td>
                <td className="mono">
                  {i.scanStartHeight}–{i.scanEndHeight}
                </td>
                <td className="mono">
                  {i.chainSourceKind}
                  {i.network ? ` (${i.network})` : ""}
                </td>
                <td>{i.savedVia}</td>
                <td className="mono">{i.createdAt.replace("T", " ").slice(0, 19)}</td>
                <td>
                  <Link href={`/results/${i.id}`}>상세 →</Link>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
