import {useEffect,useRef,useState} from "react";
import {productionApi,type ProductionRecord} from "../../shared/production";
import {errorMessage} from "../../shared/api";

const phases:Record<string,string>={starting:"准备测试目录", "runtime-connect":"启动并连接运行时", "create-availability":"核对主菜单能力", "create-archive":"创建临时存档", "save-archive":"保存并核对内容", "cold-load":"重新启动并核对存档"};
export function showCapabilityPreflight(record:ProductionRecord,stage:string){
 return stage==="plan"||(record.policy?.capabilityPreflightVersion===1&&["implementation","review"].includes(stage));
}
export function CapabilityPreflightPanel({record,active,onUpdated}:{record:ProductionRecord;active:boolean;onUpdated:()=>void}) {
 const p=record.workbench?.execution?.capabilityPreflight;
 const [busy,setBusy]=useState(false),[error,setError]=useState("");
 const started=useRef("");
 const alive=useRef(true);
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
 const key=`${record.taskId}:${record.cycle}`;
 async function run(reuse=false,retry=false){
  setBusy(true);setError("");
  try{await productionApi.preflight(record.taskId,reuse||record.currentStage==="review",retry);}
  catch(e){if(alive.current)setError(errorMessage(e));}
  finally{if(alive.current){setBusy(false);onUpdated();}}
 }
 useEffect(()=>{
  if(active&&record.currentStage==="plan"&&record.policy?.capabilityPreflightVersion===1&&started.current!==key){
   started.current=key; if(p?.state!=="running")void run();
  }
 },[key,active,record.currentStage,record.policy?.capabilityPreflightVersion]);
 const pending=p?.cleanup.state==="pending";
 const passed=p?.state==="passed"||p?.state==="workaround-verified"||p?.state==="existing-verified";
 return <section className="production-card" aria-label="基础能力预检">
  <h4>基础能力预检</h4>
  <p>{p?.state==="running"?phases[p.phase]??"正在预检":passed?(p.state==="existing-verified"?"已有存档的加载、保存及冷加载已验证":p.state==="passed"?"基础链路已通过":"替代初始化路径已验证，原问题仍保留"):p?"预检未通过，请查看问题卡片":record.currentStage==="review"?"可在当前阶段验证修改环境":"尚未测试创建、保存及冷加载能力"}</p>
  <p>{record.currentStage==="review"?"使用本任务已有编辑存档的隔离副本核对当前包，完成后继续原修改任务。":"使用 APP 当前指定的构建包。"}测试结束后自动清理临时存档，诊断回执保留。</p>
  {p?.error?.code==="runtimeCliTimeout"&&<p>无法连接本次测试实例，创建和保存能力尚未验证。请核对运行状态与 CLI 连接；当前不能确定是 PB 包缺陷。</p>}
  {p?.checks.filter(c=>c.status).map(c=><small key={c.id} style={{display:"block"}}>{phases[c.id]??c.id} · {c.status==="passed"?"通过":c.status}</small>)}
  {p&&<p role="status">临时文件：{pending?`待清理${p.cleanup.message?` · ${p.cleanup.message}`:""}`:"已清理"}</p>}
  {p?.reportPath&&<p>{record.currentStage==="plan"?"方案需引用预检报告：":"当前阶段预检报告："}<code style={{overflowWrap:"anywhere"}}>{p.reportPath}</code></p>}
  {error&&<p role="alert">{error}</p>}
  <button type="button" disabled={!active||busy||p?.state==="running"||pending} onClick={()=>void run(false,true)}>重新预检</button>
  {p?.error&&<button type="button" disabled={!active||busy||pending} onClick={()=>void run(true,true)}>验证复用本任务已有存档</button>}
 </section>;
}
