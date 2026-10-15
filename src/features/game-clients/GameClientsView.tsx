import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { CloudDownload, Download, ExternalLink, FolderOpen, Github, Pencil, Plus, RefreshCw, Settings, Trash2, Upload } from "lucide-react";
import { Modal } from "../../app/Modal";
import type { MessageKey } from "../../i18n";
import type { AppSettings } from "../../shared/types";
import { bytes, clientApi, clientError, installable, type ClientInstallation, type ClientRelease, type GameClientState, type ReleaseAsset, type ReleaseInput } from "../../shared/gameClients";
import { ClientProgress, useClientOperation } from "./ClientProgress";

type DeleteTarget = { release: ClientRelease; asset?: ReleaseAsset };

export function GameClientsView({ settings, t, notify, onOpenSettings, preferredReleaseId }: {
  settings?: AppSettings; t: (key: MessageKey) => string; notify: (message: string, error?: boolean) => void;
  onOpenSettings: (installationId?: string, focusGitHub?: boolean) => void; preferredReleaseId?: number;
}) {
  const [state, setState] = useState<GameClientState>();
  const [releases, setReleases] = useState<ClientRelease[]>([]);
  const [selectedId, setSelectedId] = useState(preferredReleaseId ?? 0);
  const [detail, setDetail] = useState<ClientRelease>();
  const [detailLoading, setDetailLoading] = useState(false);
  const [revision, setRevision] = useState(0);
  const [search, setSearch] = useState("");
  const [storage, setStorage] = useState(settings?.gameClient?.storageRoot ?? "");
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  const [editor, setEditor] = useState<ReleaseInput>();
  const [deleting, setDeleting] = useState<DeleteTarget>();
  const operation = useClientOperation();
  const busy = working || loading || !!operation?.active;

  async function refresh(prefer?: number) {
    setLoading(true);
    try {
      const next = await clientApi.state(); setState(next);
      setStorage(current => current || next.defaultStorageRoot);
      if (next.account) {
        const items = await clientApi.releases(); setReleases(items);
        setSelectedId(current => items.some(item => item.id === (prefer ?? current)) ? (prefer ?? current) : items[0]?.id ?? 0);
      } else { setReleases([]); setSelectedId(0); }
      setRevision(value => value + 1);
    } finally { setLoading(false); }
  }
  useEffect(() => { void refresh(preferredReleaseId).catch(error => setError(clientError(error, t))); }, []);
  useEffect(() => { if (preferredReleaseId) setSelectedId(preferredReleaseId); }, [preferredReleaseId]);
  useEffect(() => {
    if (operation?.kind === "install" && !operation.active) void clientApi.state().then(setState).catch(() => undefined);
  }, [operation?.active, operation?.kind]);
  useEffect(() => {
    let alive = true; setDetail(undefined);
    if (!selectedId) { setDetailLoading(false); return; }
    setDetailLoading(true);
    void clientApi.release(selectedId).then(value => { if (alive) setDetail(value); })
      .catch(error => { if (alive) setError(clientError(error, t)); }).finally(() => { if (alive) setDetailLoading(false); });
    return () => { alive = false; };
  }, [selectedId, revision]);

  async function run(action: () => Promise<unknown>, reload = false) {
    setWorking(true); setError("");
    try { await action(); if (reload) await refresh(); }
    catch (error) { setError(clientError(error, t)); }
    finally { setWorking(false); }
  }
  async function browseStorage() {
    const path = await open({ directory: true, multiple: false });
    if (typeof path === "string") setStorage(path);
  }
  async function download(release: ClientRelease, asset: ReleaseAsset) {
    await clientApi.install(release.id, asset.id, storage);
    notify(t("gcInstallDone"));
  }
  function chooseInstalled(record: ClientInstallation) { onOpenSettings(record.id); }
  const filtered = releases.filter(item => `${item.tagName} ${item.name ?? ""}`.toLowerCase().includes(search.toLowerCase()));
  const canManage = !!state?.account?.canManage;

  return <div className="workspace client-workspace">
    <header className="page-toolbar">
      <div><h1>{t("gcTitle")}</h1><p className="client-subtitle">{t("gcSubtitle")}</p></div>
      <div className="client-actions">
        <button className="secondary-button" disabled={busy} onClick={() => void run(() => refresh())}><RefreshCw size={16} />{t("refresh")}</button>
        <button className="primary-button" disabled={busy || !canManage} onClick={() => setEditor({ id: null, tagName: "", name: "", body: "", draft: true, prerelease: false })}><Plus size={16} />{t("gcNewRelease")}</button>
      </div>
    </header>
    <div className="client-content">
      <section className="client-account client-card">
        <div className="client-row">
          <Github size={23} /><div className="client-grow"><strong>{state?.account ? state.account.login : t("gcAccount")}</strong>
            <p>{t("gcRepository")}: sxg15/PlatverseOutput</p></div>
          <button className="secondary-button" onClick={() => void run(() => openUrl("https://github.com/sxg15/PlatverseOutput/releases"))}><ExternalLink size={15} />GitHub</button>
          {state?.account && <button className="secondary-button" onClick={() => onOpenSettings(undefined, true)}><Settings size={15} />{t("gcAccountSettings")}</button>}
        </div>
        {state?.account ? <div className="client-badges"><span className="client-badge">{t(canManage ? "gcCanManage" : "gcReadOnly")}</span>{!state.account.remembered && <span className="client-badge">{t("gcSessionOnly")}</span>}</div> : !loading && <div className="client-login">
          <strong>{t("gcNotLoggedIn")}</strong><p>{t("gcLoginFirst")}</p>
          <button className="primary-button" onClick={() => onOpenSettings(undefined, true)}><Github size={17} />{t("gcGoToLogin")}</button>
        </div>}
      </section>
      {error && <div className="inline-error" role="alert">{error}</div>}
      <ClientProgress operation={operation} t={t} onError={setError} />
      {loading && <p role="status">{t("gcLoading")}</p>}
      {state?.account && <div className="client-catalog">
        <section className="client-card client-release-list" aria-label={t("gcSelectRelease")}>
          <label className="field"><span>{t("gcSearch")}</span><input value={search} onChange={event => setSearch(event.target.value)} /></label>
          {!loading && releases.length === 0 && <div className="empty-state"><CloudDownload size={30} /><h3>{t("gcNoReleases")}</h3><p>{t("gcNoReleasesHelp")}</p></div>}
          {filtered.map(release => <button key={release.id} className={`client-release-item ${release.id === selectedId ? "selected" : ""}`} aria-pressed={release.id === selectedId} onClick={() => setSelectedId(release.id)}>
            <strong>{release.name || release.tagName}</strong><span>{release.tagName}</span>
            <span className="client-badges"><span className="client-badge">{t(release.draft ? "gcDraft" : release.prerelease ? "gcPrerelease" : "gcPublished")}</span>
              {settings?.gameClient?.source === "remote" && state.installations.some(item => item.id === settings.gameClient?.installationId && item.releaseId === release.id) && <span className="client-badge current">{t("gcCurrent")}</span>}
            </span>
          </button>)}
          {releases.length > 0 && filtered.length === 0 && <p className="empty-state">{t("gcNoMatches")}</p>}
        </section>
        <section className="client-card client-release-detail" aria-label={t("gcSelectedRelease")}>
          {detailLoading ? <p role="status">{t("gcLoading")}</p> : detail ? <>
            <div className="client-row">
              <div className="client-grow"><h2>{detail.name || detail.tagName}</h2><p>{detail.tagName}{detail.publishedAt ? ` · ${new Date(detail.publishedAt).toLocaleString()}` : ""}</p></div>
              <button className="icon-button" title={t("gcEditRelease")} disabled={busy || !canManage || detail.immutable} onClick={() => setEditor({ id: detail.id, tagName: detail.tagName, name: detail.name ?? "", body: detail.body ?? "", draft: detail.draft, prerelease: detail.prerelease })}><Pencil size={16} /></button>
              <button className="icon-button danger" title={t("gcDeleteRelease")} disabled={busy || !canManage || detail.immutable} onClick={() => setDeleting({ release: detail })}><Trash2 size={16} /></button>
            </div>
            {detail.immutable && <span className="client-badge">{t("gcImmutable")}</span>}
            {detail.body && <p className="client-release-notes">{detail.body}</p>}
            <div className="client-section-heading"><h3>{t("gcAssets")}</h3>
              <button className="secondary-button" disabled={busy || !canManage || detail.immutable} onClick={() => void run(async () => {
                const path = await open({ directory: false, multiple: false });
                if (typeof path === "string") await clientApi.upload(detail.id, path);
              }, true)}><Upload size={15} />{t("gcUpload")}</button>
            </div>
            <p>{t("gcInstallHint")}</p>
            <label className="field client-storage"><span>{t("gcStorage")}</span><div className="path-control"><input readOnly value={storage} /><button className="secondary-button" disabled={busy} onClick={() => void run(browseStorage)}><FolderOpen size={16} />{t("browse")}</button></div></label>
            {detail.assets.length === 0 && <p className="empty-state">{t("gcNoAssets")}</p>}
            <div className="client-assets">{detail.assets.map(asset => <div key={asset.id} className="client-asset">
              <div className="client-grow"><strong>{asset.name}</strong><p>{bytes(asset.size)}{asset.state !== "uploaded" ? ` · ${asset.state}` : ""}</p></div>
              <button className="secondary-button" disabled={busy || !installable(asset) || !storage} onClick={() => void run(() => download(detail, asset), true)}><Download size={15} />{t("gcInstall")}</button>
              <button className="icon-button danger" title={`${t("gcDeleteAsset")}: ${asset.name}`} disabled={busy || !canManage || detail.immutable} onClick={() => setDeleting({ release: detail, asset })}><Trash2 size={16} /></button>
            </div>)}</div>
          </> : <p className="empty-state">{t("gcSelectPrompt")}</p>}
        </section>
      </div>}
      <section className="client-card">
        <h2>{t("gcInstalled")}</h2>
        {!state?.installations.length && <p className="empty-state">{t("gcNoInstalled")}</p>}
        {state?.installations.map(record => <div className="client-installed" key={record.id}>
          <div className="client-grow"><strong>{record.tagName} · {record.assetName}</strong>
            {settings?.gameClient?.source === "remote" && settings.gameClient.installationId === record.id && <span className="client-badge current">{t("gcCurrent")}</span>}
            <p className="client-path">{record.directory}</p><p>{t(record.digestVerified ? "gcVerified" : "gcLocalHash")}</p>
          </div><button className="secondary-button" onClick={() => chooseInstalled(record)}>{t("gcUseInstalled")}</button>
        </div>)}
      </section>
    </div>
    {editor && <Modal title={t(editor.id ? "gcEditRelease" : "gcNewRelease")} onClose={() => { if (!working) setEditor(undefined); }}>
      <form className="client-editor" onSubmit={event => { event.preventDefault(); void run(async () => {
        const saved = await clientApi.saveRelease(editor); setEditor(undefined); await refresh(saved.id);
      }); }}>
        <label className="field"><span>{t("gcTag")}</span><input required maxLength={128} readOnly={editor.id !== null} value={editor.tagName} onChange={event => setEditor({ ...editor, tagName: event.target.value })} /></label>
        <label className="field"><span>{t("gcReleaseName")}</span><input maxLength={256} value={editor.name} onChange={event => setEditor({ ...editor, name: event.target.value })} /></label>
        <label className="field"><span>{t("gcReleaseNotes")}</span><textarea rows={7} maxLength={65536} value={editor.body} onChange={event => setEditor({ ...editor, body: event.target.value })} /></label>
        <label className="toggle-row"><input type="checkbox" checked={editor.draft} onChange={event => setEditor({ ...editor, draft: event.target.checked })} />{t("gcDraft")}</label>
        <label className="toggle-row"><input type="checkbox" checked={editor.prerelease} onChange={event => setEditor({ ...editor, prerelease: event.target.checked })} />{t("gcPrerelease")}</label>
        {error && <div className="inline-error" role="alert">{error}</div>}
        <div className="modal-actions"><button type="button" className="secondary-button" disabled={working} onClick={() => setEditor(undefined)}>{t("cancel")}</button><button className="primary-button" disabled={busy || !editor.tagName.trim()}>{t("save")}</button></div>
      </form>
    </Modal>}
    {deleting && <Modal title={t(deleting.asset ? "gcDeleteAsset" : "gcDeleteRelease")} onClose={() => { if (!working) setDeleting(undefined); }}>
      <p>{t("gcDeleteConfirm")}</p><p className="client-delete-target">{deleting.release.tagName}{deleting.asset ? ` / ${deleting.asset.name}` : ""}</p>
      {error && <div className="inline-error" role="alert">{error}</div>}
      <div className="modal-actions"><button className="secondary-button" disabled={working} onClick={() => setDeleting(undefined)}>{t("cancel")}</button><button className="danger-button" disabled={busy} onClick={() => void run(async () => {
        if (deleting.asset) await clientApi.deleteAsset(deleting.release.id, deleting.asset.id); else await clientApi.deleteRelease(deleting.release.id);
        setDeleting(undefined);
      }, true)}>{t("gcDeleteConfirmButton")}</button></div>
    </Modal>}
  </div>;
}
