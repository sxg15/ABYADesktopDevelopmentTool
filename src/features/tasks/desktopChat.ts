import { invoke } from "@tauri-apps/api/core";
import type { DevelopmentTask } from "../../shared/types";

export const briefPrompt = "使用 $abya-gameplay-brief，根据我提供的想法、文档和参考图整理玩法与美术需求。已有定稿请沿用，只指出缺口或冲突。我确认后继续制作流程。";
let latestOpen = 0;

export async function openTaskChat(taskId: string, clipboardText?: (link: string) => string) {
  const request = ++latestOpen;
  const link = await invoke<string>("prepare_desktop_chat", { taskId });
  if (request !== latestOpen) return false;
  if (clipboardText) await navigator.clipboard.writeText(clipboardText(link));
  if (request !== latestOpen) return false;
  await invoke<void>("open_desktop_chat", { taskId, expectedLink: link });
  return true;
}

export function taskPrompt(task: DevelopmentTask, link: string, instruction: string) {
  return [
    "ABYA 制作任务 · " + task.title,
    "任务 ID：" + task.id,
    "任务工作区：" + task.workspacePath,
    "[打开这项任务的 Codex 对话](" + link + ")",
    "先核对当前对话和工作区。如果打开的是其他项目，请给我上面的任务对话链接，让我回到那里继续。",
    "在对应任务根目录读取 .codex/skills/abya-desktop-workflow/SKILL.md 和 .codex/abya-desktop.json，通过 production get 核对任务 ID。",
    instruction,
  ].join("\n");
}

export async function copyAndOpenTaskChat(task: DevelopmentTask, instruction: string) {
  return openTaskChat(task.id, link => taskPrompt(task, link, instruction));
}

export const chatStatus=(taskId:string)=>invoke<{ready:boolean;bound:boolean}>("desktop_chat_status",{taskId});
