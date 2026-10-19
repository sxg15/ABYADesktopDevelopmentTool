import { useEffect, useRef, useState } from "react";
import { BookOpen, Images, Palette, Pin, Search } from "lucide-react";
import { productionApi, type TaskSkill, type ProductionView } from "../../shared/production";
import { errorMessage } from "../../shared/api";
import { productionText } from "./productionText";
import { ArtLibraryPanel } from "./ArtLibraryPanel";
import { ReadableText } from "./ReadableText";
import "./production.css";
import "./skillLibrary.css";

export function SkillLibrary({ taskId, active, visible = true, initialSection = "skills", locale, onContinue }: {
  taskId: string; active: boolean; visible?: boolean; initialSection?: "skills" | "cases"; locale?: string;
  onContinue?: (message: string, revision?: number) => Promise<void>;
}) {
  const s = productionText(locale), en = locale === "en-US";
  const t = (zh: string, english: string) => en ? english : zh;
  const [skills, setSkills] = useState<TaskSkill[]>([]), [view, setView] = useState<ProductionView>();
  const [section, setSection] = useState(initialSection), [visitedCases, setVisitedCases] = useState(initialSection === "cases");
  const [query, setQuery] = useState("");
  const provider = "codex";
  const [body, setBody] = useState<{ skill: TaskSkill; text: string }>();
  const [error, setError] = useState(""), [notice, setNotice] = useState("");
  const [loading, setLoading] = useState(true), [reading, setReading] = useState(false), [busy, setBusy] = useState(false);
  const generation = useRef(0), readRequest = useRef(0), actionLock = useRef(false);
  useEffect(() => { setSection(initialSection); if (initialSection === "cases") setVisitedCases(true); }, [initialSection]);
  useEffect(() => {
    const current = ++generation.current; readRequest.current++; actionLock.current = false;
    setSkills([]); setView(undefined); setBody(undefined); setError(""); setNotice(""); setBusy(false); setReading(false); setLoading(true);
    Promise.all([productionApi.skills(taskId), productionApi.get(taskId)]).then(([items, workflow]) => {
      if (generation.current === current) { setSkills(items); setView(workflow); }
    }).catch(e => { if (generation.current === current) setError(errorMessage(e)); })
      .finally(() => { if (generation.current === current) setLoading(false); });
    return () => { generation.current++; readRequest.current++; };
  }, [taskId]);
  async function refreshView() {
    const current = generation.current;
    try { const next = await productionApi.get(taskId); if (current === generation.current) setView(old =>
      (old?.record?.revision ?? -1) > (next.record?.revision ?? -1) ? old : next); }
    catch (e) { if (current === generation.current) setError(errorMessage(e)); }
  }
  useEffect(() => { if (visible && !loading) void refreshView(); }, [visible]);
  async function read(skill: TaskSkill) {
    const current = generation.current, request = ++readRequest.current; setReading(true); setError("");
    try { const text = await productionApi.readSkill(taskId, skill.provider, skill.id);
      if (current === generation.current && request === readRequest.current) setBody({ skill, text }); }
    catch (e) { if (current === generation.current && request === readRequest.current) setError(errorMessage(e)); }
    finally { if (current === generation.current && request === readRequest.current) setReading(false); }
  }
  async function choose(skill: TaskSkill) {
    if (!view?.record || actionLock.current) return;
    const current = generation.current; actionLock.current = true; setBusy(true); setError(""); setNotice("");
    try {
      const next = await productionApi.update(taskId, view.record.revision, "configure", { [skill.kind === "task" ? "taskTemplate" : "artTemplate"]: skill.id });
      if (current !== generation.current) return; setView(next); setNotice(t("模板选择已保存。", "Template selection saved."));
      try { await onContinue?.("用户在 APP 中调整了模板选择。请读取实际选择，修订受影响的待确认文档，再继续当前任务。", next.record?.revision); }
      catch (e) { if (current === generation.current) setNotice(`${t("选择已保存；继续处理需要注意：", "Selection saved; continuation needs attention: ")} ${errorMessage(e)}`); }
    } catch (e) { if (current === generation.current) { setError(errorMessage(e)); await refreshView(); } }
    finally { if (current === generation.current) { actionLock.current = false; setBusy(false); } }
  }
  const items = skills.filter(k => k.provider === provider && `${k.name} ${k.description} ${k.kind}`.toLowerCase().includes(query.toLowerCase()))
    .sort((a, b) => Number(b.kind === "design") - Number(a.kind === "design"));
  const title = (skill: TaskSkill) => skill.kind !== "design" ? skill.name : skill.name === "game-art-design"
    ? t("游戏美术设计", "Game art design") : t("产品界面设计", "Product interface design");
  return <section className="production-panel skill-library-workspace" aria-label={s.skills}>
    <header className="production-heading"><div><h3>{s.skills}</h3><p>{t("技能、模板与美术经验，围绕当前任务一起使用。", "Skills, templates and art experience for this task.")}</p></div><span><Pin size={14} />{s.scope}</span></header>
    <div className="skill-library-sections" role="tablist" aria-label={t("库分类", "Library sections")}>
      <button role="tab" aria-selected={section === "skills"} onClick={() => setSection("skills")}><BookOpen size={16}/>{t("技能与模板", "Skills and templates")}</button>
      <button role="tab" aria-selected={section === "cases"} onClick={() => { setSection("cases"); setVisitedCases(true); void refreshView(); }}><Images size={16}/>{t("美术设计库", "Art design library")}</button>
    </div>
    {error && <div role="alert" className="inline-error">{error}</div>}{notice && <p role="status" className="skill-library-notice">{notice}</p>}
    <div hidden={section !== "skills"}>
      {body ? <div className="skill-reading"><button className="secondary-button" onClick={() => setBody(undefined)}>{s.back}</button><h4>{title(body.skill)}</h4><small>{body.skill.provider} · {body.skill.sha256.slice(0, 12)}</small>
        {body.skill.kind === "design" && body.text.includes("visual-refinement") && <p className="skill-design-note">{t("本版本包含视觉细化与实机审查：用于方向确定后的层级、排版、配色和反馈检查，保留当前需求与风格。", "This version includes visual refinement and runtime review: hierarchy, typography, color and feedback within the current requirements and style.")}</p>}
        <div className="production-document"><ReadableText text={body.text} /></div></div> : <>
        <div className="skill-library-search"><label><Search size={16}/><input aria-label={s.search} placeholder={s.search} value={query} onChange={e => setQuery(e.target.value)}/></label></div>
        {skills.some(k => k.provider === provider && k.kind === "design") && <p className="skill-design-note"><Palette size={17}/>{t("游戏美术与通用界面设计方法随制作流程提供；当前任务保持原有版本。", "Game art and interface design methods are included in the workflow; this task keeps its pinned version.")}</p>}
        {loading && <p role="status">{t("正在读取当前任务的 Skill…", "Loading this task's skills…")}</p>}{reading && <p role="status">{t("正在读取正文…", "Loading Skill content…")}</p>}
        <div className="production-skill-grid">{items.map(skill => <article className={`production-card skill-card ${skill.kind === "design" ? "design-skill" : ""}`} key={skill.path}>
          <span className="skill-kind">{skill.kind === "design" ? t("设计方法 · 随制作流程提供", "Design method · included in workflow") : skill.kind === "art" ? t("美术模板", "Art template") : skill.kind === "task" ? t("任务模板", "Task template") : skill.kind === "workflow" ? t("制作工作流", "Production workflow") : t("用户 Skill", "User Skill")}</span>
          <h4>{title(skill)}</h4><p>{skill.description}</p><small>{skill.name} · {skill.sha256.slice(0, 12)}</small>
          <div className="production-actions"><button className="secondary-button" disabled={reading} onClick={() => void read(skill)}>{s.readSkill}</button>
            {["task", "art"].includes(skill.kind) && <button className="secondary-button" disabled={!active || !view?.record || busy || [view?.record?.taskTemplate, view?.record?.artTemplate].includes(skill.id)} onClick={() => void choose(skill)}>{[view?.record?.taskTemplate, view?.record?.artTemplate].includes(skill.id) ? s.selected : s.select}</button>}</div>
        </article>)}</div>{!loading && !items.length && <p>{s.noSkills}</p>}
      </>}
    </div>
    <div hidden={section !== "cases"}>
      {skills.some(k => k.provider === provider && k.kind === "design") && <div className="skill-design-note"><BookOpen size={16}/><strong>{t("配套设计方法", "Design methods")}</strong>{skills.filter(k => k.provider === provider && k.kind === "design").map(skill => <button key={skill.id} className="secondary-button" onClick={() => { setSection("skills"); void read(skill); }}>{title(skill)}</button>)}</div>}
      {visitedCases && view && <ArtLibraryPanel key={taskId} taskId={taskId} view={view} active={active} en={en} embedded onChanged={refreshView} onContinue={onContinue}/>}</div>
  </section>;
}
