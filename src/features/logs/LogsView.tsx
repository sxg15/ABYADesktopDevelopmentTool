import { useEffect, useMemo, useState } from "react";
import {
  Play,
  RefreshCw,
  Square,
  TerminalSquare,
  Trash2,
} from "lucide-react";
import { Modal } from "../../app/Modal";
import { api, errorMessage } from "../../shared/api";
import type {
  LogSession,
  LogSource,
  RuntimeLogEvent,
} from "../../shared/types";
import type { MessageKey } from "../../i18n";

export function LogsView({
  t,
  notify,
}: {
  t: (key: MessageKey) => string;
  notify: (message: string, error?: boolean) => void;
}) {
  const [sources, setSources] = useState<LogSource[]>([]);
  const [instanceId, setInstanceId] = useState("");
  const [sessions, setSessions] = useState<LogSession[]>([]);
  const [sessionId, setSessionId] = useState("");
  const [events, setEvents] = useState<RuntimeLogEvent[]>([]);
  const [severity, setSeverity] = useState("");
  const [provider, setProvider] = useState("");
  const [eventName, setEventName] = useState("");
  const [contains, setContains] = useState("");
  const [detail, setDetail] = useState<RuntimeLogEvent>();
  const [busy, setBusy] = useState(false);

  const selectedSession = sessions.find((session) => session.id === sessionId);
  const selectedSource = sources.find(
    (source) => source.instanceId === instanceId,
  );
  const managedSources = sources.filter((source) => source.origin === "managed");
  const externalSources = sources.filter((source) => source.origin === "external");
  const collecting = ["streaming", "connecting", "reconnecting"].includes(
    selectedSession?.status ?? "",
  );
  const providers = useMemo(
    () => [...new Set(events.map((event) => event.provider).filter(Boolean))],
    [events],
  );

  useEffect(() => {
    void loadSources();
    const timer = window.setInterval(() => void refreshSources(), 2000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (instanceId) void loadSessions(instanceId);
  }, [instanceId]);

  useEffect(() => {
    if (!sessionId) return;
    void loadEvents();
    const timer = window.setInterval(() => void loadEvents(), 1500);
    return () => window.clearInterval(timer);
  }, [sessionId, severity, provider, eventName, contains]);

  async function loadSources() {
    try {
      const result = await api.listLogSources();
      setSources(result);
      if (!instanceId) {
        setInstanceId(
          result.find((source) => source.connectionState === "connected")
            ?.instanceId ??
            result[0]?.instanceId ??
            "",
        );
      }
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function refreshSources() {
    try {
      setSources(await api.listLogSources());
    } catch {
      // Keep the last source state during transient command failures.
    }
  }

  async function refreshSelectedSource() {
    await Promise.all([refreshSources(), loadSessions()]);
  }

  async function loadSessions(target = instanceId) {
    try {
      const result = await api.listLogSessions(target);
      setSessions(result);
      if (!result.some((session) => session.id === sessionId)) {
        setSessionId(result[0]?.id ?? "");
      }
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function loadEvents() {
    if (!sessionId) return;
    try {
      setEvents(
        await api.queryLogs({
          sessionId,
          severity: severity || undefined,
          provider: provider || undefined,
          eventName: eventName || undefined,
          contains: contains || undefined,
          limit: 500,
        }),
      );
    } catch {
      // Keep the last visible page during transient collector updates.
    }
  }

  async function start() {
    if (!instanceId) return;
    setBusy(true);
    try {
      const session = await api.startLogs(instanceId);
      await loadSessions(instanceId);
      setSessionId(session.id);
    } catch (error) {
      notify(errorMessage(error), true);
    } finally {
      setBusy(false);
    }
  }

  async function stop() {
    if (!instanceId) return;
    try {
      await api.stopLogs(instanceId);
      await loadSessions(instanceId);
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function removeSession(session: LogSession) {
    if (!window.confirm(t("deleteLogSessionConfirm"))) return;
    try {
      await api.deleteLogSession(session.id);
      if (session.id === sessionId) {
        setSessionId("");
        setEvents([]);
        setDetail(undefined);
      }
      await loadSessions(instanceId);
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function removeSource() {
    if (!selectedSource || !window.confirm(t("deleteLogSourceConfirm"))) return;
    try {
      await api.deleteInstance(selectedSource.instanceId);
      setInstanceId("");
      setSessionId("");
      setEvents([]);
      await loadSources();
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  return (
    <div className="workspace logs-workspace">
      <header className="page-toolbar">
        <div>
          <h1>{t("logs")}</h1>
          <p>{selectedSource?.name ?? t("selectInstance")}</p>
        </div>
        <div className="toolbar-actions">
          <label className="toolbar-select">
            <span>{t("selectInstance")}</span>
            <select
              value={instanceId}
              onChange={(event) => setInstanceId(event.target.value)}
            >
              <option value="">-</option>
              <optgroup label={t("managedSources")}>
                {managedSources.map((source) => (
                  <option key={source.instanceId} value={source.instanceId}>
                    {source.name} · {t(source.connectionState)}
                  </option>
                ))}
              </optgroup>
              <optgroup label={t("externalSources")}>
                {externalSources.map((source) => (
                  <option key={source.instanceId} value={source.instanceId}>
                    {source.name} · {t(source.connectionState)}
                  </option>
                ))}
              </optgroup>
            </select>
          </label>
          <button
            className="icon-button"
            onClick={refreshSelectedSource}
            title={t("refresh")}
          >
            <RefreshCw size={17} />
          </button>
          {selectedSource?.origin === "external" && (
            <button
              className="icon-button danger-icon"
              onClick={removeSource}
              disabled={selectedSource.connectionState === "connected"}
              title={t("deleteLogSource")}
            >
              <Trash2 size={16} />
            </button>
          )}
          {collecting ? (
            <button className="danger-button" onClick={stop}>
              <Square size={15} />
              {t("stopCollecting")}
            </button>
          ) : (
            <button
              className="primary-button"
              onClick={start}
              disabled={
                !instanceId ||
                selectedSource?.connectionState !== "connected" ||
                busy
              }
            >
              <Play size={15} />
              {t("collectLogs")}
            </button>
          )}
        </div>
      </header>

      <div className="log-layout">
        <aside className="session-pane">
          <h2>{t("session")}</h2>
          {sessions.map((session) => (
            <div className="session-row-wrap" key={session.id}>
              <button
                className={`session-row ${session.id === sessionId ? "selected" : ""}`}
                onClick={() => setSessionId(session.id)}
              >
                <TerminalSquare size={17} />
                <span>
                  <strong>{formatDate(session.startedAt)}</strong>
                  <small>
                    {session.status} · #{session.latestSequence}
                  </small>
                </span>
              </button>
              <button
                className="inline-icon danger-icon session-delete"
                disabled={isCollecting(session)}
                onClick={() => removeSession(session)}
                title={t("deleteLogSession")}
              >
                <Trash2 size={14} />
              </button>
            </div>
          ))}
        </aside>

        <section className="log-content">
          <div className="filter-bar">
            <input
              value={contains}
              onChange={(event) => setContains(event.target.value)}
              placeholder={t("search")}
            />
            <select
              value={severity}
              onChange={(event) => setSeverity(event.target.value)}
              aria-label={t("severity")}
            >
              <option value="">{t("severity")}</option>
              {["Error", "Warning", "Info", "Debug", "Trace"].map((value) => (
                <option key={value}>{value}</option>
              ))}
            </select>
            <select
              value={provider}
              onChange={(event) => setProvider(event.target.value)}
              aria-label={t("provider")}
            >
              <option value="">{t("provider")}</option>
              {providers.map((value) => (
                <option key={value}>{value}</option>
              ))}
            </select>
            <input
              value={eventName}
              onChange={(event) => setEventName(event.target.value)}
              placeholder={t("eventName")}
            />
          </div>

          <div className="table-wrap log-table">
            <table>
              <thead>
                <tr>
                  <th>{t("sequence")}</th>
                  <th>{t("severity")}</th>
                  <th>{t("provider")}</th>
                  <th>{t("eventName")}</th>
                  <th>{t("message")}</th>
                  <th>{t("startedAt")}</th>
                </tr>
              </thead>
              <tbody>
                {events.map((event) => (
                  <tr key={`${event.logSessionId}-${event.sequence}`} onDoubleClick={() => setDetail(event)}>
                    <td className="mono-cell">{event.sequence}</td>
                    <td>
                      <span className={`severity severity-${event.severity.toLowerCase()}`}>
                        {event.severity}
                      </span>
                    </td>
                    <td>{event.provider}</td>
                    <td className="mono-cell">{event.eventName}</td>
                    <td className="message-cell" title={event.message}>
                      {event.message}
                    </td>
                    <td>{formatTime(event.utc)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {events.length === 0 && (
              <div className="empty-state large">{t("noLogs")}</div>
            )}
          </div>
        </section>
      </div>
      {detail && (
        <Modal title={`${detail.eventName} #${detail.sequence}`} onClose={() => setDetail(undefined)} wide>
          <pre className="json-detail">{JSON.stringify(detail.raw, null, 2)}</pre>
        </Modal>
      )}
    </div>
  );
}

function formatDate(value: string) {
  return new Date(value).toLocaleString();
}

function formatTime(value: string) {
  return value ? new Date(value).toLocaleTimeString() : "-";
}

function isCollecting(session: LogSession) {
  return ["streaming", "connecting", "reconnecting"].includes(session.status);
}
