import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  FileSearch,
  FolderOpen,
  KeyRound,
  Network,
  RefreshCw,
  Save,
  Wrench,
} from "lucide-react";
import { api, errorMessage } from "../../shared/api";
import type {
  AppPaths,
  AppSettings,
  DesktopCliState,
  GameConnectionState,
  LanInterface,
} from "../../shared/types";
import type { Locale, MessageKey } from "../../i18n";

export function SettingsView({
  settings,
  locale,
  t,
  onSaved,
  notify,
}: {
  settings?: AppSettings;
  locale: Locale;
  t: (key: MessageKey) => string;
  onSaved: (settings: AppSettings) => void;
  notify: (message: string, error?: boolean) => void;
}) {
  const [executable, setExecutable] = useState("");
  const [workspaceRoot, setWorkspaceRoot] = useState("");
  const [selectedLocale, setSelectedLocale] = useState<Locale>(locale);
  const [paths, setPaths] = useState<AppPaths>();
  const [cli, setCli] = useState<DesktopCliState>();
  const [gateway, setGateway] = useState<GameConnectionState>();
  const [interfaces, setInterfaces] = useState<LanInterface[]>([]);
  const [gatewayPort, setGatewayPort] = useState(47610);
  const [preferredAdapterId, setPreferredAdapterId] = useState("");
  const [broadcastEnabled, setBroadcastEnabled] = useState(true);
  const [repairing, setRepairing] = useState(false);

  useEffect(() => {
    setExecutable(settings?.gameExecutablePath ?? "");
    setWorkspaceRoot(settings?.workspaceRootPath ?? "");
    setSelectedLocale((settings?.locale as Locale) || locale);
    setGatewayPort(settings?.gameGatewayPort ?? 47610);
    setPreferredAdapterId(settings?.preferredAdapterId ?? "");
    setBroadcastEnabled(settings?.lanBroadcastEnabled ?? true);
  }, [settings, locale]);

  useEffect(() => {
    void refreshState();
    void api.listLanInterfaces().then(setInterfaces).catch(() => undefined);
    void api.paths().then(setPaths).catch(() => undefined);
  }, []);

  async function refreshState() {
    try {
      setCli(await api.desktopCliState());
      setGateway(await api.gameConnectionState());
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function browse() {
    const result = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Windows executable", extensions: ["exe"] }],
    });
    if (typeof result === "string") setExecutable(result);
  }

  async function browseWorkspaceRoot() {
    const result = await open({
      multiple: false,
      directory: true,
    });
    if (typeof result === "string") setWorkspaceRoot(result);
  }

  async function save() {
    try {
      const result = await api.updateSettings({
        gameExecutablePath: executable,
        workspaceRootPath: workspaceRoot,
        locale: selectedLocale,
        gameGatewayPort: gatewayPort,
        preferredAdapterId,
        lanBroadcastEnabled: broadcastEnabled,
      });
      onSaved(result);
      await refreshState();
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function regenerate() {
    try {
      const result = await api.regenerateDesktopToken();
      onSaved(result);
      await refreshState();
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function restart() {
    try {
      setCli(await api.restartDesktopCli());
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function restartGateway() {
    try {
      setGateway(await api.restartGameConnections());
      setInterfaces(await api.listLanInterfaces());
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function repairStorage() {
    setRepairing(true);
    try {
      const report = await api.repairStorage();
      notify(`${t("storageRepairDone")} ${Math.round((report.database.databaseBytesBefore - report.database.databaseBytesAfter) / 1024 / 1024)} MB`);
    } catch (error) { notify(errorMessage(error), true); } finally { setRepairing(false); }
  }

  return (
    <div className="workspace settings-workspace">
      <header className="page-toolbar">
        <div>
          <h1>{t("settings")}</h1>
        </div>
        <button className="primary-button" onClick={save}>
          <Save size={16} />
          {t("save")}
        </button>
      </header>

      <section className="settings-section">
        <h2>{t("gameExecutable")}</h2>
        <div className="path-control">
          <input value={executable} readOnly />
          <button className="secondary-button" onClick={browse}>
            <FileSearch size={16} />
            {t("browse")}
          </button>
        </div>
      </section>

      <section className="settings-section">
        <h2>{t("workspaceRoot")}</h2>
        <div className="path-control">
          <input value={workspaceRoot} readOnly />
          <button className="secondary-button" onClick={browseWorkspaceRoot}>
            <FolderOpen size={16} />
            {t("browse")}
          </button>
        </div>
        <p>{t("workspaceRootHint")}</p>
      </section>

      <section className="settings-section">
        <h2>{t("language")}</h2>
        <div className="segmented compact">
          <button
            className={selectedLocale === "zh-CN" ? "selected" : ""}
            onClick={() => setSelectedLocale("zh-CN")}
          >
            中文
          </button>
          <button
            className={selectedLocale === "en-US" ? "selected" : ""}
            onClick={() => setSelectedLocale("en-US")}
          >
            English
          </button>
        </div>
      </section>

      <section className="settings-section">
        <h2>{t("dataDirectory")}</h2>
        <code className="path-value">{paths?.dataDir ?? "-"}</code>
        <p>{t("storageRepairHint")}</p>
        <button className="secondary-button" onClick={repairStorage} disabled={repairing}>
          <Wrench size={16} /> {repairing ? t("repairingStorage") : t("repairStorage")}
        </button>
      </section>

      <section className="settings-section">
        <div className="settings-section-header">
          <div>
            <h2>{t("gameConnections")}</h2>
            <p>{gateway?.preferredEndpoint || gateway?.bindEndpoint || "-"}</p>
          </div>
          <span className={gateway?.running ? "status-ok" : "status-muted"}>
            {gateway?.running ? t("running") : t("stopped")}
          </span>
        </div>
        <div className="lan-warning">
          <Network size={17} />
          <span>{t("lanUnauthenticatedWarning")}</span>
        </div>
        <div className="gateway-settings-grid">
          <label className="field">
            <span>{t("gatewayPort")}</span>
            <input
              type="number"
              min={1024}
              max={65535}
              value={gatewayPort}
              onChange={(event) => setGatewayPort(Number(event.target.value))}
            />
          </label>
          <label className="field">
            <span>{t("preferredAdapter")}</span>
            <select
              value={preferredAdapterId}
              onChange={(event) => setPreferredAdapterId(event.target.value)}
            >
              <option value="">{t("automatic")}</option>
              {interfaces.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.name} · {item.address}
                </option>
              ))}
            </select>
          </label>
        </div>
        <label className="toggle-row">
          <input
            type="checkbox"
            checked={broadcastEnabled}
            onChange={(event) => setBroadcastEnabled(event.target.checked)}
          />
          <span>{t("lanBroadcast")}</span>
        </label>
        <div className="gateway-metrics">
          <span>
            {t("connectedInstances")}: <strong>{gateway?.connectedCount ?? 0}</strong>
          </span>
          <span>
            UDP {gateway?.discoveryPort ?? 47611}
          </span>
          <span>
            v{gateway?.protocolVersion ?? 1}
          </span>
        </div>
        {gateway?.advertisedAddresses.map((address) => (
          <code className="path-value gateway-address" key={address}>
            {address}
          </code>
        ))}
        <div className="settings-actions gateway-actions">
          <button className="secondary-button" onClick={restartGateway}>
            <RefreshCw size={16} />
            {t("restartGateway")}
          </button>
        </div>
        {gateway?.lastError && (
          <div className="inline-error">{gateway.lastError}</div>
        )}
      </section>

      <section className="settings-section">
        <div className="settings-section-header">
          <h2>{t("desktopCli")}</h2>
          <span className={cli?.running ? "status-ok" : "status-muted"}>{cli?.running ? t("running") : t("stopped")}</span>
        </div>
        <p>{t("cliConnectionHelp")}</p>
        <code>{cli?.endpoint}</code>
        <div className="settings-actions">
          <button className="secondary-button" onClick={restart}><RefreshCw size={16} />{t("restart")}</button>
          <button className="secondary-button" onClick={regenerate}><KeyRound size={16} />{t("resetCliConnection")}</button>
        </div>
        {cli?.lastError && <div className="inline-error">{cli.lastError}</div>}
      </section>
    </div>
  );
}
