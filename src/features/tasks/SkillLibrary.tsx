import { useEffect, useState } from "react";
import { productionApi, type TaskSkill, type ProductionView } from "../../shared/production";
import { errorMessage } from "../../shared/api";
import { productionText } from "./productionText";
import "./production.css";

export function SkillLibrary({ taskId, active, locale, onContinue }: { taskId: string; active: boolean; locale?: string; onContinue?: (message: string) => Promise<void> }) {
  const s = productionText(locale);
  const [skills, setSkills] = useState<TaskSkill[]>([]);
  const [view, setView] = useState<ProductionView>();
  const [query, setQuery] = useState("");
  const [provider, setProvider] = useState("codex");
  const [body, setBody] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let stopped = false;
    Promise.all([productionApi.skills(taskId), productionApi.get(taskId)]).then(([items, workflow]) => {
      if (!stopped) { setSkills(items); setView(workflow); }
    }).catch(e => { if (!stopped) setError(errorMessage(e)); });
    return () => { stopped = true; };
  }, [taskId]);
  async function read(skill: TaskSkill) {
    try { setBody(await productionApi.readSkill(taskId, skill.provider, skill.id)); }
    catch (e) { setError(errorMessage(e)); }
  }
  async function choose(skill: TaskSkill) {
    if (!view?.record) return;
    setBusy(true); setError("");
    try {
      setView(await productionApi.update(taskId, view.record.revision, "configure", { [skill.kind === "task" ? "taskTemplate" : "artTemplate"]: skill.id }));
      try { await onContinue?.("用户在 APP 中调整了模板选择。请通过 production get 读取实际选择，修订受影响的待确认文档，再继续当前任务。"); } catch { /* Selection is saved even if the terminal is closed. */ }
    }
    catch (e) { setError(errorMessage(e)); setView(await productionApi.get(taskId)); }
    finally { setBusy(false); }
  }
  const items = skills.filter(k => k.provider === provider && `${k.name} ${k.description} ${k.kind}`.toLowerCase().includes(query.toLowerCase()));
  return <div className="production-panel"><header className="production-heading"><h3>{s.skills}</h3><span>{s.scope}</span></header>
    {error && <div role="alert" className="inline-error">{error}</div>}
    {body ? <><button className="secondary-button" onClick={() => setBody("")}>{s.back}</button><pre className="production-document">{body}</pre></> : <>
      <div className="production-actions"><input aria-label={s.search} placeholder={s.search} value={query} onChange={e => setQuery(e.target.value)} />
        <select aria-label={s.source} value={provider} onChange={e => setProvider(e.target.value)}><option value="codex">Codex</option><option value="grok">Grok</option></select></div>
      <div className="production-skill-grid">{items.map(skill => <article className="production-card" key={skill.path}>
        <h4>{skill.name}</h4><p>{skill.description}</p><small>{skill.kind} · {skill.sha256.slice(0, 12)}</small>
        <div className="production-actions"><button className="secondary-button" onClick={() => void read(skill)}>{s.readSkill}</button>
          {["task", "art"].includes(skill.kind) && <button className="secondary-button" disabled={!active || !view?.record || busy || [view?.record?.taskTemplate, view?.record?.artTemplate].includes(skill.id)} onClick={() => void choose(skill)}>
            {[view?.record?.taskTemplate, view?.record?.artTemplate].includes(skill.id) ? s.selected : s.select}</button>}</div>
      </article>)}</div>{!items.length && <p>{s.noSkills}</p>}
    </>}
  </div>;
}
