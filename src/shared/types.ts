export type TaskStatus = "active" | "completed" | "archived";
export type LaunchMode =
  | "editor"
  | "offline"
  | "lan-host"
  | "lan-client"
  | "igp-hosted";
export type WindowMode = "windowed" | "borderless" | "fullscreen";
export type WindowVisibilityMode = "background" | "visible";
export type InstanceOrigin = "managed" | "external";
export type ProcessState =
  | "launching"
  | "running"
  | "stopping"
  | "exited"
  | "failed"
  | "interrupted"
  | "unmanaged";
export type ConnectionState = "waiting" | "connected" | "disconnected";

export interface AppError {
  code: string;
  message: string;
  detail: string;
}

export interface AppSettings {
  gameExecutablePath: string;
  workspaceRootPath: string;
  locale: string;
  desktopCliPort: number;
  gameGatewayPort: number;
  preferredAdapterId: string;
  lanBroadcastEnabled: boolean;
  toolId: string;
}

export interface AppPaths {
  dataDir: string;
  databasePath: string;
  bootstrapLogsDir: string;
  reportsDir: string;
  archiveTransfersDir: string;
  defaultArchiveRoot: string;
  defaultWorkspaceRoot: string;
}

export interface StorageRepairReport {
  database: { databaseBytesBefore: number; databaseBytesAfter: number; walBytesAfter: number; freePagesAfter: number; quickCheck: string };
  logs: { eventsDeleted: number; sessionsClosed: number };
  instanceLogs: { filesTruncated: number; bytesReclaimed: number };
}

export interface DevelopmentTask {
  id: string;
  title: string;
  description: string;
  status: TaskStatus;
  createdAt: string;
  updatedAt: string;
  completedAt?: string;
  archivedAt?: string;
  workspacePath: string;
}

export type TerminalProvider = "codex" | "grok";

export interface CodexConversation {
  id: string;
  taskId: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  nativeSessionId?: string;
}

export type TerminalConversation = CodexConversation;

export type CodexTerminalStatus =
  | "starting"
  | "running"
  | "stopping"
  | "exited"
  | "failed";

export interface CodexTerminalAvailability {
  available: boolean;
  workingDirectory: string;
  reason: string;
  observabilityAvailable: boolean;
  observabilityReason: string;
}

export type TerminalAvailability = CodexTerminalAvailability;

export interface CodexTerminalState {
  taskId: string;
  conversationId: string;
  status: CodexTerminalStatus;
  pid?: number;
  workingDirectory: string;
  exitCode?: number;
  lastError: string;
}

export type TerminalState = CodexTerminalState;

export type CodexObservabilityStatus = "native" | "compatibility";
export type CodexWorkflowTurnStatus =
  | "waitingForPlan"
  | "inProgress"
  | "completed"
  | "failed"
  | "interrupted";
export type CodexPlanStepStatus =
  | "pending"
  | "inProgress"
  | "completed"
  | "failed"
  | "unplanned";
export type CodexActivityStatus =
  | "started"
  | "progress"
  | "completed"
  | "failed";
export type CodexActivityKind =
  | "analysis"
  | "command"
  | "fileChange"
  | "mcp"
  | "gameInstance"
  | "test"
  | "web"
  | "agent"
  | "other";

export interface CodexPlanStep {
  id: string;
  step: string;
  status: CodexPlanStepStatus;
}

export interface CodexActivity {
  id: string;
  stepId?: string;
  kind: CodexActivityKind;
  status: CodexActivityStatus;
  summary: string;
  detail: string;
  source: string;
  unplanned: boolean;
  startedAt: string;
  completedAt?: string;
}

export interface CodexWorkflowTurn {
  id: string;
  status: CodexWorkflowTurnStatus;
  explanation: string;
  plan: CodexPlanStep[];
  activities: CodexActivity[];
  startedAt: string;
  completedAt?: string;
}

export interface CodexWorkflowSnapshot {
  taskId: string;
  conversationId: string;
  observability: CodexObservabilityStatus;
  warning: string;
  currentTurnId?: string;
  turns: CodexWorkflowTurn[];
  updatedAt: string;
}

export type TerminalWorkflowSnapshot = CodexWorkflowSnapshot;

export type CodexTerminalEvent =
  | { type: "output"; data: string }
  | { type: "state"; state: CodexTerminalState }
  | { type: "workflow"; workflow: CodexWorkflowSnapshot };

export type TerminalEvent = CodexTerminalEvent;

export interface LevelOption {
  levelGuid: string;
  levelName: string;
  isStart: boolean;
}

export interface ArchiveOption {
  archivePath: string;
  archiveGuid: string;
  archiveName: string;
  author: string;
  levels: LevelOption[];
}

export interface ArchiveSelection {
  archivePath: string;
  archiveGuid: string;
  archiveName: string;
  levelGuid: string;
  levelName: string;
}

export interface LaunchProfile {
  mode: LaunchMode;
  windowMode: WindowMode;
  visibilityMode: WindowVisibilityMode;
  width: number;
  height: number;
  archive?: ArchiveSelection;
  hostInstanceId?: string;
  exitOnFailure: boolean;
  language: string;
}

export interface GameInstance {
  id: string;
  taskId?: string;
  origin: InstanceOrigin;
  name: string;
  mode?: LaunchMode;
  hostInstanceId?: string;
  executablePath?: string;
  profile?: LaunchProfile;
  sanitizedArgs: string[];
  pid?: number;
  processState: ProcessState;
  connectionState: ConnectionState;
  startedAt?: string;
  endedAt?: string;
  exitCode?: number;
  failureReason: string;
  hostPort?: number;
  logFilePath?: string;
  runtimeInstanceId?: string;
  remoteAddress?: string;
  gameVersion: string;
  platform: string;
  runtimeCapabilities: string[];
  connectedAt?: string;
  disconnectedAt?: string;
  lastSeenAt?: string;
}

export interface InstanceRuntimeInfo {
  instance: GameInstance;
  gatewayEndpoint: string;
  reportPath?: string;
  processAlive: boolean;
}

export interface InstanceStopResult {
  instance: GameInstance;
  gracefulExitRequested: boolean;
  gracefulExitObserved: boolean;
  forcedTermination: boolean;
  timedOut: boolean;
  processAlive: boolean;
}

export interface RuntimeBridgeState {
  instanceId: string;
  endpoint: string;
  connected: boolean;
  runtimeInstanceId: string;
  origin: string;
  gameVersion: string;
  platform: string;
  capabilities: string[];
  cliAvailable: boolean;
  serverName: string;
  serverVersion: string;
  instructions: string;
  lastError: string;
}

export interface LanInterface {
  id: string;
  name: string;
  address: string;
  broadcastAddress: string;
}

export interface GameConnectionState {
  running: boolean;
  broadcastRunning: boolean;
  bindEndpoint: string;
  preferredEndpoint: string;
  gatewayPort: number;
  discoveryPort: number;
  path: string;
  protocolVersion: number;
  preferredAdapterId: string;
  advertisedAddresses: string[];
  connectedCount: number;
  toolId: string;
  lastError: string;
}

export interface LogSession {
  id: string;
  instanceId: string;
  serverSessionId: string;
  status: string;
  endpoint: string;
  startedAt: string;
  endedAt?: string;
  latestSequence: number;
  droppedEvents: number;
  lastError: string;
}

export interface LogSource {
  instanceId: string;
  name: string;
  origin: InstanceOrigin;
  connectionState: ConnectionState;
  remoteAddress: string;
  gameVersion: string;
  platform: string;
  latestSessionId: string;
  latestSessionStatus: string;
  latestSequence: number;
}

export interface RuntimeLogEvent {
  logSessionId: string;
  sequence: number;
  utc: string;
  severity: string;
  provider: string;
  category: string;
  eventName: string;
  message: string;
  objectId: string;
  correlationId: string;
  raw: unknown;
}

export interface DesktopCliState {
  running: boolean;
  endpoint: string;
  port: number;
  toolCount: number;
  lastError: string;
}

export type ArchiveTransferStatus =
  | "preparing"
  | "waitingAcceptance"
  | "transferring"
  | "finalizing"
  | "completed"
  | "rejected"
  | "cancelled"
  | "failed"
  | "interrupted";

export interface TransferableArchive {
  mainArchivePath: string;
  archivePath: string;
  archiveGuid: string;
  archiveName: string;
  author: string;
  fileCount: number;
  uncompressedBytes: number;
  lastModifiedAt: string;
}

export interface ArchiveTransferTarget {
  instanceId: string;
  name: string;
  origin: InstanceOrigin;
  gameVersion: string;
  platform: string;
  remoteAddress: string;
}

export interface ArchiveTransferRecord {
  id: string;
  instanceId: string;
  sourceMainPath: string;
  archivePath: string;
  archiveGuid: string;
  archiveName: string;
  author: string;
  fileCount: number;
  uncompressedBytes: number;
  packageBytes: number;
  acknowledgedBytes: number;
  sha256: string;
  status: ArchiveTransferStatus;
  phase: string;
  progressPercent: number;
  installedPath: string;
  errorCode: string;
  errorMessage: string;
  createdAt: string;
  updatedAt: string;
  completedAt?: string;
}
