import { useEffect, useRef, useState } from "react";
import { productionApi, type ProductionRecord, type ProductionView, type ProductionFeedback } from "../../shared/production";
import { errorMessage } from "../../shared/api";
import { Modal } from "../../app/Modal";
import { ArtifactMedia } from "./ArtifactPreview";
import { VisualGallery } from "./VisualGallery";
import { artRecheckReason, artVisuals, isArtFeedback } from "./artWorkflow";

export function FeedbackPanel({ taskId, record, active, en = false, selectedArtifact, onUpdated, onContinue, actions = {}, onDismiss, scope, recheckActions, onRequestRevision }: {
  taskId: string; record: ProductionRecord; active: boolean; en?: boolean; selectedArtifact?: string;
  onUpdated: (view: ProductionView) => void; onContinue?: (message: string, afterRevision: number) => Promise<void>;
  actions?: Record<string,string>; onDismiss?: () => void;
  scope?: "art"; recheckActions?: Record<string,string>; onRequestRevision?: () => void;
}) {
  const [draft, setDraft] = useState("");
  const [artifact, setArtifact] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [attachments, setAttachments] = useState<string[]>([]);
  const [open, setOpen] = useState(false);
  const [rechecking, setRechecking] = useState<ProductionFeedback>();
  const [targetVersion, setTargetVersion] = useState("");
  const [showResolved, setShowResolved] = useState(false);
  const [notice, setNotice] = useState("");
  const locked = useRef(false);
  const selected = record.artifacts?.find(a => a.id === artifact);
  const version = artifact ? selected?.sha256 ?? "" : record.currentVersion ?? "";
  const reason = !active ? "任务当前不可操作。" : targetVersion !== version && open ? "成果版本已变化，请关闭后重新查看并反馈。" : actions[artifact || "gameplay"] ?? "尚无已提交的可反馈成果。";
  function close() { setOpen(false); onDismiss?.(); }
  const generation = useRef(0);
  useEffect(() => { generation.current++; return () => { generation.current++; }; }, [taskId]);
  async function attach(file: File) {
    if (reason || locked.current) return;
    locked.current = true;
    const current = generation.current;
    if (attachments.length >= 8 || file.size > (file.type.startsWith("video/") ? 64 : 16) * 1024 * 1024) {
      setError(en ? "Up to 8 files; images 16 MiB, videos 64 MiB." : "最多8个附件，图片16 MiB、视频64 MiB以内。"); locked.current = false; return;
    }
    setBusy(true); setError("");
    try {
      const base64 = await new Promise<string>((resolve, reject) => { const reader = new FileReader();
        reader.onload = () => resolve(String(reader.result).split(",")[1]); reader.onerror = reject; reader.readAsDataURL(file); });
      const result = await productionApi.feedback(taskId, record.revision, "attach", { name: file.name, base64, artifactId: artifact || undefined });
      if (current !== generation.current) return;
      const artifacts = result.record?.artifacts ?? [];
      const id = artifacts[artifacts.length - 1]?.id;
      if (id) setAttachments(old => [...old, id]); onUpdated(result);
    } catch (reason) { if (current === generation.current) setError(errorMessage(reason)); } finally { locked.current = false; if (current === generation.current) setBusy(false); }
  }
  useEffect(() => { if (selectedArtifact) { setArtifact(selectedArtifact); setTargetVersion(record.artifacts?.find(a => a.id === selectedArtifact)?.sha256 ?? ""); setOpen(true); } }, [selectedArtifact]);
  useEffect(() => { setDraft(""); setError(""); setArtifact(selectedArtifact ?? ""); setAttachments([]); }, [taskId]);
  async function run(operation: string, data: unknown) {
    if (locked.current || operation === "add" && reason) return;
    locked.current = true;
    const current = generation.current;
    setBusy(true); setError(""); setNotice("");
    try {
      const result = await productionApi.feedback(taskId, record.revision, operation, data);
      if (current !== generation.current) return;
      onUpdated(result);
      if (operation === "resolve") setRechecking(undefined);
      if (operation === "add") {
        close();
        setDraft(""); setArtifact(""); setAttachments([]);
        try { await onContinue?.("策划已提交新的反馈，请读取制作记录，定位并修改，复验后交回策划。", result.record!.revision); }
        catch (reason) { setError((en ? "Feedback saved. " : "反馈已保存。") + errorMessage(reason)); }
      }
      if (scope === "art" && operation === "reopen") {
        setNotice(en ? "Request saved. Reconnecting to the existing conversation…" : "处理请求已保存，正在接回原会话…");
        try {
          await onContinue?.("策划重新打开了美术反馈，请读取最新制作记录中的追加处理要求，更新最新视觉候选和复验说明后交回策划；不要代为关闭反馈或批准方案。", result.record!.revision);
          if (current === generation.current) setNotice(en ? "Request saved. Check task guidance for progress." : "请求已保存，请关注顶部任务引导；若尚未打开会话，请在原会话继续任务。");
        } catch(e) { if (current === generation.current) setError((en ? "Request saved. " : "请求已保存。") + errorMessage(e)); }
      }
    } catch (reason) { if (current === generation.current) setError(errorMessage(reason)); }
    finally { locked.current = false; if (current === generation.current) setBusy(false); }
  }
  const names: Record<string, string> = { open: "待处理", "in-progress": "AI处理中", "awaiting-recheck": "等待策划复验", resolved: "策划已关闭" };
  const canResolve = (f: ProductionFeedback) => {
    if (f.artifactId) return artRecheckReason(record, f, recheckActions, en) === "";
    if (f.status !== "awaiting-recheck") return false;
    return f.candidateVersion === record.currentVersion;
  };
  const liveRecheck = record.feedback?.find(f => f.id === rechecking?.id);
  const staleRecheck = !liveRecheck || JSON.stringify(liveRecheck) !== JSON.stringify(rechecking) || !canResolve(liveRecheck);
  const scopedFeedback = (record.feedback ?? []).filter(f => scope !== "art" || isArtFeedback(f));
  const visibleFeedback = scopedFeedback.filter(f => scope !== "art" || showResolved || f.status !== "resolved");
  const selectable = scope === "art" ? artVisuals(record).current : record.artifacts ?? [];
  return <section className="production-card" id="production-feedback">
    <h4>{scope === "art" ? (en ? "Changes and rechecks" : "修改与复验") : (en ? "Feedback and rechecks" : "反馈与修改记录")}</h4>
    {scope !== "art" && <>
    <button className="secondary-button" disabled={!active || actions.gameplay !== ""} onClick={() => { setArtifact(""); setTargetVersion(record.currentVersion ?? ""); setOpen(true); }}>{en ? "Review playable version / feedback" : "查看试玩版本并反馈"}</button>
    {actions.gameplay !== "" && <p className="production-empty">{actions.gameplay || (en ? "No playable version submitted yet." : "AI 尚未提交可验收作品，当前无法反馈。")}</p>}
    </>}
    {scope === "art" && !visibleFeedback.length && <p className="production-empty">{en ? "No open art feedback. Return to the proposal to review and confirm." : "没有待处理的美术反馈。请返回本版方案，阅读后完成确认。"}</p>}
    {scope === "art" && scopedFeedback.some(f => f.status === "resolved") && <button className="text-button" onClick={() => setShowResolved(v => !v)}>{showResolved ? (en ? "Hide closed feedback" : "收起已关闭反馈") : (en ? "Show closed feedback" : "查看已关闭反馈")}</button>}
    {error && !open && <p role="alert">{error}</p>}
    {notice && <p className="task-action-notice" role="status">{notice}</p>}
    {open && <Modal wide title={en ? "Review and feedback" : "成果查看与反馈"} onClose={close}>
    <p>{selected ? `${selected.title} · ${selected.visualVersion}` : `${en ? "Playable version" : "试玩版本"} ${targetVersion}`}</p>
    {selected && <div className="feedback-preview"><ArtifactMedia taskId={taskId} artifact={selected} en={en} /></div>}
    <label className="field">{en ? "Related visual (optional)" : "关联视觉产物（可选）"}
      <select disabled={busy} value={artifact} onChange={e => { setArtifact(e.target.value); setTargetVersion(e.target.value ? record.artifacts?.find(a => a.id === e.target.value)?.sha256 ?? "" : record.currentVersion ?? ""); }}>
        {scope !== "art" && <option value="">{en ? "Gameplay / general" : "玩法或整体意见"}</option>}
        {selectable.filter(a => a.mediaType && a.sourceType !== "feedback" && !a.plannerReference).map(a => <option disabled={actions[a.id] !== ""} key={a.id} value={a.id}>{a.title} · {a.visualVersion}</option>)}
      </select></label>
    <label className="field">{en ? "What happened? What should change?" : "遇到了什么问题，希望怎样修改？"}
      <textarea disabled={busy || !!reason} value={draft} maxLength={8000} onChange={e => setDraft(e.target.value)} rows={5} /></label>
    <label className="field">{en ? "Screenshot or short video" : "附上截图或短视频"}
      <input type="file" accept="image/png,image/jpeg,image/webp,image/gif,video/mp4,video/webm" disabled={!active || busy || !!reason}
        onChange={e => { const file = e.target.files?.[0]; if (file) void attach(file); e.target.value = ""; }} />
    </label>
    {attachments.length > 0 && <p>{en ? "Attached files: " : "已添加附件："}{attachments.length}</p>}
    {reason && <p role="status">{reason}</p>}
    <div className="modal-actions review-footer"><button className="secondary-button" onClick={close}>{en ? "Close" : "关闭"}</button>
    <button className="primary-button" disabled={!active || busy || !!reason || !draft.trim()} onClick={() => void run("add", { description: draft, artifactId: artifact || undefined, attachmentIds: attachments })}>
      {en ? "Submit and continue" : "提交反馈并继续"}
    </button></div>
    {error && <p role="alert">{error}</p>}
    </Modal>}
    {visibleFeedback.map(f => <article className="production-issue-card" key={f.id}>
      <div className="production-card-heading"><strong>{f.description}</strong><span>{scope === "art" && f.status === "awaiting-recheck" && !canResolve(f) ? (en ? "Recheck material needs updating" : "复验材料待更新") : en ? f.status : names[f.status]}</span></div>
      <small>{f.artifactId ? (record.artifacts?.find(a => a.id === f.artifactId)?.visualVersion ?? (en ? "Original proposal" : "原方案")) : f.version ?? (en ? "Playable version" : "作品版本")} → {f.artifactId ? (record.artifacts?.find(a => a.id === f.candidateArtifactId)?.visualVersion ?? (en ? "Awaiting AI revision" : "等待 AI 提交修订")) : f.candidateVersion ?? "—"}</small>
      {f.fix && <p>{en ? "Changes: " : "修改："}{f.fix}</p>}{f.recheck && <p>{en ? "Recheck: " : "复验："}{f.recheck}</p>}
      {f.reopenReason && f.status === "open" && <p>{en ? "Requested follow-up: " : "追加处理要求："}{f.reopenReason}</p>}
      {f.status !== "resolved" ? <button className="secondary-button" disabled={!active || busy || !canResolve(f)}
        onClick={() => setRechecking(f)}>{en ? "Review fix" : "查看复验结果"}</button>
        : <button className="secondary-button" disabled={!active || busy} onClick={() => void run("reopen", { id: f.id })}>{en ? "Reopen" : "重新打开"}</button>}
      {scope === "art" && f.status !== "resolved" && !canResolve(f) && <div className="art-blocker-note" role="status">
        <p>{artRecheckReason(record, f, recheckActions, en)}</p>
        {f.status === "awaiting-recheck" ? <button className="secondary-button" disabled={!active || busy} onClick={() => void run("reopen",{id:f.id,reason:"当前反馈的复验材料不可用或已过期。请核对本轮最新视觉方案，将这条反馈重新关联到当前最新版方案，补齐修改说明和复验方法后交回策划；不要关闭反馈或批准文档。"})}>{en ? "Ask AI to update recheck material" : "让 AI 补齐复验材料"}</button>
          : onRequestRevision && <button className="text-button" disabled={!active || busy} onClick={onRequestRevision}>{en ? "Read document / request changes" : "查看确认单并要求修改"}</button>}
      </div>}
      {scope === "art" && canResolve(f) && <p className="art-recheck-hint">{en ? "Open the fix, compare the images, then explicitly confirm this issue is resolved." : "先查看修改结果并对照图片；确认符合要求后，再关闭这一条反馈。"}</p>}
      <details><summary>{en ? "History" : "处理历史"}</summary>{f.history?.map((h, i) => <p key={i}>{new Date(h.at).toLocaleString()} · {h.actor} · {en ? h.status : names[h.status]} {h.fix}</p>)}</details>
    </article>)}
    {rechecking && <Modal wide title={en ? "Review fix" : "复验修改结果"} onClose={() => setRechecking(undefined)}>
      <h3>{rechecking.description}</h3><p>{rechecking.fix}</p><p>{rechecking.recheck}</p>
      {rechecking.artifactId ? <VisualGallery taskId={taskId} en={en} artifacts={(record.artifacts ?? []).filter(a => a.id === rechecking.artifactId || a.id === rechecking.candidateArtifactId)} />
        : <p>{en ? "Candidate version" : "待复验作品版本"}：{rechecking.candidateVersion}</p>}
      {staleRecheck && <p role="alert">{en ? "The candidate changed. Reopen and review the latest result." : "候选版本或处理状态已变化，请重新查看最新结果。"}</p>}
      {error && <p role="alert">{error}</p>}
      <div className="modal-actions review-footer"><button className="secondary-button" onClick={() => setRechecking(undefined)}>{en ? "Close" : "关闭"}</button>
        <button className="primary-button" disabled={!active || busy || staleRecheck} onClick={() => void run("resolve",{id:rechecking.id})}>{en ? "Verified, close this issue" : "确认已复验并关闭此问题"}</button></div>
    </Modal>}
  </section>;
}
