import { invoke } from "@tauri-apps/api/core";
import type { ArtDesign } from "./artDesign";
export interface AcceptanceSession {
  id?: string; version?: string; status: string; message?: string; instanceIds?: string[];
  startedAt?: string; updatedAt?: string; cleanupFailures?: string[];
}

export interface ProductionDocument {
  artifactBindings?: Record<string, string>;
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
  implementationExecution?: {mode:string};
  iterationMode?: string;
  selfTestChecks?: string[];
  workflowVersion: string;
  stages: { id: string; name: string; approval: string | null }[];
  dimensions: { id: string; name: string }[];
  questions: { id: string; text: string; family: string }[];
}
export interface ProductionIssue {
  diagnosisStatus?: "suspected" | "confirmed";
  executionHandling?: {mode:string;reason:string;impact:string;affectedUnitIds:string[]};
  repairTarget?: "abya-source" | "abya-build" | "lua" | "other";
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
export interface WorkbenchGoal {
  id:string; stage:string; number:number; title:string; sourceType:string; sourceId:string;
  status:string; summary:string; decision?:string; recordRevision:number; createdAt:string; updatedAt:string; cycle:number;
}
export interface WorkbenchAttempt { id:string;goalId:string;requestId?:string;conversationId:string;nativeId:string;turnId?:string;status:string;reason:string;startedAt:string;updatedAt:string;predecessorId?:string;processEpoch:string;generation:number }
export interface WorkbenchState {
  execution?: {scopeHash?:string;state:string;activeUnitId?:string;updatedAt:string;units:{scopeHash?:string;id:string;goalId:string;cycle:number;title:string;status:string;reason:string;dependsOn:string[];routes:string[];route:number;attempts:number;resultPaths:string[]}[];events:{kind:string;unitId?:string;reason:string;at:string}[]};
  attempts?:WorkbenchAttempt[];
  goals:WorkbenchGoal[]; activeGoalId?:string; drafts:Record<string,string>;
  requests:{id:string;text:string;stage:string;status:string;revision:number}[];
  deferrals:{id:string;stage?:string;affectedStages?:string[];blockedOperations?:string[];description:string;reason:string;impact:string;status:string}[];
}
export interface ProductionRecord {
  workbench?:WorkbenchState;
  artDesign?: ArtDesign;
  acceptanceConfig?: { version: string; seats: number; requireUi: boolean; requiredTools: string[] } | null;
  acceptanceSessions?: AcceptanceSession[];
  feedback?: ProductionFeedback[];
  selfTests?: { id: string; status: string; observations: string; version: string; cycle: number; recordedAt: string }[];
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
  approvalStatuses?: Record<string, string>;
  artDesignStatus?: {enabled:boolean;inputHash:string;designHash:string};
  actions?: { review: Record<string,string>; approve: Record<string,string>; feedback: Record<string,string>; recheck?: Record<string,string>; resume?: Record<string,string> };
  stageStatuses?: Record<string,string>;
  artifacts?: ProductionArtifact[];
  record?: ProductionRecord; policy: ProductionPolicy; warnings: string[];
  reportPaths: string[]; availableUpdate: boolean;
}
export const productionApi = {
  reconcile:(taskId:string)=>invoke<ProductionView>("reconcile_task_execution",{taskId}),
  resumeGoal:(taskId:string,goalId:string,conversationId:string,expectedRevision:number,resumeIntentId:string,reconciliationNote?:string)=>invoke<IntakeContinuation>("resume_workbench_goal",{taskId,goalId,conversationId,expectedRevision,resumeIntentId,reconciliationNote}),
  exportDeveloperPackage:(taskId:string,expectedRevision:number,issueIds:string[])=>invoke<string>("export_developer_package",{taskId,expectedRevision,issueIds}),
  workbench: (taskId:string,expectedRevision:number,operation:string,data:unknown) => invoke<ProductionView>("mutate_production_workbench",{input:{taskId,expectedRevision,operation,data}}),
  snapshot: (taskId:string,revision:number) => invoke<ProductionRecord>("read_production_snapshot",{taskId,revision}),
  media: (taskId: string, artifactId: string) => invoke<string>("read_production_media", { taskId, artifactId }),
  feedback: (taskId: string, expectedRevision: number, operation: string, data: unknown) =>
    invoke<ProductionView>("submit_production_feedback", { input: { taskId, expectedRevision, operation, data } }),
  startAcceptance: (taskId: string) => invoke<AcceptanceSession>("start_task_acceptance", { taskId }),
  acceptanceStatus: (taskId: string) => invoke<AcceptanceSession>("get_task_acceptance", { taskId }),
  timing: (taskId:string)=>invoke<{elapsedMs:number|null;firstPlayableMs:number|null;humanWaitMs:number|null;blockedMs:number|null;feedbackReworkMs:number|null;activeWorkMs:number|null;executionMs:number|null;toolMs:number|null;answerWaitMs:number|null;approvalWaitMs:number|null;pauseMs:number|null;connectionMs:number|null;unfinishedTurns:number;note:string}>("get_production_timing",{taskId}),
  submitAnswers: (taskId:string,expectedRevision:number,id:string,answers:Record<string,string>) =>
    invoke<QuestionSubmission>("submit_intake_answers",{input:{taskId,expectedRevision,operation:"submit-answers",data:{id,answers}}}),
  continueTask: (taskId:string,conversationId:string,afterRevision?:number,resumeIntentId?:string) => invoke<IntakeContinuation>("continue_codex_task",{taskId,conversationId,afterRevision,resumeIntentId}),
  repairIssue: (taskId:string,conversationId:string,issueId:string,expectedRevision:number) => invoke<IntakeContinuation>("repair_codex_issue",{taskId,conversationId,issueId,expectedRevision}),
  flushContinuation: (taskId:string,conversationId:string,requestId:string) => invoke<IntakeContinuation>("flush_codex_continuation",{taskId,conversationId,requestId}),
  control: (taskId:string,conversationId:string) => invoke<TaskControlState>("get_codex_task_control",{taskId,conversationId}),
  revealReport: (taskId: string, phase: string) => invoke<void>("reveal_production_report", { taskId, phase }),
  exportProgrammerIssues: (taskId: string, expectedRevision: number, issueIds: string[]) =>
    invoke<string>("export_programmer_issues", { taskId, expectedRevision, issueIds }),
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
  status: "workSettled" | "needsConnection" | "started" | "resumed" | "running" | "completed" | "superseded" | "queued" | "paused" | "waitingForAnswers" | "waitingForApproval" | "needsReview";
  requestId?:string;
  conversationId: string; nativeSessionId: string; turnId?: string;
}
export interface QuestionSubmission { production:ProductionView; continuation?:IntakeContinuation; warning?:string }
export interface TaskControlState {
  autonomyNotice?:string;
  activeGoalId?:string;
  generation?:number;
  taskId:string;conversationId:string;nativeSessionId?:string;state:string;connection:string;stage?:string;
  turnId?:string;turnStartedAt?:string;queued:boolean;requestId?:string;lastError?:string;updatedAt:string;lastEventAt?:string;sequence:number;
  executionControl?: {requestId:string;operation:string;status:string;turnId?:string;error?:string;errorCode?:string};
}

export const continuationMessages: Record<IntakeContinuation["status"],string> = {
  workSettled:"当前可执行工作已收敛；未完成项和替代方案已保留，无需反复点击继续。",
  queued:"操作已保存，当前回复结束后继续。", paused:"任务已暂停，答案和历史已保存。",
  waitingForAnswers:"有待回答的问题，请先完成需求问答。",waitingForApproval:"正在等待文档确认，请在制作流程中确认对应版本。",
  needsReview:"上次执行结果需要核对，未重复发送。",needsConnection:"正在连接原会话，连接成功后继续……",
  started:"已开始处理当前任务。",resumed:"已恢复中断的执行，将先核对已完成操作。",
  running:"当前任务正在处理，未重复发送。",completed:"本组答案已处理完成，未重复发送。",
  superseded:"后续对话已经继续，请查看当前进度，未重放旧问题。",
};

export interface IntakeQuestionGroup {
  stage?: string; cycle?: number;
  id: string; title: string; provider: string; conversationId: string;
  nativeSessionId?: string; status: string; publishedAt: string; updatedAt: string;
  questions: { id: string; text: string; options: string[]; optional: boolean }[];
  draft: Record<string, string>;
  answers: { revision: number; answers: Record<string, string>; submittedAt: string }[];
}

export interface ProductionArtifact {
  plannerInput?:boolean;
  libraryReference?: boolean;
  plannerReference?: boolean;
  mediaType?: string; sourceType?: string; roles?: string[]; visualVersion?: string; groupId?: string;
  sha256?: string; temporary?: boolean; source?: string;
  id: string; path: string; stage: string; title: string; summary?: string;
  status: string; cycle?: number; updatedAt?: string; exists?: boolean; legacy?: boolean;
}

export interface ProductionFeedback {
  reopenReason?: string;
  candidateArtifactId?: string;
  id: string; description: string; status: string; version?: string; candidateVersion?: string;
  artifactId?: string; attachmentIds?: string[]; instanceIds?: string[]; stage: string;
  fix?: string; recheck?: string; createdAt: string; updatedAt: string;
  history?: { actor: string; status: string; at: string; version?: string; fix?: string; reason?: string }[];
}
