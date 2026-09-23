import { useEffect, useMemo, useState } from "react";
import { ClipboardList, ScrollText, Send, Settings, X } from "lucide-react";
import { ArchiveTransferView } from "./features/archive-transfer/ArchiveTransferView";
import { TasksView } from "./features/tasks/TasksView";
import { LogsView } from "./features/logs/LogsView";
import { SettingsView } from "./features/settings/SettingsView";
import { api, errorMessage } from "./shared/api";
import type { AppSettings } from "./shared/types";
import {
  normalizeLocale,
  translator,
  type Locale,
  type MessageKey,
} from "./i18n";
import "./App.css";

type ViewId = "tasks" | "archiveTransfer" | "logs" | "settings";

function App() {
  const [view, setView] = useState<ViewId>("tasks");
  const [settings, setSettings] = useState<AppSettings>();
  const [locale, setLocale] = useState<Locale>(
    normalizeLocale(navigator.language),
  );
  const [notification, setNotification] = useState<{
    message: string;
    error: boolean;
  }>();
  const t = useMemo(() => translator(locale), [locale]);

  useEffect(() => {
    void api
      .settings()
      .then((value) => {
        setSettings(value);
        setLocale(normalizeLocale(value.locale || navigator.language));
      })
      .catch((error) => notify(errorMessage(error), true));
  }, []);

  useEffect(() => {
    if (!notification) return;
    const timer = window.setTimeout(() => setNotification(undefined), 6000);
    return () => window.clearTimeout(timer);
  }, [notification]);

  function notify(message: string, error = false) {
    setNotification({ message, error });
  }

  function handleSaved(value: AppSettings) {
    setSettings(value);
    setLocale(normalizeLocale(value.locale));
  }

  const nav: Array<{
    id: ViewId;
    label: MessageKey;
    icon: typeof ClipboardList;
  }> = [
    { id: "tasks", label: "tasks", icon: ClipboardList },
    { id: "archiveTransfer", label: "archiveTransfer", icon: Send },
    { id: "logs", label: "logs", icon: ScrollText },
    { id: "settings", label: "settings", icon: Settings },
  ];

  return (
    <div className="app-shell">
      <aside className="app-sidebar">
        <div className="brand">
          <div className="brand-mark">A</div>
          <div>
            <strong>ABYA</strong>
            <span>Development Tool</span>
          </div>
        </div>
        <nav>
          {nav.map((item) => {
            const Icon = item.icon;
            return (
              <button
                key={item.id}
                className={view === item.id ? "active" : ""}
                onClick={() => setView(item.id)}
              >
                <Icon size={19} />
                <span>{t(item.label)}</span>
              </button>
            );
          })}
        </nav>
        <div className="sidebar-footer">
          <span className="connection-light" />
          <span>v0.1.0</span>
        </div>
      </aside>

      <main className="app-main">
        <div
          className={`app-view ${view === "tasks" ? "active" : ""}`}
          aria-hidden={view !== "tasks"}
        >
          <TasksView settings={settings} t={t} notify={notify} />
        </div>
        {view === "archiveTransfer" && (
          <ArchiveTransferView t={t} notify={notify} />
        )}
        {view === "logs" && <LogsView t={t} notify={notify} />}
        {view === "settings" && (
          <SettingsView
            settings={settings}
            locale={locale}
            t={t}
            onSaved={handleSaved}
            notify={notify}
          />
        )}
      </main>

      {notification && (
        <div
          className={`notification ${notification.error ? "notification-error" : ""}`}
          role="status"
        >
          <span>{notification.message}</span>
          <button
            className="icon-button"
            onClick={() => setNotification(undefined)}
            title={t("close")}
          >
            <X size={15} />
          </button>
        </div>
      )}
    </div>
  );
}

export default App;
