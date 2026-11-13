import {useEffect,useState,type ReactNode} from "react";
import {
  ArrowRight, ChevronRight, CircleHelp, ClipboardCheck, FilePenLine, FileText,
  History, Info, MapPin, MessageSquare, RotateCcw, TriangleAlert, Unplug, Workflow,
} from "lucide-react";
import "./taskGuidance.css";
import "./workbench.css";
import {productionApi,type ProductionRecord,type ProductionView} from "../../shared/production";
import type {DevelopmentTask} from "../../shared/types";
import {errorMessage} from "../../shared/api";
import {ReadableText} from "./ReadableText";
import {VisualGallery} from "./VisualGallery";
import {briefPrompt,openTaskChat,chatStatus,copyAndOpenTaskChat} from "./desktopChat";
import {StageCard} from "./StageCard";
import {statusName} from "./productionText";

const names:Record<string,string>={requirements:"需求整理",resources:"美术需求整理",plan:"方案预检",implementation:"Lua 实现",review:"试玩与修改",delivery:"交付验收",closeout:"收尾复盘"};
export function StageReader({task,stage,selectStage,notify,visible,children}:{task:DevelopmentTask;stage:string;selectStage:(id:string)=>void;notify:(m:string,error?:boolean)=>void;visible:boolean;children:(record:ProductionRecord)=>ReactNode}){
  const [connection,setConnection]=useState<{ready:boolean;bound:boolean}>();
  const [view,setView]=useState<ProductionView>(),[error,setError]=useState("");
  const [goalId,setGoal]=useState(""),[snapshot,setSnapshot]=useState<ProductionRecord>();
  useEffect(()=>{
    if(!visible)return;let live=true,pending=false;
    async function refresh(){if(pending)return;pending=true;try{const [v,conn]=await Promise.all([productionApi.get(task.id),chatStatus(task.id)]);if(live){setConnection(conn);setView(v);setError("");}}
      catch(e){if(live)setError(errorMessage(e));}finally{pending=false;}}
    void refresh();const timer=setInterval(refresh,3000);return()=>{live=false;clearInterval(timer);};
  },[task.id,visible]);
  const record=view?.record,current=record?.currentStage??"requirements",shown=stage||current;
  const goals=record?.workbench?.goals.filter(g=>g.stage===shown)??[];
  const goal=goals.find(g=>g.id===goalId),historical=!!goal&&goal.id!==record?.workbench?.activeGoalId;
  useEffect(()=>{let live=true;setSnapshot(undefined);
    if(goal&&historical)productionApi.snapshot(task.id,goal.recordRevision).then(s=>{if(live)setSnapshot(s);}).catch(e=>{if(live)setError(errorMessage(e));});
    return()=>{live=false;};},[goal?.id,goal?.recordRevision,historical,task.id]);
  const result=historical?snapshot:record,doc=result?.documents[shown];
  const [draft,setDraft]=useState<{path:string;hash:string;text:string}>();
  useEffect(()=>{
    setDraft(undefined);if(!doc||historical||!visible)return;
    let live=true,reading=false;
    const refresh=async()=>{if(reading)return;reading=true;
      try{const text=await productionApi.source(task.id,doc.kind);if(live)setDraft(text===null?undefined:{path:doc.path,hash:doc.sha256,text});}
      catch(e){if(live)setError(errorMessage(e));}finally{reading=false;}};
    void refresh();const timer=setInterval(refresh,3000);return()=>{live=false;clearInterval(timer);};
  },[task.id,doc?.kind,doc?.sha256,doc?.path,historical,visible]);
  const edited=doc&&draft&&draft.path===doc.path&&draft.hash===doc.sha256?draft.text:undefined;
  const issues=result?.issues.filter(i=>i.status!=="resolved"&&(i.stage===shown||i.owningStage===shown||i.affectedStages?.includes(shown)))??[];
  const artifacts=(historical?result?.artifacts:view?.artifacts)??[];
  const review=record?.stages[current]==="awaiting-confirmation"?record.documents[current]:undefined;
  const blocked=review&&(view?.actions?.approve?.[current]||view?.actions?.review?.[current]);
  const start=!record?.documents.requirements&&current==="requirements";
  const preflight=result?.workbench?.execution?.capabilityPreflight;
  const tests:Record<string,string>={"runtime-connect":"连接游戏客户端","create-availability":"检查游戏能否新建存档","create-archive":"新建测试存档","save-archive":"保存测试关卡","cold-load":"重新打开保存的关卡"};
  const failed=record?.workbench?.execution?.capabilityPreflight;
  const needsRepair=current==="plan"&&failed?.state==="failed";
  const preflightTitle=(f:typeof preflight)=>f?.phase==="create-availability"?"当前游戏客户端未能确认支持新建存档":(tests[f?.phase??""]??"检查游戏环境")+"时遇到了问题";
  const instruction=needsRepair?"请在下方查看没通过的检查。你可以在 Codex 中要求修复，或指定使用已有存档重新验证。":blocked|| (start?"在 Codex 中提供已有定稿，或写下玩法想法并附上参考图。":review?"请审阅"+names[current]+"第 "+review.revision+" 版。在 Codex 中确认这一版或直接指出要改的地方。":"回到任务对话继续制作，或告诉 Codex 要修复的具体问题。");
  const luaGuide="在已有授权内执行到可试玩包，实测输入、结算和重开后交接版本与入口。遇阻先查当前调用、字段契约和同范围回执，能修就验证并继续。若本次仅要求局部排查，按指定范围收尾。";
  const repairGuide="先核对当前版本、实际路线及最近回执，验证此问题是否仍影响下一步。修正调用或复用同范围成功证据，按实际结果更新问题记录。缺少必要权限时只申请该项。";
  const prompt=needsRepair?repairGuide:blocked?"请根据任务服务列出的待处理项修订当前成果，并说明需要我决定的事项。":start?briefPrompt:review?"我已审阅并确认「"+names[current]+"」第 "+review.revision+" 版（"+review.sha256+"），请按项目流程继续。"+(current==="plan"?luaGuide:""):current==="implementation"?luaGuide:"请读取当前任务的实际进度，继续当前阶段；如果需要我确认，请先展示对应成果。";
  async function open(copy:boolean){try{const opened=copy?await copyAndOpenTaskChat(task,prompt):await openTaskChat(task.id);if(opened&&copy)notify("提示词已复制，请在任务对话中粘贴并发送。");}catch(e){notify(errorMessage(e),true);}}
  const disconnected = connection?.ready === false;
  const attention = needsRepair || !!blocked;
  const Icon = disconnected ? Unplug : attention ? TriangleAlert : review ? ClipboardCheck : start ? FilePenLine : CircleHelp;
  const guidanceTitle = disconnected ? "开发环境已断开" : needsRepair ? "预检需要处理" : blocked ? "审阅前有待处理项" : review ? "等待审阅" : start ? "准备玩法文档" : "当前工作";
  const tone = disconnected || attention ? "error" : review || start ? "human" : "idle";
  const copyLabel = needsRepair ? "复制修复指令" : blocked ? "复制处理指令" : start ? "复制开场提示词" : review ? "复制确认指令" : "复制继续指令";
  const connectionText = connection
    ? (connection.ready ? "开发环境就绪" : "开发环境连接已断开") + (connection.bound ? " · 已关联任务对话" : " · 等待对话连接任务")
    : "正在读取任务环境";

  return <><div className="task-guidance-shell">
    <section className={"task-guidance desktop-guide tone-" + tone} aria-label="当前操作指引">
      <div className="guidance-main" role="status">
        <span className="guidance-icon"><Icon size={20} aria-hidden="true"/></span>
        <div className="guidance-copy">
          <div className="guidance-eyebrow"><strong>任务状态</strong><span>{names[current] ?? current}</span></div>
          <h2>{guidanceTitle}</h2><p>{instruction}</p>
        </div>
      </div>
      <div className="guidance-action">
        <span className="guidance-destination"><MapPin size={14}/> 操作位置：Codex 任务对话</span>
        {task.status === "active"
          ? <button className="guidance-primary" onClick={()=>void open(true)} aria-label={copyLabel+"并打开 Codex"} title={copyLabel+"并打开任务对话"}>复制并前往 Codex <ArrowRight size={15}/></button>
          : <button className="guidance-secondary" onClick={()=>void open(false)}>打开任务对话 <ArrowRight size={15}/></button>}
      </div>
      <div className="guidance-detail"><Info size={14}/><span>{connectionText}</span>
        {task.status === "active" && <button className="text-button" onClick={()=>void open(false)}><MessageSquare size={13}/> 打开任务对话</button>}
      </div>
    </section></div>
    {error&&<p role="alert" className="inline-error">{error}</p>}
    {view&&!record&&<p className="reading-sheet">此任务按需制作，可直接在 Codex 中处理具体问题。</p>}
    {record&&view&&<><header className="desktop-flow-heading">
      <h3><Workflow size={17} aria-hidden="true"/> 制作流程</h3>
      <small>本任务流程 {record.workflowVersion}</small>
    </header>
    <nav className="desktop-stages" aria-label="制作阶段">
      {view.policy.stages.map((s,i)=><StageCard key={s.id} number={i+1} name={names[s.id]??s.name}
        status={view.stageStatuses?.[s.id]??record.stages[s.id]??"not-started"}
        current={current===s.id} viewed={shown===s.id}
        onView={()=>{selectStage(s.id===current?"":s.id);setGoal("");}}/>)}
    </nav>
    <div className="desktop-stage-body"><aside className="wb-goals" aria-label="本阶段子阶段">
      <header><h3>本阶段子阶段</h3><History size={15} aria-hidden="true"/></header>
      {goals.map(g=>{
        const selected=g.id===(goalId||record.workbench?.activeGoalId);
        return <button key={g.id} className={selected?"selected":undefined} aria-pressed={selected} onClick={()=>setGoal(g.id)}>
          <span className="desktop-goal-number">{g.number}</span><div><strong>{g.title}</strong><small>{statusName(g.status)}</small></div>
          <ChevronRight size={14} aria-hidden="true"/>
        </button>;
      })}
      {goals.length===0&&<p>进入此阶段后显示子阶段。</p>}
      {(shown!==current||goalId)&&<button className="text-button" onClick={()=>{selectStage("");setGoal("");}}><RotateCcw size={13}/> 返回当前工作</button>}
    </aside>
      <section className="reading-sheet"><header><h2><FileText size={18} aria-hidden="true"/>{goal?.title??names[shown]}</h2>{doc&&<small>第 {doc.revision} 版</small>}</header>
      {historical&&!snapshot?<p>正在读取子阶段成果…</p>:<>
      {shown==="plan"&&preflight?.state==="failed"&&issues.length===0&&<article className="planner-issue"><h3>{preflightTitle(preflight)}</h3><p>这次检查没有完成。请在任务对话中让 Codex 定位问题并重新验证。</p><details><summary>检查结果</summary>{preflight.error?.message}</details></article>}
      {issues.map(i=><article className="planner-issue" key={i.id}><h3>{i.playerSummary||(i.id.startsWith("preflight:")&&preflight?.state==="failed"?preflightTitle(preflight):i.title||i.description)}</h3>{i.playerSummary&&i.description!==i.playerSummary&&<details><summary>查看问题详情</summary>{i.description}</details>}
        <p>{i.id.startsWith("preflight:")&&preflight?.state==="failed"?"此项通过后，才能验证这套方案能否在游戏里运行。可复制问题到 Codex 继续处理。":i.executionHandling?.impact||i.resumeWhen||i.fix||"可在任务对话中请 Codex 修复并重新检查。"}</p>
        <button className="text-button" onClick={()=>void copyAndOpenTaskChat(task,"请定位并修复问题 "+i.id+"："+(i.playerSummary||i.description)+"。"+repairGuide+(current==="implementation"?"修复后继续该阶段已授权的工作。":"")).catch(e=>notify(errorMessage(e),true))}>复制修复指令并打开 Codex</button></article>)}
      {doc?<>{edited!==undefined&&<p role="status">源文档已有人工修改。请在 Codex 中提交新版后再确认。</p>}<ReadableText text={edited??doc.content} taskId={task.id} basePath={doc.path} artifacts={artifacts}/>
        {!historical&&<button className="text-button" onClick={()=>void productionApi.open(task.id,doc.path).catch(e=>notify(errorMessage(e),true))}>打开 Markdown 源文件</button>}</>:<p>{goal?.summary||result?.stageUpdates?.[shown]?.summary||(shown===current?"本阶段成果会在这里显示。":"这一阶段还没有文档。")}</p>}
      <VisualGallery taskId={task.id} artifacts={artifacts.filter(a=>a.stage===shown&&(!historical||view.artifacts?.some(now=>now.id===a.id&&now.sha256===a.sha256)))} en={false}/>
      {!historical&&["implementation","review"].includes(shown)&&record&&children(record)}</>}
      </section></div></>}</>;
}
