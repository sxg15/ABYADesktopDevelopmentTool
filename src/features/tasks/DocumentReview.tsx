import { useId, useState, useEffect, type ReactNode } from "react";
import { Modal } from "../../app/Modal";
import type { ProductionDocument, ProductionView } from "../../shared/production";
import { VisualGallery } from "./VisualGallery";
import { ReadableText } from "./ReadableText";
import { isArtFeedback } from "./artWorkflow";

export function DocumentReview({ taskId, document: initial, view, active, busy, error, en = false, onClose, onDecide, onArtFeedback, embedded = false }: {
  embedded?: boolean;
  taskId: string; document: ProductionDocument; view: ProductionView; active: boolean; busy: boolean; error: string; en?: boolean;
  onClose: () => void; onDecide: (document: ProductionDocument, accepted: boolean, feedback: string) => Promise<void>;
  onArtFeedback?: () => void;
}) {
  const [snapshot, setSnapshot] = useState(initial);
  const draftKey = `abya:review:${taskId}:${initial.kind}:${initial.revision}`;
  const [notesOpen,setNotesOpen]=useState(false);
  const [decision, setDecision] = useState("");
  const [feedback, setFeedback] = useState(() => {try{return embedded ? JSON.parse(localStorage.getItem(draftKey)??"{}").feedback??"" : "";}catch{return "";}});
  useEffect(()=>{if(embedded)try{localStorage.setItem(draftKey,JSON.stringify({feedback}));}catch{/* Submission does not depend on local storage. */}},[draftKey,feedback,embedded]);
  const current = view.record?.documents[snapshot.kind];
  const stale = !current || current.sha256 !== snapshot.sha256 || current.revision !== snapshot.revision;
  const reason = stale ? (en ? "A new version is available. Read it before submitting." : "成果已更新，请阅读新版本后再提交。")
    : !active ? (en ? "Task is inactive." : "任务当前不可操作。") : view.actions?.review[snapshot.kind] ?? (en ? "Review availability is being checked." : "正在核对审查条件。");
  const pendingArt = snapshot.kind === "resources" ? (view.record?.feedback ?? []).filter(f => isArtFeedback(f) && f.status !== "resolved") : [];
  const approveReason = (view.actions?.approve[snapshot.kind] ?? (en ? "Approval checks are unavailable. Refresh before approving." : "批准条件尚未核对，请刷新后再确认。"))
    || (pendingArt.length ? (en ? "Close the art feedback after rechecking before approving." : "请先逐项复验并关闭美术反馈，再确认方案。") : "");
  const submitReason = busy ? (en ? "Saving your decision…" : "正在保存决定，请稍候…") : reason || (!decision ? (en ? "Read the proposal, then choose Approve or Request changes." : "请先阅读方案，再选择“通过此版本”或“需要修改”。")
    : decision === "accept" ? approveReason : !feedback.trim() ? (en ? "Describe the required changes before submitting." : "请填写希望修改的内容，再提交修改意见。") : "");
  const reasonId = useId();
  const artifacts = (view.artifacts ?? view.record?.artifacts ?? []).filter(a => snapshot.artifactBindings?.[a.path] === a.sha256);
  const approval = view.policy.stages.some(s => s.id === snapshot.kind && s.approval);
  return <ReviewFrame embedded={embedded} en={en} onClose={onClose}>
    <h3>{en ? "Review version" : "审阅成果"} · {snapshot.revision}</h3>
    <p className="review-context">{view.record?.taskId} · {view.policy.stages.find(s => s.id === snapshot.kind)?.name} · {snapshot.path}</p>
    <div className="review-layout">
      <div className="review-content">
        <article className="document-preview" aria-label={en ? "Document contents" : "文档内容"}>
          <ReadableText text={snapshot.content} />
        </article>
        <VisualGallery taskId={taskId} artifacts={artifacts} bindings={snapshot.artifactBindings} en={en} />
      </div>
      <aside className="review-decision">
        <h3>{en ? "Review decision" : "审查结论"}</h3>
        {!!pendingArt.length && <div className="review-blockers">
          <strong>{en ? `${pendingArt.length} art feedback items still need closure` : `确认前还需处理 ${pendingArt.length} 条美术反馈`}</strong>
          <p>{en ? "Recheck the latest fixes in the art workspace. You can still request further changes below." : "请先在统一协作区查看修改结果并逐项复验；如果仍不满意，也可以在下方选择“需要修改”。"}</p>
          {onArtFeedback && <button className="secondary-button" onClick={onArtFeedback}>{en ? "Go to changes and rechecks" : "前往修改与复验"}</button>}
        </div>}
        {reason && <p role="status">{reason}</p>}
        {view.record?.approvals.filter(a => a.kind === snapshot.kind && a.documentRevision === snapshot.revision).map((a,i) =>
          <p key={i}>{a.decision === "accepted" ? (en ? "Approved" : "已确认") : (en ? "Changes requested" : "已退回修改")} · {new Date(a.decidedAt).toLocaleString()}<br/>{a.feedback}</p>)}
        {stale && current && <button className="secondary-button" onClick={() => { setSnapshot(current); setDecision(""); }}>{en ? "Read latest version" : "阅读最新版本（保留意见）"}</button>}
        {approval && <>
          <fieldset disabled={!!reason || busy}>
            <legend>{en ? "Choose after reading" : "阅读后选择处理方式"}</legend>
            <label><input type="radio" name="review-decision" checked={decision === "accept"} onChange={() => setDecision("accept")} />{en ? "Approve this version" : "通过此版本"}</label>
            <label><input type="radio" name="review-decision" checked={decision === "reject"} onChange={() => setDecision("reject")} />{en ? "Request changes" : "需要修改"}</label>
          </fieldset>
          {embedded && decision !== "reject" && !feedback && <button className="text-button" onClick={()=>setNotesOpen(v=>!v)}>{en ? "Add a note" : "添加审查意见"}</button>}
          {(!embedded || notesOpen || decision === "reject" || !!feedback) && <label className="field">{decision === "reject" ? (en ? "Required changes (required)" : "修改意见（必填）") : (en ? "Notes (optional)" : "审查意见（可选）")}
            <textarea rows={6} value={feedback} disabled={busy} maxLength={4000} onChange={e => setFeedback(e.target.value)} /></label>}
          {!pendingArt.length && !reason && approveReason && <p className="art-blocker-note">{approveReason}</p>}
          {error && <p role="alert" className="inline-error">{error}</p>}
        </>}
      </aside>
    </div>
    <div className="modal-actions review-footer">
      {approval && <div id={reasonId} className="review-submit-guidance" role="status">{submitReason || (en ? "Ready. This submits only the version you reviewed." : "条件已满足，将提交你刚审阅的这一版决定。")}</div>}
      <button className="secondary-button" onClick={onClose}>{en ? "Close" : "关闭"}</button>
      {approval && <button className="primary-button" aria-describedby={reasonId} disabled={!!submitReason}
        onClick={() => { if (!submitReason) void onDecide(snapshot, decision === "accept", feedback); }}>{busy ? (en ? "Saving…" : "正在提交…") : decision === "reject" ? (en ? "Submit changes" : "提交修改意见") : (en ? "Confirm this version and continue" : "确认此版本并继续")}</button>}
    </div>
  </ReviewFrame>;
}

function ReviewFrame({embedded,en,onClose,children}:{embedded:boolean;en:boolean;onClose:()=>void;children:ReactNode}) {
  return embedded ? <section className="workbench-review">{children}</section> : <Modal wide title={en?"Review document":"成果审查"} onClose={onClose}>{children}</Modal>;
}
