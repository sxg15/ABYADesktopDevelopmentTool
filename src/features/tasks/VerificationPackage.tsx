import {useState} from "react";
import {productionApi,type ProductionRecord} from "../../shared/production";
import {errorMessage} from "../../shared/api";
export function VerificationPackage({record,active,en}:{record:ProductionRecord;active:boolean;en:boolean}) {
  const [error,setError]=useState(""),[busy,setBusy]=useState(false);
  const pack=record.workbench?.execution?.verificationPackage;
  if(!pack?.path)return null;
  const checks=[...(record.selfTests??[])].reverse().filter(t=>t.version===pack.version&&t.cycle===record.cycle);
  const current=pack.version===record.currentVersion&&["lua","input","core-loop","save-reload"].every(id=>checks.find(t=>t.id===(id==="input"&&checks.some(c=>c.id==="playable-input")?"playable-input":id))?.status==="passed");
  async function run(action:()=>Promise<unknown>){setBusy(true);setError("");try{await action();}catch(e){setError(errorMessage(e));}finally{setBusy(false);}}
  return <section className="goal-execution" aria-label={en?"Verified playable package":"可验证的试玩包"}>
    <strong>{en?"Verified core package":"核心实测包"} · {pack.version}</strong>
    <p>{current?(en?"Core checks passed. Remaining gaps are included; final acceptance is still pending.":"核心输入、循环及保存重载已有实测。包内附验证记录和剩余缺口，最终验收仍由你决定。"):(en?"Previous verified version. The current candidate is not yet verified.":"这是上个已验证版本；当前候选尚未形成对应验证包。")}</p>
    <button className="secondary-button" disabled={busy} onClick={()=>void run(()=>productionApi.revealDocument(record.taskId,pack!.path))}>{en?"Show package":"定位试玩包"}</button>
    {current&&<button className="primary-button" disabled={!active||busy} onClick={()=>void run(()=>productionApi.startAcceptance(record.taskId))}>{en?"Start playtest":"启动此版本试玩"}</button>}
    {error&&<p role="alert">{error}</p>}
  </section>;
}
