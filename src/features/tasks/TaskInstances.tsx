import {useEffect,useState} from "react";
import {ChevronRight, Monitor} from "lucide-react";
import {AcceptanceLauncher} from "./AcceptanceLauncher";
import {LaunchInstanceModal} from "../instances/LaunchInstanceModal";
import {InstanceDetailsModal} from "../instances/InstanceDetailsModal";
import {api,errorMessage} from "../../shared/api";
import type {AppSettings,DevelopmentTask,GameInstance} from "../../shared/types";
import type {MessageKey} from "../../i18n";
export function TaskInstances({task,t,settings,notify}:{task:DevelopmentTask;t:(k:MessageKey)=>string;settings?:AppSettings;notify:(m:string,error?:boolean)=>void}){
  const [items,setItems]=useState<GameInstance[]>([]),[launch,setLaunch]=useState(false),[detail,setDetail]=useState<GameInstance>();
  async function refresh(){setItems(await api.listInstances(task.id));}
  useEffect(()=>{let live=true;async function poll(){try{const list=await api.listInstances(task.id);if(live)setItems(list);}catch(e){if(live)notify(errorMessage(e),true);}}
    void poll();const timer=setInterval(poll,4000);return()=>{live=false;clearInterval(timer);};},[task.id]);
  return <section className="task-instances"><header><h3>{t("instances")}</h3>
    <button className="secondary-button" disabled={task.status!=="active"} onClick={()=>setLaunch(true)}>{t("launch")}</button></header>
    <AcceptanceLauncher taskId={task.id} active={task.status==="active"} en={settings?.locale==="en-US"}/>
    {items.length > 0 && <ul className="instance-cards">
      {items.map(instance => <li key={instance.id}>
        <button type="button" className="instance-card" onClick={() => setDetail(instance)}>
          <Monitor className="instance-card-icon" size={18} aria-hidden="true" />
          <span className="instance-card-body">
            <strong>{instance.name}</strong>
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
