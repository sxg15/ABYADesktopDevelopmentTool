// Real components, disposable in-memory data. No native services or real planner approvals.
import React,{useState} from "react";
import {createRoot} from "react-dom/client";
import {ArtWorkflowPanel} from "../src/features/tasks/ArtWorkflowPanel";
import {artViewFixture,artCaseFixture} from "../src/shared/artDesign.test-fixtures";
import {artLibraryApi} from "../src/shared/artDesign";
import {productionApi} from "../src/shared/production";
import "../src/App.css";
const canvas=document.createElement("canvas");canvas.width=800;canvas.height=450;
const ctx=canvas.getContext("2d")!;
ctx.fillStyle="#f0eee5";ctx.fillRect(0,0,800,450);ctx.fillStyle="#273d35";ctx.font="24px sans-serif";
ctx.fillText("隔离测试图 · 用于验证组件交互",30,45);ctx.beginPath();ctx.arc(400,190,85,0,Math.PI*2);ctx.fill();
ctx.fillStyle="#ac643e";for(let i=0;i<5;i++)ctx.fillRect(285+i*50,340,12,60);
ctx.fillStyle="#273d35";ctx.font="18px sans-serif";ctx.fillText("剩余物品区域（示意）",295,425);
const image=canvas.toDataURL("image/png");
let view=artViewFixture();let cases=[artCaseFixture()];
productionApi.media=async()=>image;productionApi.revealDocument=async()=>{};productionApi.revealReport=async()=>{};
artLibraryApi.list=async(query,history)=>structuredClone(cases.filter(c=>(history||c.status==="accepted")&&JSON.stringify(c).includes(query)));
artLibraryApi.media=async()=>image;
artLibraryApi.action=async(_task,revision,operation,data)=>{
  if(revision!==view.record!.revision)throw new Error("隔离数据版本变化");
  const d=data as {id:string;draftId:string;satisfaction:string;reason:string};
  if(operation==="collect"){
    if(!d.satisfaction?.trim())throw new Error("需说明满意范围");
    const entry={...artCaseFixture(),id:"collected-fixture",satisfaction:d.satisfaction};cases=[...cases,entry];return entry;
  }
  if(operation==="withdraw"){const c=cases.find(c=>c.id===d.id)!;c.status="withdrawn";return structuredClone(c);}
  throw new Error("导入导出需原生环境；该预览不访问真实文件。");
};
productionApi.feedback=async(_task,revision,operation,data)=>{
  if(revision!==view.record!.revision)throw new Error("隔离数据版本变化");
  const d=data as {text:string;id:string;reason:string;kind:string};
  if(operation==="add-art-note"){
    const id=crypto.randomUUID();view.record!.artDesign!.sources.push({id,kind:"note",text:d.text,hash:id});
    view.artDesignStatus!.inputHash=id;view.artDesignStatus!.designHash=id;
  } else if(operation==="withdraw-art-requirement"){
    const q=view.record!.artDesign!.requirements.find(q=>q.id===d.id)!;q.withdrawn=true;q.withdrawReason=d.reason;
  } else if(operation==="save-art-requirement"){
    const q=view.record!.artDesign!.requirements.find(q=>q.id===d.id)!;Object.assign(q,d);
  } else throw new Error("仅支持隔离追踪交互");
  view.record!.revision++;return structuredClone(view);
};
function Preview(){const[current,setCurrent]=useState(structuredClone(view));const[notice,setNotice]=useState("");return <main style={{height:"100vh",overflow:"auto",background:"#f4f5f0",padding:"20px",boxSizing:"border-box"}}>
  <div style={{maxWidth:1220,margin:"0 auto"}}><header style={{marginBottom:18}}><h2>美术设计流程 · 隔离验证</h2><p>使用真实 APP 组件与模拟数据；不代表游戏美术成果或实际策划认可。</p><p role="status">{notice}</p></header>
  <ArtWorkflowPanel taskId="art-design-test" view={current} active onUpdated={setCurrent} onReview={()=>setNotice("此处仅验证交互，未批准真实方案。")}
    onContinue={async()=>{setNotice("已验证 AI 请求入口；未启动真实会话。");}}/>
  </div></main>;}
createRoot(document.getElementById("root")!).render(<Preview/>);
