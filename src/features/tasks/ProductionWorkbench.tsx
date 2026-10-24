import { AutonomousExecution } from "./AutonomousExecution";
import { WorkbenchSplit } from "./WorkbenchSplit";
import { useEffect, useRef, useState, type ComponentProps } from "react";
import { ChevronRight, FileText, History, Maximize2, X } from "lucide-react";
import { productionApi, type ProductionView, type ProductionRecord, type TaskControlState } from "../../shared/production";
import { errorMessage } from "../../shared/api";
import { ProductionPanel } from "./ProductionPanel";
import { DocumentReview } from "./DocumentReview";
import { TaskIntakeDialog } from "./TaskIntakeDialog";
import { ReadableText } from "./ReadableText";
import { VisualGallery } from "./VisualGallery";
import { FeedbackPanel } from "./FeedbackPanel";
import { WorkbenchIssues } from "./WorkbenchIssues";
import { Modal } from "../../app/Modal";
import { statusName } from "./productionText";
import "./workbench.css";
import {GoalExecutionPanel,goalExecutionStatus,executionLabels,type ResumeGoal} from "./GoalExecutionPanel";

type Props = ComponentProps<typeof ProductionPanel> & {
  onResumeGoal?:ResumeGoal;
  control?: TaskControlState; visible:boolean; onTerminal:()=>void;
  onEnsureConnection:ComponentProps<typeof TaskIntakeDialog>["onEnsureConnection"];
  requestedGroup?:ComponentProps<typeof TaskIntakeDialog>["requestedGroup"];
};
const stageLabels:Record<string,string>={requirements:"需求整理",resources:"资源美术",plan:"方案预检",implementation:"Lua 实现",review:"试玩与修改",delivery:"交付验收",closeout:"收尾复盘"};
const english:Record<string,string>={requirements:"Requirements",resources:"Art & assets",plan:"Plan",implementation:"Implementation",review:"Playtest",delivery:"Acceptance",closeout:"Retrospective"};
export function ProductionWorkbench(props:Props) {
  const {taskId,active,locale,control,onContinue,visible}=props, en=locale==="en-US";
  const [reconciling,setReconciling]=useState(true);
  const [view,setView]=useState<ProductionView>(),[error,setError]=useState("");
  const [stage,setStage]=useState(""),[goalId,setGoalId]=useState("");
  const [snapshot,setSnapshot]=useState<ProductionRecord>(),[details,setDetails]=useState(false);
  const [expanded,setExpanded]=useState(false),[busy,setBusy]=useState(false),[notice,setNotice]=useState("");
  const [draft,setDraft]=useState(()=>localStorage.getItem(`abya:workbench:${taskId}`)??"");
  const [focusedFeedback,setFocusedFeedback]=useState("");
  const [mode,setMode]=useState<"current"|"feedback">("current");
  const [requestId,setRequestId]=useState(()=>crypto.randomUUID());
  const alive=useRef(true),sequence=useRef(0),locked=useRef(false),dirty=useRef(false);
  function accept(next:ProductionView){setView(old=>(old?.record?.revision??0)>(next.record?.revision??0)?old:next);}
  async function refresh(){if(locked.current)return; const seq=++sequence.current;
    try{const next=await productionApi.get(taskId);if(alive.current&&seq===sequence.current){accept(next);if(!dirty.current)setDraft(localStorage.getItem(`abya:workbench:${taskId}`)??next.record?.workbench?.drafts.current??"");}}
    catch(e){if(alive.current)setError(errorMessage(e));}}
  useEffect(()=>{let valid=true;setReconciling(true);if(!productionApi.reconcile){setReconciling(false);return;}
    productionApi.reconcile(taskId).then(v=>{if(valid)accept(v);}).catch(e=>{if(valid)setError(errorMessage(e));}).finally(()=>{if(valid)setReconciling(false);});
    return()=>{valid=false;};},[taskId]);
  useEffect(()=>{alive.current=true;void refresh();const timer=setInterval(()=>{if(visible)void refresh();},3000);
    return()=>{alive.current=false;sequence.current++;clearInterval(timer);};},[taskId,visible]);
  useEffect(()=>{if(!view?.record?.workbench?.attempts?.some(a=>a.status==="reconciling"))return;
    let valid=true,pending=false;const timer=setInterval(()=>{if(!pending&&productionApi.reconcile){pending=true;productionApi.reconcile(taskId).then(v=>{if(valid)accept(v);}).catch(()=>{}).finally(()=>{pending=false;});}},5000);
    return()=>{valid=false;clearInterval(timer);};},[taskId,!!view?.record?.workbench?.attempts?.some(a=>a.status==="reconciling")]);
  useEffect(()=>{const show=(e:Event)=>{const d=(e as CustomEvent<{taskId:string;stage:string}>).detail;if(d.taskId===taskId){setStage(d.stage);setGoalId("");}};
    window.addEventListener("abya:show-stage",show);return()=>window.removeEventListener("abya:show-stage",show);},[taskId]);
  useEffect(()=>{if(props.requestedReview){setStage(props.requestedReview.kind);setGoalId("");props.onReviewOpened?.();}},[props.requestedReview]);
  useEffect(()=>{if(props.requestedFeedback){setMode("feedback");setFocusedFeedback(props.requestedFeedback.id);setGoalId("");props.onFeedbackOpened?.();}},[props.requestedFeedback]);
  const r=view?.record, currentStage=stage||r?.currentStage||"requirements";
  const goals=r?.workbench?.goals.filter(g=>g.stage===currentStage)??[];
  const goal=goals.find(g=>g.id===(goalId||r?.workbench?.activeGoalId));
  const historical=currentStage!==r?.currentStage||!!goal&&goal.id!==r?.workbench?.activeGoalId;
  const running=!!control&&["running","waitingForResponse","queued","pausing","stopFailed"].includes(control.state);
  useEffect(()=>{setSnapshot(undefined);if(!historical||!goal)return;let valid=true;
    productionApi.snapshot(taskId,goal.recordRevision).then(v=>{if(valid)setSnapshot(v);}).catch(e=>{if(valid)setError(errorMessage(e));});
    return()=>{valid=false;};},[taskId,goal?.recordRevision,goalId,historical]);
  const shown=historical&&goal?snapshot:r, doc=shown?.documents[currentStage];
  const pending=r?.questionGroups?.find(g=>g.status==="pending");
  async function run(action:()=>Promise<ProductionView>){if(locked.current)return;locked.current=true;setBusy(true);setError("");
    try{const next=await action();if(alive.current)accept(next);return next;}catch(e){if(alive.current)setError(errorMessage(e));}
    finally{locked.current=false;if(alive.current)setBusy(false);}}
  async function send(){if(!r||!draft.trim()||running)return;
    if(/^(继续|继续执行|继续此子流程|continue|resume)[。.!！\s]*$/i.test(draft.trim())){setNotice(en?"Use Resume this subprocess to keep the original goal.":"请点击‘继续此子流程’恢复当前目标；无需新建名为‘继续’的目标。");return;}
    const next=await run(()=>productionApi.workbench(taskId,r.revision,"request",{id:requestId,text:draft}));
    if(!next?.record)return;setDraft("");dirty.current=false;localStorage.removeItem(`abya:workbench:${taskId}`);setRequestId(crypto.randomUUID());setNotice(en?"Request saved.":"请求已保存，无需重复提交。");
    try{await onContinue?.("",undefined,next.record.revision);}catch(e){setError((en?"Saved; connection needs attention. ":"请求已保存，续接未完成：")+errorMessage(e));}}
  async function attach(file:File){
    if(file.size>32*1024*1024){setError(en?"File exceeds 32 MiB.":"附件超过32 MiB。");return;}
    await run(async()=>{const base64=await new Promise<string>((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(String(reader.result).split(",")[1]);reader.onerror=reject;reader.readAsDataURL(file);});return productionApi.workbench(taskId,r!.revision,"attach",{name:file.name,base64});});
  }
  function edit(text:string){dirty.current=true;setDraft(text);try{localStorage.setItem(`abya:workbench:${taskId}`,text);}catch{setError(en?"Local draft could not be saved.":"本地草稿未能保存，请使用保存草稿。");}}
  if(!r||!view)return <><p role="alert">{error}</p><ProductionPanel {...props}/></>;
  const name=(id:string)=>(en?english:stageLabels)[id]??id;
  const outcome=<><header className="wb-outcome-heading"><div><h2>{goal?.title??name(currentStage)}</h2><p>{historical?(en?"Historical snapshot · read only":"历史快照 · 只读"):(r.stageUpdates?.[currentStage]?.summary||(en?"Results will appear here as work progresses.":"这一阶段的成果将在这里集中呈现。"))}</p></div><button className="icon-button" title={en?"Expand result":"放大成果"} onClick={()=>setExpanded(true)}><Maximize2 size={18}/></button></header>
    {historical&&goal&&!snapshot?<p role="status">{en?"Loading history…":"正在读取历史…"}</p>:<>
      {historical&&goal&&<section><h3>{en?"Goal & result":"目标与结果"}</h3><ReadableText text={goal.summary}/></section>}
      {doc?<article className="wb-document"><ReadableText text={doc.content} taskId={taskId} basePath={doc.path} artifacts={((historical?shown?.artifacts:view.artifacts??r.artifacts)??[]).filter(a=>!historical||view.artifacts?.some(current=>current.id===a.id&&current.sha256===a.sha256))}/></article>:<div className="wb-empty"><FileText size={28}/><h3>{en?"Waiting for the first result":"等待本阶段的第一份成果"}</h3><p>{en?"You can prepare input below, or open the original conversation.":"可以先在下方整理想法，或前往原会话开始工作。"}</p><button className="secondary-button" onClick={props.onTerminal}>{en?"Open conversation":"前往原会话"}</button></div>}
      <VisualGallery taskId={taskId} artifacts={((historical?shown?.artifacts:view.artifacts??r.artifacts)??[]).filter(a=>a.stage===currentStage)} en={en}/>
      {goal?.decision&&<section><h3>{en?"Recorded decision":"当时的决定"}</h3><ReadableText text={goal.decision}/></section>}
    </>}</>;
  return <div className="production-workbench">
    <div className="wb-policy" role="status">
      <span>{en?"Task workflow":"本任务流程"} {r.workflowVersion} · {view.policy.implementationExecution?.mode==="autonomous"?(en?"Autonomous Lua execution":"Lua 自主执行"):(en?"Legacy execution rules":"仍使用旧执行规则")}</span>
      {view.availableUpdate&&<button className="text-button" disabled={!active||busy||running} onClick={()=>{
        if(window.confirm(en?"Pause this task first. Upgrade preserves IDs, results and approvals; it does not start execution.":"请先暂停本任务。升级保留原 ID、成果、批准和历史，同时更新任务内规则，不自动开始执行。继续升级？"))void run(()=>productionApi.upgrade(taskId,r.revision));
      }}>{en?"Upgrade task workflow":"升级本任务流程"}</button>}
    </div>
    <nav className="wb-stages" aria-label={en?"Production stages":"制作阶段"}>{view.policy.stages.map((s,i)=><button key={s.id} aria-current={currentStage===s.id?"step":undefined} onClick={()=>{setStage(s.id);setGoalId("");}}><span>{i+1}</span><strong>{name(s.id)}</strong><small>{s.id==="implementation"&&r.currentStage==="implementation"&&["settledWithGaps","settled"].includes(r.workbench?.execution?.state??"")?(en?"Execution ended":"本轮已结束"):statusName(view.stageStatuses?.[s.id]??r.stages[s.id]??"pending",locale)}</small></button>)}</nav>
    <div className="wb-body"><aside className="wb-goals"><header><h3>{en?"Stage goals":"本阶段子阶段"}</h3><History size={16}/></header>
      {goals.length===0?<p>{en?"Starts when this stage begins.":"进入本阶段后按目标建立。"}</p>:goals.map(g=><button key={g.id} className={(goalId||r.workbench?.activeGoalId)===g.id?"selected":""} onClick={()=>setGoalId(g.id)}><span>{g.number}</span><div><strong>{g.title}</strong><small>{en?goalExecutionStatus(r,g,reconciling):executionLabels[goalExecutionStatus(r,g,reconciling)]??statusName(g.status,locale)}</small></div><ChevronRight size={14}/></button>)}
      <button className="text-button" onClick={()=>{setStage(r.currentStage);setGoalId("");}}>{en?"Return to current work":"返回当前工作"}</button>
      <button className="text-button" onClick={()=>setDetails(true)}>{en?"All documents & development details":"全部资料与开发详情"}</button>
    </aside><WorkbenchSplit en={en}><section className="wb-outcome">{outcome}{!historical&&<WorkbenchIssues taskId={taskId} record={r} stage={currentStage} en={en} active={active&&!running} onUpdated={accept}/>}</section>
    <section className="wb-interaction" aria-label={en?"Task interaction":"任务交互"}>
      <AutonomousExecution record={r} en={en} control={control}/>
      {goal&&["work","request","feedback"].includes(goal.sourceType)&&<GoalExecutionPanel key={goal.id} record={r} goal={goal} active={active} checking={reconciling} en={en} onResume={props.onResumeGoal?async(...args)=>{const result=await props.onResumeGoal!(...args);await refresh();return result;}:undefined}/>}
      <div hidden={historical||mode!=="current"}><TaskIntakeDialog key={taskId} taskId={taskId} active={active} embedded executionBusy={running} requestedGroup={props.requestedGroup} onEnsureConnection={props.onEnsureConnection} onContinue={onContinue}/></div>
      {historical?<div className="wb-history-note"><p>{en?"History is read only. Your current draft is retained.":"当前正在查看历史；工作草稿仍保留，不会改写当时的决定。"}</p><button className="primary-button" onClick={()=>{setGoalId("");setStage(r.currentStage);}}>{en?"Return to current work":"回到当前工作"}</button></div>:<>
      <header><h3>{pending?(en?"Your answers":"需要你回答"):r.stages[currentStage]==="awaiting-confirmation"?(en?"Your decision":"需要你决定"):(en?"Continue this goal":"继续当前目标")}</h3><div><button className="text-button" onClick={()=>setMode(mode==="feedback"?"current":"feedback")}>{mode==="feedback"?(en?"Back to input":"返回输入"):(en?"Feedback & rechecks":"反馈与复验")}</button><button className="text-button" onClick={props.onTerminal}>{en?"Execution controls":"执行控制"}</button></div></header>
      {running&&<p role="status">{en?"AI is working. Drafts remain editable; submit after completion or pause in execution controls.":"AI 正在执行，可以继续写草稿；待执行结束，或在执行控制中暂停后提交。"}</p>}
      {mode==="feedback"?<FeedbackPanel taskId={taskId} record={r} active={active} embedded executionBusy={running} en={en} focusedId={focusedFeedback} actions={view.actions?.feedback} recheckActions={view.actions?.recheck} onUpdated={accept} onContinue={(m,rev)=>onContinue?.(m,undefined,rev)??Promise.resolve()}/>
      :pending?null
      :doc&&r.stages[currentStage]==="awaiting-confirmation"?<DocumentReview key={`${taskId}:${doc.kind}`} embedded taskId={taskId} document={doc} view={view} active={active&&!running} busy={busy} error={error} en={en} onClose={()=>setDetails(true)} onArtFeedback={()=>setMode("feedback")} onDecide={async(d,accepted,feedback)=>{
        const next=await run(()=>productionApi.decide(taskId,r.revision,d.kind,d.sha256,accepted,feedback));
        if(next?.record){setNotice(en?"Decision saved.":"决定已保存。");try{await onContinue?.("",undefined,next.record.revision);}catch(e){setError(errorMessage(e));}}
      }}/>:<><label className="field"><span>{en?"New goal or additional instructions":"新目标或补充说明"}</span><textarea rows={3} maxLength={8000} value={draft} onChange={e=>edit(e.target.value)} placeholder={en?"Describe what you want to achieve next…":"描述接下来想完成的目标，或补充本轮要求…"}/></label><label className="wb-attachment">{en?"Attach Word / Markdown ZIP / images":"添加 Word / Markdown ZIP / 图片"}<input type="file" accept=".docx,.md,.txt,.zip,.png,.jpg,.jpeg,.gif,.webp" disabled={busy||!active} onChange={e=>{const file=e.target.files?.[0];if(file)void attach(file);e.target.value="";}}/></label><div className="wb-compose-actions"><button className="secondary-button" disabled={!active||busy} onClick={()=>void run(()=>productionApi.workbench(taskId,r.revision,"save-draft",{key:"current",text:draft}))}>{en?"Save draft":"保存草稿"}</button><button className="primary-button" disabled={!active||busy||running||!draft.trim()} onClick={()=>void send()}>{en?"Submit and continue":"提交并继续"}</button></div></>}
      </>}
      {!!r.artifacts?.some(a=>a.plannerInput||a.plannerReference)&&<details className="wb-input-files"><summary>{en?"Attached source material":"已添加的需求资料与参考图"}</summary>{r.artifacts.filter(a=>(a.plannerInput||a.plannerReference)&&a.status!=="withdrawn").map(a=><p key={a.id}>{a.title} <button className="text-button" onClick={()=>void productionApi.open(taskId,a.path).catch(e=>setError(errorMessage(e)))}>{en?"View source / import manifest":"查看原图或导入清单"}</button></p>)}</details>}
      {notice&&<p role="status">{notice}</p>}{error&&<p role="alert" className="inline-error">{error}<button className="text-button" onClick={()=>void refresh()}>{en?"Refresh":"刷新"}</button></p>}
    </section></WorkbenchSplit></div>
    {details&&<Modal wide title={en?"Documents & development details":"全部资料与开发详情"} onClose={()=>setDetails(false)}><ProductionPanel {...props}/></Modal>}
    {expanded&&<Modal wide title={en?"Result":"当前成果"} onClose={()=>setExpanded(false)}>{outcome}<button className="secondary-button" onClick={()=>setExpanded(false)}><X size={16}/>{en?"Close":"关闭"}</button></Modal>}
  </div>;
}
