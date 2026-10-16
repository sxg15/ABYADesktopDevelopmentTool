import { useEffect, useRef, useState } from "react";
import { Modal } from "../../app/Modal";
import { errorMessage } from "../../shared/api";
import { artDirectionFields, type ArtRequirement } from "../../shared/artDesign";
import { productionApi, type ProductionView } from "../../shared/production";
import { ArtifactPreview } from "./ArtifactPreview";
import "./artDesign.css";

export function ArtTracePanel({taskId,view,active,en=false,onUpdated,onContinue}:{
  taskId:string;view:ProductionView;active:boolean;en?:boolean;onUpdated:(v:ProductionView)=>void;
  onContinue?:(message:string,revision:number)=>Promise<void>;
}) {
  const r=view.record!, design=r.artDesign, status=view.artDesignStatus;
  const [note,setNote]=useState(""),[error,setError]=useState(""),[notice,setNotice]=useState(""),[busy,setBusy]=useState(false);
  const [editing,setEditing]=useState<ArtRequirement>(),[reason,setReason]=useState(""),[preview,setPreview]=useState<string>();
  const alive=useRef(true),lock=useRef(false);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  const t=(zh:string,english:string)=>en?english:zh;
  async function write(operation:string,data:unknown,done?:()=>void){
    if(lock.current)return;lock.current=true;setBusy(true);setError("");setNotice("");
    try {const next=await productionApi.feedback(taskId,r.revision,operation,data);
      if(alive.current){onUpdated(next);done?.();setNotice(t("已保存；AI 需要按最新要求核对方案。","Saved; AI must review the updated requirements."));}}
    catch(e){if(alive.current)setError(errorMessage(e));}
    finally {lock.current=false;if(alive.current)setBusy(false);}
  }
  if(!status?.enabled || !design)return <p>{t("本任务保留旧流程。通过“升级流程版本”显式启用要求追踪，已有记录会保留。","This task retains its pinned workflow. Explicitly upgrade to enable requirement tracking.")}</p>;
  const requirements=design.requirements.filter(q=>!q.withdrawn);
  const images=(view.artifacts??r.artifacts??[]).filter(a=>a.mediaType);
  const pending=design.sources.filter(s=>!design.sourceReviews.some(x=>x.id===s.id&&x.sourceHash===s.hash));
  return <div className="art-trace">
    <div className="art-trace-summary"><strong>{t("策划要求与落实位置","Planner requirements and traceability")}</strong>
      <span>{requirements.length} {t("条有效要求","active requirements")} · {pending.length} {t("项输入待核对","inputs awaiting review")}</span></div>
    <p>{t("原话、AI 解读和落实证据分开保留。已关联图片不代表已满足要求，请对照画面审阅。","Original wording, AI interpretation and evidence remain separate. A linked image is not proof that a requirement is satisfied.")}</p>
    {error&&<p role="alert">{error}</p>}{notice&&<p role="status">{notice}</p>}
    {active&&<form className="art-note" onSubmit={e=>{e.preventDefault();const saved=note;void write("add-art-note",{text:saved},()=>setNote(v=>v===saved?"":v));}}>
      <label>{t("补充策划要求","Add planner requirements")}<textarea value={note} maxLength={8000} onChange={e=>setNote(e.target.value)} placeholder={t("例如：不要用文字显示剩余库存，直接显示剩余物品。","For example: show remaining items instead of a stock-count label.")}/></label>
      <button className="secondary-button" disabled={busy||!note.trim()}>{t("保存要求","Save requirements")}</button>
      {onContinue&&<button type="button" className="text-button" disabled={busy} onClick={()=>{setError("");void onContinue("读取最新 production get 中 artDesign 的策划原话、要求、参考分析和未关闭反馈。逐项核对本版设计，保留所有仍有效要求，修订覆盖与还原记录后再提交方案。",r.revision).catch(e=>{if(alive.current)setError(errorMessage(e));});}}>{t("让 AI 核对最新要求","Ask AI to review current requirements")}</button>}
    </form>}
    {!requirements.length&&<p className="production-empty">{t("AI 尚未整理逐条要求。下方保留全部已记录输入，不代表没有要求。","AI has not itemized the requirements. All recorded inputs remain below.")}</p>}
    {requirements.map(q=>{
      const c=design.coverage.find(c=>c.id===q.id),actual=design.realizations.find(x=>x.id===q.id);
      const stale=!design.sources.some(s=>s.id===q.sourceId&&s.hash===q.sourceHash);
      const current=c?.inputHash===status.inputHash&&c.status==="addressed";
      const realized=actual?.designHash===status.designHash&&actual.version===r.currentVersion&&actual.cycle===r.cycle;
      return <article className="art-requirement" key={q.id}>
        <header><strong>{q.id} · {({must:t("必须满足","Required"),preference:t("偏好","Preference"),prohibited:t("禁止","Prohibited"),clarify:t("待澄清","Clarify")})[q.kind]}</strong>
          <span className={stale||!current?"art-unresolved":""}>{stale?t("来源已变更","Source changed"):current?t("已关联设计","Design linked"):t("待落实到当前设计","Current design missing")}</span></header>
        <blockquote>{q.quote||t("图片要求：请对照来源图片","Image requirement: inspect the source image")}</blockquote>
        <p><b>{t("AI 解读：","AI interpretation: ")}</b>{q.interpretation}</p><p>{q.scope}</p><p>{t("验收：","Acceptance: ")}{q.acceptance}</p>
        {c&&<p>{c.designLocation}</p>}
        <div className="art-inline-actions">{c?.artifactIds.map(id=>{const a=images.find(a=>a.id===id);return a&&<button key={id} className="text-button" onClick={()=>setPreview(id)}>{a.title}</button>;})}</div>
        {actual&&<details><summary>{realized?t("实机对照记录","Runtime comparison"):t("旧版实机记录，需复验","Old runtime evidence; recheck required")}</summary><p>{actual.target}</p><p>{actual.comparison}</p><p>{actual.differences}</p></details>}
        <details><summary>{t("查看完整原话与来源","View original source")}</summary><small>{q.sourceId}</small><p className="art-source-text">{q.sourceText}</p></details>
        {active&&<button className="text-button" onClick={()=>{setEditing({...q});setReason("");}}>{t("修订解读或撤回要求","Revise or withdraw")}</button>}
      </article>;
    })}
    <details className="art-source-list"><summary>{t("全部输入与 AI 核对记录","All inputs and AI review notes")} · {design.sources.length}</summary>
      {design.sources.map(s=>{const review=design.sourceReviews.find(x=>x.id===s.id&&x.sourceHash===s.hash);return <article key={s.id}>
        <strong>{s.asset?.title??s.id}</strong>{s.kind==="agent-transcribed-conversation"&&<small>{t("AI 摘录，请核对原会话","AI transcript excerpt; verify against the conversation")}</small>}<p className="art-source-text">{s.text||t("图片参考","Image reference")}</p>
        {s.asset&&<button className="text-button" onClick={()=>setPreview(s.asset!.id)}>{t("查看来源图片","View source image")}</button>}
        <p className={!review?"art-unresolved":""}>{review?review.reason:t("AI 尚未核对本版输入","AI has not reviewed this input version")}</p>
        {review&&<small>{review.requirementIds.join(" · ")||t("AI 判断不涉及美术要求，请核对其理由。","AI classified this as unrelated to art; review its reason.")}</small>}
      </article>;})}
    </details>
    {design.referenceAnalyses.length>0&&<details><summary>{t("参考解析与取舍","Reference analysis")}</summary>
      {design.referenceAnalyses.map(a=><article className="art-reference-analysis" key={a.id}>
        <button className="text-button" onClick={()=>setPreview(a.id)}>{images.find(x=>x.id===a.id)?.title??a.id}</button>
        <p>{a.observations}</p><p>{t("借鉴：","Borrow: ")}{a.borrow}</p><p>{t("应用：","Application: ")}{a.application}</p>
        <p>{t("不采用：","Avoid: ")}{a.avoid}</p><p>{t("冲突与取舍：","Tradeoffs: ")}{a.conflicts}</p>
      </article>)}
    </details>}
    {design.direction&&<details><summary>{t("当前设计依据与制作规格","Design rationale and production specification")}</summary>
      {artDirectionFields.map(([key,zh,english])=><div key={key}><h5>{t(zh,english)}</h5><p className="art-source-text">{design.direction?.[key]}</p></div>)}
    </details>}
    {design.caseUses.length>0&&<details><summary>{t("案例借鉴与适配","Case adaptations")}</summary>{design.caseUses.map(c=><article key={c.id}>
      <strong>{c.title} · v{c.version}</strong><p>{c.borrow}</p><p>{c.adapt}</p><p>{c.avoid}</p>
    </article>)}</details>}
    {design.requirements.some(q=>q.withdrawn)&&<details><summary>{t("已撤回要求（保留历史）","Withdrawn requirements")}</summary>
      {design.requirements.filter(q=>q.withdrawn).map(q=><p key={q.id}>{q.id} · {q.quote} — {q.withdrawReason}</p>)}
    </details>}
    {preview&&images.some(a=>a.id===preview)&&<ArtifactPreview taskId={taskId} items={images} initialId={preview} en={en} onClose={()=>setPreview(undefined)}/>}
    {editing&&<Modal title={t("明确修订这条要求","Revise this requirement")} onClose={()=>{if(!busy)setEditing(undefined);}}>
      <form className="art-edit-form" onSubmit={e=>{e.preventDefault();const source=design.sources.find(s=>s.id===editing.sourceId);void write("save-art-requirement",{...editing,sourceHash:source?.hash},()=>setEditing(undefined));}}>
        <label>{t("来源","Source")}<select value={editing.sourceId} onChange={e=>setEditing({...editing,sourceId:e.target.value})}>
          {!design.sources.some(s=>s.id===editing.sourceId)&&<option value={editing.sourceId}>{t("原来源已撤回，请选择当前来源","Original source withdrawn; choose current source")}</option>}
          {design.sources.map(s=><option value={s.id} key={s.id}>{s.asset?.title??s.id}</option>)}
        </select></label>
        <label>{t("策划原句（须来自所选来源）","Original quote from selected source")}<textarea value={editing.quote} onChange={e=>setEditing({...editing,quote:e.target.value})}/></label>
        <label>{t("要求性质","Requirement type")}<select value={editing.kind} onChange={e=>setEditing({...editing,kind:e.target.value as ArtRequirement["kind"]})}>
          <option value="must">{t("必须满足","Required")}</option><option value="preference">{t("偏好","Preference")}</option>
          <option value="prohibited">{t("禁止项","Prohibited")}</option><option value="clarify">{t("待澄清","Clarify")}</option>
        </select></label>
        {([['interpretation','具体解读','Interpretation'],['scope','影响范围','Scope'],['acceptance','验收条件','Acceptance']] as const).map(([key,zh,english])=><label key={key}>{t(zh,english)}
          <textarea value={editing[key]} maxLength={4000} required onChange={e=>setEditing({...editing,[key]:e.target.value})}/></label>)}
        <button className="primary-button" disabled={busy}>{t("保存明确修订","Save revision")}</button>
      </form>
      <div className="art-withdraw"><label>{t("撤回原因","Withdrawal reason")}<input value={reason} maxLength={2000} onChange={e=>setReason(e.target.value)}/></label>
        <button className="secondary-button" disabled={busy||!reason.trim()} onClick={()=>void write("withdraw-art-requirement",{id:editing.id,reason},()=>setEditing(undefined))}>{t("撤回这条要求并保留历史","Withdraw and retain history")}</button></div>
      {error&&<p role="alert">{error}</p>}
    </Modal>}
  </div>;
}
