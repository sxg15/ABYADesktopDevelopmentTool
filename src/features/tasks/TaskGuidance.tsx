import { useEffect, useRef, useState } from "react";
import { ArrowRight, CircleHelp, ClipboardCheck, Info, LoaderCircle, MapPin, MessageCircleQuestion, Terminal, Unplug, TriangleAlert } from "lucide-react";
import { productionApi, type ProductionDocument, type ProductionView, type TaskControlState } from "../../shared/production";
import type { TerminalWorkflowSnapshot } from "../../shared/types";
import { taskGuidanceState, type GuidancePending } from "./taskGuidanceState";
import "./taskGuidance.css";
import type { AttentionTarget } from "../../shared/notifications";
import { useTaskAttention } from "../../shared/useTaskAttention";
import { ProgrammerIssueExport } from "./ProgrammerIssueExport";
import { programmerIssue } from "./programmerIssue";
import { isArtFeedback } from "./artWorkflow";

export function TaskGuidance({ taskId, active, control, workflow, en = false, onStage, onTerminal, onReview, onPendingChange }: {
  taskId: string; active: boolean; control?: TaskControlState; workflow?: TerminalWorkflowSnapshot; en?: boolean;
  onStage: (stage: string, feedbackId?: string) => void; onTerminal: (target?: AttentionTarget) => void;
  onReview?: (document: ProductionDocument) => void; onPendingChange?: (pending: GuidancePending) => void;
}) {
  const [view, setView] = useState<ProductionView>();
  const attention = useTaskAttention(taskId);
  useEffect(() => {
    let alive = true, sequence = 0;
    setView(undefined);
    const refresh = async () => {
      const request = ++sequence;
      try {
        const next = await productionApi.get(taskId);
        if (alive && request === sequence) setView(next);
      } catch { if (alive && request === sequence) setView(undefined); }
    };
    void refresh(); const timer = window.setInterval(() => void refresh(), 3000);
    window.addEventListener("abya:control-changed", refresh);
    return () => { alive = false; window.clearInterval(timer); window.removeEventListener("abya:control-changed", refresh); };
  }, [taskId]);
  const model = taskGuidanceState(taskId, active, view, control, workflow, en, view ? attention : undefined);
  const artPending = model.action === "review" && model.stage === "resources" ? (view?.record?.feedback ?? []).filter(f => isArtFeedback(f) && f.status !== "resolved").length : 0;
  const { questions, reviews, terminal } = model.pending;
  useEffect(() => { onPendingChange?.({taskId, questions, reviews, terminal}); }, [taskId, questions, reviews, terminal, onPendingChange]);
  const previousKey = useRef<string | undefined>(undefined);
  const [emphasize, setEmphasize] = useState(false);
  useEffect(() => {
    const changed = !!previousKey.current && previousKey.current !== model.key && model.attention;
    previousKey.current = model.key; setEmphasize(changed);
    if (!changed) return;
    const timer = window.setTimeout(() => setEmphasize(false), 1000);
    return () => window.clearTimeout(timer);
  }, [model.key, model.attention]);
  function act() {
    if (model.action === "recheck") { onStage("resources", model.target?.feedbackId ?? undefined); return; }
    if (artPending) { onStage("resources"); return; }
    if (model.action === "question") window.dispatchEvent(new CustomEvent("abya:open-intake", {detail:{taskId,groupId:model.groupId}}));
    else if (model.action === "review") {
      const document = view?.record?.documents[model.reviewKind!];
      if (document && document.revision === model.target?.documentRevision
        && document.sha256 === model.target.documentHash && onReview) onReview(document);
      else onStage(model.stage);
    } else if (model.action === "stage") onStage(model.stage);
    else if (model.action === "terminal") onTerminal(model.target);
  }
  const Icon = model.tone === "programmer" ? TriangleAlert : model.tone === "waiting" ? LoaderCircle : model.tone === "terminal" ? Terminal
    : model.tone === "error" ? Unplug : model.action === "question" ? MessageCircleQuestion
      : model.tone === "human" ? ClipboardCheck : CircleHelp;
  return <div className="task-guidance-shell"><section className={`task-guidance tone-${model.tone}${emphasize ? " guidance-emphasis" : ""}`} aria-label={en ? "What to do next" : "当前操作指引"}>
    <div className="guidance-main" role="status" aria-live="polite" aria-atomic="true">
      <span className="guidance-icon"><Icon aria-hidden="true" size={26} /></span>
      <div className="guidance-copy"><div className="guidance-eyebrow">
        <strong>{model.tone === "programmer" ? (en ? "Programmer action required" : "需要程序处理") : model.attention ? (en ? "Your action is needed" : "需要你的操作") : model.tone === "waiting" ? (en ? "No action needed" : "当前无需操作") : (en ? "Task status" : "任务状态")}</strong>
        {model.stageLabel && <span>{model.stageLabel}</span>}
      </div><h2>{artPending ? (en ? "Art confirmation needs feedback rechecks" : "美术确认前，还需处理修改反馈") : model.title}</h2><p>{artPending ? (en ? `${artPending} open feedback items. Follow the art workspace guidance before approving.` : `有 ${artPending} 条反馈未关闭，请按美术协作区的引导处理后再确认。`) : model.instruction}</p></div>
    </div>
    <div className="guidance-action">
      {model.destination && <span className="guidance-destination"><MapPin size={15} aria-hidden="true" />{model.destination}</span>}
      {model.action !== "none" && <button className={model.attention ? "guidance-primary" : "guidance-secondary"} onClick={act}>{artPending ? (en ? "Open art workspace" : "前往美术协作区") : model.button}<ArrowRight size={17} aria-hidden="true" /></button>}
      {model.tone === "programmer" && view?.record?.taskId === taskId && <ProgrammerIssueExport taskId={taskId} revision={view.record.revision} issueIds={view.record.issues.filter(i => i.status !== "resolved" && programmerIssue(i)).map(i => i.id)} en={en}/>}
    </div>
    <div className="guidance-detail"><Info size={15} aria-hidden="true" /><span>{model.detail}</span>
      {model.additional > 0 && <button className="text-button" onClick={() => onStage(model.stage)}>{en ? `${model.additional} more pending` : `另有 ${model.additional} 项待办`}</button>}
    </div>
  </section></div>;
}
