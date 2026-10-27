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
  action: "terminal" | "question" | "review" | "recheck" | "stage" | "none";
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
  const autonomous = stage === "implementation" && view?.policy.implementationExecution?.mode === "autonomous";
  const programmerIssues = stage === "implementation" && !autonomous && !view?.availableUpdate ? activeProgrammerIssues(r?.issues ?? []) : [];
  const stageLabel = r ? stageName(stage, view?.policy.stages.find(s => s.id === stage)?.name ?? stage, en ? "en-US" : undefined) : "";
  // Human decisions come exclusively from the native projection used by Windows.
  const all = active && attention?.taskId === taskId && !attention.unavailable ? attention.items : [];
  const conversation = currentControl?.conversationId ?? workflow?.conversationId;
  const items = all.filter(item => !["failed", "disconnected", "needsReview"].includes(item.kind)
    || !conversation || item.target.conversationId === conversation);
  const pending = {
    taskId, questions: all.filter(i => i.kind === "question").length,
    reviews: all.filter(i => i.kind === "review" || i.kind === "recheck").length,
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
  else if (["repairRequired","externalBlocked"].includes(state)) set({key:state,tone:"neutral",action:"stage",attention:true,
    title:text(state==="repairRequired"?"尚未形成可试玩包":"存在需要处理的外部条件",state==="repairRequired"?"No playable package yet":"External condition required"),
    instruction:text("已有代码、验证记录和历史版本均保留，当前实现尚未成功结束。","Code, evidence and prior versions are retained; implementation has not completed successfully."),
    detail:text("进入当前子流程核对具体缺口，可诊断并恢复原目标；不会重置预算或重放未知写入。","Review the gap and diagnose the original goal without resetting budgets or replaying uncertain writes."),
    destination:text("处理位置：Lua 实现","Open in: Implementation"),button:text("查看修复与恢复","Review recovery")});
  else if (view && ["running", "waitingForResponse", "queued"].includes(state)) set({
    key:state,tone:"waiting",title:text(state === "queued" ? "操作已保存，等待 AI 继续" : state === "waitingForResponse" ? "正在等待 AI 输出" : "AI 正在执行", state === "queued" ? "Saved · waiting for AI" : state === "waitingForResponse" ? "Waiting for AI output" : "AI is working"),
    instruction:text("当前无需操作，请等待结果。", "No action is needed. Please wait for the result."),
    destination:text("可选查看：终端进展", "Optional: Terminal progress"),button:text("查看执行进展", "View progress"),
    detail:text("需要你回答或审查时，这里会明确提醒；不必一直盯着终端。", "Questions and reviews will appear here; you do not need to watch the terminal."),
  });
  else if (state === "stopFailed") set({key:state,tone:"error",attention:true,title:text("停止失败，执行可能仍在继续", "Stop failed · execution may continue"),instruction:text("请前往原终端重试停止，或使用强制停止此会话。", "Open the original terminal to retry or force stop this conversation."),detail:currentControl?.lastError??"",button:text("前往终端处理", "Open terminal")});
  else if (state === "workSettled") set({key:state,action:"stage",destination:text("查看位置：制作流程", "Open in: Workflow"),title:text("当前可执行工作已收敛", "Executable work has converged"),instruction:text("已有产出与待补能力已集中记录，无需反复发送继续。", "Results and remaining dependencies are recorded together."),detail:text("收敛不代表全部验收通过，未完成项仍然可见。", "Convergence is separate from acceptance."),button:text("查看结果与缺口", "View results and gaps")});
  else if (state === "interrupted") set({key:state,action:"stage",destination:text("处理位置：制作流程 · 当前子流程", "Open in: Workflow · current subprocess"),title:text("子流程被中断", "Subprocess interrupted"),instruction:text("进入原子流程，点击‘继续此子流程’恢复。", "Open the subprocess and resume it."),detail:text("保留原 ID、成果和执行历史。", "Original identity, results and history are retained."),button:text("查看原子流程", "Open subprocess")});
  else if (state === "awaitingInput") set({key:state,title:text("当前执行已中断，等待新指令", "Execution interrupted · ready for input"),instruction:text("在原终端直接输入新指令即可。", "Type a new instruction in the original terminal."),detail:text("终端和历史保持不变，已排队的自动续接不会重新启动。", "The terminal and history are retained. Queued continuations will not restart."),button:text("前往终端输入", "Open terminal")});
  else if (state === "awaitingContinue") set({key:state,title:text("终端已连接，任务等待继续", "Terminal connected · ready to continue"),instruction:text("请前往原会话终端，确认后点击“继续任务”。", "Open the original terminal and choose Continue task when ready."),detail:text("重新连接不会自动重发任务，已有答案与审查记录保留。", "Reconnecting does not resend work. Answers and reviews are retained."),button:text("前往终端继续", "Continue in terminal")});
  else if (["paused", "pausing"].includes(state)) set({key:state,title:text(state === "pausing" ? "正在暂停任务" : "任务已暂停", state === "pausing" ? "Pausing task" : "Task paused"),instruction:text("需要继续时，请前往原会话终端。", "Open the original terminal when ready to continue."),detail:text("答案与历史记录已保留，不会自动重新提交。", "Answers and history are retained and will not be resubmitted."),button:text("前往终端", "Open terminal")});
  else if (view && ["ready", "notStarted"].includes(state)) set({key:state,title:text(state === "notStarted" ? "任务尚未开始执行" : "本轮回复已结束", state === "notStarted" ? "Task not started" : "This response has ended"),instruction:text("AI 当前未在执行，请前往终端继续或输入需求。", "AI is idle. Open the terminal to continue or provide a request."),detail:text("如果出现新的阶段待办，这里会显示明确入口。", "New stage actions will appear here."),button:text("前往终端继续", "Continue in terminal")});
  if(currentControl?.autonomyNotice && !["running","waitingForResponse","queued","pausing","paused","stopFailed"].includes(state)) {
    model.detail=en?"Native permissions may require manual confirmation. Check execution settings before autonomous implementation; permissions are never changed automatically.":currentControl.autonomyNotice;
    if(!primary&&!["workSettled","closed"].includes(state)){model.title=text("自主执行前核对原生权限", "Check native permissions before autonomous execution");model.button=text("前往终端执行设置", "Open terminal settings");model.action="terminal";model.destination=text("处理位置：终端 · 执行设置", "Open in: Terminal · execution settings");}
  }
  model.additional = Math.max(0, items.length - (primary && model.tone !== "programmer" ? 1 : 0));
  if(active&&!primary&&r?.workbench?.execution?.preflight?.stage===stage&&r.workbench.execution.preflight.state==="exhausted"
    && !["running","waitingForResponse","queued","paused","pausing","stopFailed"].includes(state)) {
    set({key:"preflight-exhausted",tone:"neutral",action:"stage",attention:true,
      title:text("技术预检恢复未成功", "Technical preflight recovery ended"),
      instruction:text("有限恢复已结束，原成果与问题已保留。", "Bounded recovery ended; results and issues are preserved."),
      detail:text("查看具体恢复条件或补充新信息，无需反复点击继续。", "Review the required conditions or provide new information."),
      destination:text("查看位置：当前阶段的问题", "Open in: Current stage issues"),button:text("查看预检问题", "View preflight issues")});
  }
  if(active&&stage==="implementation"&&!autonomous&&view?.availableUpdate&&!primary&&!["running","waitingForResponse","queued","pausing","stopFailed"].includes(state)) {
    set({key:"workflow-upgrade",tone:"neutral",action:"stage",attention:true,
      title:text("本任务仍使用旧执行规则", "This task still uses legacy execution rules"),
      instruction:text(`本任务流程 ${r?.workflowVersion} 尚未启用 Lua 自主执行；更新 APP 不会自动迁移任务。`, `Task workflow ${r?.workflowVersion} has not enabled autonomous execution.`),
      detail:text("先暂停本任务，再从制作流程顶部升级；原 ID、成果和批准记录保留。", "Pause, then upgrade from the workflow toolbar. Identity, results and approvals are preserved."),
      destination:text("处理位置：制作流程顶部", "Open in: Workflow toolbar"),button:text("查看流程升级", "View workflow upgrade")});
  }
  return model;
}
