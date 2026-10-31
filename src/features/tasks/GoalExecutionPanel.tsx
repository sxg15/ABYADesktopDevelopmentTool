import { useRef, useState } from "react";
import { Modal } from "../../app/Modal";
import { errorMessage } from "../../shared/api";
import { continuationMessages, type IntakeContinuation, type ProductionRecord, type WorkbenchGoal } from "../../shared/production";

export type ResumeGoal = (goal:string,revision:number,intent:string,note?:string)=>Promise<IntakeContinuation>;
export function goalExecutionStatus(record:ProductionRecord,goal:WorkbenchGoal,checking=false) {
  if(!["work","request","feedback"].includes(goal.sourceType)||["completed","resolved","accepted","superseded"].includes(goal.status))return goal.status;
  if(checking)return "reconciling";
  const status=[...(record.workbench?.attempts??[])].reverse().find(a=>a.goalId===goal.id)?.status??"untracked";
  if(record.policy?.implementationExecution?.inPlaceRecoveryVersion===1&&goal.id===record.workbench?.activeGoalId&&["repairRequired","externalBlocked","technicalReview"].includes(record.workbench?.execution?.state??"")&&["finished","failed"].includes(status))return "technicalReview";
  if(["finished","interrupted","paused","failed"].includes(status)&&record.workbench?.execution?.preflight?.state==="exhausted"&&record.workbench.execution.preflight.goalId===goal.id&&goal.stage===record.currentStage)return "recoveryExhausted";
  if(["finished","interrupted","paused","failed"].includes(status)&&goal.id===record.workbench?.activeGoalId&&["repairRequired","externalBlocked"].includes(record.workbench?.execution?.state??""))return record.workbench!.execution!.state;
  if(["finished","interrupted","paused","failed"].includes(status)&&goal.id===record.workbench?.activeGoalId&&["waitingDependencies","settledWithGaps","settled"].includes(record.workbench?.execution?.state??""))return "workSettled";
  return status;
}
export const executionLabels:Record<string,string>={repairRequired:"尚未形成可玩包，需要继续修复",externalBlocked:"缺少明确外部条件，成果已保留",recoveryExhausted:"预检恢复未成功，查看问题与恢复条件",workSettled:"本轮已结束，查看结果与缺口",running:"进行中",sending:"正在派发",queued:"等待连接 / 恢复",finished:"待继续",paused:"已暂停",interrupted:"被中断",reconciling:"正在核对",needsReview:"结果待核对",failed:"执行失败",untracked:"待核对 / 未执行"};
export function GoalExecutionPanel({record,goal,active,checking,onResume,en=false}:{record:ProductionRecord;goal:WorkbenchGoal;active:boolean;checking:boolean;onResume?:ResumeGoal;en?:boolean}) {
  const [busy,setBusy]=useState(false),[error,setError]=useState(""),[notice,setNotice]=useState("");
  const [review,setReview]=useState(false),[note,setNote]=useState("");const locked=useRef(false);
  const status=goalExecutionStatus(record,goal,checking);
  const attempt=[...(record.workbench?.attempts??[])].reverse().find(a=>a.goalId===goal.id);
  const key=`abya:resume:${record.taskId}:${goal.id}`;
  function intent(fresh=false){let id="";try{id=localStorage.getItem(key)??"";}catch{/* In-memory request remains guarded. */}
    if(fresh||!id||record.workbench?.attempts?.some(a=>a.id===id&&["finished","paused","interrupted","failed"].includes(a.status)))id=crypto.randomUUID();
    try{localStorage.setItem(key,id);}catch{/* Native intent is also durable. */}return id;}
  const intentRef=useRef<string|undefined>(undefined);
  async function resume(){if(locked.current||!onResume)return;locked.current=true;setBusy(true);setError("");
    const id=intentRef.current??intent();intentRef.current=id;
    try{const result=await onResume(goal.id,record.revision,id,note||undefined);setNotice(continuationMessages[result.status]);
      if(["workSettled","started","resumed","running","completed","queued"].includes(result.status)){try{localStorage.removeItem(key);}catch{/* Optional cache. */}intentRef.current=undefined;setReview(false);}}
    catch(e){setError(errorMessage(e));}finally{locked.current=false;setBusy(false);}}
  const allowed=goal.stage===record.currentStage&&goal.cycle===record.cycle&&!!executionLabels[status];
  return <section className="goal-execution" aria-label={en?"Subprocess execution":"子流程执行状态"}>
    <strong>{status==="technicalReview"?(en?"Results retained · technical follow-up":"成果已保留 · 技术待补项"):en?status:executionLabels[status]??status}</strong>
    {status==="technicalReview"&&<p>{en?"No repeated Continue action is required. Review retained results and recovery conditions.":"无需反复点击继续。请查看已有成果及明确的恢复条件；未完成项没有被记为通过。"}</p>}
    {attempt?.reason&&<p>{attempt.reason}</p>}
    {status==="untracked"&&<p>{en?"This older goal has no execution binding. Review its history before choosing the original conversation.":"旧记录没有可靠的执行绑定。请核对历史并选定原会话；同名目标不会被自动合并或标记完成。"}</p>}
    {allowed&&status!=="recoveryExhausted"&&status!=="workSettled"&&status!=="running"&&status!=="sending"&&<button className="primary-button" disabled={!active||busy||checking||!onResume} onClick={()=>{
      if(["needsReview","reconciling","untracked"].includes(status)){intentRef.current=intent(true);setReview(true);}else void resume();
    }}>{busy?(en?"Resuming…":"正在恢复…"):["repairRequired","externalBlocked"].includes(status)?(en?"Diagnose and resume this goal":"诊断并恢复此目标"):en?"Resume this subprocess":"继续此子流程"}</button>}
    {notice&&<p role="status">{notice}</p>}{error&&<p role="alert">{error}</p>}
    {!!record.workbench?.attempts?.some(a=>a.goalId===goal.id)&&<details><summary>{en?"Execution history":"执行历史"}</summary>{record.workbench.attempts.filter(a=>a.goalId===goal.id).map(a=><p key={a.id}>{executionLabels[a.status]??a.status} · {new Date(a.updatedAt).toLocaleString()}<br/>{a.reason}<br/><small>{a.id}</small></p>)}</details>}
    {review&&<Modal title={en?"Review before resuming":"核对结果后恢复原子流程"} onClose={()=>setReview(false)}>
      <p>{goal.title} · {goal.id}</p><p>{en?"Confirm existing results and uncertain writes. Resuming keeps this goal and its history.":"先核对已有成果与结果未知的写入。恢复沿用这个子流程的原 ID，不重放其他目标。"}</p>
      <label className="field">{en?"What has been checked?":"核对说明"}<textarea value={note} maxLength={2000} onChange={e=>setNote(e.target.value)}/></label>
      {notice&&<p role="status">{notice}</p>}{error&&<p role="alert">{error}</p>}<button className="primary-button" disabled={busy||!note.trim()} onClick={()=>void resume()}>{en?"Confirmed, resume this subprocess":"已核对，继续此子流程"}</button>
    </Modal>}
  </section>;
}
