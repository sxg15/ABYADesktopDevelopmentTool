// Real ProductionPanel with explicitly isolated in-memory art workflow fixtures.
import React from "react";
import {createRoot} from "react-dom/client";
import {ProductionPanel} from "../src/features/tasks/ProductionPanel";
import {productionApi, type ProductionView} from "../src/shared/production";
import "../src/App.css";
const images=new Map<string,string>();
function image(id:string,version:number,dynamic=false){
  const c=document.createElement("canvas");c.width=720;c.height=400;const x=c.getContext("2d")!;
  x.fillStyle=version===4?"#eff4e7":"#f1f0eb";x.fillRect(0,0,720,400);
  x.fillStyle="#486a61";x.font="bold 28px sans-serif";x.fillText(`见缝插针 · ${dynamic?"状态示意":"布局方案"} v${version}`,28,45);
  x.fillStyle="#fffdf4";x.fillRect(28,70,435,270);x.fillStyle="#354a5c";x.beginPath();x.arc(245,195,65,0,Math.PI*2);x.fill();
  x.strokeStyle="#354a5c";x.lineWidth=5;for(let i=0;i<7;i++){const a=i*Math.PI*2/7;x.beginPath();x.moveTo(245+Math.cos(a)*64,195+Math.sin(a)*64);x.lineTo(245+Math.cos(a)*105,195+Math.sin(a)*105);x.stroke();}
  x.fillStyle="#dce9dc";x.fillRect(490,70,200,270);x.fillStyle="#385449";x.font="22px sans-serif";
  ["得分 80","时间 20秒",dynamic?"待发针 3 → 2":"待发针 3","排名 1 / 4"].forEach((s,i)=>x.fillText(s,510,112+i*60));
  x.font="16px sans-serif";x.fillText("设计示意 · 非游戏实拍 · 隔离测试图片",28,377);images.set(id,c.toDataURL("image/png"));
}
const view:ProductionView={policy:{workflowVersion:"2.1.0",iterationMode:"human-feedback",stages:[{id:"requirements",name:"需求整理",approval:"requirements"},{id:"resources",name:"美术需求与资源确认",approval:"resources"},{id:"plan",name:"方案预检",approval:"plan"}],dimensions:[],questions:[]},
  warnings:[],reportPaths:[],availableUpdate:false,actions:{review:{resources:""},approve:{resources:""},feedback:{},recheck:{}},
  record:{taskId:"art-flow",revision:10,workflowVersion:"2.1.0",questionMode:"ask",currentStage:"resources",currentRound:0,cycle:1,stages:{requirements:"passed",resources:"awaiting-confirmation",plan:"not-started"},
    documents:{resources:{kind:"resources",revision:4,path:"art-requirements-v4.md",sha256:"document-4",content:"# 见缝插针美术需求确认单\n## 本次修订\n1. 布局：独立游戏面板与右侧信息区。\n2. 交互：移除库存文字，以针的数量表示待发针。\n## 验收标准\n**请逐项复验已有修改意见，再确认本版方案。**",submittedAt:"now",artifactBindings:{"layout-4.png":"hash-layout-4","states-4.png":"hash-states-4"}}},
    approvals:[],versionDetails:{},issues:[],evidence:[],rounds:[],milestones:{},knowledge:[],skillPins:[],updatedAt:"now",artifacts:[],feedback:[]}};
for(const group of ["layout","states"])for(const version of [1,2,3,4]){
  const id=`${group}-${version}`;image(id,version,group==="states");view.record!.artifacts!.push({id,path:`${id}.png`,title:`${group==="layout"?"游戏布局":"交互状态"} · v${version}`,summary:version===4?"独立游戏区、清晰计分与待发针反馈。":"此前方案，仅供历史对照。",stage:"resources",status:"draft",cycle:1,groupId:group,visualVersion:`art-v${version}`,mediaType:"image/png",sourceType:"mockup",sha256:`hash-${id}`});
}
view.record!.feedback=[{id:"spacing",description:"游戏区与右侧计分栏需要分离",stage:"resources",artifactId:"layout-1",candidateArtifactId:"layout-4",status:"awaiting-recheck",fix:"已采用独立面板，并把得分与计时移至右侧。",recheck:"请对照最新布局图，确认分区与文字可读。",createdAt:"now",updatedAt:"now"}];
function snapshot(){const result=structuredClone(view),r=result.record!;const pending=r.feedback!.filter(f=>f.status!=="resolved");
  result.actions!.review.resources=r.stages.resources==="awaiting-confirmation"?"":"此版本当前不处于待审查状态。";
  result.actions!.approve.resources=pending.length?"还有 1 条美术反馈未关闭，请先进入修改与复验。":"";
  for(const f of r.feedback!)result.actions!.recheck![f.id]=f.status==="open"?"AI 尚未提交修复结果，请等待修改。":f.candidateArtifactId!=="layout-4"?"这条反馈尚未关联最新视觉修订，请让 AI 更新复验结果后再处理。":"";
  for(const a of r.artifacts!)result.actions!.feedback[a.id]=a.visualVersion==="art-v4"?"":"历史版本不可反馈";
  return result;
}
productionApi.get=async()=>snapshot();productionApi.media=async(_task,id)=>images.get(id)!;
productionApi.feedback=async(_task,revision,operation,data)=>{
  if(revision!==view.record!.revision)throw new Error("请刷新隔离测试数据");const d=data as {id:string;description:string;artifactId:string;name:string;base64:string;summary:string;reason?:string};
  if(operation==="resolve")view.record!.feedback!.find(f=>f.id===d.id)!.status="resolved";
  else if(operation==="reopen"){const f=view.record!.feedback!.find(f=>f.id===d.id)!;f.status="open";f.reopenReason=d.reason;view.record!.stages.resources="in-progress";}
  else if(operation==="attach-art-reference"){const id=crypto.randomUUID();images.set(id,`data:image/png;base64,${d.base64}`);view.record!.artifacts!.push({id,path:id+".png",title:d.name,summary:d.summary,stage:"resources",status:"reference",plannerReference:true,mediaType:"image/png"});}
  else if(operation==="remove-art-reference")view.record!.artifacts!.find(a=>a.id===d.id)!.status="withdrawn";
  else throw new Error("此预览仅验证复验与确认；不提交真实反馈");
  view.record!.revision++;return snapshot();
};
productionApi.decide=async(_task,revision,kind,hash,accepted,feedback)=>{
  if(revision!==view.record!.revision||hash!=="document-4")throw new Error("版本变化");
  if(accepted&&view.record!.feedback!.some(f=>f.status!=="resolved"))throw new Error("反馈尚未关闭");
  view.record!.approvals.push({kind,documentRevision:4,documentHash:hash,decision:accepted?"accepted":"rejected",feedback,decidedAt:new Date().toISOString()});
  view.record!.stages.resources=accepted?"passed":"in-progress";if(accepted){view.record!.currentStage="plan";view.record!.stages.plan="in-progress";}view.record!.revision++;document.getElementById("fixture-result")!.textContent=accepted?"测试：仅在明确确认后保存方案决定":"测试：已记录修改意见";return snapshot();
};
productionApi.revealDocument=async()=>{};productionApi.revealReport=async()=>{};
function scenario(type:string){const f=view.record!.feedback![0];f.status=type==="ready"?"resolved":type==="ai"?"open":"awaiting-recheck";f.candidateArtifactId=type==="stale"?"layout-2":"layout-4";view.record!.revision++;window.dispatchEvent(new Event("focus"));}
createRoot(document.getElementById("root")!).render(<main className="tasks-workspace" style={{display:"block",height:"100%"}}><header style={{padding:"8px 20px",height:45,display:"flex",alignItems:"center",gap:12}}><strong>美术协作 · 隔离测试</strong>{[["review","待复验"],["ai","待 AI 修改"],["stale","复验材料过期"],["ready","可确认"]].map(([id,label])=><button key={id} onClick={()=>scenario(id)}>{label}</button>)}<small id="fixture-result"/></header>
  <div className="content-pane" style={{height:"calc(100% - 45px)"}}><ProductionPanel taskId="art-flow" active locale="zh-CN" onContinue={async()=>{}}/></div></main>);
