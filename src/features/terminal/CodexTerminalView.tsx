import { useEffect, useRef, useState } from "react";
import { Channel } from "@tauri-apps/api/core";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal as XTerm } from "@xterm/xterm";
import {
  ArrowDown,
  Archive,
  ArchiveRestore,
  FolderPlus,
  Check,
  CirclePlus,
  Copy,
  Pencil,
  RefreshCw,
  ScrollText,
  Square,
  SquareTerminal,
  Trash2,
  X,
} from "lucide-react";
import "@xterm/xterm/css/xterm.css";
import { errorMessage, terminalApi } from "../../shared/api";
import type {
  CodexConversation,
  CodexTerminalAvailability,
  CodexTerminalEvent,
  CodexTerminalState,
  CodexWorkflowSnapshot,
  TerminalProvider,
} from "../../shared/types";
import type { MessageKey } from "../../i18n";
import { clampTerminalSize, hostHasTerminalLayout } from "./terminalSize";
import { installTerminalClipboard, terminalInputQueue } from "./terminalClipboard";
import { Modal } from "../../app/Modal";
import type { TerminalConnectionRequest } from "./terminalConnection";
import { productionApi, continuationMessages } from "../../shared/production";
import { useTaskControl, controlText, controlLabel } from "./useTaskControl";
import { ExecutionSettings } from "./ExecutionSettings";

const MAX_HISTORY_CHARACTERS = 2 * 1024 * 1024;
const HISTORY_TRUNCATED_MESSAGE =
  "[Earlier terminal output omitted; showing recent output.]\n";

export function DevelopmentTerminalView({
  connectionRequest,
  provider,
  taskId,
  visible,
  availability,
  t,
  onRefreshAvailability,
  onConversationChange,
  onWorkflowChange,
}: {
  connectionRequest?: TerminalConnectionRequest;
  provider: TerminalProvider;
  taskId: string;
  visible: boolean;
  availability?: CodexTerminalAvailability;
  t: (key: MessageKey) => string;
  onRefreshAvailability: () => void;
  onConversationChange?: (conversation?: CodexConversation) => void;
  onWorkflowChange?: (workflow: CodexWorkflowSnapshot) => void;
}) {
  const [conversations, setConversations] = useState<CodexConversation[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [openedIds, setOpenedIds] = useState<string[]>([]);
  const [editingId, setEditingId] = useState("");
  const [editingTitle, setEditingTitle] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [showArchived, setShowArchived] = useState(false);
  const [projectFolder, setProjectFolder] = useState<string>();
  const [recoveryCandidates, setRecoveryCandidates] = useState<{ id: string; startedAt: string }[]>([]);
  const [recoveryId, setRecoveryId] = useState("");
  const [metrics, setMetrics] = useState<string>();
  useEffect(() => { setMetrics(undefined); setRecoveryId(""); setRecoveryCandidates([]); }, [taskId, selectedId]);
  const [conversationBusy, setConversationBusy] = useState(false);
  const refreshRevision = useRef(0);
  const onConversationChangeRef = useRef(onConversationChange);
  const onWorkflowChangeRef = useRef(onWorkflowChange);

  useEffect(() => {
    onConversationChangeRef.current = onConversationChange;
  }, [onConversationChange]);

  useEffect(() => {
    onWorkflowChangeRef.current = onWorkflowChange;
  }, [onWorkflowChange]);

  async function refreshConversations() {
    const revision = ++refreshRevision.current;
    setLoading(true);
    try {
      let result = await terminalApi.listConversations(provider, taskId);
      if (revision !== refreshRevision.current) return;
      if (result.length === 0 && availability?.available && !connectionRequest) {
        result = [await terminalApi.createConversation(provider, taskId)];
      }
      if (revision !== refreshRevision.current) return;
      setConversations(result);
      const active = result.filter(item => !item.archived);
      const preferred = connectionRequest ? active.find(c => c.id === connectionRequest.conversationId) : active[0];
      setSelectedId((current) =>
        !connectionRequest && active.some((conversation) => conversation.id === current)
          ? current
          : preferred?.id ?? "",
      );
      setOpenedIds((current) => [
        ...current.filter((id) =>
          active.some((conversation) => conversation.id === id),
        ),
        ...(preferred && !current.includes(preferred.id) ? [preferred.id] : []),
      ]);
      setError(result.find(item => item.nativeSyncError)?.nativeSyncError ?? "");
    } catch (value) {
      setError(errorMessage(value));
      connectionRequest?.finish(value);
    } finally {
      if (revision === refreshRevision.current) setLoading(false);
    }
  }

  useEffect(() => {
    setShowArchived(false);
    setProjectFolder(undefined);
    void refreshConversations();
    return () => { refreshRevision.current++; };
  }, [provider, taskId, availability?.available]);

  useEffect(() => {
    if (!connectionRequest || loading) return;
    if (!availability?.available) { connectionRequest.finish(new Error("终端提供方不可用，未发送继续请求。")); return; }
    const target = conversations.find(c => c.id === connectionRequest.conversationId && !c.archived);
    if (!target) { connectionRequest.finish(new Error("原会话不存在或已归档，未创建替代对话。")); return; }
    setShowArchived(false); setSelectedId(target.id);
    setOpenedIds(ids => ids.includes(target.id) ? ids : [...ids, target.id]);
  }, [connectionRequest, loading, availability?.available, conversations]);

  useEffect(() => {
    const refresh = () => { if (visible && provider === "codex" && !conversationBusy) void refreshConversations(); };
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [visible, provider, taskId, conversationBusy]);

  async function setArchived(conversation: CodexConversation) {
    setConversationBusy(true); refreshRevision.current++;
    try {
      const updated = await terminalApi.setArchived(taskId, conversation.id, !conversation.archived);
      setConversations(current => current.map(item => item.id === updated.id ? updated : item));
      setOpenedIds(current => current.filter(id => id !== updated.id));
      if (selectedId === updated.id) setSelectedId("");
      if (!updated.archived) setShowArchived(false);
      setError("");
    } catch (value) { setError(errorMessage(value)); }
    finally { setConversationBusy(false); setLoading(false); }
  }

  useEffect(() => {
    const conversation = conversations.find((item) => item.id === selectedId);
    onConversationChangeRef.current?.(conversation);
    if (!conversation) return;
    let cancelled = false;
    void terminalApi
      .workflow(provider, taskId, conversation.id)
      .then((workflow) => {
        if (!cancelled) onWorkflowChangeRef.current?.(workflow);
      })
      .catch(() => {
        // A live workflow event or task-level refresh can provide the snapshot.
      });
    return () => {
      cancelled = true;
    };
  }, [conversations, provider, selectedId, taskId]);

  async function createConversation() {
    try {
      const conversation = await terminalApi.createConversation(provider, taskId);
      setConversations((current) => [conversation, ...current]);
      setShowArchived(false);
      setSelectedId(conversation.id);
      setOpenedIds((current) => [...current, conversation.id]);
      setError("");
    } catch (value) {
      setError(errorMessage(value));
    }
  }

  async function renameConversation(conversation: CodexConversation) {
    const title = editingTitle.trim();
    if (!title) return;
    try {
      const renamed = await terminalApi.renameConversation(
        provider,
        taskId,
        conversation.id,
        title,
      );
      setConversations((current) =>
        current.map((item) => (item.id === renamed.id ? renamed : item)),
      );
      setEditingId("");
      setEditingTitle("");
      setError("");
    } catch (value) {
      setError(errorMessage(value));
    }
  }

  async function deleteConversation(conversation: CodexConversation) {
    if (!window.confirm(t("deleteConversationConfirm"))) return;
    try {
      await terminalApi.deleteConversation(provider, taskId, conversation.id);
      const allRemaining = conversations.filter((item) => item.id !== conversation.id);
      const remaining = allRemaining.filter(item => !item.archived);
      setConversations(allRemaining);
      setOpenedIds((current) => current.filter((id) => id !== conversation.id));
      if (editingId === conversation.id) {
        setEditingId("");
        setEditingTitle("");
      }
      if (selectedId === conversation.id) {
        setSelectedId(remaining[0]?.id ?? "");
        if (remaining[0]) {
          setOpenedIds((current) =>
            current.includes(remaining[0].id)
              ? current
              : [...current, remaining[0].id],
          );
        }
      }
    } catch (value) {
      setError(errorMessage(value));
    }
  }

  if (!availability) {
    return (
      <div className="terminal-unavailable">
        <span>{t(provider === "codex" ? "checkingCodex" : "checkingGrok")}</span>
      </div>
    );
  }

  if (!availability.available) {
    return (
      <div className="terminal-unavailable">
        <SquareTerminal size={28} />
        <strong>
          {t(provider === "codex" ? "codexUnavailable" : "grokUnavailable")}
        </strong>
        <span>{t(provider === "codex" ? "codexNotFound" : "grokNotFound")}</span>
        <button
          className="icon-button"
          title={t("refresh")}
          onClick={onRefreshAvailability}
        >
          <RefreshCw size={16} />
        </button>
      </div>
    );
  }

  return (
    <div className="codex-conversation-workspace">
      <aside className="conversation-pane">
        <div className="conversation-pane-header">
          <strong>
            {t(provider === "codex" ? "codexConversations" : "grokConversations")}
          </strong>
          <button
            className="icon-button"
            title={t("newConversation")}
            disabled={conversationBusy}
            onClick={createConversation}
          >
            <CirclePlus size={16} />
          </button>
        </div>
        {provider === "codex" && <div className="conversation-filter">
          <button className="icon-button" title={t("codexProjectHelp")} onClick={() => {
            void terminalApi.projectWorkspace(taskId).then(setProjectFolder).catch(value => setError(errorMessage(value)));
          }}><FolderPlus size={14} /></button>
          <button className="icon-button" title={t("refreshConversations")} disabled={conversationBusy || loading} onClick={() => void refreshConversations()}><RefreshCw size={14} /></button>
          <button className="icon-button" title={t("showArchivedConversations")} aria-pressed={showArchived} onClick={() => setShowArchived(value => !value)}><Archive size={14} /></button>
          <small>{t(showArchived ? "archivedConversations" : "activeConversations")}</small>
        </div>}
        <div className="conversation-list">
          {conversations.filter(item => Boolean(item.archived) === showArchived).map((conversation) => (
            <div
              className={`conversation-row ${
                conversation.id === selectedId ? "selected" : ""
              }`}
              key={conversation.id}
            >
              {editingId === conversation.id ? (
                <form
                  className="conversation-rename-form"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void renameConversation(conversation);
                  }}
                >
                  <input
                    aria-label={t("conversationTitle")}
                    value={editingTitle}
                    maxLength={100}
                    autoFocus
                    onChange={(event) => setEditingTitle(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Escape") {
                        setEditingId("");
                        setEditingTitle("");
                      }
                    }}
                  />
                  <button
                    className="inline-icon"
                    title={t("save")}
                    disabled={!editingTitle.trim()}
                    type="submit"
                  >
                    <Check size={13} />
                  </button>
                  <button
                    className="inline-icon"
                    title={t("cancel")}
                    type="button"
                    onClick={() => {
                      setEditingId("");
                      setEditingTitle("");
                    }}
                  >
                    <X size={13} />
                  </button>
                </form>
              ) : (
                <>
                  <button
                    className="conversation-select"
                    disabled={conversation.archived || conversationBusy}
                    onClick={() => {
                      setSelectedId(conversation.id);
                      setOpenedIds((current) =>
                        current.includes(conversation.id)
                          ? current
                          : [...current, conversation.id],
                      );
                    }}
                  >
                    <strong>{conversation.title}</strong>
                    <small>{formatDate(conversation.updatedAt)}</small>
                  </button>
                  <div className="conversation-row-actions">
                    {provider === "codex" && <button className="icon-button conversation-edit" disabled={conversationBusy}
                      title={t(conversation.archived ? "restoreConversation" : "archiveConversation")}
                      onClick={() => void setArchived(conversation)}>
                      {conversation.archived ? <ArchiveRestore size={13} /> : <Archive size={13} />}
                    </button>}
                    <button
                      className="icon-button conversation-edit"
                      title={t("renameConversation")}
                      disabled={conversationBusy}
                      onClick={() => {
                        setEditingId(conversation.id);
                        setEditingTitle(conversation.title);
                      }}
                    >
                      <Pencil size={13} />
                    </button>
                    <button
                      className="icon-button conversation-delete"
                      title={t("deleteConversation")}
                      disabled={conversationBusy}
                      onClick={() => void deleteConversation(conversation)}
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                </>
              )}
            </div>
          ))}
          {!loading && conversations.filter(item => Boolean(item.archived) === showArchived).length === 0 && (
            <div className="empty-state">{t("noConversations")}</div>
          )}
          {loading && <div className="empty-state">{t("loading")}</div>}
        </div>
      </aside>
      {projectFolder && <Modal title={t("codexProjectHelp")} onClose={() => setProjectFolder(undefined)}>
        <div className="form-stack">
          <p>{t("codexProjectInstructions")}</p>
          <label className="field"><span>{t("codexProjectFolder")}</span>
            <input readOnly value={projectFolder} autoFocus onFocus={event => event.currentTarget.select()} />
          </label>
          <small>{t("codexProjectCopyHint")}</small>
          <label className="field">Codex 原生会话 ID<input readOnly
            value={conversations.find(c => c.id === selectedId)?.nativeSessionId ?? "尚未关联，请连接后刷新会话列表"}
            onFocus={event => event.currentTarget.select()} /></label>
          <p>使用上面的精确目录在 Codex 中添加项目，并按会话 ID 核对历史。原生关联不保证自动进入“ABYA 开发”侧栏分组。</p>
          <p>此入口用于查看和定位。在另一客户端继续执行前，请先暂停 APP 中的 AI，避免两边同时运行。</p>
          <details><summary>测试用时与原生用量</summary>
            <button className="secondary-button" onClick={() => void terminalApi.conversationMetrics(taskId, selectedId)
              .then(value => setMetrics(JSON.stringify(value, null, 2))).catch(e => setError(errorMessage(e)))}>读取当前会话统计</button>
            <p>记录轮次、答题等待和连接事件；耗时可能重叠，不能直接相加。原生用量缺失会标为 null。</p>
            {metrics && <textarea readOnly rows={12} value={metrics} aria-label="测试统计" onFocus={e => e.currentTarget.select()} />}
          </details>
          {conversations.find(c => c.id === selectedId)?.nativeSyncError && <p role="alert">原生同步失败：{conversations.find(c => c.id === selectedId)?.nativeSyncError}</p>}
          <details><summary>恢复关联错误的旧对话</summary>
            <p>先暂停 AI。只列出本任务目录中的原生历史；修复前自动备份，不合并或删除历史。</p>
            <button className="secondary-button" onClick={() => void terminalApi.recoveryCandidates(taskId).then(setRecoveryCandidates).catch(e => setError(errorMessage(e)))}>查找本任务历史</button>
            <label className="field">选择已核对的原生会话<select value={recoveryId} onChange={e => setRecoveryId(e.target.value)}>
              <option value="">请选择</option>{recoveryCandidates.map(c => <option key={c.id} value={c.id}>{c.startedAt} · {c.id}</option>)}
            </select></label>
            <button className="secondary-button" disabled={!recoveryId || conversationBusy} onClick={async () => {
              setConversationBusy(true);
              try { await terminalApi.repairBinding(taskId, selectedId, recoveryId); setProjectFolder(undefined); await refreshConversations(); }
              catch (e) { setError(errorMessage(e)); }
              finally { setConversationBusy(false); }
            }}>备份并修复关联</button>
          </details>
        </div>
      </Modal>}
      <div className="conversation-terminal-area">
        {openedIds.map((conversationId) => {
          const conversation = conversations.find(
            (item) => item.id === conversationId,
          );
          if (!conversation || conversation.archived) return null;
          return (
            <div
              className="terminal-session-panel"
              hidden={conversation.id !== selectedId}
              key={conversation.id}
            >
              <ConversationTerminal
                connectionRequest={connectionRequest?.conversationId === conversation.id ? connectionRequest : undefined}
                taskId={taskId}
                provider={provider}
                conversation={conversation}
                visible={visible && conversation.id === selectedId}
                availability={availability}
                t={t}
                onWorkflowChange={
                  conversation.id === selectedId
                    ? onWorkflowChange
                    : undefined
                }
              />
            </div>
          );
        })}
        {!selectedId && !loading && (
          <div className="terminal-unavailable">
            <span>{t("noConversations")}</span>
          </div>
        )}
        {error && <div className="terminal-error">{error}</div>}
      </div>
    </div>
  );
}

function ConversationTerminal({
  connectionRequest,
  provider,
  taskId,
  conversation,
  visible,
  availability,
  t,
  onWorkflowChange,
}: {
  connectionRequest?: TerminalConnectionRequest;
  provider: TerminalProvider;
  taskId: string;
  conversation: CodexConversation;
  visible: boolean;
  availability: CodexTerminalAvailability;
  t: (key: MessageKey) => string;
  onWorkflowChange?: (workflow: CodexWorkflowSnapshot) => void;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<XTerm | undefined>(undefined);
  const fitRef = useRef<FitAddon | undefined>(undefined);
  const visibleRef = useRef(visible);
  const startRef = useRef<(reset?: boolean) => void>(() => undefined);
  const tryStartRef = useRef<() => void>(() => undefined);
  const historyPanelRef = useRef<HTMLDivElement>(null);
  const historyContentRef = useRef<HTMLPreElement>(null);
  const historyCharacterCountRef = useRef(0);
  const historyTruncatedMarkerRef = useRef<HTMLSpanElement | undefined>(undefined);
  const historyAtBottomRef = useRef(true);
  const atBottomRef = useRef(true);
  const onWorkflowChangeRef = useRef(onWorkflowChange);
  const [viewMode, setViewMode] = useState<"terminal" | "history">("terminal");
  const [state, setState] = useState<CodexTerminalState>();
  const [error, setError] = useState("");
  const [atBottom, setAtBottom] = useState(true);
  const [historyAtBottom, setHistoryAtBottom] = useState(true);
  const [copyState, setCopyState] = useState<"idle" | "copying" | "copied">("idle");
  const copyBusyRef = useRef(false);
  const connectionRef = useRef(connectionRequest); connectionRef.current = connectionRequest;
  const control=useTaskControl(taskId,conversation.id,provider);
  const pendingContinue=useRef<string | undefined>(undefined);
  const [actionBusy,setActionBusy]=useState(false);
  const [actionNotice,setActionNotice]=useState("");
  useEffect(()=>setActionNotice(""),[control?.state]);
  async function continueTask() {
    if(actionBusy)return;setActionBusy(true);setError("");
    try {
      const result=await productionApi.continueTask(taskId,conversation.id);
      if (visibleRef.current) terminalRef.current?.focus();
      setActionNotice(continuationMessages[result.status]);
      if(result.status==="needsConnection"&&result.requestId) {
        pendingContinue.current=result.requestId; startRef.current(true);
      }
      window.dispatchEvent(new Event("abya:control-changed"));
    }catch(e){setError(errorMessage(e));}finally{setActionBusy(false);}
  }

  useEffect(() => {
    if (copyState !== "copied") return;
    const timer = window.setTimeout(() => setCopyState("idle"), 2500);
    return () => window.clearTimeout(timer);
  }, [copyState]);

  async function copyHistory() {
    if (copyBusyRef.current) return;
    copyBusyRef.current = true;
    setCopyState("copying");
    setError("");
    try {
      const copied = await terminalApi.copyHistory(provider, taskId, conversation.id);
      setCopyState(copied ? "copied" : "idle");
      if (!copied) setError(t("terminalHistoryEmpty"));
    } catch (value) {
      setCopyState("idle");
      setError(`${t("copyTerminalHistoryFailed")} ${errorMessage(value)}`);
    } finally {
      copyBusyRef.current = false;
    }
  }

  useEffect(() => {
    visibleRef.current = visible;
  }, [visible]);

  useEffect(() => {
    onWorkflowChangeRef.current = onWorkflowChange;
  }, [onWorkflowChange]);

  function clearHistory() {
    historyContentRef.current?.replaceChildren();
    historyCharacterCountRef.current = 0;
    historyTruncatedMarkerRef.current = undefined;
    historyAtBottomRef.current = true;
    setHistoryAtBottom(true);
  }

  function appendHistory(data: string) {
    const normalized = normalizeTerminalHistory(data);
    if (!normalized) return;
    const content = historyContentRef.current;
    if (!content) return;
    content.append(document.createTextNode(normalized));
    historyCharacterCountRef.current += normalized.length;

    let trimmed = false;
    while (historyCharacterCountRef.current > MAX_HISTORY_CHARACTERS) {
      const firstOutput = historyTruncatedMarkerRef.current
        ? historyTruncatedMarkerRef.current.nextSibling
        : content.firstChild;
      if (!(firstOutput instanceof Text)) break;
      const excess = historyCharacterCountRef.current - MAX_HISTORY_CHARACTERS;
      if (firstOutput.data.length <= excess) {
        historyCharacterCountRef.current -= firstOutput.data.length;
        firstOutput.remove();
      } else {
        firstOutput.data = firstOutput.data.slice(excess);
        historyCharacterCountRef.current -= excess;
      }
      trimmed = true;
    }
    if (trimmed && !historyTruncatedMarkerRef.current) {
      const marker = document.createElement("span");
      marker.className = "terminal-history-truncated";
      marker.textContent = HISTORY_TRUNCATED_MESSAGE;
      content.prepend(marker);
      historyTruncatedMarkerRef.current = marker;
    }
    if (historyAtBottomRef.current) {
      window.requestAnimationFrame(() => {
        const panel = historyPanelRef.current;
        if (panel) panel.scrollTop = panel.scrollHeight;
      });
    }
  }

  useEffect(() => {
    if (!hostRef.current) return;

    const terminal = new XTerm({
      cursorBlink: true,
      cursorStyle: "bar",
      fontFamily: '"Cascadia Code", Consolas, monospace',
      fontSize: 13,
      lineHeight: 1.15,
      scrollback: 100_000,
      scrollOnEraseInDisplay: true,
      scrollOnUserInput: false,
      convertEol: false,
      theme: {
        background: "#171c1e",
        foreground: "#d7e2de",
        cursor: "#79d1bd",
        cursorAccent: "#171c1e",
        selectionBackground: "#315e55",
        black: "#171c1e",
        brightBlack: "#65726d",
        red: "#e26d6d",
        brightRed: "#f28a87",
        green: "#64c49e",
        brightGreen: "#80d5b4",
        yellow: "#d6b86b",
        brightYellow: "#e4ca84",
        blue: "#76a9d2",
        brightBlue: "#91bce0",
        magenta: "#b69ad1",
        brightMagenta: "#cbb1df",
        cyan: "#62b9bd",
        brightCyan: "#83cdd0",
        white: "#d7e2de",
        brightWhite: "#f3f7f5",
      },
    });
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.open(hostRef.current);
    terminalRef.current = terminal;
    fitRef.current = fit;

    let disposed = false;
    let attachment=0;
    let pendingStart = false;
    let opening = false;
    let resetAfterAttach = false;
    let startFrame = 0;

    const measureSize = () => {
      if (!visibleRef.current || !hostHasTerminalLayout(hostRef.current)) {
        return undefined;
      }
      fit.fit();
      return clampTerminalSize(terminal.cols, terminal.rows);
    };

    const tryStart = () => {
      if (disposed || !pendingStart) return;
      const size = measureSize();
      if (!size) return;
      pendingStart = false;
      opening = true;
      const generation=++attachment;
      const channel = new Channel<CodexTerminalEvent>();
      let attaching = true;
      const buffered: string[] = [];
      channel.onmessage = (event) => {
        if(disposed||generation!==attachment)return;
        if (event.type === "output") {
          if (attaching) { buffered.push(event.data); return; }
          appendHistory(event.data);
          const preserveScroll = !atBottomRef.current;
          const viewportY = terminal.buffer.active.viewportY;
          terminal.write(event.data, () => {
            if (preserveScroll) {
              terminal.scrollToLine(
                Math.min(viewportY, terminal.buffer.active.baseY),
              );
            } else {
              terminal.scrollToBottom();
            }
          });
        } else if (event.type === "state") {
          setState(event.state);
          if (event.state.lastError) setError(event.state.lastError);
        } else {
          onWorkflowChangeRef.current?.(event.workflow);
        }
      };
      void terminalApi
        .open(
          provider,
          taskId,
          conversation.id,
          size.columns,
          size.rows,
          channel,
        )
        .then(next => {
          if (disposed||generation!==attachment) return;
          opening = false;
          if (resetAfterAttach) { terminal.reset(); clearHistory(); }
          attaching = false;
          for (const data of buffered) channel.onmessage({ type: "output", data });
          buffered.length = 0;
          setState(next);
          if (visibleRef.current) terminal.focus();
          const currentSize = measureSize();
          if (currentSize && (currentSize.columns !== size.columns || currentSize.rows !== size.rows)) {
            void terminalApi.resize(provider, conversation.id, currentSize.columns, currentSize.rows).catch(() => undefined);
          }
          connectionRef.current?.finish(next.status === "running" ? undefined : new Error(next.lastError || "原会话尚未连接。"));
          const request=pendingContinue.current;pendingContinue.current=undefined;
          if(request&&next.status==="running") {
            void productionApi.flushContinuation(taskId,conversation.id,request).then(result=>{
              if(!disposed){setActionNotice(continuationMessages[result.status]);window.dispatchEvent(new Event("abya:control-changed"));}
            }).catch(e=>{if(!disposed)setError(errorMessage(e));});
          }
        })
        .catch((value) => {
          if(disposed||generation!==attachment)return;
          opening = false;
          pendingContinue.current=undefined;
          connectionRef.current?.finish(value);
          setError(errorMessage(value));
          setState((current) =>
            current
              ? { ...current, status: "failed", lastError: errorMessage(value) }
              : current,
          );
        });
    };
    tryStartRef.current = tryStart;

    const resize = () => {
      if (disposed || !visibleRef.current || !hostHasTerminalLayout(hostRef.current)) {
        return;
      }
      fit.fit();
      if (pendingStart) tryStart();
    };
    const frame = window.requestAnimationFrame(resize);
    const observer = new ResizeObserver(resize);
    observer.observe(hostRef.current);

    const sendInput = terminalInputQueue(
      data => terminalApi.write(provider, conversation.id, data),
      value => setError(errorMessage(value)),
    );
    const removeClipboard = installTerminalClipboard(hostRef.current!, terminal,
      () => { void sendInput("\x16"); },
      () => setError(t("clipboardUnavailable")),
      terminalApi.readClipboard,
    );
    const input = terminal.onData(data => { void sendInput(data); });
    const terminalResize = terminal.onResize(({ cols, rows }) => {
      if (pendingStart || opening) return;
      const size = clampTerminalSize(cols, rows);
      if (!size) return;
      void terminalApi.resize(provider, conversation.id, size.columns, size.rows).catch(() => {
        // A resize may race with process exit.
      });
    });
    const terminalScroll = terminal.onScroll((position) => {
      const nextAtBottom = position >= terminal.buffer.active.baseY;
      atBottomRef.current = nextAtBottom;
      setAtBottom(nextAtBottom);
    });

    startRef.current = (reset = false) => {
      if (disposed) return;
      resetAfterAttach = reset;
      setError("");
      setState((current) => ({
        taskId,
        conversationId: conversation.id,
        status: "starting",
        pid: undefined,
        workingDirectory:
          current?.workingDirectory ?? availability.workingDirectory,
        exitCode: undefined,
        lastError: "",
      }));
      pendingStart = true;
      window.cancelAnimationFrame(startFrame);
      startFrame = window.requestAnimationFrame(tryStart);
    };
    startRef.current();

    return () => {
      disposed = true;
      pendingStart = false;
      tryStartRef.current = () => undefined;
      window.cancelAnimationFrame(frame);
      window.cancelAnimationFrame(startFrame);
      observer.disconnect();
      input.dispose();
      removeClipboard();
      terminalResize.dispose();
      terminalScroll.dispose();
      terminal.dispose();
      terminalRef.current = undefined;
      fitRef.current = undefined;
    };
  }, [availability.workingDirectory, conversation.id, provider, taskId]);

  useEffect(() => {
    if (connectionRequest && state?.status !== "starting") startRef.current(true);
    // Start/reattach once per explicit connection request, not on every terminal event.
  }, [connectionRequest?.id]);

  useEffect(() => {
    const panel = historyPanelRef.current;
    if (viewMode === "history" && panel) {
      const frame = window.requestAnimationFrame(() => {
        if (historyAtBottomRef.current) panel.scrollTop = panel.scrollHeight;
      });
      return () => window.cancelAnimationFrame(frame);
    }
    if (!visible || !terminalRef.current || !fitRef.current) return;
    const frame = window.requestAnimationFrame(() => {
      if (hostHasTerminalLayout(hostRef.current)) {
        fitRef.current?.fit();
        terminalRef.current?.focus();
      }
      tryStartRef.current();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [visible, viewMode]);

  async function stop() {
    setActionNotice("");
    pendingContinue.current=undefined;setActionBusy(true);
    setError("");
    try {
      await terminalApi.stop(provider, conversation.id);
      setState((current) =>
        current
          ? { ...current, status: "exited", pid: undefined }
          : current,
      );
    } catch (value) {
      setError(errorMessage(value));
    } finally {setActionBusy(false);window.dispatchEvent(new Event("abya:control-changed"));}
  }

  const terminalEnded =
    state?.status === "exited" || state?.status === "failed";
  const canStop =
    provider==="codex" ? control?.connection!=="disconnected" || control?.queued : state?.status === "starting" || state?.status === "running";
  const showingHistory = viewMode === "history";
  const canScrollToLatest = showingHistory ? !historyAtBottom : !atBottom;

  return (
    <div className={`codex-terminal ${provider==="codex"?"with-task-control":""}`}>
      <div className="terminal-toolbar">
        {provider==="codex"&&<ExecutionSettings taskId={taskId} conversationId={conversation.id} connected={visible&&state?.status==="running"} onResume={continueTask}/>}
        <div className="terminal-status">
          <span
            className={`terminal-state terminal-state-${state?.status ?? "starting"}`}
          />
          <strong>{conversation.title}</strong>
          <span>{provider==="codex" ? controlLabel(control) : terminalStatus(t, provider, state?.status)}</span>
          {state?.pid && <code>PID {state.pid}</code>}
        </div>
        <code
          className="terminal-working-directory"
          title={state?.workingDirectory ?? availability.workingDirectory}
        >
          {state?.workingDirectory ?? availability.workingDirectory}
        </code>
        <div className="terminal-actions">
          <span role="status" aria-live="polite">
            {copyState === "copied" && t("terminalHistoryCopied")}
          </span>
          <button
            className="icon-button"
            title={t(copyState === "copying" ? "copyingTerminalHistory" : "copyTerminalHistory")}
            aria-label={t("copyTerminalHistory")}
            disabled={copyState === "copying"}
            onClick={() => void copyHistory()}
          >
            {copyState === "copied" ? <Check size={15} /> : <Copy size={15} />}
          </button>
          <button
            className={`icon-button ${viewMode === "terminal" ? "selected" : ""}`}
            title={t("terminalView")}
            onClick={() => setViewMode("terminal")}
          >
            <SquareTerminal size={15} />
          </button>
          <button
            className={`icon-button ${showingHistory ? "selected" : ""}`}
            title={t("terminalHistory")}
            onClick={() => setViewMode("history")}
          >
            <ScrollText size={15} />
          </button>
          {canScrollToLatest && (
            <button
              className="icon-button"
              title={t("scrollToLatest")}
              onClick={() => {
                if (showingHistory) {
                  const panel = historyPanelRef.current;
                  if (panel) panel.scrollTop = panel.scrollHeight;
                  historyAtBottomRef.current = true;
                  setHistoryAtBottom(true);
                } else {
                  terminalRef.current?.scrollToBottom();
                  atBottomRef.current = true;
                  setAtBottom(true);
                }
              }}
            >
              <ArrowDown size={15} />
            </button>
          )}
          {canStop && (
            <button
              className="secondary-button terminal-stop"
              title={t(provider === "codex" ? "stopCodex" : "stopGrok")}
              onClick={stop}
              disabled={actionBusy || control?.state==="pausing"}
            >
              <Square size={15} fill="currentColor" />
              {t(provider === "codex" ? "stopCodex" : "stopGrok")}
            </button>
          )}
          {provider==="codex" && <button className="primary-button" onClick={()=>void continueTask()}
            disabled={actionBusy||!control?.nativeSessionId||["running","waitingForResponse","queued","pausing","closed"].includes(control?.state??"")}>{control?.state==="blocked"?"重新检查问题":"继续任务"}</button>}
          {terminalEnded && (
            <button
              className="secondary-button"
              title={t(provider === "codex" ? "restartCodex" : "restartGrok")}
              onClick={() => startRef.current(true)}
            >
              {t(provider === "codex" ? "restartCodex" : "restartGrok")}
              <RefreshCw size={16} />
            </button>
          )}
        </div>
      </div>
      {provider==="codex" && <div className="terminal-task-notice" role="status">
        {controlText(control)}{actionNotice&&` · ${actionNotice}`}
          {control?.turnId && !["running","queued","waitingForResponse"].includes(control.state) && <strong> · 以下为上次执行记录，并非当前运行进度</strong>}
      </div>}
      <div className="terminal-content">
        <div className="terminal-host" hidden={showingHistory} ref={hostRef} />
        <div
          className="terminal-history"
          hidden={!showingHistory}
          ref={historyPanelRef}
          onScroll={(event) => {
            const panel = event.currentTarget;
            const nextAtBottom =
              panel.scrollTop + panel.clientHeight >= panel.scrollHeight - 4;
            historyAtBottomRef.current = nextAtBottom;
            setHistoryAtBottom(nextAtBottom);
          }}
        >
          <pre ref={historyContentRef} />
        </div>
      </div>
      {error && <div className="terminal-error">{error}</div>}
    </div>
  );
}

function normalizeTerminalHistory(data: string) {
  return data
    .replace(/\u001b\][^\u0007]*(?:\u0007|\u001b\\)/g, "")
    .replace(/\u001b\[[0-?]*[ -/]*[@-~]/g, "")
    .replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g, "")
    .replace(/\r\n/g, "\n")
    .replace(/\r/g, "\n");
}

function terminalStatus(
  t: (key: MessageKey) => string,
  provider: TerminalProvider,
  status?: CodexTerminalState["status"],
) {
  const prefix = provider === "codex" ? "codex" : "grok";
  switch (status) {
    case "running":
      return t(`${prefix}Running` as MessageKey);
    case "stopping":
      return t(`${prefix}Stopping` as MessageKey);
    case "exited":
      return t(`${prefix}Exited` as MessageKey);
    case "failed":
      return t(`${prefix}Failed` as MessageKey);
    default:
      return t(`${prefix}Starting` as MessageKey);
  }
}

function formatDate(value: string) {
  return new Date(value).toLocaleString();
}
