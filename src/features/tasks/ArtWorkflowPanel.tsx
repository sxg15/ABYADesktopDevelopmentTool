import { useEffect, useRef, useState } from "react";
import { ArrowRight, CheckCircle2, ClipboardCheck, FileText, History, ImagePlus, Palette } from "lucide-react";
import { Modal } from "../../app/Modal";
import { productionApi, type ProductionDocument, type ProductionView } from "../../shared/production";
import { errorMessage } from "../../shared/api";
import { ArtReferences } from "./ArtReferences";
import { ArtTracePanel } from "./ArtTracePanel";
import { ArtLibraryPanel } from "./ArtLibraryPanel";
import { ArtProposalContext } from "./ArtProposalContext";
import { FeedbackPanel } from "./FeedbackPanel";
import { VisualGallery } from "./VisualGallery";
import { artDocument, artRecheckReason, artVisuals, isArtFeedback } from "./artWorkflow";
import "./artWorkflow.css";

export function ArtWorkflowPanel({ taskId, view, active, en = false, onUpdated, onReview, onContinue, feedbackRequest = 0 }: {
  taskId: string; view: ProductionView; active: boolean; en?: boolean;
  onUpdated: (view: ProductionView) => void; onReview: (doc: ProductionDocument) => void;
  onContinue?: (message: string, revision: number) => Promise<void>; feedbackRequest?: number;
}) {
  const r = view.record!;
  const [pane, setPane] = useState<"proposal"|"feedback"|"requirements">("proposal");
  const [library, setLibrary] = useState(false);
  const [references, setReferences] = useState(false), [error, setError] = useState("");
  const [selectedArtifact, setSelectedArtifact] = useState<string>();
  const [historyOpen, setHistoryOpen] = useState(false);
  const feedbackArea = useRef<HTMLDivElement>(null);
  useEffect(() => { if (feedbackRequest) setPane("feedback"); }, [feedbackRequest]);
  useEffect(() => {
    if (pane !== "feedback") return;
    const target = feedbackArea.current;
    target?.scrollIntoView?.({block:"nearest"}); target?.focus({preventScroll:true});
    target?.classList.add("art-navigation-focus");
    const timer = window.setTimeout(() => target?.classList.remove("art-navigation-focus"), 1100);
    return () => { window.clearTimeout(timer); target?.classList.remove("art-navigation-focus"); };
  }, [pane, feedbackRequest]);
  const artifacts = view.artifacts ?? r.artifacts ?? [];
  const record = {...r, artifacts};
  const { current, history } = artVisuals(r, artifacts);
  const document = artDocument(view);
  const pending = (r.feedback ?? []).filter(f => isArtFeedback(f) && f.status !== "resolved");
  const ready = pending.filter(f => !artRecheckReason(record, f, view.actions?.recheck, en));
  const referenceCount = artifacts.filter(a => a.plannerReference && a.status !== "withdrawn").length;
  const submitted = !!document && current.every(a => document.artifactBindings?.[a.path] === a.sha256);
  const approved = submitted && r.stages[document!.kind] === "passed" && r.approvals.some(a => a.kind === document!.kind && a.documentRevision === document!.revision && a.documentHash === document!.sha256 && a.decision === "accepted");
  const question = r.questionGroups?.find(g => g.stage === "resources" && g.status === "pending");
  const reviewReason = document ? view.actions?.review[document.kind] ?? (en ? "Checking review availability." : "正在核对审查条件。") : "";
  const approveReason = document ? view.actions?.approve[document.kind] ?? (en ? "Checking approval availability." : "正在核对批准条件。") : "";
  const canReview = !!document && !reviewReason;
  const title = !active ? (en ? "This task is read-only" : "任务已结束，当前仅供查阅") : question ? (en ? "Answer the art questions first" : "先回答本阶段的美术问题")
    : pending.length ? ready.length ? (en ? `Recheck ${ready.length} fixes before approval` : `先复验 ${ready.length} 项修改，再确认方案`) : (en ? "Waiting for AI to update the fixes" : "等待 AI 补齐修改与复验结果")
    : approved ? (en ? "This proposal is confirmed" : "本版方案已确认") : canReview ? (en ? "Review the current proposal" : "请审阅并确认本版方案") : (en ? "Waiting for AI to submit the proposal" : "等待 AI 提交完整方案");
  const hint = pending.length ? (en ? `${pending.length} open feedback items; ${pending.length-ready.length} still need AI updates. Nothing is closed automatically.` : `还有 ${pending.length} 条反馈未关闭，其中 ${pending.length-ready.length} 条仍需 AI 更新。逐项复验后，才可确认本版方案。`)
    : approved ? (en ? "The recorded decision is retained. Check task guidance for the next stage." : "确认结果已保存，请关注顶部任务引导中的后续阶段。")
    : canReview ? (approveReason || (en ? "Read the document and images, then approve or request changes in the review dialog." : "先看确认单和本版图片，再进入审查窗口选择“通过”或“需要修改”。"))
    : reviewReason || (en ? "You can add references now. No approval is needed until AI submits a reviewable proposal." : "现在可以补充参考图；AI 提交可审查的完整方案前，无需确认，也不能推进验收。");
  async function reveal(path: string) { try { await productionApi.revealDocument(taskId, path); } catch(e) { setError(errorMessage(e)); } }
  const drafts = artifacts.filter(a => a.stage === "resources" && !a.mediaType && a.path !== document?.path);
  const feedbackTarget = current.find(a => view.actions?.feedback[a.id] === "");
  const nextAction = <div className={`art-next-action ${approved ? "is-complete" : ready.length || canReview || question ? "needs-action" : "is-waiting"}`} role="status">
      <div><strong>{title}</strong><p>{hint}</p></div>
      {active && question ? <button className="primary-button" onClick={() => window.dispatchEvent(new CustomEvent("abya:open-intake",{detail:{taskId,groupId:question.id}}))}>{en ? "Answer questions" : "填写阶段问答"}<ArrowRight size={16}/></button>
        : active && pending.length ? <button className="primary-button" onClick={() => setPane("feedback")}>{ready.length ? (en ? "Recheck fixes" : "进入修改与复验") : (en ? "See what's missing" : "查看等待原因")}<ArrowRight size={16}/></button>
        : document && <button className={active && canReview && !approved ? "primary-button" : "secondary-button"} onClick={() => onReview(document)}>{canReview && active && !approved ? (en ? "Review and confirm" : "审查并确认本版") : (en ? "Read recorded document" : "阅读已登记文档")}<ArrowRight size={16}/></button>}
    </div>;
  const documentStrip = document ? <div className="art-document-strip"><FileText size={21}/><div><strong>{en ? `Review document · v${document.revision}` : `本版确认单 · 第 ${document.revision} 版`}</strong>
    <small>{approved ? (en ? "Confirmed" : "已确认") : canReview ? (en ? "Submitted for review" : "已提交审查") : (en ? "Recorded document; see guidance" : "已登记文档，请查看引导")}</small></div>
    <button className="text-button" onClick={() => onReview(document)}>{en ? "Read document" : "阅读确认单"}</button></div>
    : <p className="production-empty">{en ? "No confirmation document yet. These images are drafts." : "AI 尚未提交确认单，当前图片仅为草稿。"}</p>;
  return <section className="production-card art-workflow" aria-label={en ? "Art collaboration" : "美术方案协作区"}>
    <header className="art-workflow-heading"><div><Palette size={24}/><h4>{en ? "Art proposal · review together" : "美术方案 · 统一确认"}</h4></div>
      <div className="art-inline-actions"><button className="secondary-button" onClick={() => setReferences(true)}><ImagePlus size={16}/>{en ? "References (optional)" : "补充参考（可选）"} · {referenceCount}</button>
        <button className="secondary-button" onClick={() => setLibrary(true)}>{en ? "Art design library" : "美术设计库"}</button></div></header>
    <ol className="art-workflow-steps" aria-label={en ? "Art workflow" : "美术确认步骤"}>
      <li><ImagePlus size={17}/><span>{en ? "1 · References (optional)" : "1 · 补充参考（可选）"}</span></li>
      <li><Palette size={17}/><span>{en ? "2 · Read latest proposal" : "2 · 查看最新方案"}</span></li>
      <li><ClipboardCheck size={17}/><span>{en ? "3 · Recheck and confirm" : "3 · 复验并确认"}</span></li>
    </ol>
    {pane !== "proposal" && nextAction}
    <div className="art-workflow-tabs" role="tablist" aria-label={en ? "Art workspace panels" : "美术协作内容"}>
      <button role="tab" aria-selected={pane === "requirements"} onClick={() => setPane("requirements")}>{en ? "Requirements and references" : "要求与参考"} · {r.artDesign?.requirements.filter(q=>!q.withdrawn).length??0}</button>
      <button role="tab" aria-selected={pane === "proposal"} onClick={() => setPane("proposal")}><Palette size={16}/>{en ? "Current proposal" : "本版方案"}</button>
      <button role="tab" aria-selected={pane === "feedback"} onClick={() => setPane("feedback")}><ClipboardCheck size={16}/>{en ? "Changes and rechecks" : "修改与复验"} · {pending.length}</button>
    </div>
    {error && <p role="alert">{error}</p>}
    <div hidden={pane !== "requirements"} role="tabpanel" aria-label={en ? "Requirements and references" : "要求与参考"}>
      <ArtTracePanel key={taskId} taskId={taskId} view={view} active={active} en={en} onUpdated={onUpdated} onContinue={onContinue}/>
    </div>
    {pane === "proposal" && <div className="art-current-proposal" role="tabpanel" aria-label={en ? "Current proposal" : "本版方案"}>
      <div className="art-proposal-layout with-requirements"><div className="art-proposal-main">
      {current.length ? <><h5>{en ? "Latest visual revisions" : "本轮最新视觉方案"} · {current.length}</h5>
        {!submitted && <p className="art-blocker-note">{en ? "New images are not fully bound to the submitted document. Wait for AI to resubmit before approval." : "最新图片尚未全部关联到确认单，请等待 AI 更新并提交；此时不能确认新方案。"}</p>}
        <VisualGallery embedded taskId={taskId} artifacts={current} bindings={document?.artifactBindings} en={en}/>
      </> : <p className="production-empty">{en ? "No preview images registered. Reuse-only/no-art proposals follow the submitted document." : "暂无已登记的预览图。复用素材或无需新增美术的任务，以确认单说明为准。"}</p>}
      {active && feedbackTarget && <button className="secondary-button" onClick={() => { setSelectedArtifact(feedbackTarget.id); setPane("feedback"); }}>{en ? "Request visual changes" : "对本版图片提出修改意见"}</button>}
      </div><div className="art-proposal-side">{nextAction}{documentStrip}<ArtProposalContext view={view} en={en} pending={pending.length} onRequirements={() => setPane("requirements")} onFeedback={() => setPane("feedback")} onReview={() => { if (document) onReview(document); }}/></div></div>
      <details className="art-history" onToggle={e => setHistoryOpen(e.currentTarget.open)}><summary><History size={16}/>{en ? "History, drafts and files" : "历史版本、草稿与文件"} · {history.length + drafts.length}</summary>
        <p>{en ? "History is read-only and does not represent the current proposal." : "历史版本仅供对照，不代表当前待确认方案。"}</p>
        {historyOpen && <VisualGallery embedded taskId={taskId} artifacts={history} en={en}/>}
        {drafts.map(a => <div className="art-history-file" key={a.id}><span>{a.title}</span><button className="text-button" disabled={a.exists === false} onClick={() => void reveal(a.path)}>{en ? "Show file" : "查看文件"}</button></div>)}
        {document && <button className="text-button" onClick={() => void reveal(document.path)}>{en ? "Show confirmation document in folder" : "在文件夹中查看确认单"}</button>}
        <button className="text-button" onClick={() => void productionApi.revealReport(taskId,"plan").catch(e => setError(errorMessage(e)))}>{en ? "Show combined report" : "查看阶段汇总报告"}</button>
      </details>
    </div>}
    <div ref={feedbackArea} tabIndex={-1} hidden={pane !== "feedback"} role="tabpanel" aria-label={en ? "Changes and rechecks" : "修改与复验"}>
      <FeedbackPanel key={`art-feedback:${taskId}`} taskId={taskId} record={record} active={active} en={en} scope="art"
        selectedArtifact={selectedArtifact} onDismiss={() => setSelectedArtifact(undefined)} actions={view.actions?.feedback}
        recheckActions={view.actions?.recheck} onUpdated={onUpdated} onContinue={onContinue}
        onRequestRevision={document && canReview ? () => onReview(document) : undefined}/>
      {!pending.length && document && canReview && !approved && <button className="primary-button" disabled={!active} onClick={() => onReview(document)}><CheckCircle2 size={17}/>{en ? "Continue to proposal approval" : "反馈已处理，进入方案确认"}</button>}
    </div>
    {references && <Modal wide title={en ? "Add art references (optional)" : "补充美术参考（可选）"} onClose={() => setReferences(false)}>
      <ArtReferences taskId={taskId} record={record} active={active} en={en} onUpdated={onUpdated} onContinue={onContinue}/>
      <div className="modal-actions"><button className="secondary-button" onClick={() => setReferences(false)}>{en ? "Back to art workflow" : "返回美术协作区"}</button></div>
    </Modal>}
    {library&&<ArtLibraryPanel key={taskId} taskId={taskId} view={view} active={active} en={en} onClose={()=>setLibrary(false)} onContinue={onContinue} onChanged={() => { void productionApi.get(taskId).then(onUpdated).catch(e => setError(errorMessage(e))); }}/ >}
  </section>;
}
