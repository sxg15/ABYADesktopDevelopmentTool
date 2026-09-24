import { useEffect, useState } from "react";
import { Eye, EyeOff, Power, Radio, RefreshCw, Trash2 } from "lucide-react";
import { Modal } from "../../app/Modal";
import { api, errorMessage } from "../../shared/api";
import type {
  GameInstance,
  InstanceRuntimeInfo,
  RuntimeBridgeState,
} from "../../shared/types";
import type { MessageKey } from "../../i18n";

export function InstanceDetailsModal({
  instance,
  t,
  onClose,
  onChanged,
  onDeleted,
}: {
  instance: GameInstance;
  t: (key: MessageKey) => string;
  onClose: () => void;
  onChanged: () => void;
  onDeleted: () => void;
}) {
  const [info, setInfo] = useState<InstanceRuntimeInfo>();
  const [runtime, setRuntime] = useState<RuntimeBridgeState>();
  const [error, setError] = useState("");
  const current = info?.instance ?? instance;
  const running = current.processState === "running";
  const active =
    ["launching", "running", "stopping"].includes(current.processState) ||
    current.connectionState === "connected";

  useEffect(() => {
    void refresh();
  }, [instance.id]);

  async function refresh() {
    setError("");
    try {
      const detail = await api.getInstance(instance.id);
      setInfo(detail);
      setRuntime(await api.runtimeState(instance.id));
    } catch (value) {
      setError(errorMessage(value));
    }
  }

  async function stop() {
    try {
      await api.stopInstance(instance.id);
      await refresh();
      onChanged();
    } catch (value) {
      setError(errorMessage(value));
    }
  }

  async function toggleWindow() {
    try {
      await api.setInstanceWindowVisibility(
        instance.id,
        current.profile?.visibilityMode === "background"
          ? "visible"
          : "background",
      );
      await refresh();
      onChanged();
    } catch (value) {
      setError(errorMessage(value));
    }
  }

  async function remove() {
    if (!window.confirm(t("deleteInstanceConfirm"))) return;
    try {
      await api.deleteInstance(instance.id);
      onDeleted();
    } catch (value) {
      setError(errorMessage(value));
    }
  }

  return (
    <Modal title={instance.name} onClose={onClose} wide>
      <div className="detail-grid">
        <Detail
          label={t("processState")}
          value={t(current.processState)}
        />
        <Detail
          label={t("connectionState")}
          value={t(current.connectionState)}
        />
        <Detail label={t("pid")} value={String(info?.instance.pid ?? "-")} />
        <Detail label={t("startedAt")} value={formatDate(instance.startedAt)} />
        <Detail
          label={t("launchMode")}
          value={current.mode ? t(current.mode) : "-"}
        />
        <Detail
          label={t("windowVisibility")}
          value={t(current.profile?.visibilityMode ?? "visible")}
        />
        <Detail label={t("gatewayEndpoint")} value={info?.gatewayEndpoint ?? "-"} wide />
        <Detail label={t("remoteAddress")} value={current.remoteAddress ?? "-"} wide />
        <Detail label={t("reportPath")} value={info?.reportPath ?? "-"} wide />
      </div>

      <div className="section-heading">
        <h3>{t("runtimeCli")}</h3>
        <button className="icon-button" onClick={refresh} title={t("refresh")}>
          <RefreshCw size={16} />
        </button>
      </div>
      <div className="runtime-status">
        <Radio size={16} />
        <span className={runtime?.connected ? "status-ok" : "status-muted"}>
          {runtime?.connected ? t("connected") : t("disconnected")}
        </span>
        <code>
          {runtime?.cliAvailable
            ? `${runtime.gameVersion} · ${runtime.platform}`
            : runtime?.lastError || ""}
        </code>
      </div>

      <h3 className="compact-heading">{t("arguments")}</h3>
      <div className="argument-list">
        {instance.sanitizedArgs.map((argument, index) => (
          <code key={`${argument}-${index}`}>{argument}</code>
        ))}
      </div>
      {error && <div className="inline-error">{error}</div>}
      <footer className="modal-actions">
        {running && current.origin === "managed" && (
          <>
            <button className="secondary-button" onClick={toggleWindow}>
              {current.profile?.visibilityMode === "background" ? (
                <Eye size={16} />
              ) : (
                <EyeOff size={16} />
              )}
              {current.profile?.visibilityMode === "background"
                ? t("showInstanceWindow")
                : t("hideInstanceWindow")}
            </button>
            <button className="danger-button" onClick={stop}>
              <Power size={16} />
              {t("stop")}
            </button>
          </>
        )}
        <button className="danger-button" disabled={active} onClick={remove}>
          <Trash2 size={16} />
          {t("deleteInstance")}
        </button>
        <button className="secondary-button" onClick={onClose}>
          {t("close")}
        </button>
      </footer>
    </Modal>
  );
}

function Detail({
  label,
  value,
  wide = false,
}: {
  label: string;
  value: string;
  wide?: boolean;
}) {
  return (
    <div className={wide ? "detail-item detail-wide" : "detail-item"}>
      <span>{label}</span>
      <strong title={value}>{value}</strong>
    </div>
  );
}

function formatDate(value?: string) {
  return value ? new Date(value).toLocaleString() : "-";
}
