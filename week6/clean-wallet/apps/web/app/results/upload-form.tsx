"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { saveArtifactAction } from "../actions";

export function UploadForm() {
  const router = useRouter();
  const [json, setJson] = useState("");
  const [label, setLabel] = useState("");
  const [pending, setPending] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);

  async function handleSave() {
    setPending(true);
    setMsg(null);
    try {
      const res = await saveArtifactAction(json, label);
      if (res.ok) {
        setMsg({ ok: true, text: `저장됨 — id ${res.id}` });
        setJson("");
        setLabel("");
        router.refresh();
      } else {
        setMsg({ ok: false, text: res.error });
      }
    } finally {
      setPending(false);
    }
  }

  return (
    <details className="upload">
      <summary>+ artifact 직접 업로드 (CLI 못 쓰는 경우)</summary>
      <div className="stack">
        <label>라벨 (선택)</label>
        <input
          className="text-input"
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          placeholder="예: testnet 2.5M 구간 스캔"
        />
        <label>screening artifact JSON</label>
        <textarea
          className="code input"
          rows={8}
          value={json}
          onChange={(e) => setJson(e.target.value)}
          placeholder="스캐너 응답의 artifact JSON을 붙여넣으세요 (_debug 필드는 자동 제거됩니다)"
        />
        <button
          className="btn primary"
          onClick={handleSave}
          disabled={pending || json.trim() === ""}
        >
          {pending ? "저장 중…" : "DB에 저장"}
        </button>
        {msg && <div className={msg.ok ? "badge pass" : "badge fail"}>{msg.text}</div>}
      </div>
    </details>
  );
}
