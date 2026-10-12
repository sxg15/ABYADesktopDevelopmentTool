import { useEffect, useRef, useState } from "react";
import { ArrowRight, CircleHelp, ClipboardCheck, Info, LoaderCircle, MapPin, MessageCircleQuestion, Terminal, Unplug } from "lucide-react";
import { productionApi, type ProductionDocument, type ProductionView, type TaskControlState } from "../../shared/production";
import type { TerminalWorkflowSnapshot } from "../../shared/types";
import { taskGuidanceState, type GuidancePending } from "./taskGuidanceState";
import "./taskGuidance.css";

export function TaskGuidance({ taskId, active, control, workflow, en = false, onStage, onTerminal, onReview, onPendingChange }: {
  taskId: string; active: boolean; control?: TaskControlState; workflow?: TerminalWorkflowSnapshot; en?: boolean;
  onStage: (stage: string) => void; onTerminal: () => void;
  onReview?: (document: ProductionDocument) => void; onPendingChange?: (pending: GuidancePending) => void;
}) {
  const [view, setView] = useState<ProductionView>();
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
  const model = taskGuidanceState(taskId, active, view, control, workflow, en);
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
    if (model.action === "question") window.dispatchEvent(new CustomEvent("abya:open-intake", {detail:{taskId,groupId:model.groupId}}));
    else if (model.action === "review") {
      const document = view?.record?.documents[model.reviewKind!];
      if (document && onReview) onReview(document); else onStage(model.stage);
    } else if (model.action === "stage") onStage(model.stage);
    else if (model.action === "terminal") onTerminal();
  }
  const Icon = model.tone === "waiting" ? LoaderCircle : model.tone === "terminal" ? Terminal
    : model.tone === "error" ? Unplug : model.action === "question" ? MessageCircleQuestion
      : model.tone === "human" ? ClipboardCheck : CircleHelp;
  return <div className="task-guidance-shell"><section className={`task-guidance tone-${model.tone}${emphasize ? " guidance-emphasis" : ""}`} aria-label={en ? "What to do next" : "当前操作指引"}>
    <div className="guidance-main" role="status" aria-live="polite" aria-atomic="true">
      <span className="guidance-icon"><Icon aria-hidden="true" size={26} /></span>
      <div className="guidance-copy"><div className="guidance-eyebrow">
        <strong>{model.attention ? (en ? "Your action is needed" : "需要你的操作") : model.tone === "waiting" ? (en ? "No action needed" : "当前无需操作") : (en ? "Task status" : "任务状态")}</strong>
        {model.stageLabel && <span>{model.stageLabel}</span>}
      </div><h2>{model.title}</h2><p>{model.instruction}</p></div>
    </div>
    <div className="guidance-action">
      {model.destination && <span className="guidance-destination"><MapPin size={15} aria-hidden="true" />{model.destination}</span>}
      {model.action !== "none" && <button className={model.attention ? "guidance-primary" : "guidance-secondary"} onClick={act}>{model.button}<ArrowRight size={17} aria-hidden="true" /></button>}
    </div>
    <div className="guidance-detail"><Info size={15} aria-hidden="true" /><span>{model.detail}</span>
      {model.additional > 0 && <button className="text-button" onClick={() => onStage(model.stage)}>{en ? `${model.additional} more pending` : `另有 ${model.additional} 项待办`}</button>}
    </div>
  </section></div>;
}
