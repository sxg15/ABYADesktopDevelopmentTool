import { useEffect, useRef, useState } from "react";
import { productionApi, type AcceptanceSession } from "../../shared/production";
import { errorMessage } from "../../shared/api";

export function AcceptanceLauncher({ taskId, active, en = false, onStarted }: {
  taskId: string; active: boolean; en?: boolean; onStarted?: () => void;
}) {
  const [session, setSession] = useState<AcceptanceSession>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const generation = useRef(0);
  useEffect(() => {
    const current = ++generation.current;
    setSession(undefined); setBusy(false); setError("");
    const read = async () => {
      try { const value = await productionApi.acceptanceStatus(taskId);
        if (generation.current === current) setSession(value);
      } catch { /* A scoped task may have no production record. */ }
    };
    void read();
    const timer = window.setInterval(() => void read(), 2000);
    return () => { generation.current++; window.clearInterval(timer); };
  }, [taskId]);
  async function start() {
    const current = generation.current;
    setBusy(true); setError("");
    try {
      const value = await productionApi.startAcceptance(taskId);
      if (current === generation.current) { setSession(value); onStarted?.(); }
    } catch (reason) {
      if (current === generation.current) setError(errorMessage(reason));
    } finally { if (current === generation.current) setBusy(false); }
  }
  const launching = busy || !!session && ["checking-version", "starting-host", "joining-clients", "checking-gameplay"].includes(session.status);
  return <section className="production-card acceptance-launcher">
    <div className="production-card-heading"><strong>{en ? "Play this version" : "人工试玩"}</strong>
      <button className="primary-button" disabled={!active || launching} onClick={() => void start()}>
        {launching ? (en ? "Opening…" : "正在准备验收…") : (en ? "Start acceptance" : "一键开始验收")}
      </button></div>
    <p>{en ? "Uses the task's saved Player and archive; checks every seat before play." : "按任务登记的运行包和存档启动，检查各端就绪后交给你操作。"}</p>
    {session?.message && <p role="status">{session.version && <strong>{session.version} · </strong>}{session.message}</p>}
    {error && <p role="alert" className="inline-error">{error}</p>}
  </section>;
}
