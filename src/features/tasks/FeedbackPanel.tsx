import { useEffect, useRef, useState } from "react";
import { productionApi, type ProductionRecord, type ProductionView } from "../../shared/production";
import { errorMessage } from "../../shared/api";

export function FeedbackPanel({ taskId, record, active, en = false, selectedArtifact, onUpdated, onContinue }: {
  taskId: string; record: ProductionRecord; active: boolean; en?: boolean; selectedArtifact?: string;
  onUpdated: (view: ProductionView) => void; onContinue?: (message: string, afterRevision: number) => Promise<void>;
}) {
  const [draft, setDraft] = useState("");
  const [artifact, setArtifact] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [attachments, setAttachments] = useState<string[]>([]);
  const generation = useRef(0);
  useEffect(() => { generation.current++; return () => { generation.current++; }; }, [taskId]);
  async function attach(file: File) {
    const current = generation.current;
    if (attachments.length >= 8 || file.size > (file.type.startsWith("video/") ? 64 : 16) * 1024 * 1024) {
      setError(en ? "Up to 8 files; images 16 MiB, videos 64 MiB." : "最多8个附件，图片16 MiB、视频64 MiB以内。"); return;
    }
    setBusy(true); setError("");
    try {
      const base64 = await new Promise<string>((resolve, reject) => { const reader = new FileReader();
        reader.onload = () => resolve(String(reader.result).split(",")[1]); reader.onerror = reject; reader.readAsDataURL(file); });
      const result = await productionApi.feedback(taskId, record.revision, "attach", { name: file.name, base64 });
      if (current !== generation.current) return;
      const artifacts = result.record?.artifacts ?? [];
      const id = artifacts[artifacts.length - 1]?.id;
      if (id) setAttachments(old => [...old, id]); onUpdated(result);
    } catch (reason) { if (current === generation.current) setError(errorMessage(reason)); } finally { if (current === generation.current) setBusy(false); }
  }
  useEffect(() => { setArtifact(selectedArtifact ?? ""); }, [selectedArtifact]);
  useEffect(() => { setDraft(""); setError(""); setArtifact(""); }, [taskId]);
  async function run(operation: string, data: unknown) {
    const current = generation.current;
    setBusy(true); setError("");
    try {
      const result = await productionApi.feedback(taskId, record.revision, operation, data);
      if (current !== generation.current) return;
      onUpdated(result);
      if (operation === "add") {
        setDraft(""); setArtifact(""); setAttachments([]);
        try { await onContinue?.("策划已提交新的反馈，请读取制作记录，定位并修改，复验后交回策划。", result.record!.revision); }
        catch (reason) { setError((en ? "Feedback saved. " : "反馈已保存。") + errorMessage(reason)); }
      }
    } catch (reason) { if (current === generation.current) setError(errorMessage(reason)); }
    finally { if (current === generation.current) setBusy(false); }
  }
  const names: Record<string, string> = { open: "待处理", "in-progress": "AI处理中", "awaiting-recheck": "等待策划复验", resolved: "策划已关闭" };
  return <section className="production-card" id="production-feedback">
    <h4>{en ? "Feedback and rechecks" : "反馈与修改记录"}</h4>
    <label className="field">{en ? "Related visual (optional)" : "关联视觉产物（可选）"}
      <select value={artifact} onChange={e => setArtifact(e.target.value)}>
        <option value="">{en ? "Gameplay / general" : "玩法或整体意见"}</option>
        {(record.artifacts ?? []).filter(a => a.mediaType).map(a => <option key={a.id} value={a.id}>{a.title} · {a.visualVersion}</option>)}
      </select></label>
    <label className="field">{en ? "What happened? What should change?" : "遇到了什么问题，希望怎样修改？"}
      <textarea value={draft} maxLength={8000} onChange={e => setDraft(e.target.value)} rows={3} /></label>
    <label className="field">{en ? "Screenshot or short video" : "附上截图或短视频"}
      <input type="file" accept="image/png,image/jpeg,image/webp,image/gif,video/mp4,video/webm" disabled={!active || busy}
        onChange={e => { const file = e.target.files?.[0]; if (file) void attach(file); e.target.value = ""; }} />
    </label>
    {attachments.length > 0 && <p>{en ? "Attached files: " : "已添加附件："}{attachments.length}</p>}
    <button className="primary-button" disabled={!active || busy || !draft.trim()} onClick={() => void run("add", { description: draft, artifactId: artifact || undefined, attachmentIds: attachments })}>
      {en ? "Submit and continue" : "提交反馈并继续"}
    </button>
    {error && <p role="alert">{error}</p>}
    {(record.feedback ?? []).map(f => <article className="production-issue-card" key={f.id}>
      <div className="production-card-heading"><strong>{f.description}</strong><span>{en ? f.status : names[f.status]}</span></div>
      <small>{f.version ?? (en ? "Visual proposal" : "视觉方案")} → {f.candidateVersion ?? "—"}</small>
      {f.fix && <p>{en ? "Changes: " : "修改："}{f.fix}</p>}{f.recheck && <p>{en ? "Recheck: " : "复验："}{f.recheck}</p>}
      {f.status !== "resolved" ? <button className="secondary-button" disabled={!active || busy || (!f.artifactId && (f.status !== "awaiting-recheck" || f.candidateVersion !== record.currentVersion))}
        onClick={() => void run("resolve", { id: f.id })}>{en ? "Verified, close" : "已复验，关闭此问题"}</button>
        : <button className="secondary-button" disabled={!active || busy} onClick={() => void run("reopen", { id: f.id })}>{en ? "Reopen" : "重新打开"}</button>}
      <details><summary>{en ? "History" : "处理历史"}</summary>{f.history?.map((h, i) => <p key={i}>{new Date(h.at).toLocaleString()} · {h.actor} · {en ? h.status : names[h.status]} {h.fix}</p>)}</details>
    </article>)}
  </section>;
}
