import { useEffect, useState } from "react";
import { productionApi, type ProductionView, type TaskControlState } from "../../shared/production";
import type { TerminalWorkflowSnapshot } from "../../shared/types";

export function TaskGuidance({ taskId, active, control, workflow, en = false, onStage, onTerminal }: {
  taskId: string; active: boolean; control?: TaskControlState; workflow?: TerminalWorkflowSnapshot; en?: boolean;
  onStage: (stage: string) => void; onTerminal: () => void;
}) {
  const [view, setView] = useState<ProductionView>();
  useEffect(() => {
    let alive = true, sequence = 0;
    const refresh = async () => {
      const request = ++sequence;
      try { const next = await productionApi.get(taskId); if (alive && request === sequence) setView(next); }
      catch { if (alive && request === sequence) setView(undefined); }
    };
    void refresh(); const timer = window.setInterval(() => void refresh(), 3000);
    window.addEventListener("abya:control-changed", refresh);
    return () => { alive = false; window.clearInterval(timer); window.removeEventListener("abya:control-changed", refresh); };
  }, [taskId]);
  const r = view?.record;
  const pending = r?.questionGroups?.find(g => g.status === "pending");
  const review = Object.keys(view?.actions?.review ?? {}).find(k => view?.actions?.review[k] === "");
  const turn = workflow?.turns.find(t => t.id === workflow.currentTurnId);
  const observable = workflow?.observability === "native" && Date.now() - Date.parse(workflow.updatedAt) < 15000;
  const state = control?.state ?? (observable && turn ? ({ inProgress: "running", waitingForPlan: "waitingForResponse", completed: "ready", failed: "failed", interrupted: "paused" }[turn.status]) : "unknown");
  let message = en ? "Checking execution status. It is not yet clear whether AI is running." : "正在核对执行状态，暂时无法确认 AI 是否仍在运行。";
  let label = en ? "Check terminal" : "查看终端", action = onTerminal, attention = false;
  if (!active) { message = en ? "Task completed or archived. History is available." : "任务已完成或归档，可以查看历史记录。"; }
  else if (state === "waitingForTerminal") {
    message = en ? "The terminal needs your input or approval. Open it to respond." : "终端有一项输入或确认需要你处理，请前往终端。";
    label = en ? "Go to terminal" : "前往终端"; attention = true;
  } else if (pending) {
    message = en ? `${pending.questions.length} questions need your answers in the workflow page.` : `需要你回答 ${pending.questions.length} 个问题，请在阶段问答中完成。`;
    label = en ? "Answer questions" : "开始回答"; attention = true;
    action = () => window.dispatchEvent(new CustomEvent("abya:open-intake", { detail: { taskId, groupId: pending.id } }));
  } else if (review) {
    message = en ? "A document is ready for your review in the workflow page." : `${view?.policy.stages.find(s => s.id === review)?.name}文档已就绪，请在流程页面阅读并给出结论。`;
    label = en ? "Go to review" : "前往审查"; action = () => onStage(review); attention = true;
  } else if (["running", "waitingForResponse", "queued"].includes(state)) {
    message = state === "queued" ? (en ? "Your action is saved. Waiting for AI to continue; do not resubmit." : "操作已保存，正在等待 AI 继续，无需重复提交。")
      : state === "waitingForResponse" ? (en ? "Waiting for AI output. No action is needed." : "正在等待 AI 输出，当前无需操作。")
      : (en ? "AI is working. No action is needed; questions and reviews will appear here." : "AI 正在执行，请等待。当前无需操作；需要你回答或审查时会在此提示。");
    label = en ? "View terminal progress" : "查看终端进展";
  } else if (["paused", "pausing", "failed", "disconnected", "needsReview"].includes(state)) {
    const labels: Record<string,string> = { paused:"任务已暂停", pausing:"正在暂停任务", failed:"本轮执行失败", disconnected:"连接已断开", needsReview:"执行结果需要核对" };
    message = en ? `Execution status: ${state}. Check the terminal before continuing.` : `${labels[state]}。请前往终端核对状态并处理。`; attention = true;
  } else if (r && view?.actions?.feedback.gameplay === "" && ["review", "delivery"].includes(r.currentStage)) {
    message = en ? "A playable version is available. Play it, then submit feedback in the workflow page." : "当前作品已可验收，请试玩后在流程页面提交反馈。";
    label = en ? "Go to playtest" : "前往试玩与反馈"; action = () => onStage(r.currentStage); attention = true;
  } else if (state === "ready" || state === "notStarted") {
    message = en ? "AI is not currently executing. Open the terminal to continue or provide a request." : "AI 当前未在执行。请前往终端继续当前阶段或输入需求。";
  } else if (state === "blocked" || state === "waitingForApproval") {
    message = en ? "This stage needs attention. Check the workflow details." : "当前阶段有待核对事项，请查看流程中的具体原因。";
    label = en ? "View stage" : "查看阶段详情"; action = () => onStage(r?.currentStage ?? "requirements"); attention = true;
  }
  return <section className={`task-guidance ${attention ? "needs-attention" : ""}`} aria-label={en ? "What to do next" : "当前操作指引"}>
    <span aria-hidden="true">{attention ? "!" : "◷"}</span><p role="status">{message}</p>
    {active && <button className={attention ? "primary-button" : "secondary-button"} onClick={action}>{label}</button>}
  </section>;
}
