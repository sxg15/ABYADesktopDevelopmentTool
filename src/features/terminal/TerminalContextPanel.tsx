import { ArrowRight, BookOpen, Check, Circle, CircleAlert, ListTree, X } from "lucide-react";
import type { MessageKey } from "../../i18n";
import type { CodexWorkflowSnapshot, CodexPlanStepStatus } from "../../shared/types";
import type { TaskControlState } from "../../shared/production";
import "./terminalEnhancements.css";

export function TerminalContextPanel({ workflow, control, t, onProduction, onArtLibrary, onClose }: {
  workflow?: CodexWorkflowSnapshot; control?: TaskControlState; t: (key: MessageKey) => string;
  onProduction: () => void; onArtLibrary: () => void; onClose: () => void;
}) {
  const turn = workflow?.turns.find(item => item.id === workflow.currentTurnId) ?? workflow?.turns[workflow.turns.length - 1];
  const historical = turn && (control ? !["running", "waitingForResponse", "waitingForTerminal", "queued"].includes(control.state)
    : ["completed", "failed", "interrupted"].includes(turn.status));
  const keys: Record<CodexPlanStepStatus, MessageKey> = { pending: "stepPending", inProgress: "stepInProgress", completed: "stepCompleted", failed: "stepFailed", unplanned: "stepUnplanned" };
  return <aside className="terminal-context-panel" aria-label={t("terminalContext")}>
    <header><div><h3><ListTree size={16}/>{t("terminalContext")}</h3><p>{t(historical ? "terminalContextPast" : "terminalContextHint")}</p></div><button className="icon-button" title={t("terminalContextClose")} onClick={onClose}><X size={15}/></button></header>
    <div className="terminal-context-body">
      {workflow?.observability === "compatibility" && <p className="terminal-context-warning">{t("terminalContextUnobservable")}</p>}
      {!turn?.plan.length && <p className="terminal-context-empty">{t("terminalContextEmpty")}</p>}
      <ol>{turn?.plan.map(step => <li key={step.id} className={`context-step-${step.status}`}>
        <span className="context-step-symbol">{step.status === "completed" ? <Check size={15}/> : step.status === "failed" ? <CircleAlert size={15}/> : <Circle size={12}/>}</span>
        <div><strong>{step.step}</strong><small>{t(keys[step.status])}</small></div>
      </li>)}</ol>
      {!!turn?.plan.length && turn.plan.every(step => step.status === "completed") && <p className="terminal-context-empty">{t("terminalContextPlanDone")}</p>}
      {turn?.explanation && <p className="terminal-context-explanation">{turn.explanation}</p>}
    </div>
    <footer><button className="primary-button" onClick={onProduction}>{t("terminalViewProduction")}<ArrowRight size={15}/></button><button className="secondary-button" onClick={onArtLibrary}><BookOpen size={15}/>{t("terminalViewArtLibrary")}</button></footer>
  </aside>;
}
