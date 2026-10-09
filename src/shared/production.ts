import { invoke } from "@tauri-apps/api/core";

export interface ProductionDocument {
  kind: string; revision: number; path: string; sha256: string;
  content: string; gameVersion?: string; submittedAt: string;
}
export interface ProductionEvidence {
  stage?: string;
  id: string; path: string; sha256: string; bytes: number; kind: string;
  captureType: string; version: string; reviewed: boolean; description: string;
  recordedAt: string; cycle: number;
}
export interface TaskSkill {
  id: string; provider: string; kind: string; name: string; description: string;
  path: string; sha256: string;
}
export interface ProductionPolicy {
  workflowVersion: string;
  stages: { id: string; name: string; approval: string | null }[];
  dimensions: { id: string; name: string }[];
  questions: { id: string; text: string; family: string }[];
}
export interface ProductionIssue {
  title?: string; affectedStages?: string[]; blockedOperations?: string[];
  id: string; kind: string; status: string; stage: string; description: string;
  fix?: string; recheck?: string; resumeWhen?: string; evidenceIds?: string[];
}
export interface ProductionCheck {
  dimensionId: string; status: string; observations?: string; evidenceIds?: string[];
}
export interface ProductionRound {
  number: number; cycle: number; status: string; inputVersion?: string; outputVersion?: string;
  checks?: ProductionCheck[]; questions?: { questionId: string; status: string; conclusion: string; counterexample: string; problem: string; change: string; recheck: string }[];
  evidenceIds?: string[]; startedAt?: string; closedAt?: string;
}
export interface ProductionRecord {
  artifacts?: ProductionArtifact[];
  stageUpdates?: Record<string, { summary: string; nextAction?: string; updatedAt: string }>;
  events?: {stage:string;status:string;at:string;cycle:number;revision:number}[];
  playerMode?: "unspecified" | "single" | "multiplayer";
  questionGroups?: IntakeQuestionGroup[];
  taskId: string; revision: number; workflowVersion: string; questionMode: string;
  taskTemplate?: string; artTemplate?: string; currentStage: string; currentRound: number;
  cycle: number; stages: Record<string, string>; documents: Record<string, ProductionDocument>;
  approvals: { kind: string; documentHash: string; documentRevision: number; decision: string; feedback: string; decidedAt: string }[];
  currentVersion?: string; versionDetails: Record<string, string>;
  issues: ProductionIssue[]; evidence: ProductionEvidence[]; rounds: ProductionRound[];
  milestones: Record<string, string[]>; knowledge: { summary: string; status: string }[];
  skillPins: TaskSkill[]; legacyRecord?: string; updatedAt: string;
}
export interface ProductionView {
  stageStatuses?: Record<string,string>;
  artifacts?: ProductionArtifact[];
  record?: ProductionRecord; policy: ProductionPolicy; warnings: string[];
  reportPaths: string[]; availableUpdate: boolean;
}
export const productionApi = {
  timing: (taskId:string)=>invoke<{executionMs:number|null;toolMs:number|null;answerWaitMs:number|null;approvalWaitMs:number|null;pauseMs:number|null;connectionMs:number|null;unfinishedTurns:number;note:string}>("get_production_timing",{taskId}),
  submitAnswers: (taskId:string,expectedRevision:number,id:string,answers:Record<string,string>) =>
    invoke<QuestionSubmission>("submit_intake_answers",{input:{taskId,expectedRevision,operation:"submit-answers",data:{id,answers}}}),
  continueTask: (taskId:string,conversationId:string,afterRevision?:number) => invoke<IntakeContinuation>("continue_codex_task",{taskId,conversationId,afterRevision}),
  flushContinuation: (taskId:string,conversationId:string,requestId:string) => invoke<IntakeContinuation>("flush_codex_continuation",{taskId,conversationId,requestId}),
  control: (taskId:string,conversationId:string) => invoke<TaskControlState>("get_codex_task_control",{taskId,conversationId}),
  revealReport: (taskId: string, phase: string) => invoke<void>("reveal_production_report", { taskId, phase }),
  continueQuestions: (taskId: string, groupId: string, revision: number) =>
    invoke<IntakeContinuation>("continue_intake_questions", { taskId, groupId, revision }),
  answer: (taskId: string, expectedRevision: number, operation: string, data: unknown) =>
    invoke<ProductionView>("answer_task_questions", { input: { taskId, expectedRevision, operation, data } }),
  get: (taskId: string) => invoke<ProductionView>("get_task_production", { taskId }),
  update: (taskId: string, expectedRevision: number, operation: string, data: unknown) =>
    invoke<ProductionView>("update_task_production", { input: { taskId, expectedRevision, operation, data } }),
  decide: (taskId: string, expectedRevision: number, kind: string, documentHash: string, accepted: boolean, feedback: string) =>
    invoke<ProductionView>("decide_task_production", { input: { taskId, expectedRevision, kind, documentHash, accepted, feedback } }),
  upgrade: (taskId: string, expectedRevision: number) => invoke<ProductionView>("upgrade_task_production", { taskId, expectedRevision }),
  skills: (taskId: string) => invoke<TaskSkill[]>("get_task_skills", { taskId }),
  readSkill: (taskId: string, provider: string, skillId: string) => invoke<string>("read_task_skill", { taskId, provider, skillId }),
  open: (taskId: string, path: string) => invoke<void>("open_production_artifact", { taskId, path }),
  revealDocument: (taskId: string, path: string) => invoke<void>("open_production_artifact", { taskId, path, reveal: true }),
};

export interface IntakeContinuation {
  status: "needsConnection" | "started" | "resumed" | "running" | "completed" | "superseded" | "queued" | "paused" | "waitingForAnswers" | "waitingForApproval" | "needsReview";
  requestId?:string;
  conversationId: string; nativeSessionId: string; turnId?: string;
}
export interface QuestionSubmission { production:ProductionView; continuation?:IntakeContinuation; warning?:string }
export interface TaskControlState {
  taskId:string;conversationId:string;nativeSessionId?:string;state:string;connection:string;stage?:string;
  turnId?:string;turnStartedAt?:string;queued:boolean;requestId?:string;lastError?:string;updatedAt:string;lastEventAt?:string;sequence:number;
}

export const continuationMessages: Record<IntakeContinuation["status"],string> = {
  queued:"操作已保存，当前回复结束后继续。", paused:"任务已暂停，答案和历史已保存。",
  waitingForAnswers:"有待回答的问题，请先完成需求问答。",waitingForApproval:"正在等待文档确认，请在制作流程中确认对应版本。",
  needsReview:"上次执行结果需要核对，未重复发送。",needsConnection:"正在连接原会话，连接成功后继续……",
  started:"已开始处理当前任务。",resumed:"已恢复中断的执行，将先核对已完成操作。",
  running:"当前任务正在处理，未重复发送。",completed:"本组答案已处理完成，未重复发送。",
  superseded:"后续对话已经继续，请查看当前进度，未重放旧问题。",
};

export interface IntakeQuestionGroup {
  stage?: string; cycle?: number;
  id: string; title: string; provider: "codex" | "grok"; conversationId: string;
  nativeSessionId?: string; status: string; publishedAt: string; updatedAt: string;
  questions: { id: string; text: string; options: string[]; optional: boolean }[];
  draft: Record<string, string>;
  answers: { revision: number; answers: Record<string, string>; submittedAt: string }[];
}

export interface ProductionArtifact {
  id: string; path: string; stage: string; title: string; summary?: string;
  status: string; cycle?: number; updatedAt?: string; exists?: boolean; legacy?: boolean;
}
