"use client";

import { useState } from "react";
import { verifyArtifactAction } from "../actions";
import type { VerifyActionResult } from "../action-types";

export function VerifierForm() {
  const [json, setJson] = useState("");
  const [result, setResult] = useState<VerifyActionResult | null>(null);
  const [pending, setPending] = useState(false);

  async function handleVerify() {
    setPending(true);
    setResult(null);
    try {
      setResult(await verifyArtifactAction(json));
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="stack">
      <label>screening artifact JSON</label>
      <textarea
        className="code input"
        rows={12}
        value={json}
        onChange={(e) => setJson(e.target.value)}
        placeholder="Prover 페이지에서 생성한 artifact JSON을 붙여넣으세요"
      />
      <button
        className="btn primary"
        onClick={handleVerify}
        disabled={pending || json.trim() === ""}
      >
        {pending ? "검증 중…" : "artifact 검증"}
      </button>

      {result && !result.ok && <div className="badge fail">검증 오류: {result.error}</div>}

      {result && result.ok && (
        <div className="stack">
          <div className="row">
            <span className={result.verification.ok ? "badge pass" : "badge fail"}>
              {result.verification.ok ? "검증 통과" : "검증 실패"}
            </span>
            <span className="badge muted">
              attestation: {result.verification.attestationMode}
            </span>
          </div>
          {result.verification.note && (
            <div className="note">{result.verification.note}</div>
          )}
          <ul className="checks">
            {result.verification.checks.map((c) => (
              <li key={c.name} className={c.delegated ? "delegated" : c.ok ? "ok" : "no"}>
                <span className="mark">{c.delegated ? "⧉" : c.ok ? "✔" : "✗"}</span>
                <span className="cname">{c.name}</span>
                <span className="cdetail">{c.detail}</span>
              </li>
            ))}
          </ul>
          <p className="muted">
            신뢰 가능한 결과:{" "}
            <strong>
              {result.verification.trustedResult ?? "(없음 — 검증 실패 또는 위임 미완료)"}
            </strong>
          </p>
        </div>
      )}
    </div>
  );
}
