import {useEffect, useRef, useState} from "react";
import {errorMessage} from "../../shared/api";
import {productionApi} from "../../shared/production";
import {taskLaunchApi, type TaskLaunchSources} from "../../shared/taskLaunch";

export function PreviewPlayerRecovery({taskId, active, en=false, reuse, onVerified}:{
  taskId:string; active:boolean; en?:boolean; reuse:boolean; onVerified:()=>void;
}) {
  const [binding,setBinding]=useState<TaskLaunchSources["player"]>();
  const [error,setError]=useState("");
  const [busy,setBusy]=useState(false);
  const [reload,setReload]=useState(0);
  const locked=useRef(false);
  useEffect(()=>{
    let current=true;
    setBinding(undefined);
    taskLaunchApi.binding(taskId).then(value=>{if(current)setBinding(value);})
      .catch(reason=>{if(current)setError(errorMessage(reason));});
    return ()=>{current=false;};
  },[taskId,reload]);
  async function verify(){
    if(!binding||locked.current)return;
    locked.current=true;
    setBusy(true);
    setError("");
    try {
      if(!binding.matches)await taskLaunchApi.adopt(taskId,binding.selectedHash);
      const result=await productionApi.preflight(taskId,reuse,true);
      if(!["passed","existing-verified","workaround-verified"].includes(result.state)||result.cleanup.state!=="completed"){
        throw new Error(result.error?.message||(en?"Preflight incomplete.":"预检未通过，请查看预检记录。"));
      }
      onVerified();
    } catch(reason) {
      setError(errorMessage(reason));
      setReload(value=>value+1);
    } finally {
      locked.current=false;
      setBusy(false);
    }
  }
  return <div className="inline-warning" aria-label={en?"PB build selection":"PB 构建包选择"}>
    {binding&&<><p>{en?"App build":"App 当前 PB"} <code style={{overflowWrap:"anywhere"}}>{binding.selectedPath}</code> · {binding.selectedHash.slice(0,12)}</p>
      {!binding.matches&&<p>{en?"Task build":"任务原 PB"} <code style={{overflowWrap:"anywhere"}}>{binding.previous?.path}</code> · {binding.previous?.hash?.slice(0,12)}</p>}
      <button type="button" disabled={!active||busy} onClick={()=>void verify()}>{busy?(en?"Checking…":"正在核验…"):binding.matches?(en?"Run preflight":"用当前 PB 重新预检"):(en?"Use this build and run preflight":"确认使用当前 PB 并预检")}</button></>}
    {error&&<p role="alert">{error}</p>}
    {!binding&&error&&<button type="button" onClick={()=>setReload(value=>value+1)}>{en?"Refresh":"刷新构建包"}</button>}
  </div>;
}
