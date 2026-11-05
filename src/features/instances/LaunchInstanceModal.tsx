import { useEffect, useMemo, useRef, useState } from "react";
import { Play, RefreshCw } from "lucide-react";
import { Modal } from "../../app/Modal";
import { api, errorMessage } from "../../shared/api";
import { productionApi } from "../../shared/production";
import { taskLaunchApi, type TaskLaunchSources } from "../../shared/taskLaunch";
import type {
  GameInstance,
  LaunchMode,
  LaunchProfile,
  WindowMode,
  WindowVisibilityMode,
} from "../../shared/types";
import type { MessageKey } from "../../i18n";

const resolutions = [
  [1280, 720],
  [1600, 900],
  [1920, 1080],
  [2560, 1440],
] as const;

export function LaunchInstanceModal({
  taskId,
  taskInstances,
  t,
  onClose,
  onLaunched,
}: {
  taskId: string;
  executablePath: string;
  taskInstances: GameInstance[];
  t: (key: MessageKey) => string;
  onClose: () => void;
  onLaunched: () => void;
}) {
  const [name, setName] = useState("");
  const [mode, setMode] = useState<LaunchMode>("editor");
  const [windowMode, setWindowMode] = useState<WindowMode>("windowed");
  const [visibilityMode, setVisibilityMode] =
    useState<WindowVisibilityMode>("background");
  const [resolution, setResolution] = useState("1280x720");
  const [sources, setSources] = useState<TaskLaunchSources | null>(null);
  const archives = sources?.entries ?? [];
  const generation = useRef(0);
  const [exportMessage, setExportMessage] = useState("");
  const [archiveGuid, setArchiveGuid] = useState("");
  const [levelGuid, setLevelGuid] = useState("");
  const [hostInstanceId, setHostInstanceId] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const sourceLoaded = sources !== null;
  const launchExecutable = sources?.player.selectedPath ?? "";
  useEffect(() => {
    setSources(null); setArchiveGuid(""); setExportMessage(""); setError("");
    void refreshArchives();
    return () => { generation.current++; };
  }, [taskId]);

  const hosts = taskInstances.filter(
    (instance) =>
      instance.mode === "lan-host" && instance.processState === "running",
  );
  const selectedEntry = archives.find(entry => entry.id === archiveGuid);
  const selectedArchive = selectedEntry?.archive;

  useEffect(() => {
    if (!selectedArchive) {
      setLevelGuid("");
      return;
    }
    const start =
      selectedArchive.levels.find((level) => level.isStart) ??
      selectedArchive.levels[0];
    setLevelGuid(start?.levelGuid ?? "");
  }, [selectedArchive]);

  const canLaunch = useMemo(() => {
    if (!sourceLoaded || !sources?.player.matches || !name.trim() || !launchExecutable) return false;
    if (mode === "lan-client") return Boolean(hostInstanceId);
    return Boolean(selectedArchive);
  }, [selectedArchive, sources, launchExecutable, sourceLoaded, hostInstanceId, mode, name]);

  async function refreshArchives() {
    const current = ++generation.current;
    try {
      const value = await taskLaunchApi.sources(taskId);
      if (generation.current === current) { setSources(value); setError(""); }
    } catch (value) {
      if (generation.current === current) { setSources(null); setError(errorMessage(value)); }
    }
  }

  async function launch() {
    const [width, height] = resolution.split("x").map(Number);
    const profile: LaunchProfile = {
      mode,
      windowMode,
      visibilityMode,
      width,
      height,
      exitOnFailure: true,
      language: "",
    };
    if (mode !== "lan-client" && selectedArchive) {
      const level = selectedArchive.levels.find(
        (candidate) => candidate.levelGuid === levelGuid,
      );
      profile.archive = {
        archivePath: selectedArchive.archivePath,
        archiveGuid: selectedArchive.archiveGuid,
        archiveName: selectedArchive.archiveName,
        levelGuid,
        levelName: level?.levelName ?? "",
      };
    }
    if (mode === "lan-client") profile.hostInstanceId = hostInstanceId;
    setBusy(true);
    setError("");
    try {
      await api.launchInstance(taskId, name, launchExecutable, profile);
      onLaunched();
      onClose();
    } catch (value) {
      setError(errorMessage(value));
    } finally {
      setBusy(false);
    }
  }

  async function assetAction(action: "adopt" | "export" | "preview") {
    if (!sources) return;
    const current = generation.current;
    setBusy(true); setError("");
    try {
      if (action === "adopt") await taskLaunchApi.adopt(taskId, sources.player.selectedHash);
      if (action === "preview") await productionApi.startPreview(taskId);
      if (action === "export" && sources.candidate) {
        const result = await taskLaunchApi.export(taskId, sources.candidate.version);
        if (generation.current === current) setExportMessage(`${result.outcome === "already-present" ? "已存在相同版本" : "已导出"}：${result.version} → ${result.destination}`);
      }
      if (generation.current === current) await refreshArchives();
    } catch (reason) { if (generation.current === current) setError(errorMessage(reason)); }
    finally { setBusy(false); }
  }

  return (
    <Modal title={t("launch")} onClose={onClose}>
      {sources && <div className="inline-warning">
        <div>当前 APP 构建包：{sources.player.selectedPath}</div>
        <div>构建指纹：{sources.player.selectedHash}</div>
        {sources.player.previous?.path && <div>任务记录：{sources.player.previous.path} · {sources.player.previous.hash}</div>}
        {!sources.player.matches && <>
          <p>任务记录与当前设置不同。确认后使用当前包；旧版本验证结果保留，需重新验证。</p>
          <button type="button" disabled={busy} onClick={() => void assetAction("adopt")}>确认使用 APP 当前构建包</button>
        </>}
      </div>}
      {sources?.warnings.map(warning => <p key={warning} role="alert">{warning}</p>)}
      {sources?.candidate && <div>
        <p>候选版本：{sources.candidate.version} · {sources.candidate.path}</p>
        {sources.previewVersion && <button type="button" disabled={busy || !sources.player.matches} onClick={() => void assetAction("preview")}>试玩已验证版本 {sources.previewVersion}</button>}
        <button type="button" disabled={busy} onClick={() => void assetAction("export")}>导出候选到全局存档目录</button>
        <p>导出位置：{sources.globalRoot}；导出不代表验收通过。</p>
      </div>}
      {exportMessage && <p role="status">{exportMessage}</p>}
      {selectedArchive && <p>保存/启动位置：{selectedArchive.archivePath}</p>}
      {!launchExecutable && <div className="inline-warning">{t("executableMissing")}</div>}
      <div className="form-grid">
        <label className="field field-span">
          <span>{t("instanceName")}</span>
          <input
            value={name}
            onChange={(event) => setName(event.target.value)}
            autoFocus
            maxLength={80}
          />
        </label>

        <fieldset className="field field-span">
          <legend>{t("launchMode")}</legend>
          <div className="segmented">
            {(["editor", "offline", "lan-host", "lan-client"] as LaunchMode[]).map((value) => (
              <button
                key={value}
                className={mode === value ? "selected" : ""}
                onClick={() => setMode(value)}
                type="button"
              >
                {t(value)}
              </button>
            ))}
          </div>
        </fieldset>

        <label className="field">
          <span>{t("resolution")}</span>
          <select
            value={resolution}
            onChange={(event) => setResolution(event.target.value)}
          >
            {resolutions.map(([width, height]) => (
              <option key={width} value={`${width}x${height}`}>
                {width} x {height}
              </option>
            ))}
          </select>
        </label>

        <fieldset className="field">
          <legend>{t("windowVisibility")}</legend>
          <div className="segmented">
            {(["background", "visible"] as WindowVisibilityMode[]).map(
              (value) => (
                <button
                  key={value}
                  className={visibilityMode === value ? "selected" : ""}
                  onClick={() => {
                    setVisibilityMode(value);
                    if (value === "background") setWindowMode("windowed");
                  }}
                  type="button"
                >
                  {t(value)}
                </button>
              ),
            )}
          </div>
        </fieldset>

        <label className="field">
          <span>{t("windowMode")}</span>
          <select
            value={windowMode}
            disabled={visibilityMode === "background"}
            onChange={(event) =>
              setWindowMode(event.target.value as WindowMode)
            }
          >
            <option value="windowed">{t("windowed")}</option>
            <option value="borderless">{t("borderless")}</option>
            <option value="fullscreen">{t("fullscreen")}</option>
          </select>
        </label>

        {mode !== "lan-client" && (
          <>
            <label className="field">
              <span className="label-with-action">
                {t("archive")}
                <button
                  className="inline-icon"
                  onClick={refreshArchives}
                  title={t("refresh")}
                  type="button"
                >
                  <RefreshCw size={14} />
                </button>
              </span>
              <select
                aria-label="存档来源"
                value={archiveGuid}
                onChange={(event) => setArchiveGuid(event.target.value)}
              >
                <option value="">-</option>
                {archives.map((entry) => (
                  <option key={entry.id} value={entry.id}>
                    {entry.origin === "task" ? "本任务编辑存档" : "全局存档"} · {entry.archive.archiveName}
                  </option>
                ))}
              </select>
            </label>
            <label className="field">
              <span>{t("level")}</span>
              <select
                value={levelGuid}
                onChange={(event) => setLevelGuid(event.target.value)}
              >
                <option value="">-</option>
                {selectedArchive?.levels.map((level) => (
                  <option key={level.levelGuid} value={level.levelGuid}>
                    {level.levelName}
                  </option>
                ))}
              </select>
            </label>
          </>
        )}

        {mode === "lan-client" && (
          <label className="field field-span">
            <span>{t("hostInstance")}</span>
            <select
              value={hostInstanceId}
              onChange={(event) => setHostInstanceId(event.target.value)}
            >
              <option value="">-</option>
              {hosts.map((host) => (
                <option key={host.id} value={host.id}>
                  {host.name} · PID {host.pid}
                </option>
              ))}
            </select>
          </label>
        )}
      </div>
      {error && <div className="inline-error">{error}</div>}
      <footer className="modal-actions">
        <button className="secondary-button" onClick={onClose}>
          {t("cancel")}
        </button>
        <button
          className="primary-button"
          onClick={launch}
          disabled={!canLaunch || busy}
        >
          <Play size={16} />
          {t("launch")}
        </button>
      </footer>
    </Modal>
  );
}
