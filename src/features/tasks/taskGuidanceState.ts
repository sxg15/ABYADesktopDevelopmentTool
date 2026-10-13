import type { ProductionView, TaskControlState } from "../../shared/production";
import type { TerminalWorkflowSnapshot } from "../../shared/types";
import { stageName } from "./productionText";

export type GuidanceTone = "waiting" | "human" | "terminal" | "error" | "neutral";
export interface GuidancePending { taskId: string; questions: number; reviews: number; terminal: number }
export interface GuidanceModel {
  key: string; tone: GuidanceTone; title: string; instruction: string; detail: string;
  destination: string; button: string; attention: boolean; stage: string; stageLabel: string;
  action: "terminal" | "question" | "review" | "stage" | "none";
  groupId?: string; reviewKind?: string; pending: GuidancePending; additional: number;
}

export function taskGuidanceState(taskId: string, active: boolean, view: ProductionView | undefined,
  control: TaskControlState | undefined, workflow: TerminalWorkflowSnapshot | undefined, en: boolean): GuidanceModel {
  const text = (zh: string, english: string) => en ? english : zh;
  const r = view?.record?.taskId === taskId ? view.record : undefined;
  const groups = r?.questionGroups?.filter(g => g.status === "pending") ?? [];
  const reviews = r ? (view?.policy.stages ?? []).filter(s => view?.actions?.review[s.id] === "" && r.documents[s.id]) : [];
  const turn = workflow?.turns.find(t => t.id === workflow.currentTurnId);
  const observed = workflow?.taskId === taskId && workflow.observability === "native"
    && Date.now() - Date.parse(workflow.updatedAt) < 15000;
  const currentControl = !control?.taskId || control.taskId === taskId ? control : undefined;
  const state = currentControl?.state ?? (observed && turn ? ({inProgress:"running",waitingForPlan:"waitingForResponse",completed:"ready",failed:"failed",interrupted:"paused"}[turn.status]) : "unknown");
  const stage = r?.currentStage ?? "requirements";
  const stageLabel = r ? stageName(stage, view?.policy.stages.find(s => s.id === stage)?.name ?? stage, en ? "en-US" : undefined) : "";
  const pending = { taskId, questions: active ? groups.length : 0, reviews: active ? reviews.length : 0, terminal: active && state === "waitingForTerminal" ? 1 : 0 };
  const model: GuidanceModel = {
    key: "unknown", tone: "neutral", title: text("正在核对任务状态", "Checking task status"),
    instruction: text("暂时无法确认 AI 是否仍在执行。", "AI execution status is not yet confirmed."),
    detail: text("请先核对状态，避免重复提交已有操作。", "Check the current state before repeating an action."),
    destination: text("查看位置：终端", "Check in: Terminal"), button: text("查看终端状态", "Check terminal status"),
    attention: false, stage, stageLabel, action: "terminal", pending, additional: 0,
  };
  const set = (value: Partial<GuidanceModel>) => Object.assign(model, value);
  if (!active) set({key:"closed",title:text("任务已结束", "Task ended"),instruction:text("可以查看已保存的成果与历史记录。", "Saved results and history remain available."),detail:text("恢复任务后才能继续操作。", "Reactivate the task to continue."),action:"none",destination:""});
  else if (state === "waitingForTerminal") set({
    key:"terminal",tone:"terminal",attention:true,title:text("终端需要你处理", "Your input is needed in the terminal"),
    instruction:text("一项输入或确认正在等待你的处理。", "An input or approval request needs your response."),
    destination:text("处理位置：原会话终端", "Respond in: Original terminal"),button:text("前往终端处理", "Open terminal request"),
    detail:text("请在终端阅读当前请求；处理完成后，AI 才能继续执行。", "Read the terminal request and respond so AI can continue."),
  });
  else if (groups.length) set({
    key:`question:${groups[0].id}`,tone:"human",attention:true,action:"question",groupId:groups[0].id,
    title:text("需要你回答阶段问题", "Stage questions need your answers"),
    instruction:text(`需要你回答 ${groups[0].questions.length} 个问题，提交后 AI 将继续处理。`, `Answer ${groups[0].questions.length} questions so AI can continue.`),
    destination:text("处理位置：阶段问答", "Respond in: Stage questions"),button:text("开始回答", "Answer questions"),
    detail:text("答案在 APP 内提交，无需到终端重复回复。", "Submit answers here; no duplicate reply in the terminal is needed."),
  });
  else if (reviews.length) {
    const kind = reviews[0].id, doc = r!.documents[kind];
    set({key:`review:${kind}:${doc.revision}:${doc.sha256}`,tone:"human",attention:true,action:"review",reviewKind:kind,stage:kind,
      title:text(kind === "resources" ? "需要你审查美术需求" : "需要你审查阶段成果", "Stage results need your review"),
      instruction:text(`文档第 ${doc.revision} 版已提交，请阅读后给出结论。`, `Version ${doc.revision} is submitted. Read it and choose a decision.`),
      destination:text("处理位置：制作流程", "Review in: Production"),button:text("查看并审查", "Read and review"),
      detail:text("AI 正在等待你的审查结果；无需到终端重复回复。", "AI is waiting for your review; no duplicate terminal reply is needed."),
      stageLabel:stageName(kind,reviews[0].name,en ? "en-US" : undefined),
    });
  } else if (view && ["running", "waitingForResponse", "queued"].includes(state)) set({
    key:state,tone:"waiting",title:text(state === "queued" ? "操作已保存，等待 AI 继续" : state === "waitingForResponse" ? "正在等待 AI 输出" : "AI 正在执行", state === "queued" ? "Saved · waiting for AI" : state === "waitingForResponse" ? "Waiting for AI output" : "AI is working"),
    instruction:text("当前无需操作，请等待结果。", "No action is needed. Please wait for the result."),
    destination:text("可选查看：终端进展", "Optional: Terminal progress"),button:text("查看执行进展", "View progress"),
    detail:text("需要你回答或审查时，这里会明确提醒；不必一直盯着终端。", "Questions and reviews will appear here; you do not need to watch the terminal."),
  });
  else if (state === "awaitingContinue") set({key:state,title:text("终端已连接，任务等待继续", "Terminal connected · ready to continue"),instruction:text("请前往原会话终端，确认后点击“继续任务”。", "Open the original terminal and choose Continue task when ready."),detail:text("重新连接不会自动重发任务，已有答案与审查记录保留。", "Reconnecting does not resend work. Answers and reviews are retained."),button:text("前往终端继续", "Continue in terminal")});
  else if (["failed", "disconnected", "needsReview"].includes(state)) {
    const titles: Record<string, string> = {failed:text("本轮执行遇到错误", "This run encountered an error"),disconnected:text("原对话需要接回", "Reconnect the original conversation"),needsReview:text("执行结果需要核对", "Execution needs review")};
    set({key:state,tone:"error",attention:true,title:titles[state],instruction:text("请先核对原会话状态，再决定如何继续。", "Check the original conversation before continuing."),destination:text("处理位置：终端", "Check in: Terminal"),button:text(state === "disconnected" ? "查看连接状态" : "查看原因", "Check details"),detail:text("已有答案与阶段记录保留，请勿重复提交。", "Answers and stage records are saved; do not resubmit them.")});
  } else if (["paused", "pausing"].includes(state)) set({key:state,title:text(state === "pausing" ? "正在暂停任务" : "任务已暂停", state === "pausing" ? "Pausing task" : "Task paused"),instruction:text("需要继续时，请前往原会话终端。", "Open the original terminal when ready to continue."),detail:text("答案与历史记录已保留，不会自动重新提交。", "Answers and history are retained and will not be resubmitted."),button:text("前往终端", "Open terminal")});
  else if (r && view?.actions?.feedback.gameplay === "" && ["review", "delivery"].includes(stage)) set({key:`playtest:${r.currentVersion}`,tone:"human",attention:true,action:"stage",title:text("需要你试玩并反馈", "Ready for your playtest"),instruction:text("当前作品已可验收，请试玩后提交反馈。", "Play the current version and submit feedback."),detail:text("请核对实际体验，满意后再确认最终交付。", "Verify the experience before accepting the final delivery."),destination:text("处理位置：制作流程", "Respond in: Production"),button:text("前往试玩与反馈", "Open playtest and feedback")});
  else if (view && ["ready", "notStarted"].includes(state)) set({key:state,title:text(state === "notStarted" ? "任务尚未开始执行" : "本轮回复已结束", state === "notStarted" ? "Task not started" : "This response has ended"),instruction:text("AI 当前未在执行，请前往终端继续或输入需求。", "AI is idle. Open the terminal to continue or provide a request."),detail:text("如果出现新的阶段待办，这里会显示明确入口。", "New stage actions will appear here."),button:text("前往终端继续", "Continue in terminal")});
  else if (view && ["blocked", "waitingForApproval"].includes(state)) set({key:state,tone:"human",attention:true,action:"stage",title:text("当前阶段需要核对", "Check this stage"),instruction:text("请查看成果和阻塞说明，确认下一步处理方式。", "Check results and blockers for the next action."),detail:text("尚未满足审查条件的成果不会开放确认。", "Approval remains unavailable until review requirements are met."),destination:text("处理位置：制作流程", "Check in: Production"),button:text("查看阶段详情", "View stage details")});
  const total = pending.questions + pending.reviews + pending.terminal;
  model.additional = Math.max(0, total - (["question", "review"].includes(model.action) || model.key === "terminal" ? 1 : 0));
  return model;
}
