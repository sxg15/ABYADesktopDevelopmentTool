// Isolated UI fixture: no native task writes, exports or provider sessions.
import React from "react";
import { createRoot } from "react-dom/client";
import { ProductionPanel } from "../src/features/tasks/ProductionPanel";
import { TaskGuidance } from "../src/features/tasks/TaskGuidance";
import { productionApi, type ProductionView } from "../src/shared/production";
import { attentionApi } from "../src/shared/notifications";
import "../src/App.css";

const view: ProductionView = {
  policy:{workflowVersion:"2.1.0",iterationMode:"human-feedback",stages:[{id:"implementation",name:"Lua 实现与完整初版",approval:null}],dimensions:[],questions:[]},
  warnings:[],reportPaths:[],availableUpdate:false,
  record:{taskId:"fixture",revision:12,workflowVersion:"2.1.0",questionMode:"no-followup",currentStage:"implementation",currentRound:0,cycle:1,
    stages:{implementation:"blocked"},documents:{},approvals:[],versionDetails:{playerHash:"fixture-build-hash"},issues:[
      {id:"source-input",title:"新版 Player 未派发输入回调",kind:"blocker",status:"open",stage:"implementation",repairTarget:"abya-source",description:"点击棋盘后未收到输入事件，独立客户端与 Host 均可复现。",resumeWhen:"程序修复输入派发后提供新构建包，复验双端点击与回合切换。"},
      {id:"build-resource",title:"构建包缺少字体资源",kind:"defect",status:"open",stage:"implementation",repairTarget:"abya-build",description:"中文提示文本无法显示。",fix:"检查 Addressables 字体打包配置。"},
      {id:"lua-ordinary",title:"普通 Lua 文案问题",kind:"defect",status:"open",stage:"implementation",repairTarget:"lua",description:"规则说明文案需要调整。"},
    ],evidence:[],rounds:[],milestones:{},knowledge:[],skillPins:[],updatedAt:"2026-10-15T09:00:00Z"},
};
productionApi.get = async()=>structuredClone(view);
productionApi.skills = async()=>[];
productionApi.timing = async()=>({note:"隔离界面预览"}) as Awaited<ReturnType<typeof productionApi.timing>>;
attentionApi.get = async()=>({taskId:"fixture",items:[],unavailable:false});
productionApi.exportProgrammerIssues = async(taskId,revision,ids)=>{
  if(taskId!=="fixture" || revision!==12 || ids.some(id=>id==="lua-ordinary")) throw new Error("Unexpected export selection");
  document.getElementById("export-result")!.textContent=`已模拟导出：${ids.join("、")}`;
  return "artifacts/game-development/reports/programmer-issues-r12-preview.md";
};
productionApi.revealDocument = async()=>{};
createRoot(document.getElementById("root")!).render(<main style={{padding:24,maxWidth:1400,margin:"auto"}}>
  <p>隔离界面预览 · 导出仅模拟，不访问用户任务</p><p id="export-result"/>
  <TaskGuidance taskId="fixture" active onStage={()=>{}} onTerminal={()=>{}}/>
  <ProductionPanel taskId="fixture" active/>
</main>);
