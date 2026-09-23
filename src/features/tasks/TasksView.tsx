import { useEffect, useMemo, useRef, useState } from "react";
import { flushSync } from "react-dom";
import {
  Archive,
  Check,
  ChevronRight,
  CirclePlus,
  Eye,
  EyeOff,
  Gamepad2,
  Play,
  Power,
  RotateCcw,
  SquareTerminal,
  Trash2,
} from "lucide-react";
import { Modal } from "../../app/Modal";
import { LaunchInstanceModal } from "../instances/LaunchInstanceModal";
import { InstanceDetailsModal } from "../instances/InstanceDetailsModal";
import { DevelopmentTerminalView } from "../terminal/CodexTerminalView";
import { CodexWorkflowBar } from "./CodexWorkflowBar";
import { api, errorMessage, terminalApi } from "../../shared/api";
import type {
  AppSettings,
  CodexConversation,
  CodexTerminalAvailability,
  CodexWorkflowSnapshot,
  DevelopmentTask,
  GameInstance,
  TaskStatus,
  TerminalProvider,
} from "../../shared/types";
import type { MessageKey } from "../../i18n";

export function TasksView({
  settings,
  t,
  notify,
}: {
  settings?: AppSettings;
  t: (key: MessageKey) => string;
  notify: (message: string, error?: boolean) => void;
}) {
  const [tasks, setTasks] = useState<DevelopmentTask[]>([]);
  const [instances, setInstances] = useState<GameInstance[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [showCreate, setShowCreate] = useState(false);
  const [showLaunch, setShowLaunch] = useState(false);
  const [detail, setDetail] = useState<GameInstance>();
  const [taskTabs, setTaskTabs] = useState<Record<string, TaskTab>>({});
  const [openedTerminalIds, setOpenedTerminalIds] = useState<string[]>([]);
  const [openedProviderKeys, setOpenedProviderKeys] = useState<string[]>([]);
  const [terminalProviders, setTerminalProviders] =
    useState<Record<string, TerminalProvider>>(loadTerminalProviders);
  const [terminalAvailability, setTerminalAvailability] = useState<
    Partial<Record<TerminalProvider, CodexTerminalAvailability>>
  >({});
  const [terminalContexts, setTerminalContexts] = useState<
    Record<string, TaskTerminalContext>
  >({});
  const selectedConversationIdsRef = useRef<Record<string, string>>({});

  const selected = tasks.find((task) => task.id === selectedId);
  const selectedTab = selected ? (taskTabs[selected.id] ?? "instances") : "instances";
  const selectedProvider = selected
    ? (terminalProviders[selected.id] ?? "codex")
    : "codex";
  const selectedContextKey = selected
    ? terminalContextKey(selected.id, selectedProvider)
    : "";
  const selectedTerminalContext = selected
    ? terminalContexts[selectedContextKey]
    : undefined;
  const taskInstances = useMemo(
    () => instances.filter((instance) => instance.taskId === selectedId),
    [instances, selectedId],
  );

  useEffect(() => {
    void refresh();
    void refreshTerminalAvailability();
    const timer = window.setInterval(() => void refreshInstances(), 2000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!selectedId && tasks[0]) setSelectedId(tasks[0].id);
  }, [tasks, selectedId]);

  useEffect(() => {
    const taskIds = new Set(tasks.map((task) => task.id));
    setOpenedTerminalIds((current) =>
      current.filter((taskId) => taskIds.has(taskId)),
    );
    setTerminalContexts((current) =>
      Object.fromEntries(
        Object.entries(current).filter(([key]) =>
          taskIds.has(key.split(":", 1)[0]),
        ),
      ),
    );
    setOpenedProviderKeys((current) =>
      current.filter((key) => taskIds.has(key.split(":", 1)[0])),
    );
    setTerminalProviders((current) =>
      Object.fromEntries(
        Object.entries(current).filter(([taskId]) => taskIds.has(taskId)),
      ),
    );
    selectedConversationIdsRef.current = Object.fromEntries(
      Object.entries(selectedConversationIdsRef.current).filter(([key]) =>
        taskIds.has(key.split(":", 1)[0]),
      ),
    );
  }, [tasks]);

  useEffect(() => {
    if (!selectedId) return;
    const provider = terminalProviders[selectedId] ?? "codex";
    const contextKey = terminalContextKey(selectedId, provider);
    let cancelled = false;
    setTerminalContexts((current) => ({
      ...current,
      [contextKey]: {
        ...current[contextKey],
        loading: true,
      },
    }));
    void terminalApi
      .listConversations(provider, selectedId)
      .then(async (conversations) => {
        if (cancelled) return;
        const preferredConversationId =
          selectedConversationIdsRef.current[contextKey];
        const conversation =
          conversations.find(
            (candidate) => candidate.id === preferredConversationId,
          ) ?? conversations[0];
        if (conversation) {
          selectedConversationIdsRef.current[contextKey] = conversation.id;
        } else {
          delete selectedConversationIdsRef.current[contextKey];
        }
        const workflow = conversation
          ? await terminalApi.workflow(provider, selectedId, conversation.id)
          : undefined;
        if (
          cancelled ||
          selectedConversationIdsRef.current[contextKey] !== conversation?.id
        ) {
          return;
        }
        setTerminalContexts((current) => ({
          ...current,
          [contextKey]: {
            conversation,
            workflow,
            loading: false,
          },
        }));
      })
      .catch(() => {
        if (cancelled) return;
        setTerminalContexts((current) => ({
          ...current,
          [contextKey]: {
            ...current[contextKey],
            loading: false,
          },
        }));
      });
    return () => {
      cancelled = true;
    };
  }, [selectedId, terminalProviders]);

  useEffect(() => {
    const conversationId = selectedTerminalContext?.conversation?.id;
    if (!selectedId || !conversationId) return;
    const taskId = selectedId;
    const provider = selectedProvider;
    const contextKey = terminalContextKey(taskId, provider);
    const currentConversationId: string = conversationId;
    let cancelled = false;
    async function refreshWorkflow() {
      try {
        const workflow = await terminalApi.workflow(
          provider,
          taskId,
          currentConversationId,
        );
        if (cancelled) return;
        setTerminalContexts((current) => ({
          ...current,
          ...(current[contextKey]?.conversation?.id === currentConversationId
            ? {
                [contextKey]: {
                  ...current[contextKey],
                  workflow,
                  loading: false,
                },
              }
            : {}),
        }));
      } catch {
        // Live terminal events remain authoritative while transient reads fail.
      }
    }
    const timer = window.setInterval(() => void refreshWorkflow(), 1500);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [
    selectedId,
    selectedProvider,
    selectedTerminalContext?.conversation?.id,
  ]);

  async function refresh() {
    try {
      const [taskResult, instanceResult] = await Promise.all([
        api.listTasks(),
        api.listInstances(),
      ]);
      setTasks(taskResult);
      setInstances(instanceResult);
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function refreshInstances() {
    try {
      setInstances(await api.listInstances());
    } catch {
      // Poll failures are surfaced on explicit actions.
    }
  }

  async function refreshTerminalAvailability() {
    try {
      const [codex, grok] = await Promise.all([
        terminalApi.availability("codex"),
        terminalApi.availability("grok"),
      ]);
      setTerminalAvailability({ codex, grok });
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  function selectTab(tab: TaskTab) {
    if (!selected) return;
    setTaskTabs((current) => ({ ...current, [selected.id]: tab }));
    if (tab === "terminal") {
      setOpenedTerminalIds((current) =>
        current.includes(selected.id) ? current : [...current, selected.id],
      );
      const provider = terminalProviders[selected.id] ?? "codex";
      const key = terminalContextKey(selected.id, provider);
      setOpenedProviderKeys((current) =>
        current.includes(key) ? current : [...current, key],
      );
    }
  }

  function selectTerminalProvider(taskId: string, provider: TerminalProvider) {
    setTerminalProviders((current) => {
      const next = { ...current, [taskId]: provider };
      window.localStorage.setItem(TERMINAL_PROVIDERS_KEY, JSON.stringify(next));
      return next;
    });
    const key = terminalContextKey(taskId, provider);
    setOpenedProviderKeys((current) =>
      current.includes(key) ? current : [...current, key],
    );
  }

  function updateTerminalConversation(
    taskId: string,
    provider: TerminalProvider,
    conversation?: CodexConversation,
  ) {
    const contextKey = terminalContextKey(taskId, provider);
    if (conversation) {
      selectedConversationIdsRef.current[contextKey] = conversation.id;
    } else {
      delete selectedConversationIdsRef.current[contextKey];
    }
    setTerminalContexts((current) => ({
      ...current,
      [contextKey]: {
        conversation,
        workflow:
          current[contextKey]?.conversation?.id === conversation?.id
            ? current[contextKey]?.workflow
            : undefined,
        loading: Boolean(conversation),
      },
    }));
    if (!conversation) return;
    void terminalApi
      .workflow(provider, taskId, conversation.id)
      .then((workflow) => {
        setTerminalContexts((current) => {
          if (current[contextKey]?.conversation?.id !== conversation.id) {
            return current;
          }
          return {
            ...current,
            [contextKey]: {
              ...current[contextKey],
              workflow,
              loading: false,
            },
          };
        });
      })
      .catch(() => {
        setTerminalContexts((current) => ({
          ...current,
          ...(current[contextKey]?.conversation?.id === conversation.id
            ? {
                [contextKey]: {
                  ...current[contextKey],
                  loading: false,
                },
              }
            : {}),
        }));
      });
  }

  function updateTerminalWorkflow(
    taskId: string,
    provider: TerminalProvider,
    workflow: CodexWorkflowSnapshot,
  ) {
    const contextKey = terminalContextKey(taskId, provider);
    setTerminalContexts((current) => {
      if (current[contextKey]?.conversation?.id !== workflow.conversationId) {
        return current;
      }
      return {
        ...current,
        [contextKey]: {
          ...current[contextKey],
          workflow,
          loading: false,
        },
      };
    });
  }

  async function setStatus(status: TaskStatus) {
    if (!selected) return;
    try {
      await api.setTaskStatus(selected.id, status);
      await refresh();
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function removeTask() {
    if (!selected) return;
    const taskId = selected.id;
    const restoreTerminal = openedTerminalIds.includes(taskId);
    const restoreProviderKeys = openedProviderKeys.filter(
      (key) => key.split(":", 1)[0] === taskId,
    );
    // Unmount live terminals first so PTY output cannot block webview IPC
    // while the delete command stops process trees.
    flushSync(() => {
      setOpenedTerminalIds((current) =>
        current.filter((openedId) => openedId !== taskId),
      );
      setOpenedProviderKeys((current) =>
        current.filter((key) => key.split(":", 1)[0] !== taskId),
      );
    });
    try {
      await api.deleteTask(taskId);
      setSelectedId("");
      await refresh();
    } catch (error) {
      if (restoreTerminal) {
        setOpenedTerminalIds((current) =>
          current.includes(taskId) ? current : [...current, taskId],
        );
      }
      if (restoreProviderKeys.length > 0) {
        setOpenedProviderKeys((current) => [
          ...current,
          ...restoreProviderKeys.filter((key) => !current.includes(key)),
        ]);
      }
      notify(errorMessage(error), true);
    }
  }

  async function stopInstance(instance: GameInstance) {
    try {
      await api.stopInstance(instance.id);
      await refreshInstances();
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function toggleInstanceWindow(instance: GameInstance) {
    const current = instance.profile?.visibilityMode ?? "visible";
    try {
      await api.setInstanceWindowVisibility(
        instance.id,
        current === "background" ? "visible" : "background",
      );
      await refreshInstances();
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function deleteInstance(instance: GameInstance) {
    if (!window.confirm(t("deleteInstanceConfirm"))) return;
    try {
      await api.deleteInstance(instance.id);
      if (detail?.id === instance.id) setDetail(undefined);
      await refreshInstances();
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  return (
    <div className="workspace tasks-workspace">
      <aside className="task-list-pane">
        <div className="pane-header">
          <h1>{t("tasks")}</h1>
          <button
            className="icon-button primary-icon"
            title={t("newTask")}
            onClick={() => setShowCreate(true)}
          >
            <CirclePlus size={18} />
          </button>
        </div>
        <div className="task-list">
          {tasks.map((task) => (
            <button
              key={task.id}
              className={`task-row ${task.id === selectedId ? "selected" : ""}`}
              onClick={() => setSelectedId(task.id)}
            >
              <span className={`status-dot status-${task.status}`} />
              <span className="task-row-copy">
                <strong>{task.title}</strong>
                <small>{t(task.status)}</small>
              </span>
              <ChevronRight size={16} />
            </button>
          ))}
          {tasks.length === 0 && <div className="empty-state">{t("noTasks")}</div>}
        </div>
      </aside>

      <section className="content-pane">
        {!selected ? (
          <div className="empty-state large">{t("selectTask")}</div>
        ) : (
          <>
            <header className="content-header">
              <div>
                <div className="eyebrow">{t(selected.status)}</div>
                <h2>{selected.title}</h2>
                {selected.description && <p>{selected.description}</p>}
                <code className="task-workspace-path" title={selected.workspacePath}>
                  {t("taskWorkspace")}: {selected.workspacePath}
                </code>
              </div>
              <div className="header-actions">
                {selected.status === "active" && (
                  <button className="secondary-button" onClick={() => setStatus("completed")}>
                    <Check size={16} />
                    {t("complete")}
                  </button>
                )}
                {selected.status !== "active" && (
                  <button className="secondary-button" onClick={() => setStatus("active")}>
                    <RotateCcw size={16} />
                    {t("reactivate")}
                  </button>
                )}
                {selected.status !== "archived" && (
                  <button className="icon-button" onClick={() => setStatus("archived")} title={t("archiveTask")}>
                    <Archive size={17} />
                  </button>
                )}
                <button className="icon-button danger-icon" onClick={removeTask} title={t("delete")}>
                  <Trash2 size={17} />
                </button>
              </div>
            </header>

            <CodexWorkflowBar
              provider={selectedProvider}
              conversation={selectedTerminalContext?.conversation}
              workflow={selectedTerminalContext?.workflow}
              loading={selectedTerminalContext?.loading}
              t={t}
            />

            <div className="task-tabs" role="tablist">
              <button
                className={selectedTab === "instances" ? "selected" : ""}
                role="tab"
                aria-selected={selectedTab === "instances"}
                onClick={() => selectTab("instances")}
              >
                <Gamepad2 size={16} />
                {t("instances")}
              </button>
              <button
                className={selectedTab === "terminal" ? "selected" : ""}
                role="tab"
                aria-selected={selectedTab === "terminal"}
                onClick={() => selectTab("terminal")}
              >
                <SquareTerminal size={16} />
                {t("terminal")}
              </button>
            </div>

            <div className="task-tab-content">
              <div
                className="instance-tab-panel"
                hidden={selectedTab !== "instances"}
              >
                <div className="section-heading instances-heading">
                  <div>
                    <h3>{t("instances")}</h3>
                    <span>{taskInstances.length}</span>
                  </div>
                  <button
                    className="primary-button"
                    disabled={selected.status !== "active"}
                    onClick={() => setShowLaunch(true)}
                  >
                    <Play size={16} />
                    {t("launch")}
                  </button>
                </div>

                <div className="table-wrap instances-table">
                  <table>
                    <thead>
                      <tr>
                        <th>{t("instanceName")}</th>
                        <th>{t("launchMode")}</th>
                        <th>{t("processState")}</th>
                        <th>{t("connectionState")}</th>
                        <th>{t("pid")}</th>
                        <th>{t("startedAt")}</th>
                        <th aria-label="Actions" />
                      </tr>
                    </thead>
                    <tbody>
                      {taskInstances.map((instance) => (
                        <tr key={instance.id}>
                          <td className="strong-cell">{instance.name}</td>
                          <td>{instance.mode ? t(instance.mode) : "-"}</td>
                          <td>
                            <span className={`state-badge state-${instance.processState}`}>
                              {t(instance.processState)}
                            </span>
                          </td>
                          <td>
                            <span className={`state-badge state-${instance.connectionState}`}>
                              {t(instance.connectionState)}
                            </span>
                          </td>
                          <td className="mono-cell">{instance.pid ?? "-"}</td>
                          <td>{formatDate(instance.startedAt)}</td>
                          <td className="row-actions">
                            {instance.processState === "running" && (
                              <>
                                {instance.origin === "managed" && (
                                  <button
                                    className="icon-button"
                                    onClick={() => toggleInstanceWindow(instance)}
                                    title={
                                      instance.profile?.visibilityMode === "background"
                                        ? t("showInstanceWindow")
                                        : t("hideInstanceWindow")
                                    }
                                  >
                                    {instance.profile?.visibilityMode === "background" ? (
                                      <Eye size={16} />
                                    ) : (
                                      <EyeOff size={16} />
                                    )}
                                  </button>
                                )}
                                <button
                                  className="icon-button danger-icon"
                                  onClick={() => stopInstance(instance)}
                                  title={t("stop")}
                                >
                                  <Power size={16} />
                                </button>
                              </>
                            )}
                            <button
                              className="text-button"
                              onClick={() => setDetail(instance)}
                            >
                              {t("details")}
                            </button>
                            <button
                              className="icon-button danger-icon"
                              disabled={isActiveInstance(instance)}
                              onClick={() => deleteInstance(instance)}
                              title={t("deleteInstance")}
                            >
                              <Trash2 size={16} />
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  {taskInstances.length === 0 && (
                    <div className="empty-state large">{t("instances")}: 0</div>
                  )}
                </div>
              </div>

              {openedTerminalIds.map((taskId) => (
                <div
                  key={taskId}
                  className="terminal-tab-panel"
                  hidden={
                    selected.id !== taskId || selectedTab !== "terminal"
                  }
                >
                  <div className="terminal-provider-bar">
                    <span>{t("terminalProvider")}</span>
                    <div className="segmented-control">
                      {(["codex", "grok"] as const).map((provider) => (
                        <button
                          key={provider}
                          className={
                            (terminalProviders[taskId] ?? "codex") === provider
                              ? "selected"
                              : ""
                          }
                          onClick={() => selectTerminalProvider(taskId, provider)}
                        >
                          {provider === "codex" ? "Codex" : "Grok"}
                        </button>
                      ))}
                    </div>
                  </div>
                  {(["codex", "grok"] as const).map((provider) => {
                    const key = terminalContextKey(taskId, provider);
                    if (!openedProviderKeys.includes(key)) return null;
                    const selectedTaskProvider =
                      terminalProviders[taskId] ?? "codex";
                    return (
                      <div
                        className="terminal-provider-panel"
                        hidden={selectedTaskProvider !== provider}
                        key={provider}
                      >
                        <DevelopmentTerminalView
                          provider={provider}
                          taskId={taskId}
                          visible={
                            selected.id === taskId &&
                            selectedTab === "terminal" &&
                            selectedTaskProvider === provider
                          }
                          availability={terminalAvailability[provider]}
                          t={t}
                          onRefreshAvailability={refreshTerminalAvailability}
                          onConversationChange={(conversation) =>
                            updateTerminalConversation(
                              taskId,
                              provider,
                              conversation,
                            )
                          }
                          onWorkflowChange={(workflow) =>
                            updateTerminalWorkflow(taskId, provider, workflow)
                          }
                        />
                      </div>
                    );
                  })}
                </div>
              ))}
            </div>
          </>
        )}
      </section>

      {showCreate && (
        <CreateTaskModal
          t={t}
          onClose={() => setShowCreate(false)}
          onCreated={async (task) => {
            setSelectedId(task.id);
            await refresh();
          }}
        />
      )}
      {showLaunch && selected && (
        <LaunchInstanceModal
          taskId={selected.id}
          executablePath={settings?.gameExecutablePath ?? ""}
          taskInstances={taskInstances}
          t={t}
          onClose={() => setShowLaunch(false)}
          onLaunched={refreshInstances}
        />
      )}
      {detail && (
        <InstanceDetailsModal
          instance={detail}
          t={t}
          onClose={() => setDetail(undefined)}
          onChanged={refreshInstances}
          onDeleted={async () => {
            setDetail(undefined);
            await refreshInstances();
          }}
        />
      )}
    </div>
  );
}

function CreateTaskModal({
  t,
  onClose,
  onCreated,
}: {
  t: (key: MessageKey) => string;
  onClose: () => void;
  onCreated: (task: DevelopmentTask) => void;
}) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [error, setError] = useState("");

  async function create() {
    try {
      onCreated(await api.createTask(title, description));
      onClose();
    } catch (value) {
      setError(errorMessage(value));
    }
  }

  return (
    <Modal title={t("newTask")} onClose={onClose}>
      <div className="form-stack">
        <label className="field">
          <span>{t("taskTitle")}</span>
          <input
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            autoFocus
          />
        </label>
        <label className="field">
          <span>{t("description")}</span>
          <textarea
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            rows={4}
          />
        </label>
      </div>
      {error && <div className="inline-error">{error}</div>}
      <footer className="modal-actions">
        <button className="secondary-button" onClick={onClose}>
          {t("cancel")}
        </button>
        <button className="primary-button" disabled={!title.trim()} onClick={create}>
          {t("create")}
        </button>
      </footer>
    </Modal>
  );
}

function formatDate(value?: string) {
  return value ? new Date(value).toLocaleString() : "-";
}

function isActiveInstance(instance: GameInstance) {
  return (
    ["launching", "running", "stopping"].includes(instance.processState) ||
    instance.connectionState === "connected"
  );
}

type TaskTab = "instances" | "terminal";

interface TaskTerminalContext {
  conversation?: CodexConversation;
  workflow?: CodexWorkflowSnapshot;
  loading?: boolean;
}

const TERMINAL_PROVIDERS_KEY = "abya.taskTerminalProviders";

function terminalContextKey(taskId: string, provider: TerminalProvider) {
  return `${taskId}:${provider}`;
}

function loadTerminalProviders(): Record<string, TerminalProvider> {
  try {
    const value = JSON.parse(
      window.localStorage.getItem(TERMINAL_PROVIDERS_KEY) ?? "{}",
    ) as Record<string, string>;
    return Object.fromEntries(
      Object.entries(value).filter(
        (entry): entry is [string, TerminalProvider] =>
          entry[1] === "codex" || entry[1] === "grok",
      ),
    );
  } catch {
    return {};
  }
}
