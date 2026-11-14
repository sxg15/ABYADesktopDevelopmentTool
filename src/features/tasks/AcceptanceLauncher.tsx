import { useEffect, useRef, useState, type ReactNode } from "react";
import { productionApi, type AcceptanceSession, type ProductionRecord } from "../../shared/production";
import { errorMessage } from "../../shared/api";
import {PlaytestFolder} from "./PlaytestFolder";
import { PlaytestSeats } from "./PlaytestSeats";

export function AcceptanceLauncher({ taskId, active, visible = true, en = false, onStarted, versionLabel,archiveVersion }: {
  taskId: string; active: boolean; visible?: boolean; en?: boolean; onStarted?: () => void; versionLabel?:ReactNode; archiveVersion?:string;
}) {
  const [session, setSession] = useState<AcceptanceSession>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [record, setRecord] = useState<ProductionRecord>();
  const [seats, setSeats] = useState<number>();
  const generation = useRef(0);
  const readSequence = useRef(0), reading = useRef(false), starting = useRef(false);
  useEffect(() => {
    generation.current++; starting.current=false;
    setSession(undefined); setBusy(false); setError(""); setRecord(undefined); setSeats(undefined);
    return () => { generation.current++; };
  }, [taskId]);
  useEffect(() => {
    if (!visible) return;
    let disposed = false;
    async function readMode() {
      try {
        const view = await productionApi.get(taskId);
        if (!disposed) setRecord(view.record);
      } catch { /* The backend still validates the task before launching. */ }
    }
    void readMode();
    return () => { disposed = true; };
  }, [taskId, visible]);
  useEffect(() => {
    if (!visible) return;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const schedule = () => { if (!disposed) timer=setTimeout(()=>void read(),2000); };
    const read = async () => {
      if (reading.current) { schedule(); return; }
      reading.current=true;
      const request=++readSequence.current;
      try { const value = await productionApi.acceptanceStatus(taskId);
        if (!disposed && request === readSequence.current) setSession(value);
      } catch { /* A scoped task may have no production record. */ }
      finally { reading.current=false; schedule(); }
    };
    void read();
    return () => { disposed=true; readSequence.current++; if(timer!==undefined)clearTimeout(timer); };
  }, [taskId,visible]);
  async function start() {
    if (!active || !visible || starting.current || launching) return;
    starting.current=true; readSequence.current++;
    const current = generation.current;
    setBusy(true); setError("");
    try {
      const value = await productionApi.startAcceptance(taskId, record?.playerMode === "multiplayer" ? selectedSeats : undefined);
      if (current === generation.current) { readSequence.current++; setSession(value); onStarted?.(); }
    } catch (reason) {
      if (current === generation.current) setError(errorMessage(reason));
    } finally { if (current === generation.current) { starting.current=false; setBusy(false); } }
  }
  const launching = busy || !!session && ["checking-version", "starting-host", "joining-clients", "checking-gameplay"].includes(session.status);
  const selectedSeats = seats ?? record?.acceptanceConfig?.seats ?? 2;
  return <section className="production-card acceptance-launcher">
    <div className="production-card-heading"><strong>{en ? "Play this version" : "人工试玩"}</strong>
      <button className="primary-button" disabled={!active || launching} onClick={() => void start()}>
        {launching ? (en ? "Opening…" : "正在准备验收…") : (en ? "Start acceptance" : "一键开始验收")}
      </button></div>
    {versionLabel}
    <PlaytestFolder taskId={taskId} version={archiveVersion??record?.currentVersion} en={en}/>
    {record?.playerMode === "multiplayer" && <PlaytestSeats value={selectedSeats} onChange={setSeats} disabled={!active || launching} en={en}/>}
    <p>{en ? "Uses the task's saved Player and archive; checks every seat before play." : "按任务登记的运行包和存档启动，检查各端就绪后交给你操作。"}</p>
    {session?.message && <p role="status">{session.version && <strong>{session.version} · </strong>}{session.message}</p>}
    {error && <p role="alert" className="inline-error">{error}</p>}
  </section>;
}
