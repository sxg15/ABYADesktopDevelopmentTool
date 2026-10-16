// Real TasksView with isolated browser fixtures; no task or terminal is created.
import React, {useState} from "react";
import {createRoot} from "react-dom/client";
import {TasksView} from "../src/features/tasks/TasksView";
import {api, terminalApi} from "../src/shared/api";
import {productionApi, type ProductionView, type TaskControlState} from "../src/shared/production";
import {translator} from "../src/i18n";
import type {AppSettings} from "../src/shared/types";
import "../src/App.css";
const doc={kind:"resources",revision:2,path:"artifacts/art-requirements.md",sha256:"fixture-hash",submittedAt:"2026-10-12T10:00:00Z",content:"# 美术需求确认单\n## 整体风格\n清晰、轻量的桌游界面。\n## 资源范围\n复用棋盘，新增玩家标记与结算面板。\n## 验收标准\n当前玩家、操作入口和得分变化均可直接识别。"};
const view:ProductionView={policy:{workflowVersion:"2.1.0",iterationMode:"human-feedback",stages:[{id:"requirements",name:"需求整理",approval:"requirements"},{id:"resources",name:"美术需求与资源确认",approval:"resources"},{id:"plan",name:"方案预检",approval:"plan"},{id:"implementation",name:"Lua 实现",approval:null},{id:"review",name:"试玩修改",approval:null},{id:"delivery",name:"交付验收",approval:"delivery"},{id:"closeout",name:"收尾复盘",approval:null}],dimensions:[],questions:[]},
  actions:{review:{resources:""},approve:{resources:""},feedback:{gameplay:"尚无可验收作品"}},artifacts:[],warnings:[],reportPaths:[],availableUpdate:false,
  record:{taskId:"fixture",revision:2,workflowVersion:"2.1.0",questionMode:"ask",currentStage:"resources",currentRound:0,cycle:1,stages:{requirements:"passed",resources:"awaiting-confirmation",plan:"not-started",implementation:"not-started",review:"not-started",delivery:"not-started",closeout:"not-started"},documents:{resources:doc},approvals:[],versionDetails:{},issues:[],evidence:[],rounds:[],milestones:{},knowledge:[],skillPins:[],updatedAt:"now"}};
let state="review";
const snapshot=()=>{const next=structuredClone(view);if(state!=="review"){next.actions!.review.resources="尚未提交审查";next.record!.stages.resources="in-progress";}return next;};
const longDescription = new URLSearchParams(location.search).has("long-description");
const description = longDescription ? ["一句话概述：30 秒内在旋转的毛线团上插针，插得越多分越高，总分最高者获胜。",
  "3.1 基础规则：人数 2–20 人；时长 30 秒。每成功插 1 针得 5 分，插针失败不扣分。",
  "3.2 玩法规则：按下鼠标左键或空格插针。插针判定为球周围 5° 的扇区是否有针；有针则失败，无针则成功。球上的针随球旋转。每秒获得 1 根针，最后 10 秒加速旋转与发针。",
  ...Array.from({length:5}, (_,i)=>`${i+1}. 美术设计：中间展示游戏区域，右侧显示计分。使用温暖的粉彩纸感配色，毛线团、针和背景保持清晰对比。按键时在插入的位置播放星形粒子特效。设计示意必须与实际游戏效果分别标注。`),
  "长路径折行验证："+"very-long-reference-name-".repeat(16)].join("\n\n") : "浏览器隔离测试数据";
api.listTasks=async()=>[{id:"fixture",title:longDescription?"见缝插针":"舒尔特方格 · 联机竞技",description,workspacePath:"D:/Fixtures/舒尔特方格",status:"active",createdAt:"2026-10-12T10:00:00Z",updatedAt:"2026-10-12T10:00:00Z"}];
api.listInstances=async()=>[];
terminalApi.availability=async()=>({available:false,reason:"浏览器测试不启动原生终端",workingDirectory:"D:/Fixtures",executablePath:"",version:"fixture"}) as Awaited<ReturnType<typeof terminalApi.availability>>;
terminalApi.listConversations=async()=>[{id:"original",taskId:"fixture",title:"当前任务原会话",createdAt:"now",updatedAt:"now",nativeSessionId:"fixture-native"}];
terminalApi.workflow=async()=>({taskId:"fixture",conversationId:"original",observability:"native",warning:"",currentTurnId:"turn",turns:[{id:"turn",status:"inProgress",explanation:"",startedAt:"now",plan:[],activities:[]}],updatedAt:new Date().toISOString()});
productionApi.get=async()=>snapshot();
productionApi.control=async()=>({taskId:"fixture",conversationId:"original",state:state==="review"?"waitingForApproval":state,connection:"connected",queued:false,sequence:1,updatedAt:"now"}) as TaskControlState;
productionApi.acceptanceStatus=async()=>({status:"not-started"});
productionApi.timing=async()=>({note:"隔离测试，无实际运行用时"}) as Awaited<ReturnType<typeof productionApi.timing>>;
productionApi.skills=async()=>[];
productionApi.decide=async()=>{throw new Error("预览不会提交真实审查决定");};
function App(){const[selected,setSelected]=useState(state);return <>
  <div style={{height:40,display:"flex",gap:10,alignItems:"center",padding:"0 16px",background:"#fff",fontSize:12}}>
    <span>隔离测试</span>{[{id:"review",label:"阶段待办"},{id:"running",label:"等待 AI"},{id:"waitingForTerminal",label:"终端待处理"},{id:"disconnected",label:"连接异常"}].map(item=><button key={item.id} aria-pressed={selected===item.id} onClick={()=>{state=item.id;setSelected(state);window.dispatchEvent(new Event("abya:control-changed"));}}>{item.label}</button>)}
  </div><div className="app-shell" style={{height:"calc(100% - 40px)"}}><aside className="app-sidebar"><div className="brand"><div className="brand-mark">A</div><div>ABYA 开发工具</div></div><nav><button className="active">开发任务</button></nav></aside>
  <div className="app-main"><TasksView t={translator("zh-CN")} settings={{locale:"zh-CN"} as AppSettings} notify={()=>{}}/></div></div>
 </>;}
createRoot(document.getElementById("root")!).render(<App/>);
