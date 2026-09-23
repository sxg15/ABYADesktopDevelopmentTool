import { useEffect, useMemo, useState } from "react";
import { Play, RefreshCw } from "lucide-react";
import { Modal } from "../../app/Modal";
import { api, errorMessage } from "../../shared/api";
import type {
  ArchiveOption,
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
  executablePath,
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
  const [archives, setArchives] = useState<ArchiveOption[]>([]);
  const [archiveGuid, setArchiveGuid] = useState("");
  const [levelGuid, setLevelGuid] = useState("");
  const [hostInstanceId, setHostInstanceId] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const hosts = taskInstances.filter(
    (instance) =>
      instance.mode === "lan-host" && instance.processState === "running",
  );
  const selectedArchive = archives.find(
    (archive) => archive.archiveGuid === archiveGuid,
  );

  useEffect(() => {
    void refreshArchives();
  }, []);

  useEffect(() => {
    if (!selectedArchive) {
      setLevelGuid("");
      return;
    }
    const start =
      selectedArchive.levels.find((level) => level.isStart) ??
      selectedArchive.levels[0];
    setLevelGuid(start?.levelGuid ?? "");
  }, [archiveGuid]);

  const canLaunch = useMemo(() => {
    if (!name.trim() || !executablePath) return false;
    if (mode === "lan-client") return Boolean(hostInstanceId);
    return Boolean(archiveGuid);
  }, [archiveGuid, executablePath, hostInstanceId, levelGuid, mode, name]);

  async function refreshArchives() {
    try {
      setArchives(await api.discoverArchives());
    } catch (value) {
      setError(errorMessage(value));
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
      await api.launchInstance(taskId, name, executablePath, profile);
      onLaunched();
      onClose();
    } catch (value) {
      setError(errorMessage(value));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal title={t("launch")} onClose={onClose}>
      {!executablePath && <div className="inline-warning">{t("executableMissing")}</div>}
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
                value={archiveGuid}
                onChange={(event) => setArchiveGuid(event.target.value)}
              >
                <option value="">-</option>
                {archives.map((archive) => (
                  <option key={archive.archiveGuid} value={archive.archiveGuid}>
                    {archive.archiveName}
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
