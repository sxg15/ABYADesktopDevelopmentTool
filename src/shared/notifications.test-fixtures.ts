import type { TaskAttention, TaskAttentionSnapshot } from "./notifications";

export function attentionFixture(kind: TaskAttention["kind"] = "review", taskId = "task"): TaskAttentionSnapshot {
  const action = kind === "question" ? "question" : kind === "review" ? "review" : kind === "playtest" || kind === "blocked" ? "stage" : "terminal";
  const copy = kind === "question"
    ? {title:"需要你回答 1 个问题",instruction:"请回答本组问题。",button:"开始回答",destination:"处理位置：阶段问答",detail:"答案保存后由你继续。"}
    : kind === "review"
    ? {title:"需要你审查美术需求",instruction:"第 1 版文档已提交，点击查看并审查。",button:"查看并审查",destination:"处理位置：制作流程",detail:"请阅读后决定。"}
    : {title:"终端需要你处理",instruction:"请前往原会话。",button:"前往终端处理",destination:"处理位置：终端",detail:"等待你的确认。"};
  return {taskId,unavailable:false,items:[{key:kind + "-key",taskId,taskTitle:"翻卡片",kind,
    tone:kind === "terminal" ? "terminal" : ["failed","disconnected","needsReview"].includes(kind) ? "error" : "human",
    stageName:"美术需求",target:{taskId,action,stage:"resources",provider:"codex",conversationId:"original",
      groupId:kind === "question" ? "q" : null,documentRevision:kind === "review" ? 1 : null,documentHash:kind === "review" ? "old" : null},zh:copy,en:copy}]};
}
