import {useEffect,useMemo,useState} from "react";
import {ChevronRight, Monitor} from "lucide-react";
import {LuaPlaytest} from "./LuaPlaytest";
import {AcceptanceLauncher} from "./AcceptanceLauncher";
import {LaunchInstanceModal} from "../instances/LaunchInstanceModal";
import {InstanceDetailsModal} from "../instances/InstanceDetailsModal";
import {api,errorMessage} from "../../shared/api";
import type {AppSettings,DevelopmentTask,GameInstance} from "../../shared/types";
import type {MessageKey} from "../../i18n";
import {productionApi,type ProductionRecord} from "../../shared/production";
import {playVersionInfo,PlayVersionLabel} from "./PlayVersionLabel";
export function TaskInstances({task,record,t,settings,notify,preview}:{preview:boolean;task:DevelopmentTask;record:ProductionRecord;t:(k:MessageKey)=>string;settings?:AppSettings;notify:(m:string,error?:boolean)=>void}){
  const [items,setItems]=useState<GameInstance[]>([]),[launch,setLaunch]=useState(false),[detail,setDetail]=useState<GameInstance>();
  const [dates,setDates]=useState<Record<string,string>>({});
  const pack=record.workbench?.execution?.verificationPackage;
  const latest=record.currentVersion??pack?.version;
  const en=settings?.locale==="en-US";
  const sessions=useMemo(()=>{
    const result=new Map<string,string>();
    for(const session of [...(record.acceptanceSessions??[]),...(record.workbench?.execution?.previewSessions??[])]){
      if(session.version)for(const id of session.instanceIds??[])result.set(id,session.version);
    }
    return result;
  },[record]);
  useEffect(()=>{let active=true;productionApi.playVersionDates(task.id).then(v=>{if(active)setDates(v);}).catch(e=>{if(active)notify(errorMessage(e),true);});return()=>{active=false};},[task.id,record.currentVersion,pack?.version]);
  function label(version:string|undefined){return version&&<PlayVersionLabel version={version} latest={latest===version} at={dates[version]??(pack?.version===version?pack.createdAt:undefined)} en={en}/>;}
  async function refresh(){setItems(await api.listInstances(task.id));}
  useEffect(()=>{let live=true;async function poll(){try{const list=await api.listInstances(task.id);if(live)setItems(list);}catch(e){if(live)notify(errorMessage(e),true);}}
    void poll();const timer=setInterval(poll,4000);return()=>{live=false;clearInterval(timer);};},[task.id]);
  return <section className="task-instances"><header><h3>{t("instances")}</h3>
    <button className="secondary-button" disabled={task.status!=="active"} onClick={()=>setLaunch(true)}>{t("launch")}</button></header>
    {preview?<LuaPlaytest key={task.id} record={record} active={task.status==="active"} en={en} label={label}/>:<AcceptanceLauncher taskId={task.id} active={task.status==="active"} en={en} versionLabel={label(latest)}/>}
    {items.length > 0 && <ul className="instance-cards">
      {items.map(instance => <li key={instance.id}>
        <button type="button" className="instance-card" onClick={() => setDetail(instance)}>
          <Monitor className="instance-card-icon" size={18} aria-hidden="true" />
          <span className="instance-card-body">
            <strong>{instance.name}</strong>
            {label(playVersionInfo(instance,sessions,dates,latest))}
            <span className="instance-card-meta">
              <span className="instance-card-process" data-state={instance.processState}>{t(instance.processState)}</span>
              <span>{t(instance.connectionState)}</span>
              {instance.mode && <span>{t(instance.mode)}</span>}
            </span>
          </span>
          <ChevronRight size={16} aria-hidden="true" />
        </button>
      </li>)}
    </ul>}
    {launch&&<LaunchInstanceModal taskId={task.id} executablePath={settings?.gameExecutablePath??""} taskInstances={items} t={t} onClose={()=>setLaunch(false)} onLaunched={refresh}/>}
    {detail&&<InstanceDetailsModal instance={detail} t={t} onClose={()=>setDetail(undefined)} onChanged={refresh} onDeleted={async()=>{setDetail(undefined);await refresh();}}/>}</section>;
}
