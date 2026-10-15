// Isolated real-component fixture; never connects to native tasks or providers.
import React from "react";
import { createRoot } from "react-dom/client";
import { ProductionPanel } from "../src/features/tasks/ProductionPanel";
import { productionApi, type ProductionView } from "../src/shared/production";
import "../src/App.css";
const canvas=document.createElement("canvas");canvas.width=720;canvas.height=360;
const ctx=canvas.getContext("2d")!;ctx.fillStyle="#ecf5ed";ctx.fillRect(0,0,720,360);
ctx.fillStyle="#286b60";ctx.font="bold 32px sans-serif";ctx.fillText("参考配色 · 棋盘界面",32,54);
for(let i=0;i<12;i++){ctx.fillStyle=["#d6e7c5","#f5ca72","#e38769","#6ba99d"][i%4];ctx.fillRect(32+i%6*112,90+Math.floor(i/6)*112,92,92);}
const image=canvas.toDataURL("image/png");
const images=new Map<string,string>();
const view:ProductionView={policy:{workflowVersion:"2.1.0",iterationMode:"human-feedback",stages:[
  {id:"requirements",name:"需求整理",approval:"requirements"},{id:"resources",name:"美术需求与资源确认",approval:"resources"},
  {id:"plan",name:"方案预检",approval:"plan"},{id:"implementation",name:"Lua 实现",approval:null},{id:"review",name:"试玩修改",approval:null}],dimensions:[],questions:[]},
  warnings:[],reportPaths:[],availableUpdate:false,actions:{review:{},approve:{},feedback:{}},
  record:{taskId:"art-fixture",revision:2,workflowVersion:"2.1.0",questionMode:"ask",currentStage:"resources",currentRound:0,cycle:1,
    stages:{requirements:"passed",resources:"in-progress",plan:"not-started",implementation:"not-started",review:"not-started"},
    documents:{},approvals:[],versionDetails:{},issues:[],evidence:[],rounds:[],milestones:{},knowledge:[],skillPins:[],updatedAt:"now",artifacts:[]}};
productionApi.get=async()=>structuredClone(view);
productionApi.media=async(_task,id)=>images.get(id)!;
productionApi.feedback=async(_task,revision,operation,data)=>{
  if(revision!==view.record!.revision)throw new Error("隔离数据版本已变化，请刷新");
  const d=data as {name:string;base64:string;summary:string;id:string};
  if(operation==="attach-art-reference"){
    const id=crypto.randomUUID();images.set(id,`data:image/png;base64,${d.base64}`);
    view.record!.artifacts!.push({id,path:`artifacts/art-references/${id}.png`,title:d.name,summary:d.summary,stage:"resources",status:"reference",mediaType:"image/png",sourceType:"reference",plannerReference:true});
  }else if(operation==="remove-art-reference")view.record!.artifacts!.find(a=>a.id===d.id)!.status="withdrawn";
  else throw new Error("预览不执行真实反馈或验收");
  view.record!.revision++;return structuredClone(view);
};
function simulatePaste(){
  const area=document.querySelector<HTMLElement>('.art-reference-paste')!;
  const bytes=Uint8Array.from(atob(image.split(',')[1]),c=>c.charCodeAt(0));
  const clipboard=new DataTransfer();clipboard.items.add(new File([bytes],"棋盘参考.png",{type:"image/png"}));
  area.focus();area.dispatchEvent(new ClipboardEvent("paste",{clipboardData:clipboard,bubbles:true,cancelable:true}));
}
createRoot(document.getElementById("root")!).render(<main style={{maxWidth:1100,margin:"auto",padding:24}}>
  <header style={{display:"flex",gap:20,alignItems:"center",flexWrap:"wrap"}}><h2>美术节点 · 交互验证</h2>
    <button onClick={simulatePaste}>模拟截图粘贴</button><small>隔离测试数据，不操作真实任务</small></header>
  <ProductionPanel taskId="art-fixture" active locale="zh-CN" onContinue={async()=>{document.getElementById("fixture-result")!.textContent="已接收到明确的继续操作（测试，无 AI 调用）";}}/>
  <p id="fixture-result" role="status" />
</main>);
