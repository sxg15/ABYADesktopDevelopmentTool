import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { CloudDownload, Download, RefreshCw } from "lucide-react";
import { Modal } from "../../app/Modal";
import type { MessageKey } from "../../i18n";
import type { AppSettings } from "../../shared/types";
import { bytes, clientApi, clientError, type ClientInstallation, type ClientRetention, type ClientUpdate } from "../../shared/gameClients";
import { ClientProgress, useClientOperation } from "./ClientProgress";

interface ClientActions {
  download: (releaseId: number, assetId: number, storageRoot: string) => void;
  useInstalled: (record: ClientInstallation) => void;
  checkUpdate: () => void;
  refreshUpdates: () => void;
  remoteSource: boolean;
  busy: boolean;
  retention?: ClientRetention;
  retentionUnavailable: boolean;
}
const Context = createContext<ClientActions | null>(null);
export function useClientWorkflow() {
  const value = useContext(Context);
  if (!value) throw new Error("ClientWorkflowProvider is required");
  return value;
}

export function ClientRetentionNotice({ t }: { t: (key: MessageKey) => string }) {
  const { retention, retentionUnavailable } = useClientWorkflow();
  if (retentionUnavailable) return <p className="client-retention" role="status">{t("gcCleanupUnavailable")}</p>;
  if (!retention?.deferred.length) return null;
  const reasons: Record<string, MessageKey> = { task: "gcRetainedTask", running: "gcRetainedRunning", local: "gcRetainedLocal", unavailable: "gcRetainedUnavailable" };
  return <div className="client-retention" role="status"><strong>{t("gcCleanupDeferred")}</strong>
    {retention.deferred.map(item => <p key={item.installationId}>{item.tagName} · {t(reasons[item.reason] ?? "gcRetainedUnavailable")}</p>)}
  </div>;
}

/** Stays mounted above navigation so a download cannot lose its selection step. */
export function ClientWorkflowProvider({ children, settings, t, onSaved, notify }: {
  children: ReactNode; settings?: AppSettings; t: (key: MessageKey) => string;
  onSaved: (value: AppSettings) => void; notify: (message: string, error?: boolean) => void;
}) {
  const [working, setWorking] = useState(false);
  const [installOpen, setInstallOpen] = useState(false);
  const [pending, setPending] = useState<ClientInstallation>();
  const [executable, setExecutable] = useState("");
  const [error, setError] = useState("");
  const [update, setUpdate] = useState<ClientUpdate | null>(null);
  const [updateOpen, setUpdateOpen] = useState(false);
  const [assetId, setAssetId] = useState(0);
  const [retention, setRetention] = useState<ClientRetention>();
  const [retentionUnavailable, setRetentionUnavailable] = useState(false);
  const lock = useRef(false);
  const checking = useRef(false);
  const maintaining = useRef(false);
  const seen = useRef(new Set<string>());
  const latest = useRef({ settings, t, onSaved, notify });
  latest.current = { settings, t, onSaved, notify };
  const operation = useClientOperation();
  const busy = working || !!operation?.active;
  const selectionKey = `${settings?.gameClient?.source}:${settings?.gameClient?.installationId}`;
  const selection = useRef(selectionKey); selection.current = selectionKey;

  async function maintain() {
    if (maintaining.current || lock.current) return;
    maintaining.current = true;
    try { setRetention(await clientApi.maintain()); setRetentionUnavailable(false); }
    catch (reason) {
      if (!(typeof reason === "object" && reason !== null && "code" in reason && reason.code === "clientBusy")) setRetentionUnavailable(true);
    }
    finally { maintaining.current = false; }
  }
  useEffect(() => {
    void maintain();
    const timer = window.setInterval(() => void maintain(), 60_000);
    return () => window.clearInterval(timer);
  }, []);

  async function check(manual = false) {
    if (checking.current || lock.current || latest.current.settings?.gameClient?.source !== "remote") return;
    checking.current = true;
    const expected = selection.current;
    try {
      const next = await clientApi.checkUpdate();
      if (selection.current !== expected) return;
      setUpdate(next);
      if (next) {
        const key = `${expected}:${next.key}`;
        if (manual || !seen.current.has(key)) {
          seen.current.add(key); setAssetId(next.preferredAssetId ?? 0); setUpdateOpen(true);
        }
      } else { setUpdateOpen(false); if (manual) latest.current.notify(latest.current.t("gcNoUpdate")); }
    } catch (reason) { if (manual) latest.current.notify(clientError(reason, latest.current.t), true); }
    finally { checking.current = false; if (selection.current !== expected) void check(); }
  }
  useEffect(() => {
    setUpdate(null); setUpdateOpen(false);
    void check();
    const timer = window.setInterval(() => void check(), 30 * 60 * 1000);
    const onFocus = () => void check();
    window.addEventListener("focus", onFocus);
    return () => { window.clearInterval(timer); window.removeEventListener("focus", onFocus); };
  }, [selectionKey]);

  async function activate(record: ClientInstallation, path: string) {
    const saved = await clientApi.activate(record.id, path);
    latest.current.onSaved(saved);
    setPending(undefined); setInstallOpen(false); setUpdateOpen(false); setUpdate(null);
    latest.current.notify(latest.current.t("gcInstallDone"));
  }
  async function begin(action: () => Promise<ClientInstallation>) {
    if (lock.current) return;
    lock.current = true; setWorking(true); setError(""); setInstallOpen(true); setUpdateOpen(false); setPending(undefined);
    try {
      const record = await action(); setPending(record);
      const path = record.executableCandidates.length === 1 ? record.executableCandidates[0] : "";
      setExecutable(path);
      if (path) await activate(record, path);
    } catch (reason) { setError(clientError(reason, latest.current.t)); }
    finally { lock.current = false; setWorking(false); void maintain(); }
  }
  async function confirmExecutable() {
    if (lock.current || !pending || !executable) return;
    lock.current = true; setWorking(true); setError("");
    try { await activate(pending, executable); }
    catch (reason) { setError(clientError(reason, latest.current.t)); }
    finally { lock.current = false; setWorking(false); void maintain(); }
  }
  async function closeInstall() {
    if (lock.current) return;
    lock.current = true; setWorking(true);
    try {
      if (pending) await clientApi.discardPending(pending.id);
      setPending(undefined); setInstallOpen(false); setError("");
    } catch (reason) { setError(clientError(reason, latest.current.t)); }
    finally { lock.current = false; setWorking(false); void maintain(); }
  }
  const download = (releaseId: number, selectedAsset: number, storageRoot: string) => {
    void begin(() => clientApi.install(releaseId, selectedAsset, storageRoot));
  };

  return <Context.Provider value={{ download, useInstalled: record => void begin(async () => record), checkUpdate: () => void check(true), refreshUpdates: () => void check(), remoteSource: settings?.gameClient?.source === "remote", busy: busy || installOpen, retention, retentionUnavailable }}>
    {children}
    {update && !updateOpen && !installOpen && <button className="client-update-reminder primary-button" onClick={() => { setAssetId(update.preferredAssetId ?? 0); setUpdateOpen(true); }}>
      <CloudDownload size={20} />{t("gcUpdateNow")} · {update.tagName}
    </button>}
    {updateOpen && update && !installOpen && <Modal title={t("gcUpdateTitle")} onClose={() => setUpdateOpen(false)}>
      <div className="client-update-dialog">
        <CloudDownload size={40} /><p>{t("gcUpdateQuestion")}</p>
        <div className="client-version-change"><span>{update.currentTag}</span><strong>→ {update.tagName}</strong></div>
        {update.assets.length > 1 ? <label className="field"><span>{t("gcSelectAsset")}</span><select value={assetId} onChange={event => setAssetId(Number(event.target.value))}>
          <option value={0}>{t("gcSelectAsset")}</option>{update.assets.map(asset => <option key={asset.id} value={asset.id}>{asset.name} · {bytes(asset.size)}</option>)}
        </select></label> : <p className="client-path">{update.assets[0]?.name} · {bytes(update.assets[0]?.size ?? 0)}</p>}
        <p>{t("gcReplaceHint")}</p>
        <div className="modal-actions"><button className="secondary-button" onClick={() => setUpdateOpen(false)}>{t("gcLater")}</button>
          <button className="primary-button client-download-primary" disabled={busy || !assetId} onClick={() => download(update.releaseId, assetId, settings?.gameClient?.storageRoot ?? "")}>
            <Download size={20} />{t("gcUpdateNow")}</button></div>
      </div>
    </Modal>}
    {installOpen && <Modal title={t(working ? "gcInstalling" : pending ? "gcChooseExeTitle" : "gcInstall")} onClose={() => { void closeInstall(); }}>
      <div className="client-install-dialog">
        {working && <p role="status">{t("gcInstalling")}</p>}
        <ClientProgress operation={operation} t={t} onError={setError} />
        {pending && !working && <><p>{pending.tagName} · {pending.assetName}</p><label className="field"><span>{t("gcExe")}</span>
          <select value={executable} onChange={event => setExecutable(event.target.value)}><option value="">{t("gcChooseExe")}</option>
            {pending.executableCandidates.map(path => <option key={path} value={path}>{path}</option>)}</select></label></>}
        {error && <div className="inline-error" role="alert">{error}</div>}
        <p>{t("gcReplaceHint")}</p>
        <div className="modal-actions"><button className="secondary-button" disabled={working} onClick={() => void closeInstall()}>{t(pending ? "cancel" : "close")}</button>
          {pending && <button className="primary-button client-download-primary" disabled={working || !executable} onClick={() => void confirmExecutable()}><RefreshCw size={18} />{t("gcUseNow")}</button>}</div>
      </div>
    </Modal>}
  </Context.Provider>;
}
