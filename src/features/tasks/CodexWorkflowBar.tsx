import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import {
  AlertTriangle,
  ArrowRight,
  Bot,
  Check,
  Circle,
  CircleAlert,
  FilePenLine,
  Gamepad2,
  Globe2,
  ListTree,
  LoaderCircle,
  Network,
  Search,
  SquareTerminal,
  TestTube2,
  X,
} from "lucide-react";
import { RuntimeArtifactPreviews } from "./RuntimeArtifactPreviews";
import type { MessageKey } from "../../i18n";
import type {
  CodexActivity,
  CodexActivityKind,
  CodexConversation,
  CodexPlanStep,
  CodexPlanStepStatus,
  CodexWorkflowSnapshot,
  CodexWorkflowTurn,
  CodexWorkflowTurnStatus,
  TerminalProvider,
} from "../../shared/types";

interface HoveredStep {
  step: CodexPlanStep;
  activities: CodexActivity[];
  left: number;
  top: number;
}

export function CodexWorkflowBar({
  provider,
  conversation,
  workflow,
  loading = false,
  t,
}: {
  provider: TerminalProvider;
  conversation?: CodexConversation;
  workflow?: CodexWorkflowSnapshot;
  loading?: boolean;
  t: (key: MessageKey) => string;
}) {
  const [hovered, setHovered] = useState<HoveredStep>();
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [selectedStepId, setSelectedStepId] = useState("");
  const currentTurn = useMemo(
    () => selectCurrentTurn(workflow),
    [workflow],
  );
  const steps = currentTurn?.plan ?? [];

  function showStepPopover(
    step: CodexPlanStep,
    element: HTMLButtonElement,
  ) {
    const bounds = element.getBoundingClientRect();
    const activities = (currentTurn?.activities ?? [])
      .filter((activity) => activity.stepId === step.id)
      .slice(-3)
      .reverse();
    setHovered({
      step,
      activities,
      left: Math.max(12, Math.min(bounds.left, window.innerWidth - 332)),
      top: Math.min(bounds.bottom + 7, window.innerHeight - 180),
    });
  }

  function openTimeline(stepId = "") {
    setHovered(undefined);
    setSelectedStepId(stepId);
    setDrawerOpen(true);
  }

  return (
    <>
      <section className="workflow-overview" aria-label={t("workflowPlan")}>
        <div className="workflow-overview-heading">
          <div className="workflow-overview-title">
            <ListTree size={16} />
            <strong>{t("workflowPlan")}</strong>
            <span>{conversation?.title ?? t("workflowNoConversation")}</span>
            {currentTurn && (
              <span
                className={`workflow-turn-status workflow-turn-${currentTurn.status}`}
              >
                {turnStatusLabel(t, currentTurn.status)}
              </span>
            )}
          </div>
          <button
            className="icon-button workflow-details-button"
            title={t("workflowDetails")}
            disabled={!workflow || workflow.turns.length === 0}
            onClick={() => openTimeline()}
          >
            <ListTree size={16} />
          </button>
        </div>

        <div className="workflow-track-viewport">
          {loading ? (
            <div className="workflow-placeholder">
              <LoaderCircle className="spin" size={16} />
              <span>{t("loading")}</span>
            </div>
          ) : workflow?.observability === "compatibility" ? (
            <div
              className="workflow-placeholder workflow-compatibility"
              title={workflow.warning}
            >
              <AlertTriangle size={16} />
              <span>
                {t(
                  provider === "codex"
                    ? "codexCompatibilityWarning"
                    : "grokCompatibilityWarning",
                )}
              </span>
            </div>
          ) : !conversation ? (
            <div className="workflow-placeholder">
              <Circle size={15} />
              <span>{t("workflowNoConversation")}</span>
            </div>
          ) : steps.length === 0 ? (
            <div className="workflow-placeholder">
              <LoaderCircle
                className={currentTurn?.status === "waitingForPlan" ? "spin" : ""}
                size={16}
              />
              <span>
                {t(
                  provider === "codex"
                    ? "workflowNoPlanCodex"
                    : "workflowNoPlanGrok",
                )}
              </span>
            </div>
          ) : (
            <div className="workflow-track">
              {steps.map((step, index) => {
                const activities = (currentTurn?.activities ?? []).filter(
                  (activity) => activity.stepId === step.id,
                );
                const latest = activities[activities.length - 1];
                return (
                  <div className="workflow-step-group" key={step.id}>
                    <button
                      className={`workflow-step workflow-step-${step.status}`}
                      aria-label={`${stepTitle(t, step)}: ${stepStatusLabel(t, step.status)}`}
                      onMouseEnter={(event) =>
                        showStepPopover(step, event.currentTarget)
                      }
                      onMouseLeave={() => setHovered(undefined)}
                      onFocus={(event) =>
                        showStepPopover(step, event.currentTarget)
                      }
                      onBlur={() => setHovered(undefined)}
                      onClick={() => openTimeline(step.id)}
                    >
                      <span className="workflow-step-marker">
                        {stepStatusIcon(step.status)}
                      </span>
                      <span className="workflow-step-copy">
                        <strong>{stepTitle(t, step)}</strong>
                        <small>
                          {latest
                            ? activitySummary(t, latest.summary)
                            : stepStatusLabel(t, step.status)}
                        </small>
                      </span>
                    </button>
                    {index < steps.length - 1 && (
                      <ArrowRight
                        className="workflow-step-arrow"
                        aria-hidden="true"
                        size={18}
                      />
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </section>

      {hovered &&
        createPortal(
          <div
            className="workflow-popover"
            role="tooltip"
            style={{ left: hovered.left, top: hovered.top }}
          >
            <div className="workflow-popover-heading">
              <strong>{stepTitle(t, hovered.step)}</strong>
              <span>{stepStatusLabel(t, hovered.step.status)}</span>
            </div>
            <div className="workflow-popover-label">{t("recentActivity")}</div>
            {hovered.activities.length === 0 ? (
              <div className="workflow-popover-empty">{t("noActivity")}</div>
            ) : (
              hovered.activities.map((activity) => (
                <div className="workflow-popover-activity" key={activity.id}>
                  {activityKindIcon(activity.kind)}
                  <span>
                    <strong>{activitySummary(t, activity.summary)}</strong>
                    <small>{activityStatusLabel(t, activity.status)}</small>
                  </span>
                </div>
              ))
            )}
          </div>,
          document.body,
        )}

      {drawerOpen &&
        workflow &&
        createPortal(
          <WorkflowTimelineDrawer
            conversation={conversation}
            workflow={workflow}
            selectedStepId={selectedStepId}
            t={t}
            onClose={() => setDrawerOpen(false)}
          />,
          document.body,
        )}
    </>
  );
}

function WorkflowTimelineDrawer({
  conversation,
  workflow,
  selectedStepId,
  t,
  onClose,
}: {
  conversation?: CodexConversation;
  workflow: CodexWorkflowSnapshot;
  selectedStepId: string;
  t: (key: MessageKey) => string;
  onClose: () => void;
}) {
  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);

  return (
    <div className="workflow-drawer-layer">
      <div className="workflow-drawer-backdrop" onMouseDown={onClose} />
      <aside
        className="workflow-drawer"
        role="dialog"
        aria-modal="true"
        aria-label={t("workflowDetails")}
      >
        <header className="workflow-drawer-header">
          <div>
            <strong>{t("workflowDetails")}</strong>
            <span>{conversation?.title ?? t("workflowNoConversation")}</span>
          </div>
          <button
            className="icon-button"
            title={t("close")}
            autoFocus
            onClick={onClose}
          >
            <X size={17} />
          </button>
        </header>
        <div className="workflow-drawer-body">
          {workflow.observability === "compatibility" && (
            <div className="workflow-drawer-warning" title={workflow.warning}>
              <AlertTriangle size={16} />
              <span>{t("codexCompatibilityWarning")}</span>
            </div>
          )}
          {[...workflow.turns].reverse().map((turn, turnIndex) => (
            <section className="workflow-turn-section" key={turn.id}>
              <div className="workflow-turn-heading">
                <div>
                  <strong>
                    {t("workflowTurn")} {workflow.turns.length - turnIndex}
                  </strong>
                  <span>{formatDate(turn.startedAt)}</span>
                </div>
                <span
                  className={`workflow-turn-status workflow-turn-${turn.status}`}
                >
                  {turnStatusLabel(t, turn.status)}
                </span>
              </div>
              {turn.explanation && (
                <p className="workflow-turn-explanation">{turn.explanation}</p>
              )}
              <div className="workflow-drawer-plan">
                {turn.plan.map((step, index) => (
                  <div
                    className={`workflow-drawer-step ${
                      step.id === selectedStepId ? "selected" : ""
                    }`}
                    key={step.id}
                  >
                    <span className={`workflow-step-marker workflow-step-${step.status}`}>
                      {stepStatusIcon(step.status)}
                    </span>
                    <span>{index + 1}</span>
                    <strong>{stepTitle(t, step)}</strong>
                    <small>{stepStatusLabel(t, step.status)}</small>
                  </div>
                ))}
              </div>
              <div className="workflow-activity-heading">
                {t("workflowActivities")}
              </div>
              <div className="workflow-activity-list">
                {turn.activities.length === 0 ? (
                  <div className="workflow-activity-empty">{t("noActivity")}</div>
                ) : (
                  turn.activities.map((activity) => (
                    <div
                      className={`workflow-activity-row ${
                        activity.stepId === selectedStepId ? "selected" : ""
                      }`}
                      key={activity.id}
                    >
                      <span className="workflow-activity-icon">
                        {activityKindIcon(activity.kind)}
                      </span>
                      <div className="workflow-activity-copy">
                        <div>
                          <strong>
                            {activitySummary(t, activity.summary)}
                          </strong>
                          <span>
                            {activityKindLabel(t, activity.kind)} ·{" "}
                            {activityStatusLabel(t, activity.status)}
                          </span>
                        </div>
                        <time>{formatDate(activity.startedAt)}</time>
                        {activity.detail && <pre>{activity.detail}</pre>}
                        <RuntimeArtifactPreviews taskId={workflow.taskId} detail={activity.detail} t={t} />
                      </div>
                    </div>
                  ))
                )}
              </div>
            </section>
          ))}
        </div>
      </aside>
    </div>
  );
}

function selectCurrentTurn(
  workflow?: CodexWorkflowSnapshot,
): CodexWorkflowTurn | undefined {
  if (!workflow) return undefined;
  return (
    workflow.turns.find((turn) => turn.id === workflow.currentTurnId) ??
    workflow.turns[workflow.turns.length - 1]
  );
}

function stepTitle(
  t: (key: MessageKey) => string,
  step: CodexPlanStep,
) {
  return step.status === "unplanned" ? t("unplannedOperation") : step.step;
}

function stepStatusIcon(status: CodexPlanStepStatus) {
  switch (status) {
    case "completed":
      return <Check size={14} strokeWidth={2.5} />;
    case "inProgress":
      return <LoaderCircle className="spin" size={14} />;
    case "failed":
      return <X size={14} strokeWidth={2.5} />;
    case "unplanned":
      return <CircleAlert size={14} />;
    default:
      return <Circle size={12} />;
  }
}

function activityKindIcon(kind: CodexActivityKind) {
  switch (kind) {
    case "analysis":
      return <Search size={14} />;
    case "command":
      return <SquareTerminal size={14} />;
    case "fileChange":
      return <FilePenLine size={14} />;
    case "mcp":
      return <Network size={14} />;
    case "gameInstance":
      return <Gamepad2 size={14} />;
    case "test":
      return <TestTube2 size={14} />;
    case "web":
      return <Globe2 size={14} />;
    case "agent":
      return <Bot size={14} />;
    default:
      return <Circle size={12} />;
  }
}

function turnStatusLabel(
  t: (key: MessageKey) => string,
  status: CodexWorkflowTurnStatus,
) {
  const keys: Record<CodexWorkflowTurnStatus, MessageKey> = {
    waitingForPlan: "workflowWaitingForPlan",
    inProgress: "workflowInProgress",
    completed: "workflowCompleted",
    failed: "workflowFailed",
    interrupted: "workflowInterrupted",
  };
  return t(keys[status]);
}

function stepStatusLabel(
  t: (key: MessageKey) => string,
  status: CodexPlanStepStatus,
) {
  const keys: Record<CodexPlanStepStatus, MessageKey> = {
    pending: "stepPending",
    inProgress: "stepInProgress",
    completed: "stepCompleted",
    failed: "stepFailed",
    unplanned: "stepUnplanned",
  };
  return t(keys[status]);
}

function activityStatusLabel(
  t: (key: MessageKey) => string,
  status: CodexActivity["status"],
) {
  const keys: Record<CodexActivity["status"], MessageKey> = {
    started: "activityStarted",
    progress: "activityProgress",
    completed: "activityCompleted",
    failed: "activityFailed",
  };
  return t(keys[status]);
}

function activityKindLabel(
  t: (key: MessageKey) => string,
  kind: CodexActivityKind,
) {
  const keys: Record<CodexActivityKind, MessageKey> = {
    analysis: "activityAnalysis",
    command: "activityCommand",
    fileChange: "activityFileChange",
    mcp: "activityMcp",
    gameInstance: "activityGameInstance",
    test: "activityTest",
    web: "activityWeb",
    agent: "activityAgent",
    other: "activityOther",
  };
  return t(keys[kind]);
}

function activitySummary(
  t: (key: MessageKey) => string,
  summary: string,
) {
  const keys: Partial<Record<string, MessageKey>> = {
    "Analyzing the task": "activitySummaryAnalyzing",
    "Reading project files": "activitySummaryReadingFiles",
    "Listing project files": "activitySummaryListingFiles",
    "Searching the workspace": "activitySummarySearchingWorkspace",
    "Running a command": "activitySummaryRunningCommand",
    "Updating project files": "activitySummaryUpdatingFiles",
    "Researching information": "activitySummaryResearching",
    "Coordinating a sub-agent": "activitySummaryCoordinatingAgent",
    "Inspecting an image": "activitySummaryInspectingImage",
    "Starting a game instance": "activitySummaryStartingInstance",
    "Waiting for the game process": "activitySummaryWaitingProcess",
    "Waiting for the game Runtime MCP": "activitySummaryWaitingMcp",
    "Updating the game window": "activitySummaryUpdatingWindow",
    "Stopping a game instance": "activitySummaryStoppingInstance",
    "Using a game runtime tool": "activitySummaryUsingRuntimeTool",
    "Inspecting game runtime capabilities":
      "activitySummaryInspectingRuntimeCapabilities",
    "Inspecting game logs": "activitySummaryInspectingLogs",
  };
  const key = keys[summary];
  return key ? t(key) : summary;
}

function formatDate(value: string) {
  return new Date(value).toLocaleString();
}
