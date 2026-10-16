import { useState } from "react";
import { Modal } from "../../app/Modal";
import type { ProductionDocument, ProductionView } from "../../shared/production";
import { VisualGallery } from "./VisualGallery";
import { ReadableText } from "./ReadableText";

export function DocumentReview({ taskId, document: initial, view, active, busy, error, en = false, onClose, onDecide }: {
  taskId: string; document: ProductionDocument; view: ProductionView; active: boolean; busy: boolean; error: string; en?: boolean;
  onClose: () => void; onDecide: (document: ProductionDocument, accepted: boolean, feedback: string) => Promise<void>;
}) {
  const [snapshot, setSnapshot] = useState(initial);
  const [decision, setDecision] = useState("");
  const [feedback, setFeedback] = useState("");
  const current = view.record?.documents[snapshot.kind];
  const stale = !current || current.sha256 !== snapshot.sha256 || current.revision !== snapshot.revision;
  const reason = stale ? (en ? "A new version is available. Read it before submitting." : "成果已更新，请阅读新版本后再提交。")
    : !active ? (en ? "Task is inactive." : "任务当前不可操作。") : view.actions?.review[snapshot.kind] ?? (en ? "Review availability is being checked." : "正在核对审查条件。");
  const approveReason = view.actions?.approve[snapshot.kind] ?? reason;
  const artifacts = (view.artifacts ?? []).filter(a => snapshot.artifactBindings?.[a.path] === a.sha256);
  const approval = view.policy.stages.some(s => s.id === snapshot.kind && s.approval);
  return <Modal wide title={`${en ? "Review document" : "成果审查"} · ${en ? "Version" : "第"} ${snapshot.revision}${en ? "" : " 版"}`} onClose={onClose}>
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
          <label className="field">{decision === "reject" ? (en ? "Required changes (required)" : "修改意见（必填）") : (en ? "Notes (optional)" : "审查意见（可选）")}
            <textarea rows={6} value={feedback} disabled={busy} maxLength={4000} onChange={e => setFeedback(e.target.value)} /></label>
          {decision === "accept" && approveReason && <p role="alert">{approveReason}</p>}
          {error && <p role="alert" className="inline-error">{error}</p>}
        </>}
      </aside>
    </div>
    <div className="modal-actions review-footer">
      <button className="secondary-button" onClick={onClose}>{en ? "Close" : "关闭"}</button>
      {approval && <button className="primary-button" disabled={busy || !!reason || !decision || (decision === "accept" ? !!approveReason : !feedback.trim())}
        onClick={() => void onDecide(snapshot, decision === "accept", feedback)}>{busy ? (en ? "Saving…" : "正在提交…") : decision === "reject" ? (en ? "Submit changes" : "提交修改意见") : (en ? "Confirm this version and continue" : "确认此版本并继续")}</button>}
    </div>
  </Modal>;
}
