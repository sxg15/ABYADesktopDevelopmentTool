import type { ProductionView, TaskControlState } from "../../shared/production";
import type { TerminalWorkflowSnapshot } from "../../shared/types";
import type { AttentionTarget, TaskAttentionSnapshot } from "../../shared/notifications";
import { stageName } from "./productionText";
import { activeProgrammerIssues, programmerIssue } from "./programmerIssue";

export type GuidanceTone = "waiting" | "human" | "terminal" | "error" | "neutral" | "programmer";
export interface GuidancePending { taskId: string; questions: number; reviews: number; terminal: number }
export interface GuidanceModel {
  key: string; tone: GuidanceTone; title: string; instruction: string; detail: string;
  destination: string; button: string; attention: boolean; stage: string; stageLabel: string;
  action: "terminal" | "question" | "review" | "stage" | "none";
  groupId?: string; reviewKind?: string; target?: AttentionTarget;
  pending: GuidancePending; additional: number;
}

export function taskGuidanceState(taskId: string, active: boolean, view: ProductionView | undefined,
  control: TaskControlState | undefined, workflow: TerminalWorkflowSnapshot | undefined, en: boolean,
  attention?: TaskAttentionSnapshot): GuidanceModel {
  const text = (zh: string, english: string) => en ? english : zh;
  const r = view?.record?.taskId === taskId ? view.record : undefined;
  const turn = workflow?.turns.find(t => t.id === workflow.currentTurnId);
  const observed = workflow?.taskId === taskId && workflow.observability === "native"
    && Date.now() - Date.parse(workflow.updatedAt) < 15000;
  const currentControl = !control?.taskId || control.taskId === taskId ? control : undefined;
  const state = currentControl?.state ?? (observed && turn ? ({inProgress:"running",waitingForPlan:"waitingForResponse",completed:"ready",failed:"failed",interrupted:"paused"}[turn.status]) : "unknown");
  const stage = r?.currentStage ?? "requirements";
  const programmerIssues = stage === "implementation" ? activeProgrammerIssues(r?.issues ?? []) : [];
  const stageLabel = r ? stageName(stage, view?.policy.stages.find(s => s.id === stage)?.name ?? stage, en ? "en-US" : undefined) : "";
  // Human decisions come exclusively from the native projection used by Windows.
  const all = active && attention?.taskId === taskId && !attention.unavailable ? attention.items : [];
  const conversation = currentControl?.conversationId ?? workflow?.conversationId;
  const items = all.filter(item => !["failed", "disconnected", "needsReview"].includes(item.kind)
    || !conversation || item.target.conversationId === conversation);
  const pending = {
    taskId, questions: all.filter(i => i.kind === "question").length,
    reviews: all.filter(i => i.kind === "review").length,
    terminal: all.filter(i => i.kind === "terminal").length,
  };
  const model: GuidanceModel = {
    key: "unknown", tone: "neutral", title: text("正在核对任务状态", "Checking task status"),
    instruction: text("暂时无法确认 AI 是否仍在执行。", "AI execution status is not yet confirmed."),
    detail: text("请先核对状态，避免重复提交已有操作。", "Check the current state before repeating an action."),
    destination: text("查看位置：终端", "Check in: Terminal"), button: text("查看终端状态", "Check terminal status"),
    attention: false, stage, stageLabel, action: "terminal", pending, additional: 0,
  };
  const set = (value: Partial<GuidanceModel>) => Object.assign(model, value);
  const primary = items[0];
  if (!active) set({key:"closed",title:text("任务已结束", "Task ended"),instruction:text("可以查看已保存的成果与历史记录。", "Saved results and history remain available."),detail:text("恢复任务后才能继续操作。", "Reactivate the task to continue."),action:"none",destination:""});
  else if (programmerIssues.length) set({key:`programmer:${programmerIssues.map(i=>i.id).join(",")}`,tone:"programmer",attention:true,action:"stage",
    title:text(programmerIssues.every(i=>programmerIssue(i)?.suspected) ? "疑似 ABYA 程序问题 · 需要排查" : "ABYA 程序缺陷 · 需要程序修复", "ABYA source/build issue · programmer required"),
    instruction:text(`Lua 执行阶段有 ${programmerIssues.length} 项源码／构建包问题需要程序处理。`, `${programmerIssues.length} source/build issues need programmer attention during Lua implementation.`),
    destination:text("处理位置：Lua 阶段的问题与阻塞", "Review in: Lua issues and blockers"),button:text("查看程序待修复问题", "View programmer issues"),
    detail:text("请程序排查或修复后提供更新的构建包，复验通过再继续受影响步骤。", "Investigate or fix, provide an updated build, and recheck before resuming affected steps.")});
  else if (primary) set({
    ...primary[en ? "en" : "zh"], key: primary.key, tone: primary.tone, attention: true,
    action: primary.target.action, target: primary.target, stage: primary.target.stage,
    stageLabel: stageName(primary.target.stage, primary.stageName, en ? "en-US" : undefined),
    groupId: primary.target.groupId ?? undefined,
    reviewKind: primary.kind === "review" ? primary.target.stage : undefined,
  });
  else if (view && ["running", "waitingForResponse", "queued"].includes(state)) set({
    key:state,tone:"waiting",title:text(state === "queued" ? "操作已保存，等待 AI 继续" : state === "waitingForResponse" ? "正在等待 AI 输出" : "AI 正在执行", state === "queued" ? "Saved · waiting for AI" : state === "waitingForResponse" ? "Waiting for AI output" : "AI is working"),
    instruction:text("当前无需操作，请等待结果。", "No action is needed. Please wait for the result."),
    destination:text("可选查看：终端进展", "Optional: Terminal progress"),button:text("查看执行进展", "View progress"),
    detail:text("需要你回答或审查时，这里会明确提醒；不必一直盯着终端。", "Questions and reviews will appear here; you do not need to watch the terminal."),
  });
  else if (state === "awaitingInput") set({key:state,title:text("当前执行已中断，等待新指令", "Execution interrupted · ready for input"),instruction:text("在原终端直接输入新指令即可。", "Type a new instruction in the original terminal."),detail:text("终端和历史保持不变，已排队的自动续接不会重新启动。", "The terminal and history are retained. Queued continuations will not restart."),button:text("前往终端输入", "Open terminal")});
  else if (state === "awaitingContinue") set({key:state,title:text("终端已连接，任务等待继续", "Terminal connected · ready to continue"),instruction:text("请前往原会话终端，确认后点击“继续任务”。", "Open the original terminal and choose Continue task when ready."),detail:text("重新连接不会自动重发任务，已有答案与审查记录保留。", "Reconnecting does not resend work. Answers and reviews are retained."),button:text("前往终端继续", "Continue in terminal")});
  else if (["paused", "pausing"].includes(state)) set({key:state,title:text(state === "pausing" ? "正在暂停任务" : "任务已暂停", state === "pausing" ? "Pausing task" : "Task paused"),instruction:text("需要继续时，请前往原会话终端。", "Open the original terminal when ready to continue."),detail:text("答案与历史记录已保留，不会自动重新提交。", "Answers and history are retained and will not be resubmitted."),button:text("前往终端", "Open terminal")});
  else if (view && ["ready", "notStarted"].includes(state)) set({key:state,title:text(state === "notStarted" ? "任务尚未开始执行" : "本轮回复已结束", state === "notStarted" ? "Task not started" : "This response has ended"),instruction:text("AI 当前未在执行，请前往终端继续或输入需求。", "AI is idle. Open the terminal to continue or provide a request."),detail:text("如果出现新的阶段待办，这里会显示明确入口。", "New stage actions will appear here."),button:text("前往终端继续", "Continue in terminal")});
  model.additional = Math.max(0, items.length - (primary && model.tone !== "programmer" ? 1 : 0));
  return model;
}
