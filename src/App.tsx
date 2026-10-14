import { useEffect, useMemo, useState } from "react";
import { ClipboardList, CloudDownload, ScrollText, Send, Settings, X } from "lucide-react";
import { ArchiveTransferView } from "./features/archive-transfer/ArchiveTransferView";
import { TasksView } from "./features/tasks/TasksView";
import { LogsView } from "./features/logs/LogsView";
import { SettingsView } from "./features/settings/SettingsView";
import { GameClientsView } from "./features/game-clients/GameClientsView";
import { clientApi, type ClientUpdate } from "./shared/gameClients";
import { api, errorMessage } from "./shared/api";
import type { AppSettings } from "./shared/types";
import {
  normalizeLocale,
  translator,
  type Locale,
  type MessageKey,
} from "./i18n";
import "./App.css";
import { IconButtonTooltips } from "./app/IconButtonTooltips";
import { useNotificationActivations } from "./app/useNotificationActivations";
import type { NotificationActivation } from "./shared/notifications";

type ViewId = "tasks" | "archiveTransfer" | "logs" | "settings" | "gameClients";

function App() {
  const [view, setView] = useState<ViewId>("tasks");
  const [notificationNavigation, setNotificationNavigation] = useState<NotificationActivation>();
  const [settings, setSettings] = useState<AppSettings>();
  const [clientUpdate, setClientUpdate] = useState<ClientUpdate | null>(null);
  const [preferredInstallationId, setPreferredInstallationId] = useState<string>();
  const [preferredReleaseId, setPreferredReleaseId] = useState<number>();
  const [buildInfo,setBuildInfo]=useState<Awaited<ReturnType<typeof api.buildInfo>>>();
  const [locale, setLocale] = useState<Locale>(
    normalizeLocale(navigator.language),
  );
  const [notification, setNotification] = useState<{
    message: string;
    error: boolean;
  }>();
  const t = useMemo(() => translator(locale), [locale]);
  useNotificationActivations(activation => {
    if (activation.status === "test") {
      setView("settings");
      notify(t("notificationTestOpened"));
    } else if (activation.status === "missing" || !activation.target) {
      setView("tasks");
      notify(t("notificationTaskMissing"));
    } else {
      setView("tasks");
      setNotificationNavigation(activation);
    }
  });

  useEffect(() => {
    void api.buildInfo().then(setBuildInfo).catch(()=>undefined);
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

  useEffect(() => {
    setClientUpdate(null);
    if (settings?.gameClient?.source !== "remote") return;
    let alive = true;
    let checking = false;
    const check = async () => {
      if (checking) return;
      checking = true;
      try { const update = await clientApi.checkUpdate(); if (alive) setClientUpdate(update); }
      catch { /* Offline clients remain usable. The manager offers an explicit retry. */ }
      finally { checking = false; }
    };
    void check();
    const timer = window.setInterval(() => void check(), 30 * 60 * 1000);
    return () => { alive = false; window.clearInterval(timer); };
  }, [settings?.gameClient?.source, settings?.gameClient?.installationId]);

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
    { id: "gameClients", label: "gameClients", icon: CloudDownload },
    { id: "logs", label: "logs", icon: ScrollText },
    { id: "settings", label: "settings", icon: Settings },
  ];

  return (
    <div className="app-shell">
      <IconButtonTooltips closeLabel={t("close")} />
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
                aria-label={t(item.label)}
                title={t(item.label)}
                className={view === item.id ? "active" : ""}
                onClick={() => { setPreferredInstallationId(undefined); setPreferredReleaseId(undefined); setView(item.id); }}
              >
                <Icon size={19} />
                <span>{t(item.label)}</span>
              </button>
            );
          })}
        </nav>
        {clientUpdate && <button className="client-sidebar-update" onClick={() => { setPreferredReleaseId(clientUpdate.releaseId); setView("gameClients"); }}>
          <CloudDownload size={17} /><span><strong>{t("gcUpdate")}</strong><span>{clientUpdate.tagName}</span></span>
        </button>}
        <div className="sidebar-footer">
          <span className="connection-light" />
          <strong>APP {buildInfo?.version??"版本核对中"}{buildInfo?.testEnvironment?" · 测试环境":""}</strong>
          <span>{buildInfo?.release??""}</span>
          <span>{buildInfo?.builtAt && buildInfo.builtAt!=="development" ? new Date(buildInfo.builtAt).toLocaleString(locale) : "开发构建"}</span>
          {buildInfo?.verification!=="verified" && <span>未确认构建，请核对发布入口</span>}
        </div>
      </aside>

      <main className="app-main">
        <div
          className={`app-view ${view === "tasks" ? "active" : ""}`}
          aria-hidden={view !== "tasks"}
        >
          <TasksView settings={settings} t={t} notify={notify}
            notificationNavigation={notificationNavigation}
            onNotificationHandled={id => setNotificationNavigation(current => current?.id === id ? undefined : current)} />
        </div>
        {view === "archiveTransfer" && (
          <ArchiveTransferView t={t} notify={notify} />
        )}
        {view === "logs" && <LogsView t={t} notify={notify} />}
        {view === "gameClients" && <GameClientsView settings={settings} t={t} notify={notify} preferredReleaseId={preferredReleaseId}
          onOpenSettings={id => { setPreferredInstallationId(id); setView("settings"); }} />}
        {view === "settings" && (
          <SettingsView
            settings={settings}
            locale={locale}
            t={t}
            onSaved={handleSaved}
            notify={notify}
            onManageClients={() => setView("gameClients")}
            preferredInstallationId={preferredInstallationId}
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
