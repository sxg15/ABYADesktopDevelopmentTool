import {useEffect,useRef,useState} from "react";
import {CirclePlus,Gamepad2} from "lucide-react";
import type {MessageKey} from "../../i18n";
import type {AppSettings,DevelopmentTask} from "../../shared/types";
import type {NotificationActivation} from "../../shared/notifications";
import {api,errorMessage} from "../../shared/api";
import {CreateTaskModal} from "./CreateTaskModal";
import {TaskDescription} from "./TaskDescription";
import {SkillLibrary} from "./SkillLibrary";
import {StageReader} from "./StageReader";
import {openTaskChat} from "./desktopChat";
import {TaskInstances} from "./TaskInstances";
import {TaskCardMenu} from "./TaskCardMenu";


export function TasksView({settings,t,notify,notificationNavigation,onNotificationHandled,library=false,visible=true}:{
  settings?:AppSettings;t:(k:MessageKey)=>string;notify:(msg:string,error?:boolean)=>void;
  notificationNavigation?:NotificationActivation;onNotificationHandled?:(id:string)=>void;library?:boolean;visible?:boolean;
}){
  const [tasks,setTasks]=useState<DevelopmentTask[]>([]);
  const [id,setId]=useState(()=>localStorage.getItem("abya:last-desktop-task")??"");
  const [create,setCreate]=useState(false),[stage,setStage]=useState("");
  const [menu,setMenu]=useState<{task:DevelopmentTask;x:number;y:number}>();
  const opened=useRef<string | undefined>(undefined),selected=tasks.find(task=>task.id===id);
  useEffect(()=>{
    let live=true;
    api.listTasks().then(items=>{if(!live)return;setTasks(items);setId(prev=>items.some(i=>i.id===prev)?prev:(items.find(i=>i.status==="active")??items[0])?.id??"");})
      .catch(e=>{if(live)notify(errorMessage(e),true);});
    return ()=>{live=false;};
  },[]);
  useEffect(()=>{
    if(!notificationNavigation)return;
    const target=notificationNavigation.target;
    if(target){setId(target.taskId);setStage(target.stage);api.listTasks().then(setTasks).catch(e=>notify(errorMessage(e),true));}
    onNotificationHandled?.(notificationNavigation.id);
  },[notificationNavigation?.id]);
  useEffect(()=>{
    if(!selected||library||!visible)return;
    localStorage.setItem("abya:last-desktop-task",selected.id);
    if(opened.current===selected.id||selected.status!=="active")return;
    opened.current=selected.id;
    void openTaskChat(selected.id).catch(e=>notify(errorMessage(e),true));
  },[selected?.id,library,visible]);
  if(library)return <SkillLibrary key={id} taskId={selected?.id??""} active={selected?.status==="active"} locale={settings?.locale}/>;
  return <div className="workspace tasks-workspace desktop-workspace">
    <aside className="task-list-pane"><div className="pane-header"><h1>{t("tasks")}</h1>
      <button className="icon-button primary-icon" title={t("newTask")} onClick={()=>setCreate(true)}><CirclePlus size={18}/></button></div>
      <div className="task-list">{tasks.map(task=><button key={task.id} aria-haspopup="menu" className={"task-row "+(id===task.id?"selected":"")} onClick={()=>{setStage("");setId(task.id);}}
        onContextMenu={event=>{event.preventDefault();event.currentTarget.focus();const rect=event.currentTarget.getBoundingClientRect();setMenu({task,x:event.clientX||rect.left,y:event.clientY||rect.bottom});}}>
        <span className={"status-dot status-"+task.status}/><span className="task-row-copy"><strong>{task.title}</strong><small>{t(task.status)}</small></span></button>)}</div></aside>
    <section className="content-pane desktop-task">{selected?<>
      <header className="content-header task-content-header">
        <div className="task-heading-title"><span className="task-title-icon"><Gamepad2 size={23} aria-hidden="true"/></span>
          <div><div className="eyebrow">{t(selected.status)}</div><h2>{selected.title}</h2></div>
        </div>
      <TaskDescription key={id} taskId={id} description={selected.description} workspacePath={selected.workspacePath} en={settings?.locale==="en-US"}/></header>
      <StageReader key={id} task={selected} stage={stage} selectStage={setStage} notify={notify} visible={visible}>
        <TaskInstances task={selected} t={t} settings={settings} notify={notify}/>
      </StageReader></>:<p className="empty-state">{tasks.length?t("selectTask"):t("noTasks")}</p>}</section>
    {create&&<CreateTaskModal t={t} locale={settings?.locale} onClose={()=>setCreate(false)} onCreated={task=>{setTasks(old=>[task,...old.filter(x=>x.id!==task.id)]);setId(task.id);setStage("");}}/>}
    {menu&&visible&&<TaskCardMenu {...menu} en={settings?.locale==="en-US"} close={()=>setMenu(undefined)} notify={notify}
      onChanged={async()=>{const items=await api.listTasks();setTasks(items);if(!items.some(i=>i.id===id)){setStage("");setId((items.find(i=>i.status==="active")??items[0])?.id??"");}}}/>}
  </div>;
}
