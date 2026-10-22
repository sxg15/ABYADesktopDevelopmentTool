import { useEffect, useMemo, useRef, useState } from "react";
import { flushSync } from "react-dom";
import {
  Archive,
  BookOpen,
  Check,
  ChevronRight,
  CirclePlus,
  Eye,
  EyeOff,
  Gamepad2,
  ListTree,
  Maximize2,
  Minimize2,
  Play,
  Power,
  RotateCcw,
  SquareTerminal,
  Trash2,
  Workflow,
} from "lucide-react";
import { Modal } from "../../app/Modal";
import { LaunchInstanceModal } from "../instances/LaunchInstanceModal";
import { AcceptanceLauncher } from "./AcceptanceLauncher";
import { InstanceDetailsModal } from "../instances/InstanceDetailsModal";
import { DevelopmentTerminalView } from "../terminal/CodexTerminalView";
import { TerminalContextPanel } from "../terminal/TerminalContextPanel";
import { CodexWorkflowBar } from "./CodexWorkflowBar";
import { ProductionWorkbench } from "./ProductionWorkbench";

import { TaskGuidance } from "./TaskGuidance";
import { TaskDescription } from "./TaskDescription";
import type { GuidancePending } from "./taskGuidanceState";
import { SkillLibrary } from "./SkillLibrary";
import { productionText } from "./productionText";
import { productionApi, continuationMessages } from "../../shared/production";
import type { IntakeQuestionGroup, ProductionDocument } from "../../shared/production";
import type { TerminalConnectionRequest } from "../terminal/terminalConnection";
import { useTaskControl } from "../terminal/useTaskControl";
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
import type { AttentionTarget, NotificationActivation } from "../../shared/notifications";

export function TasksView({
  settings,
  t,
  notify,
  notificationNavigation,
  onNotificationHandled,
}: {
  settings?: AppSettings;
  t: (key: MessageKey) => string;
  notify: (message: string, error?: boolean) => void;
  notificationNavigation?: NotificationActivation;
  onNotificationHandled?: (id: string) => void;
}) {
  const [tasks, setTasks] = useState<DevelopmentTask[]>([]);
  const [instances, setInstances] = useState<GameInstance[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const selectedIdRef = useRef(selectedId); selectedIdRef.current = selectedId;
  const [pendingNavigation, setPendingNavigation] = useState<NotificationActivation>();
  const [requestedQuestion, setRequestedQuestion] = useState<{taskId: string; requestId: string; groupId: string}>();
  const navigationRevision = useRef(0);
  const onNotificationHandledRef = useRef(onNotificationHandled); onNotificationHandledRef.current = onNotificationHandled;
  const [guidancePending, setGuidancePending] = useState<GuidancePending>();
  const [requestedReview, setRequestedReview] = useState<{taskId:string;document:ProductionDocument}>();
  const [requestedFeedback, setRequestedFeedback] = useState<{taskId:string;id:string}>();
  const currentPending = guidancePending?.taskId === selectedId ? guidancePending : undefined;
  const [showCreate, setShowCreate] = useState(false);
  const [showLaunch, setShowLaunch] = useState(false);
  const [detail, setDetail] = useState<GameInstance>();
  const [taskTabs, setTaskTabs] = useState<Record<string, TaskTab>>({});
  const [openedSkillIds, setOpenedSkillIds] = useState<string[]>([]);
  const [skillSections, setSkillSections] = useState<Record<string, "skills" | "cases">>({});
  const [terminalFocus, setTerminalFocus] = useState(false);
  const [terminalContextOpen, setTerminalContextOpen] = useState(() => window.innerWidth >= 1440);
  const [terminalContextDialog, setTerminalContextDialog] = useState(false);
  const tabContentRef = useRef<HTMLDivElement>(null);
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
  const [connectionRequest, setConnectionRequest] = useState<TerminalConnectionRequest>();
  const pendingConnection = useRef<TerminalConnectionRequest | undefined>(undefined);

  useEffect(() => {
    if (pendingConnection.current && pendingConnection.current.taskId !== selectedId) {
      pendingConnection.current.finish(new Error("已切换任务，未发送继续请求。"));
    }
  }, [selectedId]);
  useEffect(() => () => pendingConnection.current?.finish(new Error("页面已关闭，未发送继续请求。")), []);

  function ensureIntakeConnection(group: Pick<IntakeQuestionGroup, "provider" | "conversationId">): Promise<void> {
    return new Promise((resolve, reject) => {
      if (group.provider !== "codex") { reject(new Error("此历史会话的服务商已移除，不能恢复或转接到 Codex。请在 Codex 对话中读取已保存的任务记录。")); return; }
      if (!selectedId || pendingConnection.current) { reject(new Error("正在连接，请稍候。")); return; }
      const request: TerminalConnectionRequest = { id: crypto.randomUUID(), taskId: selectedId,
        provider: group.provider, conversationId: group.conversationId,
        finish: error => {
          if (pendingConnection.current !== request) return;
          window.clearTimeout(timeout); pendingConnection.current = undefined; setConnectionRequest(undefined);
          if (error) reject(error); else resolve();
        } };
      const timeout = window.setTimeout(() => request.finish(new Error("原会话连接超时，答案已保存，未发送继续请求。")), 60000);
      pendingConnection.current = request; setConnectionRequest(request);
      selectTerminalProvider(selectedId, group.provider);
      setOpenedTerminalIds(ids => ids.includes(selectedId) ? ids : [...ids, selectedId]);
      setTaskTabs(tabs => ({ ...tabs, [selectedId]: "terminal" }));
    });
  }

  async function continueSelectedTask(_message: string, afterRevision?: number) {
    const conversation = selectedTerminalContext?.conversation;
    if (!selected || !conversation) throw new Error("请打开原会话后点击继续任务，决定已保存。");
    let result = await productionApi.continueTask(selected.id, conversation.id, afterRevision);
    if (result.status === "needsConnection" && result.requestId) {
      await ensureIntakeConnection({ provider: "codex", conversationId: conversation.id });
      result = await productionApi.flushContinuation(selected.id, conversation.id, result.requestId);
    }
    window.dispatchEvent(new Event("abya:control-changed"));
    notify(continuationMessages[result.status]);
  }

  async function repairSelectedIssue(issueId: string, revision: number) {
    const conversation = selectedTerminalContext?.conversation;
    const taskId = selected?.id;
    if (!taskId || !conversation || selectedProvider !== "codex") throw new Error("请先打开本任务的 Codex 原会话，再请求 AI 修复。");
    if (!conversation.nativeSessionId) await ensureIntakeConnection({provider:"codex",conversationId:conversation.id});
    let result = await productionApi.repairIssue(taskId,conversation.id,issueId,revision);
    if (result.status === "needsConnection" && result.requestId) {
      await ensureIntakeConnection({provider:"codex",conversationId:conversation.id});
      result = await productionApi.flushContinuation(taskId,conversation.id,result.requestId);
    }
    const messages:Record<string,string>={started:"AI 已收到此问题的修复请求，将检查原因并验证修复结果。",resumed:"已恢复此问题的修复。",queued:"此问题已排队，当前回复结束后由 AI 处理，无需重复点击。",running:"此问题已有处理请求，未重复发送。",completed:"本次 AI 处理已结束，请查看问题记录与复验结果；不代表问题自动解决。",superseded:"问题记录已变化或已解决，未发送过期修复请求，请刷新后核对。",needsReview:"上次请求结果需要核对，请查看原会话，未自动重发。",paused:"修复请求已暂停；请在原会话核对后继续。"};
    const message=messages[result.status]??continuationMessages[result.status];
    window.dispatchEvent(new Event("abya:control-changed"));notify(message);
    return message;
  }

  const selected = tasks.find((task) => task.id === selectedId);
  const [taskListOpen, setTaskListOpen] = useState(false);
  const selectedTab = selected ? (taskTabs[selected.id] ?? "production") : "instances";
  useEffect(() => {
    const area = tabContentRef.current;
    if (!terminalContextOpen || selectedTab !== "terminal" || !area) return;
    const fit = () => { const box = area.getBoundingClientRect(); if (box.width > 0 && (box.width < 980 || box.height < 420)) setTerminalContextOpen(false); };
    fit(); const observer = new ResizeObserver(fit); observer.observe(area); return () => observer.disconnect();
  }, [terminalContextOpen, selectedId, selectedTab]);
  useEffect(() => setTerminalContextDialog(false), [selectedId, selectedTab]);
  function toggleTerminalContext() {
    const box = tabContentRef.current?.getBoundingClientRect();
    if (box && box.width > 0 && (box.width < 980 || box.height < 420)) { setTerminalContextOpen(false); setTerminalContextDialog(true); }
    else setTerminalContextOpen(value => !value);
  }
  const selectedProvider = selected
    ? (terminalProviders[selected.id] ?? "codex")
    : "codex";
  const selectedContextKey = selected
    ? terminalContextKey(selected.id, selectedProvider)
    : "";
  const selectedTerminalContext = selected
    ? terminalContexts[selectedContextKey]
    : undefined;
  const selectedControl=useTaskControl(selected?.id??"",selectedTerminalContext?.conversation?.id,selectedProvider);
  useEffect(() => {
    if (selectedTab === "skills" && selectedId) setOpenedSkillIds(ids => ids.includes(selectedId) ? ids : [...ids, selectedId]);
  }, [selectedId, selectedTab]);
  useEffect(() => {
    if (!terminalFocus) return;
    const exit = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.isComposing && !document.querySelector('[aria-modal="true"]')) {
        event.preventDefault(); event.stopPropagation(); setTerminalFocus(false);
      }
    };
    window.addEventListener("keydown", exit, true); return () => window.removeEventListener("keydown", exit, true);
  }, [terminalFocus]);
  useEffect(() => {
    if (!notificationNavigation?.target) return;
    const activation = notificationNavigation, target = activation.target!;
    const revision = ++navigationRevision.current;
    let alive = true;
    void api.listTasks().then(next => {
      if (!alive || revision !== navigationRevision.current) return;
      setTasks(next);
      if (!next.some(task => task.id === target.taskId)) {
        notify(t("notificationTaskMissing"));
        onNotificationHandledRef.current?.(activation.id);
        return;
      }
      setShowCreate(false); setShowLaunch(false); setDetail(undefined);
      setRequestedReview(undefined); setRequestedQuestion(undefined);
      setRequestedFeedback(undefined);
      setSelectedId(target.taskId);
      setPendingNavigation(activation);
    }).catch(error => {
      if (alive) { notify(errorMessage(error), true); onNotificationHandledRef.current?.(activation.id); }
    });
    return () => { alive = false; };
  }, [notificationNavigation?.id]);

  function showNotificationStage(taskId: string, stage: string) {
    try { sessionStorage.setItem("abya:production-stage:" + taskId, stage); } catch { /* Optional navigation memory. */ }
    setTaskTabs(tabs => ({ ...tabs, [taskId]: "production" }));
    window.dispatchEvent(new CustomEvent("abya:show-stage", { detail: { taskId, stage } }));
  }

  async function openAttentionTerminal(target?: AttentionTarget) {
    if (!target?.provider || !target.conversationId) { selectTab("terminal"); return; }
    // Attach only the exact existing conversation; no answer/approval/Continue is sent.
    selectedConversationIdsRef.current[terminalContextKey(target.taskId, target.provider)] = target.conversationId;
    try { await ensureIntakeConnection({ provider: target.provider, conversationId: target.conversationId }); }
    catch (error) { notify(errorMessage(error), true); }
  }

  useEffect(() => {
    const activation = pendingNavigation, target = activation?.target;
    if (!activation || !target || selected?.id !== target.taskId) return;
    setPendingNavigation(undefined);
    const revision = navigationRevision.current;
    const current = () => revision === navigationRevision.current && selectedIdRef.current === target.taskId;
    void (async () => {
      if (activation.status === "current" && target.action === "terminal") {
        await openAttentionTerminal(target);
        return;
      }
      const view = await productionApi.get(target.taskId);
      if (!current()) return;
      if (view.record) showNotificationStage(target.taskId, view.record.currentStage);
      else setTaskTabs(tabs => ({ ...tabs, [target.taskId]: "instances" }));
      if (activation.status !== "current" || selected.status !== "active") {
        notify(t(activation.status === "unavailable" ? "notificationStateUnavailable" : "notificationAlreadyHandled"));
        return;
      }
      if (target.action === "question") {
        const group = view.record?.questionGroups?.find(g => g.id === target.groupId && g.status === "pending");
        if (!group) { notify(t("notificationAlreadyHandled")); return; }
        showNotificationStage(target.taskId, group.stage ?? target.stage);
        setRequestedQuestion({taskId:target.taskId,requestId:activation.id,groupId:group.id});
      } else if (target.action === "recheck") {
        const feedback = view.record?.feedback?.find(f => f.id === target.feedbackId);
        if (!feedback || feedback.status !== "awaiting-recheck" || feedback.candidateArtifactId !== target.candidateArtifactId
          || view.actions?.recheck?.[feedback.id] !== "") { notify(t("notificationAlreadyHandled")); return; }
        showNotificationStage(target.taskId, "resources");
        setRequestedFeedback({taskId:target.taskId,id:feedback.id});
      } else if (target.action === "review") {
        const doc = view.record?.documents[target.stage];
        if (!doc || doc.revision !== target.documentRevision || doc.sha256 !== target.documentHash
          || view.actions?.review[target.stage] !== "") { notify(t("notificationAlreadyHandled")); return; }
        showNotificationStage(target.taskId, target.stage);
        setRequestedReview({taskId:target.taskId,document:doc});
      } else showNotificationStage(target.taskId, target.stage);
    })().catch(error => { if (current()) notify(errorMessage(error), true); })
      .finally(() => onNotificationHandledRef.current?.(activation.id));
  }, [pendingNavigation, selected?.id]);
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
      const codex = await terminalApi.availability("codex");
      setTerminalAvailability({ codex });
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
    <div className={`workspace tasks-workspace ${selected && !taskListOpen ? "task-list-collapsed" : ""} ${terminalFocus && selectedTab === "terminal" ? "terminal-focus" : ""}`}>
      <aside className="task-list-pane" hidden={!!selected && !taskListOpen}>
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
              onClick={() => { setSelectedId(task.id); setTaskListOpen(false); }}
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
            <header className="content-header task-content-header">
              <div className="task-heading-title">
                <span className="task-title-icon"><Gamepad2 size={24} aria-hidden="true" /></span>
                <div>
                <div className="eyebrow">{t(selected.status)}</div>
                <h2>{selected.title}</h2>
                </div>
              </div>
              <div className="header-actions">
                <button className="secondary-button" aria-expanded={taskListOpen} onClick={() => setTaskListOpen(v => !v)}>{settings?.locale === "en-US" ? "Switch task" : "切换任务"}</button>
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
              <TaskDescription key={`description:${selected.id}`} taskId={selected.id} description={selected.description}
                workspacePath={selected.workspacePath} en={settings?.locale === "en-US"} />
            </header>

            <TaskGuidance key={`guidance:${selected.id}`} taskId={selected.id} active={selected.status === "active"}
              control={selectedControl} workflow={selectedTerminalContext?.workflow} en={settings?.locale === "en-US"}
              onPendingChange={setGuidancePending} onTerminal={target => void openAttentionTerminal(target)}
              onReview={document => { setRequestedReview({taskId:selected.id,document}); selectTab("production"); }}
              onStage={(stage, feedbackId) => {
                if (feedbackId) setRequestedFeedback({taskId:selected.id,id:feedbackId});
                try { sessionStorage.setItem(`abya:production-stage:${selected.id}`, stage); } catch { /* Navigation remains available. */ }
                selectTab("production");
                window.dispatchEvent(new CustomEvent("abya:show-stage", { detail: { taskId: selected.id, stage } }));
              }} />
            <div hidden={selectedTab !== "terminal"}><CodexWorkflowBar
              control={selectedControl}
              provider={selectedProvider}
              conversation={selectedTerminalContext?.conversation}
              workflow={selectedTerminalContext?.workflow}
              loading={selectedTerminalContext?.loading}
              t={t}
            /></div>

            <div className="task-tabs" role="tablist">
              <button role="tab" className={selectedTab === "production" ? "selected" : ""} aria-selected={selectedTab === "production"} onClick={() => selectTab("production")}>
                <Workflow size={16} aria-hidden="true" />
                {productionText(settings?.locale).title}
                {!!currentPending?.questions && <span className="task-tab-badge">{settings?.locale === "en-US" ? "Questions" : "待回答"} {currentPending.questions}</span>}
                {!!currentPending?.reviews && <span className="task-tab-badge">{settings?.locale === "en-US" ? "Review" : "待审查"} {currentPending.reviews}</span>}
              </button>
              <button role="tab" className={selectedTab === "skills" ? "selected" : ""} aria-selected={selectedTab === "skills"} onClick={() => selectTab("skills")}><BookOpen size={16} aria-hidden="true" />{productionText(settings?.locale).skills}</button>
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
                {!!currentPending?.terminal && <span className="task-tab-badge terminal-badge">{settings?.locale === "en-US" ? "Action needed" : "待处理"} {currentPending.terminal}</span>}
              </button>
            </div>

            <div className="task-tab-content" ref={tabContentRef}>
              <div className="workbench-tab-panel" hidden={selectedTab !== "production"}>
              <ProductionWorkbench key={selected.id} taskId={selected.id} active={selected.status === "active"} locale={settings?.locale}
                onResumeGoal={async(goal,revision,intent,note)=>{
                  const conversation=selectedTerminalContext?.conversation;
                  let result=await productionApi.resumeGoal(selected.id,goal,conversation?.id??"",revision,intent,note);
                  if(result.status==="needsConnection"&&result.requestId){await ensureIntakeConnection({provider:"codex",conversationId:result.conversationId});result=await productionApi.flushContinuation(selected.id,result.conversationId,result.requestId);}
                  window.dispatchEvent(new Event("abya:control-changed"));return result;
                }}
                visible={selectedTab === "production"} control={selectedControl} onTerminal={() => selectTab("terminal")}
                onEnsureConnection={ensureIntakeConnection} requestedGroup={requestedQuestion?.taskId === selected.id ? requestedQuestion : undefined}
                onRepairIssue={selectedProvider === "codex" && selectedTerminalContext?.conversation ? repairSelectedIssue : undefined}
                requestedReview={requestedReview?.taskId === selected.id ? requestedReview.document : undefined} onReviewOpened={() => setRequestedReview(undefined)}
                requestedFeedback={requestedFeedback?.taskId === selected.id ? requestedFeedback : undefined} onFeedbackOpened={() => setRequestedFeedback(undefined)}
                onContinue={async (message, _group, afterRevision) => continueSelectedTask(message, afterRevision)} /></div>
              <div className="skill-tab-panel" hidden={selectedTab !== "skills"}>
                {openedSkillIds.includes(selected.id) && <SkillLibrary key={selected.id} taskId={selected.id} active={selected.status === "active"} locale={settings?.locale}
                  visible={selectedTab === "skills"} initialSection={skillSections[selected.id] ?? "skills"} onContinue={continueSelectedTask} />}
              </div>
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

                <AcceptanceLauncher key={selected.id} taskId={selected.id} visible={selectedTab === "instances"} active={selected.status === "active"} en={settings?.locale === "en-US"} />
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
                    <span>Codex</span>
                    <div className="terminal-workspace-actions">
                      <button className="secondary-button" aria-pressed={terminalContextOpen || terminalContextDialog} onClick={toggleTerminalContext}><ListTree size={15}/>{t("terminalContext")}</button>
                      <button className="text-button" aria-pressed={terminalFocus} onClick={() => setTerminalFocus(value => !value)}>{terminalFocus ? <Minimize2 size={15}/> : <Maximize2 size={15}/>} {t(terminalFocus ? "terminalExitFocus" : "terminalFocusMode")}</button>
                    </div>
                  </div>
                  <div className={`terminal-workbench ${terminalContextOpen ? "with-context" : ""}`}>
                  <div className="terminal-provider-stack">
                  {(["codex"] as const).map((provider) => {
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
                          connectionRequest={connectionRequest?.taskId === taskId && connectionRequest.provider === provider ? connectionRequest : undefined}
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
                  <div className="terminal-context-slot" hidden={!terminalContextOpen}>
                    <TerminalContextPanel workflow={terminalContexts[terminalContextKey(taskId, terminalProviders[taskId] ?? "codex")]?.workflow}
                      control={taskId === selectedId ? selectedControl : undefined} t={t} onClose={() => setTerminalContextOpen(false)}
                      onProduction={() => selectTab("production")} onArtLibrary={() => { setSkillSections(sections => ({ ...sections, [taskId]: "cases" })); selectTab("skills"); }}/>
                  </div>
                  </div>
                </div>
              ))}
            </div>
          </>
        )}
      </section>

      {terminalContextDialog && selected && <Modal title={t("terminalContext")} onClose={() => setTerminalContextDialog(false)}>
        <div className="terminal-context-dialog"><TerminalContextPanel workflow={selectedTerminalContext?.workflow} control={selectedControl} t={t}
          onClose={() => setTerminalContextDialog(false)} onProduction={() => { setTerminalContextDialog(false); selectTab("production"); }}
          onArtLibrary={() => { setTerminalContextDialog(false); setSkillSections(sections => ({...sections,[selected.id]:"cases"})); selectTab("skills"); }}/></div>
      </Modal>}
      {showCreate && (
        <CreateTaskModal
          t={t}
          locale={settings?.locale}
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
  locale,
  onClose,
  onCreated,
}: {
  t: (key: MessageKey) => string;
  locale?: string;
  onClose: () => void;
  onCreated: (task: DevelopmentTask) => void;
}) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [error, setError] = useState("");

  const [mode, setMode] = useState("full");
  const [questions, setQuestions] = useState("ask");
  const [playerMode, setPlayerMode] = useState("unspecified");
  const [created, setCreated] = useState<DevelopmentTask>();
  const [creating, setCreating] = useState(false);
  const s = productionText(locale);

  async function create() {
    setCreating(true);
    try {
      const task = created ?? await api.createTask(title, description);
      setCreated(task);
      if (mode === "full") {
        const existing = await productionApi.get(task.id);
        if (!existing.record) await productionApi.update(task.id, 0, "initialize", { questionMode: questions, playerMode });
      }
      onCreated(task);
      onClose();
    } catch (value) {
      setError(errorMessage(value));
    } finally { setCreating(false); }
  }

  return (
    <Modal title={t("newTask")} onClose={onClose}>
      <div className="form-stack">
        <label className="field"><span>{s.taskMode}</span><select value={mode} disabled={creating || Boolean(created)} onChange={e => setMode(e.target.value)}><option value="full">{s.fullTask}</option><option value="scoped">{s.scopedTask}</option></select></label>
        {mode === "full" && <label className="field"><span>{s.questionMode}</span><select value={questions} disabled={creating} onChange={e => setQuestions(e.target.value)}><option value="ask">{s.ask}</option><option value="no-followup">{s.noAsk}</option></select></label>}
        {mode === "full" && <label className="field">玩法人数<select value={playerMode} disabled={creating} onChange={e => setPlayerMode(e.target.value)}>
          <option value="unspecified">未明确，在需求阶段敲定</option><option value="single">单人</option><option value="multiplayer">多人</option>
        </select></label>}
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
        <button className="primary-button" disabled={!title.trim() || creating} onClick={create}>
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

type TaskTab = "instances" | "terminal" | "production" | "skills";

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
          entry[1] === "codex",
      ),
    );
  } catch {
    return {};
  }
}
