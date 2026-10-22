import { useEffect, useRef, useState } from "react";
import { memoryApi, type MemoryPolicy, type MemoryStatus } from "../../shared/memory";
import { errorMessage } from "../../shared/api";
import "./memory.css";

export function MemoryPanel({id,managed,running,en=false}:{id:string;managed:boolean;running:boolean;en?:boolean}) {
  const [status,setStatus]=useState<MemoryStatus>(),[policy,setPolicy]=useState<MemoryPolicy>();
  const [error,setError]=useState(""),[busy,setBusy]=useState(false);
  const generation=useRef(0), t=(zh:string,english:string)=>en?english:zh;
  useEffect(()=>{
    const epoch=++generation.current;let timer:ReturnType<typeof setTimeout>;
    setStatus(undefined);setPolicy(undefined);setError("");setBusy(false);
    void memoryApi.policy().then(p=>{if(epoch===generation.current)setPolicy(p);}).catch(e=>{if(epoch===generation.current)setError(errorMessage(e));});
    const poll=async()=>{try{const value=await memoryApi.status(id);if(epoch===generation.current)setStatus(value);}
      catch(e){if(epoch===generation.current)setError(errorMessage(e));}
      finally{if(epoch===generation.current)timer=setTimeout(poll,2000);}};
    void poll();return()=>{generation.current++;clearTimeout(timer);};
  },[id]);
  const act=async(action:()=>Promise<void>)=>{if(busy)return;const epoch=generation.current;setBusy(true);setError("");
    try{await action();const next=await memoryApi.status(id);if(epoch===generation.current)setStatus(next);}
    catch(e){if(epoch===generation.current)setError(errorMessage(e));}finally{if(epoch===generation.current)setBusy(false);}};
  const gib=(bytes?:number)=>bytes===undefined?"—":`${(bytes/1073741824).toFixed(2)} GiB`;
  const latest=status?.latest;
  return <section aria-label={t("内存监测与异常记录","Memory monitoring and incidents")} className="production-card">
    <h3>{t("内存监测与异常记录","Memory monitoring and incidents")}</h3>
    <p role="status">{!running&&latest?t("实例已停止，以下保留最后采样与异常记录。","Instance stopped; last samples and incident records remain."):status?.pressured?t("内存保护中：已限制高开销操作，请检查报告。","Memory pressure: expensive operations are restricted. Inspect the report."):latest?t("持续采样中；未触发保护不代表没有泄漏。","Sampling; no trigger is not proof of leak-free operation."):t("暂无采样。外部或已退出的实例不接受自动停止控制。","No sample. External or exited instances cannot be armed.")}</p>
    {status?.warning&&<p role="status">{status.warning}</p>}
    {latest&&<small>{t("采样时间：","Sample time: ")}{new Date(latest.at).toLocaleString()}</small>}
    <dl className="detail-grid memory-metrics">
      {[[t("私有提交","Private commit"),gib(latest?.privateBytes)],[t("物理驻留","Working set"),gib(latest?.workingSetBytes)],
        [t("采样峰值私有提交","Sampled peak private commit"),gib(latest?status?.peakPrivateBytes:undefined)],[t("系统可用物理内存","System available RAM"),gib(latest?.availableBytes)],
        [t("系统提交 / 上限","System commit / limit"),`${gib(latest?.commitBytes)} / ${gib(latest?.commitLimitBytes)}`]].map(([label,value])=><div className="detail-item" key={label}><dt>{label}</dt><dd>{value}</dd></div>)}
    </dl>
    {status?.incidentId&&<p>{t("事件编号：","Incident: ")}{status.incidentId}</p>}
    {(error||status?.error)&&<p role="alert" className="inline-error">{error||status?.error}</p>}
    <label className="memory-stop-choice"><input type="checkbox" checked={status?.autoStop??false} disabled={busy||!managed||!running||!latest}
      onChange={e=>void act(()=>memoryApi.arm(id,e.target.checked))}/><span>{t("允许超限时自动停止此实例（未保存内容可能丢失）","Allow automatic stop on pressure (unsaved work may be lost)")}</span></label>
    <p>{t("系统可用内存与提交余量不足时仅警告。游戏自身占用或增长超限时保留操作保护；自动停止仅对本次实例有效。","Low system memory and commit headroom only warn. Per-game usage or growth limits retain operation protection; auto-stop applies only to this instance.")}</p>
    <div className="modal-actions"><button className="secondary-button" onClick={()=>void act(()=>memoryApi.reports())}>{t("打开全部异常记录","Open incident records")}</button>
      {status?.reportPath&&<button className="secondary-button" onClick={()=>void act(()=>memoryApi.reports(id))}>{t("打开本次报告","Open this report")}</button>}</div>
    {policy&&<details><summary>{t("保护阈值（全部托管实例）","Thresholds (all managed instances)")}</summary>
      <form onSubmit={e=>{e.preventDefault();void act(()=>memoryApi.save(policy));}}>
        {([['privateLimitMib',t('私有提交上限 MiB','Private commit limit MiB')],['growthLimitMib',t('一分钟增长上限 MiB','One-minute growth limit MiB')],['availableFloorMib',t('系统可用内存下限 MiB','Available RAM floor MiB')],['commitLimitPercent',t('系统提交上限 %','System commit ceiling %')]] as const).map(([key,label])=><label key={key}>{label}<input type="number" step="1" required value={policy[key]} onChange={e=>setPolicy({...policy,[key]:Number(e.target.value)})}/></label>)}
        <button className="secondary-button" disabled={busy}>{t("保存阈值","Save thresholds")}</button>
      </form></details>}
  </section>;
}
