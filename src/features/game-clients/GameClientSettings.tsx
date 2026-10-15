import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Download, FileSearch, FolderOpen, RefreshCw } from "lucide-react";
import type { MessageKey } from "../../i18n";
import { bytes, clientApi, clientError, installable, type ClientInstallation, type ClientRelease, type ClientUpdate, type GameClientSelection, type GameClientState } from "../../shared/gameClients";
import { ClientProgress, useClientOperation } from "./ClientProgress";

export function GameClientSettings({ value, onChange, t, onManage, preferredInstallationId, onBusyChange, accountRevision = 0 }: {
  value: GameClientSelection; onChange: (value: GameClientSelection) => void; t: (key: MessageKey) => string;
  onManage: () => void; preferredInstallationId?: string; onBusyChange: (busy: boolean) => void;
  accountRevision?: number;
}) {
  const [state, setState] = useState<GameClientState>();
  const [releases, setReleases] = useState<ClientRelease[]>([]);
  const [releaseId, setReleaseId] = useState(0);
  const [detail, setDetail] = useState<ClientRelease>();
  const [revision, setRevision] = useState(0);
  const [assetId, setAssetId] = useState(0);
  const [working, setWorking] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [update, setUpdate] = useState<ClientUpdate | null>();
  const operation = useClientOperation();
  const appliedPreferred = useRef("");
  const busy = working || !!operation?.active;
  const storageRoot = value.storageRoot || state?.defaultStorageRoot || "";
  const selected = state?.installations.find(item => item.id === value.installationId);

  async function refresh() {
    setLoading(true);
    try {
      const next = await clientApi.state(); setState(next);
      if (next.account) {
        const items = (await clientApi.releases()).filter(item => !item.draft);
        setReleases(items); setReleaseId(current => items.some(item => item.id === current) ? current : items[0]?.id ?? 0);
      } else { setReleases([]); setReleaseId(0); }
      setRevision(current => current + 1);
    } finally { setLoading(false); }
  }
  useEffect(() => { setError(""); void refresh().catch(error => setError(clientError(error, t))); }, [accountRevision]);
  useEffect(() => { onBusyChange(busy); }, [busy, onBusyChange]);
  useEffect(() => {
    let alive = true; setDetail(undefined); setAssetId(0);
    if (!releaseId) return;
    void clientApi.release(releaseId).then(item => {
      if (alive) { setDetail(item); setAssetId(item.assets.find(installable)?.id ?? 0); }
    }).catch(error => { if (alive) setError(clientError(error, t)); });
    return () => { alive = false; };
  }, [releaseId, revision]);
  useEffect(() => {
    if (!preferredInstallationId || appliedPreferred.current === preferredInstallationId) return;
    const record = state?.installations.find(item => item.id === preferredInstallationId);
    if (record) { appliedPreferred.current = record.id; choose(record); }
  }, [preferredInstallationId, state]);
  useEffect(() => {
    if (operation?.kind === "install" && !operation.active) {
      void clientApi.state().then(setState).catch(() => undefined);
    }
  }, [operation?.active, operation?.kind]);

  function choose(record: ClientInstallation) {
    onChange({ ...value, source: "remote", installationId: record.id,
      storageRoot: record.storageRoot || value.storageRoot,
      executableRelativePath: record.executableCandidates.length === 1 ? record.executableCandidates[0] : "" });
  }
  async function run(action: () => Promise<unknown>) {
    setWorking(true); setError("");
    try { await action(); } catch (error) { setError(clientError(error, t)); }
    finally { setWorking(false); }
  }
  const executable = value.source === "local" ? value.localExecutablePath
    : selected && value.executableRelativePath ? `${selected.directory}\\${value.executableRelativePath.replace(/\//g, "\\")}` : "";

  return <section className="settings-section client-settings">
    <h2>{t("gameExecutable")}</h2>
    <div className="segmented client-source" role="group" aria-label={t("gcSource")}>
      <button disabled={busy} className={value.source === "local" ? "selected" : ""} aria-pressed={value.source === "local"} onClick={() => onChange({ ...value, source: "local" })}>{t("gcLocal")}</button>
      <button disabled={busy} className={value.source === "remote" ? "selected" : ""} aria-pressed={value.source === "remote"} onClick={() => onChange({ ...value, source: "remote" })}>{t("gcRemote")}</button>
    </div>
    {value.source === "local" ? <div className="path-control">
      <input aria-label={t("gameExecutable")} value={value.localExecutablePath} readOnly />
      <button className="secondary-button" disabled={busy} onClick={() => void run(async () => {
        const path = await open({ multiple: false, directory: false, filters: [{ name: "Windows executable", extensions: ["exe"] }] });
        if (typeof path === "string") onChange({ ...value, localExecutablePath: path });
      })}><FileSearch size={16} />{t("browse")}</button>
    </div> : <div className="client-settings-remote">
      <div className="client-row"><button className="secondary-button" onClick={onManage}>{t("gcManage")}</button>
        <button className="secondary-button" disabled={busy || loading} onClick={() => void run(refresh)}><RefreshCw size={15} />{t("refresh")}</button>
        <button className="secondary-button" disabled={busy || !state?.account} onClick={() => void run(async () => { setUpdate(await clientApi.checkUpdate()); })}>{t("gcCheckUpdate")}</button>
      </div>
      {update && <div className="client-update"><strong>{t("gcUpdate")}: {update.tagName}</strong><p>{t("gcUpdateHint")}</p></div>}
      {update === null && <p role="status">{t("gcNoUpdate")}</p>}
      <label className="field"><span>{t("gcSelectInstalled")}</span><select disabled={busy} value={value.installationId} onChange={event => {
        const record = state?.installations.find(item => item.id === event.target.value);
        if (record) choose(record); else onChange({ ...value, installationId: "", executableRelativePath: "" });
      }}><option value="">{t("gcSelectInstalled")}</option>{state?.installations.map(record => <option key={record.id} value={record.id}>{record.tagName} · {record.assetName} · {new Date(record.installedAt).toLocaleString()}</option>)}</select></label>
      {selected && <label className="field"><span>{t("gcExe")}</span><select disabled={busy} value={value.executableRelativePath} onChange={event => onChange({ ...value, executableRelativePath: event.target.value })}>
        <option value="">{t("gcChooseExe")}</option>{selected.executableCandidates.map(path => <option key={path} value={path}>{path}</option>)}
      </select></label>}
      {selected && <p className="client-path">{selected.directory}</p>}
      <div className="client-download-form">
        <label className="field"><span>{t("gcStorage")}</span><div className="path-control"><input readOnly value={storageRoot} /><button className="secondary-button" disabled={busy} onClick={() => void run(async () => {
          const path = await open({ multiple: false, directory: true }); if (typeof path === "string") onChange({ ...value, storageRoot: path });
        })}><FolderOpen size={16} />{t("browse")}</button></div></label>
        <p>{t("gcStorageHint")}</p>
        {state?.account ? <>
          <div className="client-download-selects">
            <label className="field"><span>{t("gcSelectRelease")}</span><select value={releaseId} disabled={busy || loading} onChange={event => setReleaseId(Number(event.target.value))}>
              <option value={0}>{loading ? t("gcLoading") : t("gcSelectRelease")}</option>{releases.map(item => <option value={item.id} key={item.id}>{item.tagName}{item.prerelease ? ` · ${t("gcPrerelease")}` : ""}</option>)}
            </select></label>
            <label className="field"><span>{t("gcSelectAsset")}</span><select value={assetId} disabled={busy || !detail} onChange={event => setAssetId(Number(event.target.value))}>
              <option value={0}>{t("gcSelectAsset")}</option>{detail?.assets.filter(installable).map(asset => <option key={asset.id} value={asset.id}>{asset.name} · {bytes(asset.size)}</option>)}
            </select></label>
          </div>
          {!loading && releases.length === 0 && <p>{t("gcNoReleases")}</p>}
          <button className="primary-button" disabled={busy || !assetId || !storageRoot} onClick={() => void run(async () => {
            const installed = await clientApi.install(releaseId, assetId, storageRoot);
            setState(await clientApi.state());
            onChange({ ...value, source: "remote", storageRoot, installationId: installed.id,
              executableRelativePath: installed.executableCandidates.length === 1 ? installed.executableCandidates[0] : "" });
          })}><Download size={16} />{t("gcInstall")}</button>
          <p>{t("gcInstallHint")}</p>
        </> : <p>{t("gcLoginFirst")}</p>}
      </div>
      {executable && <label className="field"><span>{t("gcActivePath")}</span><code className="client-path">{executable}</code></label>}
      <p>{t("gcSaveHint")}</p>
    </div>}
    <ClientProgress operation={operation} t={t} onError={setError} />
    {error && <div className="inline-error" role="alert">{error}</div>}
  </section>;
}
