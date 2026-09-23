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
  DesktopMcpState,
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
  paths: () => invoke<AppPaths>("get_app_paths"),
  settings: () => invoke<AppSettings>("get_settings"),
  updateSettings: (input: {
    gameExecutablePath: string;
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
    invoke<AppSettings>("regenerate_desktop_mcp_token"),
  desktopMcpState: () => invoke<DesktopMcpState>("get_desktop_mcp_state"),
  restartDesktopMcp: () => invoke<DesktopMcpState>("restart_desktop_mcp"),
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
  grokTerminalAvailability: () =>
    invoke<CodexTerminalAvailability>("get_grok_terminal_availability"),
  openGrokTerminal: (
    taskId: string,
    conversationId: string,
    columns: number,
    rows: number,
    onEvent: Channel<CodexTerminalEvent>,
  ) =>
    invoke<CodexTerminalState>("open_grok_terminal", {
      taskId,
      conversationId,
      columns,
      rows,
      onEvent,
    }),
  writeGrokTerminal: (conversationId: string, data: string) =>
    invoke<void>("write_grok_terminal", { conversationId, data }),
  resizeGrokTerminal: (conversationId: string, columns: number, rows: number) =>
    invoke<void>("resize_grok_terminal", { conversationId, columns, rows }),
  stopGrokTerminal: (conversationId: string) =>
    invoke<void>("stop_grok_terminal", { conversationId }),
  listGrokConversations: (taskId: string) =>
    invoke<CodexConversation[]>("list_grok_conversations", { taskId }),
  createGrokConversation: (taskId: string, title?: string) =>
    invoke<CodexConversation>("create_grok_conversation", {
      taskId,
      title: title ?? null,
    }),
  renameGrokConversation: (
    taskId: string,
    conversationId: string,
    title: string,
  ) =>
    invoke<CodexConversation>("rename_grok_conversation", {
      taskId,
      conversationId,
      title,
    }),
  getGrokWorkflow: (taskId: string, conversationId: string) =>
    invoke<CodexWorkflowSnapshot>("get_grok_workflow", {
      taskId,
      conversationId,
    }),
  deleteGrokConversation: (taskId: string, conversationId: string) =>
    invoke<void>("delete_grok_conversation", { taskId, conversationId }),

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
  availability: (provider: TerminalProvider) =>
    provider === "codex"
      ? api.codexTerminalAvailability()
      : api.grokTerminalAvailability(),
  open: (
    provider: TerminalProvider,
    taskId: string,
    conversationId: string,
    columns: number,
    rows: number,
    onEvent: Channel<CodexTerminalEvent>,
  ) =>
    provider === "codex"
      ? api.openCodexTerminal(
          taskId,
          conversationId,
          columns,
          rows,
          onEvent,
        )
      : api.openGrokTerminal(
          taskId,
          conversationId,
          columns,
          rows,
          onEvent,
        ),
  write: (provider: TerminalProvider, conversationId: string, data: string) =>
    provider === "codex"
      ? api.writeCodexTerminal(conversationId, data)
      : api.writeGrokTerminal(conversationId, data),
  resize: (
    provider: TerminalProvider,
    conversationId: string,
    columns: number,
    rows: number,
  ) =>
    provider === "codex"
      ? api.resizeCodexTerminal(conversationId, columns, rows)
      : api.resizeGrokTerminal(conversationId, columns, rows),
  stop: (provider: TerminalProvider, conversationId: string) =>
    provider === "codex"
      ? api.stopCodexTerminal(conversationId)
      : api.stopGrokTerminal(conversationId),
  listConversations: (provider: TerminalProvider, taskId: string) =>
    provider === "codex"
      ? api.listCodexConversations(taskId)
      : api.listGrokConversations(taskId),
  createConversation: (
    provider: TerminalProvider,
    taskId: string,
    title?: string,
  ) =>
    provider === "codex"
      ? api.createCodexConversation(taskId, title)
      : api.createGrokConversation(taskId, title),
  renameConversation: (
    provider: TerminalProvider,
    taskId: string,
    conversationId: string,
    title: string,
  ) =>
    provider === "codex"
      ? api.renameCodexConversation(taskId, conversationId, title)
      : api.renameGrokConversation(taskId, conversationId, title),
  workflow: (
    provider: TerminalProvider,
    taskId: string,
    conversationId: string,
  ) =>
    provider === "codex"
      ? api.getCodexWorkflow(taskId, conversationId)
      : api.getGrokWorkflow(taskId, conversationId),
  deleteConversation: (
    provider: TerminalProvider,
    taskId: string,
    conversationId: string,
  ) =>
    provider === "codex"
      ? api.deleteCodexConversation(taskId, conversationId)
      : api.deleteGrokConversation(taskId, conversationId),
};

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object") {
    const value = error as { message?: string; detail?: string };
    return [value.message, value.detail].filter(Boolean).join(" ");
  }
  return "Unknown error";
}
