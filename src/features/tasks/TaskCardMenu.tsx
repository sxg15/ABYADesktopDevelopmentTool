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
    if(confirm)return;
    function outside(e:PointerEvent){if(!ref.current?.contains(e.target as Node))close();}
    document.addEventListener("pointerdown",outside,true);
    window.addEventListener("resize",close);
    return ()=>{document.removeEventListener("pointerdown",outside,true);window.removeEventListener("resize",close);};
  },[close,confirm]);
  async function run(action:()=>Promise<unknown>,refresh=false){
    if(busy)return;
    setBusy(true);
    try{await action();if(refresh)await onChanged();close();}
    catch(e){notify(errorMessage(e),true);if(refresh)await onChanged().catch(()=>{});close();}
  }
  const remove=en?"Delete":"删除";
  if(confirm)return <Modal title={en?"Delete task":"删除开发任务"} onClose={()=>{if(!busy)close();}}>
    <p>{en?`Delete “${task.title}” and its production records?`:`删除「${task.title}」及其制作记录？`}</p>
    <label className="delete-workspace-option"><input type="checkbox" checked={cleanWorkspace} disabled={busy} onChange={e=>setCleanWorkspace(e.target.checked)}/>{en?"Also delete workspace files":"同时清理工作区文件"}</label>
    {cleanWorkspace&&<div className="delete-workspace-detail"><p>{en?"This folder and its contents will be permanently deleted.":"以下目录及其中的文件将被永久删除。"}</p><code>{task.workspacePath}</code></div>}
    <footer className="modal-actions"><button disabled={busy} onClick={close}>{en?"Cancel":"取消"}</button>
      <button className="danger-button" disabled={busy} onClick={()=>void run(()=>api.deleteTask(task.id,cleanWorkspace),true)}>{remove}</button></footer>
  </Modal>;
  return createPortal(<div ref={ref} className="task-card-menu" role="menu" aria-label={task.title}
    onContextMenu={e=>e.preventDefault()} onKeyDown={e=>{
      if(e.key==="Escape"||e.key==="Tab"){close();return;}
      if(e.key!=="ArrowDown"&&e.key!=="ArrowUp")return;
      e.preventDefault();const buttons=Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"));
      buttons[(buttons.indexOf(document.activeElement as HTMLButtonElement)+(e.key==="ArrowDown"?1:-1)+buttons.length)%buttons.length]?.focus();
    }}>
    <button role="menuitem" disabled={busy} onClick={()=>void run(()=>api.openTaskFolder(task.id))}><FolderOpen size={16}/>{en?"Open in File Explorer":"在文件资源管理器中打开"}</button>
    <button role="menuitem" disabled={busy||task.status==="archived"} onClick={()=>void run(()=>api.setTaskStatus(task.id,"archived"),true)}><Archive size={16}/>{en?"Archive":"归档"}</button>
    <button role="menuitem" className="task-delete-action" disabled={busy} onClick={()=>setConfirm(true)}><Trash2 size={16}/>{remove}</button>
  </div>,document.body);
}
