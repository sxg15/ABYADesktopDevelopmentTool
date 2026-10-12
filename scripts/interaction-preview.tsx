// Isolated browser fixtures. No Tauri calls, native conversations or user data.
import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { Modal } from "../src/app/Modal";
import { IntakeQuestions } from "../src/features/tasks/IntakeQuestions";
import { DocumentReview } from "../src/features/tasks/DocumentReview";
import { VisualGallery } from "../src/features/tasks/VisualGallery";
import { TaskGuidance } from "../src/features/tasks/TaskGuidance";
import { productionApi, type ProductionView, type IntakeQuestionGroup, type TaskControlState } from "../src/shared/production";
import "../src/App.css";
import "../src/features/tasks/production.css";
const art={id:"image",path:"artifacts/concept.png",title:"桌游界面概念图",stage:"resources",status:"draft",mediaType:"image/png",sourceType:"mockup",visualVersion:"v1",sha256:"image-hash",summary:"用于验证缩略图入口、放大和返回审查窗口。"};
const doc={kind:"resources",revision:1,path:"artifacts/art-requirements.md",sha256:"doc-hash",submittedAt:"2026-10-12T10:00:00Z",artifactBindings:{[art.path]:art.sha256},
  content:"# 美术需求确认单\n## 整体风格\n以温暖、清晰的桌游界面为主要方向，优先复用现有棋盘与按钮资源。\n## 资源范围\n棋盘 1 套、玩家标识 4 组、行动按钮及结算面板。\n## 制作约束\n支持 1280×720 及以上窗口；文字与背景保持清晰对比。\n## 验收标准\n当前玩家、操作入口、得分和结算结果均可直接识别。"};
const group:IntakeQuestionGroup={id:"fixture",title:"玩法与人数选择",provider:"codex",conversationId:"fixture",status:"pending",publishedAt:"now",updatedAt:"now",draft:{},answers:[],questions:[
  {id:"players",text:"一局希望支持多少人、采用什么关系？",options:["2 人联机竞技（推荐）","2～4 人联机竞技","2 人合作接力"],optional:false},
  {id:"rules",text:"希望采用什么计分规则？",options:[],optional:false}]};
let view:ProductionView={policy:{workflowVersion:"2.1.0",stages:[{id:"resources",name:"美术需求",approval:"resources"}],dimensions:[],questions:[]},
  actions:{review:{resources:""},approve:{resources:""},feedback:{gameplay:"尚无作品",image:""}},artifacts:[art],warnings:[],reportPaths:[],availableUpdate:false,
  record:{taskId:"浏览器测试任务",revision:1,workflowVersion:"2.1.0",questionMode:"ask",currentStage:"resources",currentRound:0,cycle:1,stages:{resources:"awaiting-confirmation"},documents:{resources:doc},approvals:[],versionDetails:{},issues:[],evidence:[],rounds:[],milestones:{},knowledge:[],skillPins:[],updatedAt:"now"}};
productionApi.get=async()=>structuredClone(view);
productionApi.answer=async(_id,_revision,_operation,data)=>{ await new Promise(r=>setTimeout(r,900)); view={...view,record:{...view.record!,revision:view.record!.revision+1}}; return view; };
productionApi.media=async()=>"data:image/svg+xml,"+encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="900" height="560"><rect width="900" height="560" fill="#f5ead5"/><rect x="160" y="80" width="580" height="390" rx="24" fill="#d0b082"/><path d="M170 180H730M170 280H730M170 380H730M300 90V460M450 90V460M600 90V460" stroke="#f5ead5" stroke-width="4"/><circle cx="360" cy="220" r="30" fill="#297366"/><circle cx="570" cy="335" r="30" fill="#b8684e"/><text x="30" y="40" font-size="22">概念预览 · 交互测试示意</text></svg>');
function App(){const[mode,setMode]=useState("home"),[current,setCurrent]=useState(view),[notice,setNotice]=useState("");
 return <main style={{padding:24,maxWidth:1100,margin:"auto",fontFamily:"Segoe UI, Microsoft YaHei, sans-serif"}}>
  <h1>交互回归预览</h1><p>独立测试数据 · 不连接真实任务</p><div className="production-actions">
  <button onClick={()=>setMode("questions")}>阶段问答</button><button onClick={()=>setMode("review")}>美术确认单</button><button onClick={()=>setMode("gallery")}>概念图</button></div>
  <TaskGuidance taskId="fixture" active control={{state:"running"} as TaskControlState} onStage={()=>setMode("review")} onTerminal={()=>setNotice("前往终端")} />
  {notice&&<p role="status">{notice}</p>}
  {mode==="questions"&&<Modal wide title="阶段问答" onClose={()=>setMode("home")}><IntakeQuestions taskId="browser-fixture" group={group} revision={current.record!.revision} active onSaved={setCurrent}
    onSubmit={async()=>{setMode("home");setNotice("答案已提交，AI 正在继续处理。");return current;}} onDefer={()=>setMode("home")}/></Modal>}
  {mode==="review"&&<DocumentReview taskId="fixture" document={doc} view={current} active busy={false} error="" onClose={()=>setMode("home")} onDecide={async()=>{setMode("home");setNotice("此版本已确认。");}}/>}
  {mode==="gallery"&&<VisualGallery taskId="fixture" artifacts={[art]} />}
 </main>;
}
createRoot(document.getElementById("root")!).render(<App/>);
