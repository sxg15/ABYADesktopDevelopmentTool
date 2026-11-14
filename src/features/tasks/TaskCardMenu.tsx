import {useEffect,useLayoutEffect,useRef,useState} from "react";
import {Archive,FolderOpen,Trash2} from "lucide-react";
import {createPortal} from "react-dom";
import {Modal} from "../../app/Modal";
import {api,errorMessage} from "../../shared/api";
import type {DevelopmentTask} from "../../shared/types";

export function TaskCardMenu({task,x,y,en,close,notify,onChanged}:{
  task:DevelopmentTask;x:number;y:number;en:boolean;close:()=>void;
  notify:(msg:string,error?:boolean)=>void;onChanged:()=>Promise<void>;
}){
  const ref=useRef<HTMLDivElement>(null);
  const [migration,setMigration]=useState(false);
  const [confirm,setConfirm]=useState(false),[busy,setBusy]=useState(false);
  const [cleanWorkspace,setCleanWorkspace]=useState(false);
  useLayoutEffect(()=>{
    const menu=ref.current;
    if(!menu)return;
    menu.style.left=Math.max(4,Math.min(x,innerWidth-menu.offsetWidth-4))+"px";
    menu.style.top=Math.max(4,Math.min(y,innerHeight-menu.offsetHeight-4))+"px";
    menu.querySelector("button")?.focus();
  },[task.id,x,y]);
  useEffect(()=>{
    if(confirm||migration)return;
    function outside(e:PointerEvent){if(!ref.current?.contains(e.target as Node))close();}
    document.addEventListener("pointerdown",outside,true);
    window.addEventListener("resize",close);
    return ()=>{document.removeEventListener("pointerdown",outside,true);window.removeEventListener("resize",close);};
  },[close,confirm,migration]);
  async function run(action:()=>Promise<unknown>,refresh=false){
    if(busy)return;
    setBusy(true);
    try{await action();if(refresh)await onChanged();close();}
    catch(e){notify(errorMessage(e),true);if(refresh)await onChanged().catch(()=>{});close();}
  }

  const [migrationError,setMigrationError]=useState("");
  async function migrate(){
    if(busy)return;
    setBusy(true);setMigrationError("");
    const key="abya:chat-migration:"+task.id;
    const request=localStorage.getItem(key)||crypto.randomUUID();
    localStorage.setItem(key,request);
    try{
      const result=await api.migrateTaskChat(task.id,request);
      await api.openMigratedTaskChat(task.id,result.link);
      localStorage.removeItem(key);if(result.archivePending)notify(en?"New chat ready. The previous chat will be archived once Codex releases it.":"新对话已就绪。旧对话仍被 Codex 占用，释放后将自动归档。");close();
    }catch(e){setMigrationError(errorMessage(e));}finally{setBusy(false);}
  }
  if(migration)return <Modal title={en?"Migrate Codex chat":"新建并迁移 Codex 对话"} onClose={()=>{if(!busy)close();}}>
    <p>{en?"Create a new chat for this project, bring over its progress and recent discussion, then archive the previous chat.":"为本项目新建对话，带入当前进度、已确认方案及近期讨论。新对话绑定成功后归档旧对话。"}</p>
    <p>{en?"Uses Astra High. Stop any running turn before migrating.":"新对话使用 Astra 高。请先停止旧对话中正在执行的工作。"}</p>
    {migrationError&&<p role="alert">{migrationError}</p>}
    <footer className="modal-actions"><button disabled={busy} onClick={close}>{en?"Cancel":"取消"}</button><button className="primary-button" disabled={busy} onClick={()=>void migrate()}>{busy?(en?"Migrating…":"正在迁移…"):(en?"Create and migrate":"新建并迁移")}</button></footer>
  </Modal>;
  const remove=en?"Delete":"删除";
  if(confirm)return <Modal title={en?"Delete task":"删除开发任务"} onClose={()=>{if(!busy)close();}}>
    <p>{en?`Delete “${task.title}” and its production records?`:`删除「${task.title}」及其制作记录？`}</p>
    <label className="delete-workspace-option"><input type="checkbox" checked={cleanWorkspace} disabled={busy} onChange={e=>setCleanWorkspace(e.target.checked)}/>{en?"Also delete workspace files":"同时清理工作区文件"}</label>
    {cleanWorkspace&&<div className="delete-workspace-detail"><p>{en?"This folder and its contents will be permanently deleted.":"以下目录及其中的文件将被永久删除。正在被占用的目录会在占用解除后自动清理。"}</p><code>{task.workspacePath}</code></div>}
    <footer className="modal-actions"><button disabled={busy} onClick={close}>{en?"Cancel":"取消"}</button>
      <button className="danger-button" disabled={busy} onClick={()=>void run(()=>api.deleteTask(task.id,cleanWorkspace).then(pending=>{if(pending)notify(en?"Task deleted. The app will retry workspace cleanup when the folder is released.":"任务已删除，工作区已加入清理队列。占用解除后，APP 会自动清理。");}),true)}>{remove}</button></footer>
  </Modal>;
  return createPortal(<div ref={ref} className="task-card-menu" role="menu" aria-label={task.title}
    onContextMenu={e=>e.preventDefault()} onKeyDown={e=>{
      if(e.key==="Escape"||e.key==="Tab"){close();return;}
      if(e.key!=="ArrowDown"&&e.key!=="ArrowUp")return;
      e.preventDefault();const buttons=Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"));
      buttons[(buttons.indexOf(document.activeElement as HTMLButtonElement)+(e.key==="ArrowDown"?1:-1)+buttons.length)%buttons.length]?.focus();
    }}>
    <button role="menuitem" disabled={busy} onClick={()=>void run(()=>api.openTaskFolder(task.id))}><FolderOpen size={16}/>{en?"Open in File Explorer":"在文件资源管理器中打开"}</button>
    <button role="menuitem" disabled={busy} onClick={()=>void run(()=>api.openTaskArchives(task.id))}><FolderOpen size={16}/>{en?"Open saves in File Explorer":"在资源管理器中打开存档位置"}</button>
    <button role="menuitem" disabled={busy} onClick={()=>setMigration(true)}><FolderOpen size={16}/>{en?"New Codex chat with context":"新建并迁移 Codex 对话"}</button>
    <button role="menuitem" disabled={busy||task.status==="archived"} onClick={()=>void run(()=>api.setTaskStatus(task.id,"archived"),true)}><Archive size={16}/>{en?"Archive":"归档"}</button>
    <button role="menuitem" className="task-delete-action" disabled={busy} onClick={()=>setConfirm(true)}><Trash2 size={16}/>{remove}</button>
  </div>,document.body);
}
