import { invoke } from "@tauri-apps/api/core";

export interface ProductionDocument {
  kind: string; revision: number; path: string; sha256: string;
  content: string; gameVersion?: string; submittedAt: string;
}
export interface ProductionEvidence {
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
  record?: ProductionRecord; policy: ProductionPolicy; warnings: string[];
  reportPaths: string[]; availableUpdate: boolean;
}
export const productionApi = {
  get: (taskId: string) => invoke<ProductionView>("get_task_production", { taskId }),
  update: (taskId: string, expectedRevision: number, operation: string, data: unknown) =>
    invoke<ProductionView>("update_task_production", { input: { taskId, expectedRevision, operation, data } }),
  decide: (taskId: string, expectedRevision: number, kind: string, documentHash: string, accepted: boolean, feedback: string) =>
    invoke<ProductionView>("decide_task_production", { input: { taskId, expectedRevision, kind, documentHash, accepted, feedback } }),
  upgrade: (taskId: string, expectedRevision: number) => invoke<ProductionView>("upgrade_task_production", { taskId, expectedRevision }),
  skills: (taskId: string) => invoke<TaskSkill[]>("get_task_skills", { taskId }),
  readSkill: (taskId: string, provider: string, skillId: string) => invoke<string>("read_task_skill", { taskId, provider, skillId }),
  open: (taskId: string, path: string) => invoke<void>("open_production_artifact", { taskId, path }),
};
