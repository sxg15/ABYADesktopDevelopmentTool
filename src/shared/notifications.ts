import { invoke } from "@tauri-apps/api/core";
import type { TerminalProvider } from "./types";

export interface AttentionTarget {
  taskId: string;
  action: "question" | "review" | "terminal" | "stage";
  stage: string;
  provider: TerminalProvider | null;
  conversationId: string | null;
  groupId: string | null;
  documentRevision: number | null;
  documentHash: string | null;
}
export interface AttentionText {
  title: string; instruction: string; detail: string; destination: string; button: string;
}
export interface TaskAttention {
  key: string; taskId: string; taskTitle: string;
  kind: "terminal" | "question" | "review" | "playtest" | "blocked" | "failed" | "disconnected" | "needsReview";
  tone: "human" | "terminal" | "error";
  stageName: string;
  target: AttentionTarget;
  zh: AttentionText;
  en: AttentionText;
}
export interface TaskAttentionSnapshot { taskId: string; items: TaskAttention[]; unavailable: boolean }
export interface NotificationActivation {
  id: string; target: AttentionTarget | null;
  status: "current" | "resolved" | "missing" | "unavailable" | "test";
}
export interface NotificationSettings {
  enabled: boolean; available: boolean; reason: string; identity: string; testEnvironment: boolean;
}
export const attentionApi = {
  get: (taskId: string) => invoke<TaskAttentionSnapshot>("get_task_attention", { taskId }),
  settings: () => invoke<NotificationSettings>("get_task_notification_settings"),
  setEnabled: (enabled: boolean) => invoke<NotificationSettings>("set_task_notification_enabled", { enabled }),
  test: () => invoke<void>("test_task_notification"),
  takeActivations: () => invoke<NotificationActivation[]>("take_task_notification_activations"),
  acknowledgeActivations: (ids: string[]) => invoke<void>("acknowledge_task_notification_activations", { ids }),
};
