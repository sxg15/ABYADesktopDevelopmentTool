import { useEffect, useState } from "react";
import { clientApi, bytes, clientError, type ClientOperation } from "../../shared/gameClients";
import type { MessageKey } from "../../i18n";

export function useClientOperation() {
  const [operation, setOperation] = useState<ClientOperation>();
  useEffect(() => {
    let alive = true;
    let reading = false;
    const refresh = async () => {
      if (reading) return;
      reading = true;
      try { const value = await clientApi.operation(); if (alive) setOperation(value); } catch { /* The owning view shows connection errors. */ }
      finally { reading = false; }
    };
    void refresh(); const timer = window.setInterval(() => void refresh(), 750);
    return () => { alive = false; window.clearInterval(timer); };
  }, []);
  return operation;
}

export function ClientProgress({ operation, t, onError }: {
  operation?: ClientOperation; t: (key: MessageKey) => string; onError: (message: string) => void;
}) {
  const [cancelling, setCancelling] = useState(false);
  useEffect(() => { if (!operation?.active) setCancelling(false); }, [operation?.active]);
  if (!operation?.active || ["cleanup", "select"].includes(operation.kind)) return null;
  const keys: Record<string, MessageKey> = { connecting: "gcConnecting", downloading: "gcDownloading", uploading: "gcUploading", extracting: "gcExtracting", complete: "gcComplete" };
  const percent = operation.totalBytes ? Math.min(100, Math.floor(operation.completedBytes / operation.totalBytes * 100)) : 0;
  return <div className="client-progress" role="status">
    <div className="client-row"><strong>{t(keys[operation.stage] ?? "gcConnecting")}</strong>
      <span>{operation.totalBytes > 0 && `${bytes(operation.completedBytes)} / ${bytes(operation.totalBytes)} · ${percent}%`}</span>
      {["install", "upload"].includes(operation.kind) && <button className="secondary-button" disabled={cancelling} onClick={() => {
        setCancelling(true); void clientApi.cancel().catch(error => { setCancelling(false); onError(clientError(error, t)); });
      }}>{t(cancelling ? "gcCancelPending" : "gcCancelOperation")}</button>}
    </div>
    <progress max={100} value={operation.totalBytes ? percent : undefined} aria-label={t(keys[operation.stage] ?? "gcConnecting")} />
  </div>;
}
