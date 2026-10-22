// Real components with disposable, explicitly synthetic data. No native IPC, AI or game process.
import React, {useState} from "react";
import {createRoot} from "react-dom/client";
import {ClipboardList, Send, CloudDownload, ScrollText, Settings} from "lucide-react";
import {TasksView} from "../src/features/tasks/TasksView";
import {api, terminalApi} from "../src/shared/api";
import {productionApi} from "../src/shared/production";
import {artLibraryApi} from "../src/shared/artDesign";
import {artCaseFixture, artViewFixture} from "../src/shared/artDesign.test-fixtures";
import {attentionApi} from "../src/shared/notifications";
import {attentionFixture} from "../src/shared/notifications.test-fixtures";
import {translator} from "../src/i18n";
import type {AppSettings, CodexWorkflowSnapshot} from "../src/shared/types";
import "../src/App.css";
import "../src/app/workspaceRefinement.css";
import "../src/app/workbenchTheme.css";
let callbackId=0;
(window as unknown as {__TAURI_INTERNALS__:unknown}).__TAURI_INTERNALS__={transformCallback:()=>++callbackId,unregisterCallback:()=>{},invoke:()=>Promise.reject(new Error("Native IPC is disabled in this fixture"))};
const view=artViewFixture();view.record!.taskId="task";
view.policy.iterationMode="human-feedback";view.policy.stages=[{id:"requirements",name:"需求整理",approval:"requirements"},{id:"resources",name:"资源美术",approval:"resources"},{id:"plan",name:"方案预检",approval:"plan"},{id:"implementation",name:"Lua 实现",approval:null},{id:"review",name:"试玩修改",approval:null},{id:"delivery",name:"交付验收",approval:"delivery"},{id:"closeout",name:"收尾复盘",approval:null}];
view.record!.stages={requirements:"passed",resources:"awaiting-confirmation",plan:"not-started",implementation:"not-started",review:"not-started",delivery:"not-started",closeout:"not-started"};
view.record!.documents.resources={kind:"resources",revision:3,path:"artifacts/art.md",sha256:"fixture-doc",submittedAt:"2026-10-17T09:00:00Z",content:"# 隔离测试美术确认单\n保留完整要求，以实物轮廓表达剩余库存。此画面仅验证 UI，不代表游戏验收。\n\n| 区域 | 设计要求 | 状态 |\n| --- | --- | --- |\n| 物品栏 | 以图形显示剩余数量 | 待审阅 |\n| 计时区 | 保持清晰对比 | 已提交 |\n\n- [x] 梳理参考图\n- [ ] 实机复验\n\n```lua\nlocal remaining = 5\nprint(remaining)\n```",artifactBindings:{"artifacts/layout.png":"layout-hash"}};
view.record!.currentVersion="synthetic-playable-version";view.actions!.feedback.gameplay="";
view.record!.artifacts![0].status="registered";view.actions!.review.resources="";view.actions!.approve.resources="";
view.record!.workbench={activeGoalId:"doc",drafts:{},requests:[],deferrals:[],goals:[{id:"source",stage:"resources",number:1,title:"确认视觉方向",sourceType:"work",sourceId:"source",status:"submitted",summary:"已整理策划原话与参考图",recordRevision:1,createdAt:"fixture",updatedAt:"fixture",cycle:1},{id:"doc",stage:"resources",number:2,title:"审阅资源与美术方案",sourceType:"document",sourceId:"doc",status:"awaiting-confirmation",summary:"等待审阅",recordRevision:2,createdAt:"fixture",updatedAt:"fixture",cycle:1}]};
productionApi.snapshot=async()=>structuredClone(view.record!);
const canvas=document.createElement("canvas");canvas.width=800;canvas.height=450;const ctx=canvas.getContext("2d")!;
ctx.fillStyle="#e9dfca";ctx.fillRect(0,0,800,450);ctx.fillStyle="#544d37";ctx.font="600 28px Microsoft YaHei UI";ctx.fillText("见缝插针 · 设计示意",38,57);ctx.font="14px Microsoft YaHei UI";ctx.fillText("浏览器隔离测试 / 非游戏成果",38,82);
ctx.fillStyle="#bd7450";ctx.beginPath();ctx.arc(435,210,92,0,2*Math.PI);ctx.fill();ctx.strokeStyle="#8b6349";ctx.lineWidth=3;ctx.beginPath();ctx.arc(435,210,84,0,2*Math.PI);ctx.stroke();
ctx.fillStyle="#faf0d7";ctx.font="24px Microsoft YaHei UI";ctx.fillText("保持节奏",386,219);ctx.fillStyle="#4c705b";for(let n=0;n<5;n++)ctx.fillRect(313+n*42,360,12,43);ctx.font="16px Microsoft YaHei UI";ctx.fillText("剩余物品 / 逐件显示",313,335);ctx.fillStyle="#8a633b";ctx.font="30px Consolas";ctx.fillText("00:05",642,67);
const image=canvas.toDataURL("image/png"), entry=artCaseFixture();entry.title="物品库存 · 纸质桌游 HUD";
const methods=["game-art-design","product-interface-design"].map((name,index)=>({id:index?"abya-game-development-task/references/game-art-design/references/product-interface-design":"abya-game-development-task/references/game-art-design",provider:"codex",name,kind:"design",description:index?"从用户任务、平台和输入方式推导界面结构、视觉与实现。":"保留策划原话，拆解风格参考，对照设计和运行画面复验。",path:`.codex/skills/${name}`,sha256:"fixture-design-hash"}));
api.listTasks=async()=>[{id:"task",title:"见缝插针 · 多人竞速",description:"制作四人竞速游戏，保留纸质桌游质感。不要用数字显示库存，直接显示剩余物品。\n保留玩家身份颜色，最后一枚物品也要清楚。\n这是隔离验证数据，不代表真实策划或游戏成果。",workspacePath:"D:/Fixture/见缝插针",status:"active",createdAt:"2026-10-17T09:00:00Z",updatedAt:"2026-10-17T09:00:00Z"}];
api.listInstances=async()=>[];
productionApi.get=async()=>structuredClone(view);productionApi.reconcile=async()=>structuredClone(view);productionApi.media=async()=>image;
productionApi.skills=async()=>structuredClone(methods);productionApi.readSkill=async(_task,_provider,id)=>`# ${id.split('/').pop()}\n\n隔离测试正文。实际发布包从任务固定版本中读取内容。`;
productionApi.revealDocument=async()=>{};productionApi.revealReport=async()=>{};
productionApi.acceptanceStatus=async()=>({status:"not-started"});productionApi.timing=async()=>({note:"隔离测试，无实际执行时间"}) as Awaited<ReturnType<typeof productionApi.timing>>;
productionApi.decide=async()=>{throw new Error("隔离预览不会批准真实任务");};
productionApi.control=async()=>({taskId:"task",conversationId:"original",state:"waitingForApproval",connection:"connected",stage:"resources",queued:false,sequence:1,updatedAt:"fixture",nativeSessionId:"fixture-native"});
productionApi.continueTask=async()=>({status:"waitingForApproval",conversationId:"original",nativeSessionId:"fixture-native"});
attentionApi.get=async()=>{const result=attentionFixture("review","task");result.items[0].target.documentRevision=3;result.items[0].target.documentHash="fixture-doc";result.items[0].zh={title:"美术方案已更新，待你审阅",instruction:"请对照策划要求与画面，明确确认本版方案或提出修改。",button:"查看并审阅本版",destination:"查看位置：制作流程 / 资源美术",detail:"设计预览不代表实机验收；确认仍需明确选择。"};return result;};
artLibraryApi.list=async()=>[structuredClone(entry),{...structuredClone(entry),id:"case-two",title:"固定镜头 · 玩家身份层次",tags:["桌面","身份色"],satisfaction:"只认可身份色表达"}];artLibraryApi.media=async()=>image;
artLibraryApi.request=async(_id,revision,kind,caseId,version)=>{if(revision!==view.record!.revision)throw new Error("版本已变化");view.record!.revision++;const prompt=`隔离案例请求：${kind} ${caseId??""} ${version??""}`;view.record!.artDesign!.caseRequests=[{id:"fixture-request",selection:{kind,caseId:caseId??null,version:version??null,cycle:1,designHash:"design-1"},prompt,at:"fixture",origin:"app-user"}];return {revision:view.record!.revision,prompt};};
artLibraryApi.action=async()=>{throw new Error("请使用原生隔离验收验证案例写入；此预览不导入或收录。");};
const workflow:CodexWorkflowSnapshot={taskId:"task",conversationId:"original",observability:"native",warning:"",currentTurnId:"turn",turns:[{id:"turn",status:"completed",explanation:"美术方案已提交，等待人工审阅。",startedAt:"2026-10-17T09:00:00Z",plan:[{id:"source",step:"核对策划原话",status:"completed"},{id:"reference",step:"拆解风格参考",status:"completed"},{id:"design",step:"提交当前设计方案",status:"completed"}],activities:[]}],updatedAt:"fixture"};
terminalApi.availability=async()=>({available:true,reason:"",workingDirectory:"D:/Fixture",observabilityAvailable:true,observabilityReason:""});
terminalApi.listConversations=async(_p,taskId)=>[{id:"original",taskId,title:"美术方案调整",createdAt:"2026-10-17T09:00:00Z",updatedAt:"2026-10-17T09:00:00Z",nativeSessionId:"fixture-native"}];
terminalApi.workflow=async()=>structuredClone(workflow);
terminalApi.open=async(_p,taskId,conversationId,_cols,_rows,channel)=>{queueMicrotask(()=>channel.onmessage({type:"output",data:"\x1b[36mABYA / 浏览器隔离终端\x1b[0m\r\n\r\n已核对策划原话与风格参考。\r\n\r\n\x1b[32m✓ 保留物品库存表达\r\n✓ 保留玩家身份色\r\n✓ 已提交当前美术方案\x1b[0m\r\n\r\n等待人工审阅，不代表游戏或任务已通过验收。\r\n\r\n可测试：切换工作区、展开步骤侧栏、专注模式与历史查找。\r\n本页面不创建 AI 会话或游戏进程。\r\n"}));return {taskId,conversationId,status:"running",workingDirectory:"D:/Fixture",lastError:""};};
terminalApi.write=async()=>{};terminalApi.resize=async()=>{};terminalApi.stop=async()=>{};terminalApi.copyHistory=async()=>true;
terminalApi.executionSettings=async()=>({settings:{model:"fixture-model",effort:"medium"},models:[],running:false,connected:true});
function Preview(){const[notice,setNotice]=useState("");return <div className="app-shell"><aside className="app-sidebar"><div className="brand"><div className="brand-mark">A</div><div><strong>ABYA</strong><span>Development Tool</span></div></div><nav>{[[ClipboardList,"开发任务"],[Send,"传输存档"],[CloudDownload,"游戏客户端管理"],[ScrollText,"游戏日志"],[Settings,"设置"]].map(([Icon,label],i)=>{const Symbol=Icon as typeof ClipboardList;return <button key={String(label)} className={i===0?"active":""}><Symbol size={19}/><span>{String(label)}</span></button>;})}</nav><div className="sidebar-footer"><strong>真实组件 · 隔离验证</strong><span>无原生服务、AI 或游戏进程</span></div></aside><main className="app-main"><div className="app-view active"><TasksView t={translator("zh-CN")} settings={{locale:"zh-CN"} as AppSettings} notify={setNotice}/></div></main>{notice&&<div className="notification" role="status">{notice}</div>}</div>;}
// Long-question mode exercises the real embedded form without touching any task.
if (new URLSearchParams(location.search).has("questions")) {
  view.record!.questionGroups=[{id:"split-test",title:"《大小记忆》首版需求：集中决定 3 项",provider:"codex",conversationId:"original",status:"pending",draft:{},answers:[],publishedAt:"fixture",updatedAt:"fixture",
    questions:[{id:"rules",text:"是否采用首版补全方案：首数记忆 1 秒且计入 30 秒；左键卡内起手，纵向位移 ≥ 卡高 10% 且大于横向位移；答对 +5，答错和无效操作不加分；反馈 0.5 秒；公共题序、各自推进；1–99，相邻不相等；高分胜、同分并列；返回后真人重新准备、机器人保持准备；背景柔和且不显示历史数字。",options:["采用上述首版方案","调整部分规则后再确认"],optional:false},
      {id:"art",text:"美术参考需要怎样适配到数字记忆玩法？",options:["按文档手工 UI 与蜡笔背景执行","先看方案再选择"],optional:false},
      {id:"note",text:"还有其他需要补充的要求吗？",options:[],optional:true}]}];
  productionApi.answer=async(_task,_revision,operation,data)=>{if(operation!=="save-draft")throw new Error("This fixture only saves drafts");view.record!.questionGroups![0].draft=data.answers as Record<string,string>;view.record!.revision++;return structuredClone(view);};
  attentionApi.get=async()=>({...attentionFixture("question","task"),items:[]});
}
createRoot(document.getElementById("root")!).render(<Preview/>);
