import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  AppPaths,
  AppSettings,
  ArchiveOption,
  ArchiveTransferRecord,
  ArchiveTransferTarget,
  CodexTerminalAvailability,
  CodexConversation,
  CodexTerminalEvent,
  CodexTerminalState,
  CodexWorkflowSnapshot,
  DesktopCliState,
  DevelopmentTask,
  GameConnectionState,
  GameInstance,
  InstanceRuntimeInfo,
  InstanceStopResult,
  LaunchProfile,
  LogSession,
  LogSource,
  LanInterface,
  RuntimeBridgeState,
  RuntimeLogEvent,
  TaskStatus,
  TransferableArchive,
  WindowVisibilityMode,
  TerminalProvider,
  StorageRepairReport,
} from "./types";

export const api = {
  buildInfo: () => invoke<{version:string;release:string;builtAt:string;verification:string;dataDirectory:string;executable:string;testEnvironment:boolean}>("get_build_info"),
  readRuntimeArtifact: (taskId: string, path: string) => invoke<string>("read_runtime_artifact", { taskId, path }),
  paths: () => invoke<AppPaths>("get_app_paths"),
  settings: () => invoke<AppSettings>("get_settings"),
  updateSettings: (input: {
    gameExecutablePath: string;
    gameClient?: import("./gameClients").GameClientSelection;
    workspaceRootPath: string;
    locale: string;
    gameGatewayPort: number;
    preferredAdapterId: string;
    lanBroadcastEnabled: boolean;
  }) =>
    invoke<AppSettings>("update_settings", {
      input,
    }),
  regenerateDesktopToken: () =>
    invoke<AppSettings>("regenerate_desktop_cli_token"),
  desktopCliState: () => invoke<DesktopCliState>("get_desktop_cli_state"),
  restartDesktopCli: () => invoke<DesktopCliState>("restart_desktop_cli"),
  listLanInterfaces: () => invoke<LanInterface[]>("list_lan_interfaces"),
  gameConnectionState: () =>
    invoke<GameConnectionState>("get_game_connection_state"),
  restartGameConnections: () =>
    invoke<GameConnectionState>("restart_game_connections"),
  repairStorage: () => invoke<StorageRepairReport>("repair_storage"),

  listTasks: () => invoke<DevelopmentTask[]>("list_tasks"),
  createTask: (title: string, description: string) =>
    invoke<DevelopmentTask>("create_task", {
      input: { title, description },
    }),
  updateTask: (id: string, title: string, description: string) =>
    invoke<DevelopmentTask>("update_task", {
      id,
      input: { title, description },
    }),
  setTaskStatus: (id: string, status: TaskStatus) =>
    invoke<DevelopmentTask>("set_task_status", { input: { id, status } }),
  deleteTask: (id: string) => invoke<void>("delete_task", { id }),

  codexTerminalAvailability: () =>
    invoke<CodexTerminalAvailability>("get_codex_terminal_availability"),
  openCodexTerminal: (
    taskId: string,
    conversationId: string,
    columns: number,
    rows: number,
    onEvent: Channel<CodexTerminalEvent>,
  ) =>
    invoke<CodexTerminalState>("open_codex_terminal", {
      taskId,
      conversationId,
      columns,
      rows,
      onEvent,
    }),
  writeCodexTerminal: (conversationId: string, data: string) =>
    invoke<void>("write_codex_terminal", { conversationId, data }),
  resizeCodexTerminal: (conversationId: string, columns: number, rows: number) =>
    invoke<void>("resize_codex_terminal", { conversationId, columns, rows }),
  stopCodexTerminal: (conversationId: string) =>
    invoke<void>("stop_codex_terminal", { conversationId }),
  listCodexConversations: (taskId: string) =>
    invoke<CodexConversation[]>("list_codex_conversations", { taskId }),
  createCodexConversation: (taskId: string, title?: string) =>
    invoke<CodexConversation>("create_codex_conversation", {
      taskId,
      title: title ?? null,
    }),
  renameCodexConversation: (
    taskId: string,
    conversationId: string,
    title: string,
  ) =>
    invoke<CodexConversation>("rename_codex_conversation", {
      taskId,
      conversationId,
      title,
    }),
  getCodexWorkflow: (taskId: string, conversationId: string) =>
    invoke<CodexWorkflowSnapshot>("get_codex_workflow", {
      taskId,
      conversationId,
    }),
  deleteCodexConversation: (taskId: string, conversationId: string) =>
    invoke<void>("delete_codex_conversation", { taskId, conversationId }),
  listInstances: (taskId?: string) =>
    invoke<GameInstance[]>("list_instances", { taskId: taskId ?? null }),
  getInstance: (id: string) =>
    invoke<InstanceRuntimeInfo>("get_instance", { id }),
  discoverArchives: (root?: string) =>
    invoke<ArchiveOption[]>("discover_archives", { root: root ?? null }),
  listArchiveTransferTargets: () =>
    invoke<ArchiveTransferTarget[]>("list_archive_transfer_targets"),
  listArchiveTransferSources: () =>
    invoke<TransferableArchive[]>("list_archive_transfer_sources"),
  inspectArchiveTransferSource: (mainArchivePath: string) =>
    invoke<TransferableArchive>("inspect_archive_transfer_source", {
      mainArchivePath,
    }),
  startArchiveTransfer: (instanceId: string, mainArchivePath: string) =>
    invoke<ArchiveTransferRecord>("start_archive_transfer", {
      input: { instanceId, mainArchivePath },
    }),
  listArchiveTransfers: () =>
    invoke<ArchiveTransferRecord[]>("list_archive_transfers"),
  getArchiveTransfer: (id: string) =>
    invoke<ArchiveTransferRecord>("get_archive_transfer", { id }),
  cancelArchiveTransfer: (id: string) =>
    invoke<ArchiveTransferRecord>("cancel_archive_transfer", { id }),
  launchInstance: (
    taskId: string,
    name: string,
    executablePath: string,
    profile: LaunchProfile,
  ) =>
    invoke<GameInstance>("launch_instance", {
      input: { taskId, name, executablePath, profile },
    }),
  stopInstance: (id: string) =>
    invoke<InstanceStopResult>("stop_instance", { id }),
  setInstanceWindowVisibility: (
    id: string,
    visibilityMode: WindowVisibilityMode,
  ) =>
    invoke<GameInstance>("set_instance_window_visibility", {
      id,
      visibilityMode,
    }),
  deleteInstance: (id: string) =>
    invoke<void>("delete_instance", { id }),
  runtimeState: (instanceId: string) =>
    invoke<RuntimeBridgeState>("get_runtime_bridge_state", { instanceId }),

  listLogSessions: (instanceId?: string) =>
    invoke<LogSession[]>("list_log_sessions", {
      instanceId: instanceId ?? null,
    }),
  listLogSources: () => invoke<LogSource[]>("list_log_sources"),
  startLogs: (instanceId: string) =>
    invoke<LogSession>("start_log_collection", { instanceId }),
  stopLogs: (instanceId: string) =>
    invoke<void>("stop_log_collection", { instanceId }),
  deleteLogSession: (id: string) =>
    invoke<void>("delete_log_session", { id }),
  queryLogs: (filter: {
    sessionId: string;
    severity?: string;
    provider?: string;
    eventName?: string;
    contains?: string;
    beforeSequence?: number;
    limit?: number;
  }) => invoke<RuntimeLogEvent[]>("query_log_events", { filter }),
};

export const terminalApi = {
  executionSettings: (taskId:string,conversationId:string) => invoke<import("./types").ExecutionSettingsView>("get_codex_execution_settings",{taskId,conversationId}),
  updateExecutionSettings: (taskId:string,conversationId:string,input:Record<string,unknown>) => invoke<import("./types").ExecutionSettingsView>("update_codex_execution_settings",{taskId,conversationId,input}),
  conversationMetrics: (taskId: string, conversationId: string) => invoke<Record<string, unknown>>("codex_conversation_metrics", { taskId, conversationId }),
  recoveryCandidates: (taskId: string) => invoke<{ id: string; startedAt: string }[]>("codex_recovery_candidates", { taskId }),
  repairBinding: (taskId: string, conversationId: string, nativeId: string) =>
    invoke<CodexConversation>("repair_codex_binding", { taskId, conversationId, nativeId }),
  copyHistory: (provider: TerminalProvider, taskId: string, conversationId: string) =>
    withCodex(provider, () => invoke<boolean>("copy_terminal_history", { provider, taskId, conversationId })),
  projectWorkspace: (taskId: string) => invoke<string>("get_codex_project_workspace", { taskId }),
  setArchived: (taskId: string, conversationId: string, archived: boolean) =>
    invoke<CodexConversation>("set_codex_conversation_archived", { taskId, conversationId, archived }),
  readClipboard: () => invoke<{ kind: "text"; text: string } | { kind: "image" | "empty" }>("read_terminal_clipboard"),
  availability: (provider: TerminalProvider) =>
    withCodex(provider, () => api.codexTerminalAvailability()),
  open: (
    provider: TerminalProvider,
    taskId: string,
    conversationId: string,
    columns: number,
    rows: number,
    onEvent: Channel<CodexTerminalEvent>,
  ) =>
    withCodex(provider, () => api.openCodexTerminal(
          taskId,
          conversationId,
          columns,
          rows,
          onEvent,
        )),
  write: (provider: TerminalProvider, conversationId: string, data: string) =>
    withCodex(provider, () => api.writeCodexTerminal(conversationId, data)),
  resize: (
    provider: TerminalProvider,
    conversationId: string,
    columns: number,
    rows: number,
  ) =>
    withCodex(provider, () => api.resizeCodexTerminal(conversationId, columns, rows)),
  stop: (provider: TerminalProvider, conversationId: string) =>
    withCodex(provider, () => api.stopCodexTerminal(conversationId)),
  listConversations: (provider: TerminalProvider, taskId: string) =>
    withCodex(provider, () => api.listCodexConversations(taskId)),
  createConversation: (
    provider: TerminalProvider,
    taskId: string,
    title?: string,
  ) =>
    withCodex(provider, () => api.createCodexConversation(taskId, title)),
  renameConversation: (
    provider: TerminalProvider,
    taskId: string,
    conversationId: string,
    title: string,
  ) =>
    withCodex(provider, () => api.renameCodexConversation(taskId, conversationId, title)),
  workflow: (
    provider: TerminalProvider,
    taskId: string,
    conversationId: string,
  ) =>
    withCodex(provider, () => api.getCodexWorkflow(taskId, conversationId)),
  deleteConversation: (
    provider: TerminalProvider,
    taskId: string,
    conversationId: string,
  ) =>
    withCodex(provider, () => api.deleteCodexConversation(taskId, conversationId)),
};

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object") {
    const value = error as { message?: string; detail?: string };
    return [value.message, value.detail].filter(Boolean).join(" ");
  }
  return "Unknown error";
}

function withCodex<T>(provider: TerminalProvider, action: () => Promise<T>): Promise<T> {
  if (provider !== "codex") return Promise.reject(new Error("Unsupported terminal provider."));
  return action();
}
