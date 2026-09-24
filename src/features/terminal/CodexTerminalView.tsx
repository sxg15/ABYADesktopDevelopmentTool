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

const MAX_HISTORY_CHARACTERS = 2 * 1024 * 1024;
const HISTORY_TRUNCATED_MESSAGE =
  "[Earlier terminal output omitted; showing recent output.]\n";

export function DevelopmentTerminalView({
  provider,
  taskId,
  visible,
  availability,
  t,
  onRefreshAvailability,
  onConversationChange,
  onWorkflowChange,
}: {
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
      if (result.length === 0 && availability?.available) {
        result = [await terminalApi.createConversation(provider, taskId)];
      }
      if (revision !== refreshRevision.current) return;
      setConversations(result);
      const active = result.filter(item => !item.archived);
      setSelectedId((current) =>
        active.some((conversation) => conversation.id === current)
          ? current
          : active[0]?.id ?? "",
      );
      setOpenedIds((current) => [
        ...current.filter((id) =>
          active.some((conversation) => conversation.id === id),
        ),
        ...(active[0] && !current.includes(active[0].id) ? [active[0].id] : []),
      ]);
      setError(result.find(item => item.nativeSyncError)?.nativeSyncError ?? "");
    } catch (value) {
      setError(errorMessage(value));
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
  provider,
  taskId,
  conversation,
  visible,
  availability,
  t,
  onWorkflowChange,
}: {
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
    let pendingStart = false;
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
      const channel = new Channel<CodexTerminalEvent>();
      channel.onmessage = (event) => {
        if (event.type === "output") {
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
        .then(setState)
        .catch((value) => {
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
      if (pendingStart) return;
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
      if (reset) {
        terminal.reset();
        clearHistory();
        atBottomRef.current = true;
        setAtBottom(true);
      }
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
    }
  }

  const terminalEnded =
    state?.status === "exited" || state?.status === "failed";
  const canStop =
    state?.status === "starting" || state?.status === "running";
  const showingHistory = viewMode === "history";
  const canScrollToLatest = showingHistory ? !historyAtBottom : !atBottom;

  return (
    <div className="codex-terminal">
      <div className="terminal-toolbar">
        <div className="terminal-status">
          <span
            className={`terminal-state terminal-state-${state?.status ?? "starting"}`}
          />
          <strong>{conversation.title}</strong>
          <span>{terminalStatus(t, provider, state?.status)}</span>
          {state?.pid && <code>PID {state.pid}</code>}
        </div>
        <code
          className="terminal-working-directory"
          title={state?.workingDirectory ?? availability.workingDirectory}
        >
          {state?.workingDirectory ?? availability.workingDirectory}
        </code>
        <div className="terminal-actions">
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
              className="icon-button terminal-stop"
              title={t(provider === "codex" ? "stopCodex" : "stopGrok")}
              onClick={stop}
            >
              <Square size={15} fill="currentColor" />
            </button>
          )}
          {terminalEnded && (
            <button
              className="icon-button"
              title={t(provider === "codex" ? "restartCodex" : "restartGrok")}
              onClick={() => startRef.current(true)}
            >
              <RefreshCw size={16} />
            </button>
          )}
        </div>
      </div>
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
