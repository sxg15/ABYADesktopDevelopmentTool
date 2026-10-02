import { useEffect, useRef, useState } from "react";
import { productionApi, type ProductionView, type ProductionDocument } from "../../shared/production";
import { errorMessage } from "../../shared/api";
import { productionText, stageName, statusName } from "./productionText";
import "./production.css";

export function ProductionPanel({ taskId, active, locale, onContinue }: {
  taskId: string; active: boolean; locale?: string; onContinue?: (message: string) => Promise<void>;
}) {
  const s = productionText(locale);
  const [view, setView] = useState<ProductionView>();
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [question, setQuestion] = useState("ask");
  const [feedback, setFeedback] = useState("");
  const sequence = useRef(0);
  const alive = useRef(true);
  const mutating = useRef(false);
  async function refresh() {
    if (mutating.current) return;
    const request = ++sequence.current;
    try { const result = await productionApi.get(taskId); if (alive.current && request === sequence.current) setView(result); }
    catch (e) { if (alive.current && request === sequence.current) setError(errorMessage(e)); }
  }
  useEffect(() => {
    alive.current = true; void refresh();
    const timer = window.setInterval(() => void refresh(), 4000);
    return () => { alive.current = false; sequence.current++; window.clearInterval(timer); };
  }, [taskId]);
  async function change(run: () => Promise<ProductionView>) {
    setBusy(true); mutating.current = true; setError(""); sequence.current++;
    try { const result = await run(); sequence.current++; if (alive.current) setView(result); }
    catch (e) { if (alive.current) { setError(errorMessage(e)); } }
    finally { mutating.current = false; if (alive.current) { setBusy(false); void refresh(); } }
  }
  const r = view?.record?.taskId === taskId ? view.record : undefined;
  async function decide(d: ProductionDocument, accepted: boolean) {
    if (!r) return;
    await change(async () => {
      const result = await productionApi.decide(taskId, r.revision, d.kind, d.sha256, accepted, feedback);
      setFeedback(""); setNotice(s.continueHint);
      try { await onContinue?.(`桌面制作流程记录已更新：${d.kind} 文档第 ${d.revision} 版已${accepted ? "确认" : "退回"}。请通过 production get 读取实际状态和反馈，再继续当前任务。`); }
      catch { /* Approval remains saved if the terminal is not open. */ }
      return result;
    });
  }
  const open = async (path: string) => { try { await productionApi.open(taskId, path); } catch (e) { setError(errorMessage(e)); } };
  return <div className="production-panel">
    <header className="production-heading"><h3>{s.title}</h3><button className="secondary-button" onClick={() => void refresh()} disabled={busy}>{s.refresh}</button></header>
    {error && <div role="alert" className="inline-error">{error}</div>}
    {notice && <p role="status">{notice}</p>}
    {!view ? <p>{s.loading}</p> : !r ? <section className="production-card">
      <h4>{s.noRecord}</h4><p>{s.enableHint}</p>
      <label className="field">{s.questionMode}<select value={question} onChange={e => setQuestion(e.target.value)}><option value="ask">{s.ask}</option><option value="no-followup">{s.noAsk}</option></select></label>
      <button className="primary-button" disabled={busy || !active} onClick={() => void change(() => productionApi.update(taskId, 0, "initialize", { questionMode: question }))}>{s.enable}</button>
    </section> : <>
      <p className="production-meta">{s.version} {r.workflowVersion} · {s.revision} {r.revision} · {s.cycle} {r.cycle} · {s.round} R{r.currentRound}</p>
      {view.availableUpdate && <div className="production-warning">{s.newVersion} <button disabled={busy || !active} className="text-button" onClick={() => { if (window.confirm(s.upgradeConfirm)) void change(() => productionApi.upgrade(taskId, r.revision)); }}>{s.upgrade}</button></div>}
      {view.warnings.map(w => <div className="production-warning" key={w}>{w}</div>)}
      <ol className="production-stages" aria-label={s.method}>{view.policy.stages.map(stage => <li key={stage.id} className={r.currentStage === stage.id ? "current" : ""}>
        <strong>{stageName(stage.id, stage.name, locale)}</strong><span>{statusName(r.stages[stage.id], locale)}</span>
      </li>)}</ol>
      {Object.values(r.documents).filter(d => ["requirements", "plan", "delivery"].includes(d.kind)).map(d => {
        const decision = [...r.approvals].reverse().find(a => a.kind === d.kind && a.documentRevision === d.revision);
        const pending = r.stages[d.kind] === "awaiting-confirmation";
        return <section className="production-card" key={d.kind}>
          <h4>{stageName(d.kind, view.policy.stages.find(x => x.id === d.kind)?.name ?? d.kind, locale)} · v{d.revision} · {decision ? statusName(decision.decision, locale) : s.document}</h4>
          <details open={pending}><summary>{s.details}</summary><pre className="production-document">{d.content}</pre><small>{d.submittedAt}</small></details>
          {pending && <><label className="field">{s.feedback}<textarea value={feedback} onChange={e => setFeedback(e.target.value)} rows={3} /></label>
            <div className="production-actions"><button className="primary-button" disabled={busy || !active || view.warnings.some(w => w.startsWith(d.kind))} onClick={() => void decide(d, true)}>{s.confirm}</button>
            <button className="secondary-button" disabled={busy || !active || !feedback.trim()} onClick={() => void decide(d, false)}>{s.reject}</button></div></>}
        </section>;
      })}
      <section className="production-card"><h4>{s.reports}</h4><div className="production-actions">{[["plan", s.planReport], ["review", s.reviewReport], ["closeout", s.closeoutReport]].map(([phase, label]) =>
        <button className="secondary-button" key={phase} onClick={() => void open(`artifacts/game-development/reports/${phase}-report.html`)}>{label}</button>)}</div></section>
      <section className="production-card"><h4>{s.issues}</h4>{!r.issues.length ? <p>{s.noIssues}</p> : r.issues.map(i =>
        <article className="production-issue" key={i.id}><strong>{i.id} · {statusName(i.status, locale)}</strong><p>{i.description}</p>
          {i.resumeWhen && <p>{i.resumeWhen}</p>}{i.fix && <p>{i.fix}</p>}{i.recheck && <p>{i.recheck}</p>}</article>)}</section>
      <section className="production-card"><h4>{s.rounds}</h4>{!r.rounds.length && <p>{s.noRounds}</p>}{r.rounds.map(round =>
        <details key={`${round.cycle}-${round.number}`}><summary>{s.cycle} {round.cycle} · R{round.number} · {statusName(round.status, locale)}</summary>
          <p>{round.inputVersion ?? s.none} → {round.outputVersion ?? s.none}</p>
          <ul>{round.checks?.map(c => <li key={c.dimensionId}>{view.policy.dimensions.find(d => d.id === c.dimensionId)?.name ?? c.dimensionId} · {statusName(c.status, locale)} · {c.observations}</li>)}</ul>
          {round.questions?.map(q => <div className="production-answer" key={q.questionId}><strong>{q.questionId} · {view.policy.questions.find(x => x.id === q.questionId)?.text}</strong>
            <p>{q.conclusion}</p><p>{q.counterexample}</p><p>{q.problem}</p><p>{q.change}</p><p>{q.recheck}</p></div>)}
        </details>)}</section>
      <section className="production-card"><h4>{s.evidence}</h4>{!r.evidence.length && <p>{s.noEvidence}</p>}{r.evidence.map(e =>
        <div className="production-evidence" key={e.id}><span><strong>{e.id}</strong> · {e.description}<small>{e.version} · {e.captureType} · {e.reviewed ? s.verified : s.unreviewed}</small></span>
          <button className="text-button" onClick={() => void open(e.path)}>{s.open}</button></div>)}</section>
      <section className="production-card"><h4>{s.knowledge}</h4>{r.knowledge.map((k, i) => <p key={i}>{k.status} · {k.summary}</p>)}</section>
      <details className="production-card"><summary>{s.acceptance}</summary>{r.approvals.map((a, i) => <p key={i}>{a.kind} v{a.documentRevision} · {statusName(a.decision, locale)} · {a.decidedAt} · {a.feedback}</p>)}</details>
      <p className="production-meta">{s.limit}</p>
    </>}
  </div>;
}
