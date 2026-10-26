import { useEffect, useState } from "react";
import { api, errorMessage } from "../../shared/api";
import type { GameInstance } from "../../shared/types";
import { productionApi, type ProductionRecord, type ProductionView } from "../../shared/production";

export function PrototypeAuthorization({record,active,en,onUpdated}:{record:ProductionRecord;active:boolean;en:boolean;onUpdated:(v:ProductionView)=>void}) {
  const [open,setOpen]=useState(false),[instances,setInstances]=useState<GameInstance[]>([]);
  const [source,setSource]=useState(""),[note,setNote]=useState(""),[busy,setBusy]=useState(false),[error,setError]=useState("");
  useEffect(()=>{if(!open)return;let valid=true;
    api.listInstances(record.taskId).then(v=>{if(valid)setInstances(v.filter(i=>i.mode==="editor"&&i.processState==="running"));}).catch(e=>{if(valid)setError(errorMessage(e));});
    return()=>{valid=false;};},[open,record.taskId]);
  async function run(action:()=>Promise<ProductionView>){setBusy(true);setError("");try{onUpdated(await action());}catch(e){setError(errorMessage(e));}finally{setBusy(false);}}
  const grants=(record.workbench?.prototypeGrants??[]).filter(g=>g.effective&&Date.parse(g.expiresAt)>Date.now());
  return <details open={open} onToggle={e=>setOpen(e.currentTarget.open)} className="wb-prototype">
    <summary>{en?"Optional isolated prototype":"可选：授权隔离交互小样"}</summary>
    <p>{en?"Design can be reviewed without a running game. If needed, authorize 30 minutes of experiments on a copy of a saved archive. The source remains open; save it first. This does not approve the design or implementation plan.":"设计稿可以直接提交审核。确需验证交互时，可授权 AI 在已保存存档的独立副本上预研 30 分钟。复制前请保存源存档；此操作不代表批准美术稿或执行计划。"}</p>
    {open&&<><label className="field">{en?"Source editor":"当前任务的源编辑实例"}<select value={source} onChange={e=>setSource(e.target.value)} disabled={busy||!active}><option value="">{en?"Select a running editor":"选择正在运行的编辑实例"}</option>{instances.map(i=><option key={i.id} value={i.id}>{i.name}</option>)}</select></label>
      {!instances.length&&<p>{en?"Open a saved archive in this task’s game instances first.":"如需预研，请先在本任务的“游戏实例”中打开已保存的存档。"}</p>}
      <label className="field">{en?"Experiment goal":"本次预研范围"}<textarea value={note} maxLength={2000} onChange={e=>setNote(e.target.value)} placeholder={en?"For example: button hit area only":"例如：只验证比较按钮的点击区域"}/></label>
      <button className="secondary-button" disabled={!active||busy||!source||!note.trim()} onClick={()=>void run(()=>productionApi.startPrototype(record.taskId,record.revision,source,note))}>{en?"Create and authorize a copy":"创建副本并授权预研"}</button>
    </>}
    {grants.map(g=><p key={g.id}>{g.note} · {en?"Expires":"有效至"} {new Date(g.expiresAt).toLocaleTimeString()} <button className="text-button" disabled={!active||busy} onClick={()=>void run(()=>productionApi.workbench(record.taskId,record.revision,"revoke-prototype",{id:g.id}))}>{en?"Revoke":"撤销授权"}</button></p>)}
    {error&&<p role="alert">{error}</p>}
  </details>;
}
