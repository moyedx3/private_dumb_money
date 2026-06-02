"use client";

import { useState } from "react";
import { runScanAction } from "../actions";
import type { ScanActionResult } from "../action-types";

export function ProverForm() {
  const [scope, setScope] = useState<"clean" | "tainted">("clean");
  const [result, setResult] = useState<ScanActionResult | null>(null);
  const [pending, setPending] = useState(false);

  async function handleScan() {
    setPending(true);
    setResult(null);
    try {
      setResult(await runScanAction(scope));
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="stack">
      <div className="field">
        <label>지갑 viewing scope</label>
        <div className="choices">
          <button
            type="button"
            className={scope === "clean" ? "choice active" : "choice"}
            onClick={() => setScope("clean")}
          >
            scope-clean — 깨끗한 지갑
          </button>
          <button
            type="button"
            className={scope === "tainted" ? "choice active" : "choice"}
            onClick={() => setScope("tainted")}
          >
            scope-tainted — 제재 수취인 포함
          </button>
        </div>
      </div>

      <button className="btn primary" onClick={handleScan} disabled={pending}>
        {pending ? "스캔 중…" : "스캔 실행 → artifact 생성"}
      </button>

      {result && (
        <div className="stack">
          <div className={result.result === "PASS" ? "badge pass" : "badge fail"}>
            스캔 결과: {result.result}
          </div>
          <p className="muted">
            도출된 출금 record {result.recordCount}건 — enclave 내부에만 존재하며 아래
            artifact에는 포함되지 않습니다.
          </p>
          <label>screening artifact (이 JSON을 Verifier에 붙여넣으세요)</label>
          <pre className="code">{JSON.stringify(result.artifact, null, 2)}</pre>
        </div>
      )}
    </div>
  );
}
