import {invoke} from "@tauri-apps/api/core";
export const briefPrompt="请先读取本任务 .codex/skills/abya-desktop-workflow/SKILL.md 并连接开发 APP。使用 $abya-gameplay-brief，根据我提供的想法、文档和参考图整理玩法与美术需求。已有定稿请沿用，只指出缺口或冲突。我确认后继续制作流程。";
export const openTaskChat=(taskId:string,prompt:string)=>invoke<void>("open_desktop_chat",{taskId,prompt});

export const chatStatus=(taskId:string)=>invoke<{ready:boolean;bound:boolean}>("desktop_chat_status",{taskId});
