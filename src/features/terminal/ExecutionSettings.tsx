import { useEffect, useRef, useState } from "react";
import { terminalApi, errorMessage } from "../../shared/api";

import type { ExecutionSettingsView } from "../../shared/types";

export function ExecutionSettings({
  taskId,
  conversationId,
  connected,
  onResume,
}: {
  taskId: string;
  conversationId: string;
  connected: boolean;
  onResume: () => Promise<void>;
}) {
  const [view, setView] = useState<ExecutionSettingsView>();
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [draft, setDraft] = useState<Record<string, string>>({});
  const sequence = useRef(0);
  const alive = useRef(false);
  const editing = useRef(false);
  useEffect(() => {
    alive.current = true;
    setView(undefined);
    setDraft({});
    setMessage("");
    setError("");
    async function refresh() {
      if (!connected || editing.current) return;
      const seq = ++sequence.current;
      try {
        const result = await terminalApi.executionSettings(
          taskId,
          conversationId,
        );
        if (alive.current && seq === sequence.current) {
          setView(result);
          setError("");
        }
      } catch (e) {
        if (alive.current && seq === sequence.current)
          setError(errorMessage(e));
      }
    }
    void refresh();
    const timer = window.setInterval(() => void refresh(), 6000);
    return () => {
      alive.current = false;
      sequence.current++;
      window.clearInterval(timer);
    };
  }, [taskId, conversationId, connected]);
  const settings = view?.settings ?? {};
  const permission =
    settings.approvalPolicy === "never" &&
    settings.sandboxPolicy?.type === "dangerFullAccess"
      ? "full-access"
      : settings.approvalsReviewer === "auto_review" ||
          settings.approvalsReviewer === "guardian_subagent"
        ? "auto-review"
        : "custom";
  const model = draft.model ?? settings.model ?? "";
  const entry = view?.models.find((m) => m.model === model);
  function edit(key: string, value: string) {
    setMessage("");
    setError("");
    setDraft((old) => ({
      ...old,
      [key]: value,
      ...(key === "model"
        ? {
            effort:
              view?.models.find((m) => m.model === value)
                ?.defaultReasoningEffort ?? "medium",
          }
        : {}),
    }));
  }
  async function apply(applyNow = false) {
    editing.current = true;
    sequence.current++;
    setBusy(true);
    setMessage("");
    setError("");
    try {
      const result = await terminalApi.updateExecutionSettings(
        taskId,
        conversationId,
        {
          ...draft,
          ...(draft.effort && !draft.model ? { model: settings.model } : {}),
          applyNow,
        },
      );
      if (!alive.current) return;
      setView(result);
      setDraft({});
      setMessage(
        result.running
          ? "已保存，将用于下一轮；当前轮次继续使用原设置。"
          : "执行设置已生效。",
      );
      if (applyNow) {
        await onResume();
        setMessage("设置已应用，已请求继续当前任务。");
      }
    } catch (e) {
      if (alive.current) setError(errorMessage(e));
    } finally {
      editing.current = false;
      if (alive.current) setBusy(false);
    }
  }
  return (
    <div className="execution-settings" aria-label="会话执行设置">
      <label>
        模型
        <select
          aria-label="模型"
          value={model}
          disabled={!view?.connected || busy}
          onChange={(e) => edit("model", e.target.value)}
        >
          <option value="" disabled>
            {connected ? "读取中" : "连接后可设置"}
          </option>
          {settings.model &&
            !view?.models.some((m) => m.model === settings.model) && (
              <option value={settings.model}>{settings.model}</option>
            )}
          {view?.models.map((m) => (
            <option key={m.model} value={m.model}>
              {m.displayName}
            </option>
          ))}
        </select>
      </label>
      <label>
        思考强度
        <select
          aria-label="思考强度"
          value={
            draft.effort ??
            settings.effort ??
            entry?.defaultReasoningEffort ??
            ""
          }
          disabled={!view?.connected || busy}
          onChange={(e) => edit("effort", e.target.value)}
        >
          <option value="" disabled>
            默认
          </option>
          {entry?.supportedReasoningEfforts.map((e) => (
            <option key={e.reasoningEffort} value={e.reasoningEffort}>
              {e.reasoningEffort}
            </option>
          ))}
        </select>
      </label>
      <label>
        权限
        <select
          aria-label="权限"
          value={draft.permissionMode ?? permission}
          disabled={!view?.connected || busy}
          onChange={(e) => edit("permissionMode", e.target.value)}
        >
          <option value="custom" disabled>
            当前自定义设置
          </option>
          <option value="auto-review">AI 自动审核</option>
          <option value="full-access">完全同意</option>
        </select>
      </label>
      {Object.keys(draft).length > 0 && (
        <>
          <button
            className="secondary-button"
            disabled={busy}
            onClick={() => void apply()}
          >
            {view?.running ? "下一轮应用" : "应用"}
          </button>
          {view?.running && (
            <button
              className="secondary-button"
              disabled={busy}
              onClick={() => void apply(true)}
            >
              暂停并应用
            </button>
          )}
          <button
            className="text-button"
            disabled={busy}
            onClick={() => setDraft({})}
          >
            取消
          </button>
        </>
      )}
      {message && <small role="status">{message}</small>}
      {error && <small role="alert">{error}</small>}
    </div>
  );
}
