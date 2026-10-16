import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Modal } from "../../app/Modal";
import { errorMessage } from "../../shared/api";
import { artCaseFields, artLibraryApi, type ArtCase } from "../../shared/artDesign";
import { productionApi, type ProductionView } from "../../shared/production";
import { ArtifactMedia } from "./ArtifactPreview";
import "./artDesign.css";

function CaseMedia({entry,index,en=false}:{entry:ArtCase;index:number;en?:boolean}) {
  const [src,setSrc]=useState(""),[error,setError]=useState("");
  useEffect(()=>{let alive=true;setSrc("");setError("");void artLibraryApi.media(entry.id,entry.version!,index)
    .then(s=>{if(alive)setSrc(s);}).catch(e=>{if(alive)setError(errorMessage(e));});return()=>{alive=false;};},[entry.id,entry.version,index]);
  const media=entry.media![index];
  return <figure>{error?<p role="alert">{error}</p>:src?(media.mediaType.startsWith("video/")?<video src={src} controls/>:<img src={src} alt={media.title}/>):<span>…</span>}<figcaption>{media.sourceType==="reference"?(en?"Original reference · ":"原始参考 · "):""}{media.title}</figcaption></figure>;
}

export function ArtLibraryPanel({taskId,view,active,en=false,onClose,onContinue}:{
  taskId:string;view:ProductionView;active:boolean;en?:boolean;onClose:()=>void;
  onContinue?:(message:string,revision:number)=>Promise<void>;
}) {
  const [entries,setEntries]=useState<ArtCase[]>([]),[selected,setSelected]=useState<ArtCase>();
  const [query,setQuery]=useState(""),[history,setHistory]=useState(false),[drafts,setDrafts]=useState(false);
  const [busy,setBusy]=useState(false),[error,setError]=useState(""),[notice,setNotice]=useState("");
  const [satisfaction,setSatisfaction]=useState(""),[withdrawReason,setWithdrawReason]=useState(""),[withdrawing,setWithdrawing]=useState(false);
  const alive=useRef(true),sequence=useRef(0),lock=useRef(false);
  const r=view.record!, t=(zh:string,english:string)=>en?english:zh;
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;sequence.current++;};},[]);
  async function search(includeHistory=history) {
    const serial=++sequence.current;setError("");
    try {const result=await artLibraryApi.list(query,includeHistory);if(alive.current&&serial===sequence.current)setEntries(result);}
    catch(e){if(alive.current&&serial===sequence.current)setError(errorMessage(e));}
  }
  useEffect(()=>{void search();},[]);
  function select(entry:ArtCase){setSelected(entry);setSatisfaction("");setWithdrawing(false);setWithdrawReason("");setError("");setNotice("");}
  async function act(operation:string,data:unknown) {
    if(lock.current)return;lock.current=true;setBusy(true);setError("");setNotice("");
    try {const result=await artLibraryApi.action(taskId,r.revision,operation,data);
      if(!alive.current)return;
      if(operation==="export"&&result.path) {setNotice(t("案例包已导出。","Case package exported."));await productionApi.revealDocument(taskId,result.path);}
      else {select(result);setDrafts(false);await search();setNotice(t("已保存案例库变更。","Library changes saved."));}}
    catch(e){if(alive.current)setError(errorMessage(e));}
    finally{lock.current=false;if(alive.current)setBusy(false);}
  }
  async function request(message:string) {
    if(!onContinue||lock.current)return;lock.current=true;setBusy(true);setError("");
    try{await onContinue(message,r.revision);if(alive.current)setNotice(t("已请求 AI 处理，请关注任务进展。","AI request sent; follow task progress."));}
    catch(e){if(alive.current)setError(errorMessage(e));}
    finally{lock.current=false;if(alive.current)setBusy(false);}
  }
  const shown=drafts?r.artDesign?.caseDrafts??[]:entries;
  const isDraft=selected&&!selected.version;
  return <Modal wide title={t("美术设计库","Art design library")} onClose={()=>{if(!busy)onClose();}}>
    <p>{t("保存策划满意的具体设计及其上下文。检索时先看玩法、平台与适用条件，再看风格。","Curated designs with their context. Compare gameplay, platform and applicability before style.")}</p>
    <div className="art-library-toolbar">
      <button className={drafts?"secondary-button":"primary-button"} onClick={()=>{setDrafts(false);setSelected(undefined);}}>{t("已收录案例","Saved cases")}</button>
      <button className={drafts?"primary-button":"secondary-button"} onClick={()=>{setDrafts(true);setSelected(undefined);}}>{t("待审阅草稿","Drafts to review")} · {r.artDesign?.caseDrafts.length??0}</button>
      <button className="text-button" disabled={busy} onClick={()=>{void open({multiple:false,filters:[{name:"Art case",extensions:["zip"]}]}).then(path=>{if(typeof path==="string"&&alive.current)void act("import",{path});}).catch(e=>{if(alive.current)setError(errorMessage(e));});}}>{t("导入案例包","Import case package")}</button>
    </div>
    {!drafts&&<form className="art-library-search" onSubmit={e=>{e.preventDefault();void search();}}>
      <label>{t("按玩法、镜头、平台或风格检索","Search gameplay, camera, platform or style")}<input value={query} maxLength={200} onChange={e=>setQuery(e.target.value)}/></label>
      <button className="secondary-button" type="submit">{t("检索","Search")}</button>
      <label className="art-check"><input type="checkbox" checked={history} onChange={e=>{setHistory(e.target.checked);void search(e.target.checked);}}/>{t("包含旧版与已撤回案例","Include history and withdrawn cases")}</label>
    </form>}
    {error&&<p role="alert">{error}</p>}{notice&&<p role="status">{notice}</p>}
    <div className="art-library-layout"><div className="art-library-list">
      {!shown.length&&<p>{drafts?t("尚无案例草稿。满意后可让 AI 整理，收录前会展示具体内容。","No draft yet. Ask AI to prepare one after a satisfactory result."):t("没有匹配案例，可继续按当前需求设计。","No matching cases; design from the current requirements.")}</p>}
      {shown.map(c=><button className="art-case-row" aria-pressed={selected?.id===c.id&&selected?.version===c.version} key={`${c.id}:${c.version??"draft"}`} onClick={()=>select(c)}>
        <strong>{c.title}</strong><span>{c.version?`v${c.version}`:t("待审阅","Draft")} · {c.status==="withdrawn"?t("已撤回","Withdrawn"):c.provenance==="imported"?t("导入案例","Imported"):c.runtimeVerified?t("含实机验证","Runtime evidence"):t("设计参考","Design reference")}</span>
        <small>{c.tags.join(" · ")}</small></button>)}
      {drafts&&active&&onContinue&&view.artDesignStatus?.enabled&&<button className="secondary-button" disabled={busy} onClick={()=>void request("请读取当前美术方案与策划反馈，使用 save-art-case-draft 整理值得复用的具体成果及上下文，明确范围、适用条件、制作方法和限制。草稿不是认可或入库，不得代替策划声明满意。")}>{t("让 AI 整理案例草稿","Ask AI to prepare a case")}</button>}
    </div><div className="art-case-detail">
      {selected?<>
        <h3>{selected.title}</h3>
        <div className="art-case-media">{isDraft?[...(selected.artifactIds??[]),...(selected.referenceIds??[])].map(id=>{const a=(view.artifacts??r.artifacts??[]).find(a=>a.id===id);return a&&<figure key={id}><ArtifactMedia taskId={taskId} artifact={a} en={en}/><figcaption>{a.sourceType==="reference"?t("原始参考 · ","Original reference · "):""}{a.title}</figcaption></figure>;}):selected.media?.map((_,i)=><CaseMedia key={`${selected.id}:${selected.version}:${i}`} entry={selected} index={i} en={en}/>)}</div>
        {artCaseFields.map(([key,zh,english])=><section key={key}><h4>{t(zh,english)}</h4><p className="art-source-text">{selected[key]}</p></section>)}
        {selected.satisfaction&&<section><h4>{t("策划满意的具体范围","Planner satisfaction scope")}</h4><p>{selected.satisfaction}</p></section>}
        {!isDraft&&<p>{selected.provenance==="imported"?t("导入包保留来源说明，其认可和验证声明不代表本机策划已复验。","Imported approval and verification claims are not local revalidation."):selected.runtimeVerified?t("包含收录时对应作品版本的视觉验证；用于新项目仍需重新验证。","Contains visual evidence for the collected game version; new projects require new validation."):t("已认可设计方向，未标记为已完成实机验证。","Approved design direction; not marked as runtime verified.")}</p>}
        {isDraft?<form className="art-collect-form" onSubmit={e=>{e.preventDefault();void act("collect",{draftId:selected.id,satisfaction});}}>
          <label>{t("我满意的具体部分与原因","What I am satisfied with and why")}<textarea value={satisfaction} maxLength={4000} required onChange={e=>setSatisfaction(e.target.value)}/></label>
          <p>{t("仅收录这里展示的成果和上下文，不会批准整个任务。","Collects only the displayed work and context; does not approve the whole task.")}</p>
          <button className="primary-button" disabled={busy||!satisfaction.trim()||selected.designHash!==view.artDesignStatus?.designHash}>{t("满意并收录此案例","Satisfied — collect this case")}</button>
          {selected.designHash!==view.artDesignStatus?.designHash&&<p role="status">{t("草稿对应的要求或设计已变化，请让 AI 更新。","Requirements or design changed; ask AI to update the draft.")}</p>}
        </form>:<div className="art-inline-actions">
          {active&&onContinue&&view.artDesignStatus?.enabled&&selected.status==="accepted"&&<button className="primary-button" disabled={busy} onClick={()=>void request(`策划选择参考美术案例 ${selected.id} 第 ${selected.version} 版。先通过 art get 阅读上下文、inspect-art-case 获取候选媒体并实际查看，再判断适用性；决定借鉴后用 use-art-case 记录 borrow/adapt/avoid。当前策划要求优先，不直接套用布局，不把旧案例验证当作本任务验收。`)}>{t("让 AI 评估并参考","Ask AI to evaluate and reference")}</button>}
          <button className="secondary-button" disabled={busy} onClick={()=>void act("export",{id:selected.id,version:selected.version})}>{t("导出完整案例包","Export complete case")}</button>
          {active&&onContinue&&view.artDesignStatus?.enabled&&<button className="text-button" disabled={busy} onClick={()=>void request(`请为当前任务的满意成果整理案例修订草稿，关联 parentCaseId=${selected.id}、parentVersion=${selected.version}，保留旧版，不自动入库；先核对当前任务实际设计和证据。`)}>{t("整理该案例的新版本","Prepare a revised case")}</button>}
          {selected.status==="accepted"&&<button className="text-button" disabled={busy} onClick={()=>setWithdrawing(v=>!v)}>{t("停止推荐此版本","Stop recommending this version")}</button>}
        </div>}
        {withdrawing&&<form className="art-withdraw" onSubmit={e=>{e.preventDefault();void act("withdraw",{id:selected.id,version:selected.version,reason:withdrawReason});}}>
          <label>{t("撤回原因（保留历史与既有引用）","Reason (history and existing references remain)")}<input value={withdrawReason} maxLength={2000} required onChange={e=>setWithdrawReason(e.target.value)}/></label>
          <button className="secondary-button" disabled={busy||!withdrawReason.trim()}>{t("确认停止推荐","Confirm withdrawal")}</button>
        </form>}
      </>:<p>{t("选择案例查看画面、设计依据、制作方法与适用边界。","Select a case to inspect visuals, rationale, production and applicability.")}</p>}
    </div></div>
  </Modal>;
}
