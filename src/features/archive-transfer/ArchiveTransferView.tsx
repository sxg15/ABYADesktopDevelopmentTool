import { useEffect, useMemo, useState } from "react";
import { FileSearch, RefreshCw, Send, Square } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import type { MessageKey } from "../../i18n";
import { api, errorMessage } from "../../shared/api";
import type {
  ArchiveTransferRecord,
  ArchiveTransferStatus,
  ArchiveTransferTarget,
  TransferableArchive,
} from "../../shared/types";

const ACTIVE_STATUSES: ArchiveTransferStatus[] = [
  "preparing",
  "waitingAcceptance",
  "transferring",
  "finalizing",
];

export function ArchiveTransferView({
  t,
  notify,
}: {
  t: (key: MessageKey) => string;
  notify: (message: string, error?: boolean) => void;
}) {
  const [targets, setTargets] = useState<ArchiveTransferTarget[]>([]);
  const [sources, setSources] = useState<TransferableArchive[]>([]);
  const [transfers, setTransfers] = useState<ArchiveTransferRecord[]>([]);
  const [targetId, setTargetId] = useState("");
  const [sourcePath, setSourcePath] = useState("");
  const [manualSource, setManualSource] = useState<TransferableArchive>();
  const [busy, setBusy] = useState(false);

  const selectedTarget = targets.find((target) => target.instanceId === targetId);
  const allSources = useMemo(() => {
    if (
      manualSource &&
      !sources.some(
        (source) => source.mainArchivePath === manualSource.mainArchivePath,
      )
    ) {
      return [manualSource, ...sources];
    }
    return sources;
  }, [manualSource, sources]);
  const selectedSource = allSources.find(
    (source) => source.mainArchivePath === sourcePath,
  );
  const targetHasActiveTransfer = transfers.some(
    (transfer) =>
      transfer.instanceId === targetId &&
      ACTIVE_STATUSES.includes(transfer.status),
  );

  useEffect(() => {
    void refreshAll();
    const targetTimer = window.setInterval(() => void refreshTargets(), 2000);
    const transferTimer = window.setInterval(
      () => void refreshTransfers(),
      500,
    );
    return () => {
      window.clearInterval(targetTimer);
      window.clearInterval(transferTimer);
    };
  }, []);

  useEffect(() => {
    if (targetId && !targets.some((target) => target.instanceId === targetId)) {
      setTargetId("");
      setSourcePath("");
    }
  }, [targetId, targets]);

  async function refreshAll() {
    try {
      const [targetResult, sourceResult, transferResult] = await Promise.all([
        api.listArchiveTransferTargets(),
        api.listArchiveTransferSources(),
        api.listArchiveTransfers(),
      ]);
      setTargets(targetResult);
      setSources(sourceResult);
      setTransfers(transferResult);
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function refreshTargets() {
    try {
      setTargets(await api.listArchiveTransferTargets());
    } catch {
      // Preserve the last connection snapshot during transient polling failures.
    }
  }

  async function refreshTransfers() {
    try {
      setTransfers(await api.listArchiveTransfers());
    } catch {
      // Preserve visible progress during transient polling failures.
    }
  }

  async function browseSource() {
    if (!targetId) {
      notify(t("selectTargetFirst"), true);
      return;
    }
    const result = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "ABYA Main.PBArc", extensions: ["PBArc"] }],
    });
    if (typeof result !== "string") return;
    try {
      const source = await api.inspectArchiveTransferSource(result);
      setManualSource(source);
      setSourcePath(source.mainArchivePath);
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  async function startTransfer() {
    if (!targetId) {
      notify(t("selectTargetFirst"), true);
      return;
    }
    if (!sourcePath) {
      notify(t("selectArchiveSource"), true);
      return;
    }
    setBusy(true);
    try {
      await api.startArchiveTransfer(targetId, sourcePath);
      notify(t("transferStarted"));
      await refreshTransfers();
    } catch (error) {
      notify(errorMessage(error), true);
    } finally {
      setBusy(false);
    }
  }

  async function cancelTransfer(id: string) {
    try {
      await api.cancelArchiveTransfer(id);
      await refreshTransfers();
    } catch (error) {
      notify(errorMessage(error), true);
    }
  }

  return (
    <div className="workspace archive-transfer-workspace">
      <header className="page-toolbar">
        <div>
          <h1>{t("archiveTransfer")}</h1>
          <p>{t("compatibleConnectedInstances")}</p>
        </div>
        <button
          className="icon-button"
          onClick={refreshAll}
          title={t("refresh")}
        >
          <RefreshCw size={17} />
        </button>
      </header>

      <div className="transfer-content">
        <section className="transfer-step">
          <div className="step-index">1</div>
          <div className="transfer-step-body">
            <h2>{t("transferTarget")}</h2>
            <select
              value={targetId}
              onChange={(event) => {
                setTargetId(event.target.value);
                setSourcePath("");
              }}
            >
              <option value="">{t("selectInstance")}</option>
              {targets.map((target) => (
                <option key={target.instanceId} value={target.instanceId}>
                  {target.name} ·{" "}
                  {t(
                    target.origin === "managed"
                      ? "managedInstance"
                      : "externalInstance",
                  )}
                </option>
              ))}
            </select>
            {targets.length === 0 && (
              <p className="transfer-empty">{t("noTransferTargets")}</p>
            )}
            {selectedTarget && (
              <div className="transfer-meta">
                <span>{selectedTarget.remoteAddress}</span>
                <span>{selectedTarget.platform || "-"}</span>
                <span>{selectedTarget.gameVersion || "-"}</span>
              </div>
            )}
          </div>
        </section>

        <section className={`transfer-step ${!targetId ? "step-disabled" : ""}`}>
          <div className="step-index">2</div>
          <div className="transfer-step-body">
            <h2>{t("transferSource")}</h2>
            <div className="transfer-source-control">
              <select
                value={sourcePath}
                disabled={!targetId}
                onChange={(event) => setSourcePath(event.target.value)}
              >
                <option value="">{t("automaticArchiveScan")}</option>
                {allSources.map((source) => (
                  <option
                    key={source.mainArchivePath}
                    value={source.mainArchivePath}
                  >
                    {source.archiveName} · {formatBytes(source.uncompressedBytes)}
                  </option>
                ))}
              </select>
              <button
                className="secondary-button"
                onClick={browseSource}
                disabled={!targetId}
              >
                <FileSearch size={16} />
                {t("chooseMainArchive")}
              </button>
            </div>
            {sources.length === 0 && !manualSource && (
              <p className="transfer-empty">{t("noTransferSources")}</p>
            )}
            {selectedSource && (
              <div className="archive-summary">
                <div>
                  <span>{t("archiveGuid")}</span>
                  <strong>{selectedSource.archiveGuid}</strong>
                </div>
                <div>
                  <span>{t("author")}</span>
                  <strong>{selectedSource.author || "-"}</strong>
                </div>
                <div>
                  <span>{t("fileCount")}</span>
                  <strong>{selectedSource.fileCount}</strong>
                </div>
                <div>
                  <span>{t("modifiedAt")}</span>
                  <strong>{formatDate(selectedSource.lastModifiedAt)}</strong>
                </div>
              </div>
            )}
          </div>
        </section>

        <section
          className={`transfer-step ${!targetId || !sourcePath ? "step-disabled" : ""}`}
        >
          <div className="step-index">3</div>
          <div className="transfer-step-body transfer-submit">
            <h2>{t("transferAction")}</h2>
            <button
              className="primary-button"
              disabled={
                !targetId ||
                !sourcePath ||
                targetHasActiveTransfer ||
                busy
              }
              onClick={startTransfer}
            >
              <Send size={16} />
              {t("transferAction")}
            </button>
          </div>
        </section>

        <section className="transfer-history">
          <div className="section-heading">
            <h2>{t("transferHistory")}</h2>
            <span>{transfers.length}</span>
          </div>
          <div className="table-wrap transfer-table">
            <table>
              <thead>
                <tr>
                  <th>{t("archive")}</th>
                  <th>{t("transferTarget")}</th>
                  <th>{t("transferStatus")}</th>
                  <th>{t("transferProgress")}</th>
                  <th>{t("startedAt")}</th>
                  <th aria-label="Actions" />
                </tr>
              </thead>
              <tbody>
                {transfers.map((transfer) => (
                  <tr key={transfer.id}>
                    <td className="strong-cell" title={transfer.archivePath}>
                      {transfer.archiveName}
                    </td>
                    <td>{targetName(targets, transfer.instanceId)}</td>
                    <td>
                      <span
                        className={`state-badge transfer-state-${transfer.status}`}
                        title={transfer.errorMessage || transfer.phase}
                      >
                        {t(statusKey(transfer.status))}
                      </span>
                    </td>
                    <td>
                      <TransferProgress transfer={transfer} />
                    </td>
                    <td>{formatDate(transfer.createdAt)}</td>
                    <td className="row-actions">
                      {ACTIVE_STATUSES.includes(transfer.status) && (
                        <button
                          className="icon-button danger-icon"
                          onClick={() => cancelTransfer(transfer.id)}
                          title={t("cancelTransfer")}
                        >
                          <Square size={14} />
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {transfers.length === 0 && (
              <div className="empty-state large">{t("noTransferHistory")}</div>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}

function TransferProgress({
  transfer,
}: {
  transfer: ArchiveTransferRecord;
}) {
  const percent = Math.max(0, Math.min(100, transfer.progressPercent));
  return (
    <div className="transfer-progress" title={transfer.phase}>
      <div className="progress-track">
        <span style={{ width: `${percent}%` }} />
      </div>
      <small>
        {percent.toFixed(0)}% · {formatBytes(transfer.acknowledgedBytes)}/
        {formatBytes(transfer.packageBytes)}
      </small>
    </div>
  );
}

function statusKey(status: ArchiveTransferStatus): MessageKey {
  const keys: Record<ArchiveTransferStatus, MessageKey> = {
    preparing: "preparingTransfer",
    waitingAcceptance: "waitingAcceptanceTransfer",
    transferring: "transferringTransfer",
    finalizing: "finalizingTransfer",
    completed: "completedTransfer",
    rejected: "rejectedTransfer",
    cancelled: "cancelledTransfer",
    failed: "failedTransfer",
    interrupted: "interruptedTransfer",
  };
  return keys[status];
}

function targetName(targets: ArchiveTransferTarget[], instanceId: string) {
  return (
    targets.find((target) => target.instanceId === instanceId)?.name ??
    instanceId.slice(0, 8)
  );
}

function formatBytes(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const index = Math.min(
    Math.floor(Math.log(value) / Math.log(1024)),
    units.length - 1,
  );
  return `${(value / 1024 ** index).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

function formatDate(value?: string) {
  return value ? new Date(value).toLocaleString() : "-";
}
