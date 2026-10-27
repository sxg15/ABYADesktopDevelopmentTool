import { useState } from "react";
import { productionApi, type ProductionRecord, type ProductionView } from "../../shared/production";
import { errorMessage } from "../../shared/api";
import { ProgrammerIssueExport } from "./ProgrammerIssueExport";
import { activeProgrammerIssues } from "./programmerIssue";
import { confirmedSourceDefect } from "./PlaytestIssues";
export function WorkbenchIssues({taskId,record:r,stage,en,active,onUpdated}:{taskId:string;record:ProductionRecord;stage:string;en:boolean;active:boolean;onUpdated:(v:ProductionView)=>void}){
  const [selected,setSelected]=useState(""),[reason,setReason]=useState(""),[impact,setImpact]=useState("");
  const [error,setError]=useState(""),[busy,setBusy]=useState(false);
  const issues=r.issues.filter(i=>i.status!=="resolved"&&(i.stage===stage||i.affectedStages?.includes(stage))&&!(stage==="review"&&confirmedSourceDefect(r,i.id)));
  const deferred=(id:string)=>{const issue=r.issues.find(i=>i.id===id);if(!issue||issue.kind!=="suggestion")return undefined;
    return r.workbench?.deferrals.find(d=>d.status==="deferred" && d.stage===issue.stage && d.description.replace(/\s+/g," ").trim()===issue.description.replace(/\s+/g," ").trim() && JSON.stringify(d.affectedStages??null)===JSON.stringify(issue.affectedStages??null) && JSON.stringify(d.blockedOperations??null)===JSON.stringify(issue.blockedOperations??null));};
  const open=issues.filter(i=>!deferred(i.id)),skipped=issues.filter(i=>deferred(i.id));
  async function save(operation:string,id:string){setBusy(true);setError("");try{onUpdated(await productionApi.workbench(taskId,r.revision,operation,{id,reason,impact}));setSelected("");setReason("");setImpact("");}catch(e){setError(errorMessage(e));}finally{setBusy(false);}}
  return <div className="wb-issues">
    {!!open.length&&<details><summary>{en?"Issues requiring attention":"需要关注的问题"} · {open.length}</summary>{open.map(i=><div key={i.id}><p><strong>{i.title??i.description}</strong></p>{i.blockingDisposition==="tracked-not-gating-design"&&<p>{en?"Recorded for follow-up; design review can continue.":"已记录待验证事项，不阻止本阶段设计稿提交。"}</p>}{i.executionHandling?<><p>{({diagnosing:"AI 正在排查",repairing:"AI 正在修复",workaround:"已采用替代方案",deferred:"受影响工作暂缓，其余继续","awaiting-condition":"等待相关条件恢复"} as Record<string,string>)[i.executionHandling.mode]}</p><p>{i.executionHandling.reason}</p><p>影响：{i.executionHandling.impact}</p></>:i.resumeWhen&&<p>{i.resumeWhen}</p>}{i.kind==="suggestion"&&<button className="text-button" disabled={!active||busy} onClick={()=>setSelected(i.id)}>{en?"Defer for this task":"本任务持续略过"}</button>}</div>)}<ProgrammerIssueExport taskId={taskId} revision={r.revision} issueIds={activeProgrammerIssues(open).map(i=>i.id)} en={en}/></details>}
    {!!skipped.length&&<details><summary>{en?"Deferred items":"已略过事项"} · {skipped.length}</summary>{skipped.map(i=><div key={i.id}><strong>{i.description}</strong><p>{deferred(i.id)?.impact}</p><p>{deferred(i.id)?.reason}</p><button className="text-button" disabled={!active||busy} onClick={()=>void save("restore",deferred(i.id)!.id)}>{en?"Restore":"恢复关注"}</button></div>)}</details>}
    {selected&&<section><h3>{en?"Defer this optional suggestion":"略过这条可选建议"}</h3><label className="field">{en?"Reason":"略过原因"}<textarea value={reason} onChange={e=>setReason(e.target.value)} maxLength={1000}/></label><label className="field">{en?"Impact on this task":"对本任务的影响"}<textarea value={impact} onChange={e=>setImpact(e.target.value)} maxLength={1000}/></label><p>{en?"This is not a fix or approval. Required gates remain.":"略过不代表修复或通过；必要检查仍须完成。"}</p><button className="secondary-button" onClick={()=>setSelected("")}>{en?"Cancel":"取消"}</button><button className="primary-button" disabled={busy||!active||!reason.trim()||!impact.trim()} onClick={()=>void save("defer",selected)}>{en?"Defer for this task":"本任务持续略过"}</button></section>}
    {error&&<p role="alert">{error}</p>}
  </div>;
}
