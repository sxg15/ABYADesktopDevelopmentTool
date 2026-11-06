import React from "react";
import {createRoot} from "react-dom/client";
import {ArtLibraryPanel} from "../src/features/tasks/ArtLibraryPanel";
import {artLibraryApi,type ArtCase} from "../src/shared/artDesign";
import type {ProductionView} from "../src/shared/production";
import catalog from "../.codex/skills/abya-game-development-task/assets/art-library/catalog.json";
import "../src/App.css";
const entries:ArtCase[]=catalog.entries.map(preset=>({id:`builtin.${preset.id}`,version:1,presetId:preset.id,title:preset.title,
 status:"available",provenance:"builtin",context:preset.purpose,tags:preset.tags,plannerRequirements:"按当前游戏选择",
 visualRules:"反馈服务玩法",decisions:"独立参数采样",production:"共享源码与配方",applicability:preset.purpose,
 limitations:"Web 预览不代表原生验证",sources:"项目原创",designApproved:false,runtimeVerified:false,media:[],
 resources:{schema:"abya.art-resources/v1",kind:preset.kind,license:"ABYA 项目原创",compatibility:"Web 演示；原生接入需验证",
 lifecycle:"触发、暂停、恢复、取消和销毁分别处理",integration:"按当前游戏适配坐标、输入和资源",
 verification:"尚未完成 Abya 实机验证",parameters:{strength:{default:1,min:0,max:2}},requiredCapabilities:[],
 tests:["正常播放","取消恢复","重复触发"],files:[{name:"effects.mjs",path:"effects.mjs",sha256:"fixture",role:"source"}]}}));
artLibraryApi.list=async query=>entries.filter(entry=>JSON.stringify(entry).includes(query));
createRoot(document.getElementById("root")!).render(<React.StrictMode><main style={{padding:24,maxWidth:1400,margin:"auto"}}>
 <p>界面隔离验证 · 数据为显式夹具</p><ArtLibraryPanel taskId="fixture" view={{record:null} as unknown as ProductionView} active={false} embedded/>
</main></React.StrictMode>);
