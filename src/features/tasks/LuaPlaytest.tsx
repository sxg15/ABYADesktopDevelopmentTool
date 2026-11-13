import {useRef,useState,type ReactNode} from "react";
import {productionApi,type ProductionRecord} from "../../shared/production";
import {errorMessage} from "../../shared/api";
import {PlaytestSeats} from "./PlaytestSeats";
import {PreviewPlayerRecovery} from "./PreviewPlayerRecovery";

export function LuaPlaytest({record,active,en,label}:{record:ProductionRecord;active:boolean;en:boolean;label:(v?:string)=>ReactNode}){
  const version=record.currentStage==="review"?record.currentVersion:record.workbench?.execution?.verificationPackage?.version;
  const [seats,setSeats]=useState(2),[busy,setBusy]=useState(false);
  const [message,setMessage]=useState(""),[error,setError]=useState(""),[recover,setRecover]=useState(false);
  const locked=useRef(false);
  async function start(){
    if(!active||!version||locked.current)return;
    locked.current=true;setBusy(true);setError("");setMessage("");setRecover(false);
    try{
      const result=await productionApi.startPreview(record.taskId,version,record.playerMode==="multiplayer"?seats:undefined);
      setMessage(result.message??(en?"Playtest opened.":"试玩窗口已打开。"));
    }catch(e){
      const code=(e as {code?:string})?.code;
      setRecover(code==="player_pin_mismatch"||code==="app_player_mismatch"||code==="preview_preflight_required");
      setError(errorMessage(e));
    }finally{locked.current=false;setBusy(false);}
  }
  return <section className="production-card acceptance-launcher">
    <div className="production-card-heading"><strong>{en?"Playtest":"人工试玩"}</strong>
      <button className="primary-button" disabled={!active||busy||!version} onClick={()=>void start()}>
        {busy?(en?"Opening…":"正在打开试玩…"):(en?"Start playtest":"启动试玩")}</button></div>
    {version?label(version):<p>{en?"Generate a playable checkpoint first.":"生成可试玩的版本后，就可以从这里启动。"}</p>}
    {record.playerMode==="multiplayer"&&<PlaytestSeats value={seats} onChange={setSeats} disabled={!active||busy} en={en}/>}
    {message&&<p role="status">{message}</p>}
    {error&&<p role="alert" className="inline-error">{error}</p>}
    {recover&&<PreviewPlayerRecovery taskId={record.taskId} active={active&&!busy} en={en} reuse={record.currentStage==="review"||!!record.currentVersion}
      onVerified={()=>{setRecover(false);setError("");setMessage(en?"Preflight passed. Start the playtest again.":"当前 PB 预检通过，可以启动试玩。");}}/>}
  </section>;
}
