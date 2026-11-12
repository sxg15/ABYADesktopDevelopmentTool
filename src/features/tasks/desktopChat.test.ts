// @vitest-environment jsdom
import {afterEach, expect, it, vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {copyAndOpenTaskChat,openTaskChat} from "./desktopChat";
import type {DevelopmentTask} from "../../shared/types";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
afterEach(()=>vi.resetAllMocks());
const task={id:"task-a",title:"见缝插针",workspacePath:"D:/任务 空间/见缝插针"} as DevelopmentTask;
it("copies the verified task identity before focusing its existing chat",async()=>{
  const link="codex://threads/thread-a",writeText=vi.fn(async()=>{});
  Object.defineProperty(navigator,"clipboard",{value:{writeText},configurable:true});
  vi.mocked(invoke).mockResolvedValueOnce(link).mockResolvedValueOnce(undefined);
  await copyAndOpenTaskChat(task,"整理美术需求");
  const text=String(writeText.mock.calls[0]);
  for(const part of [task.id,task.workspacePath,link,"整理美术需求"])expect(text).toContain(part);
  expect(invoke).toHaveBeenLastCalledWith("open_desktop_chat",{taskId:task.id,expectedLink:link});
});
it("a slower previous selection cannot take focus from the current task",async()=>{
  let finish!:(link:string)=>void;
  vi.mocked(invoke).mockReturnValueOnce(new Promise(resolve=>{finish=resolve;})).mockResolvedValueOnce("codex://threads/b");
  const first=openTaskChat("a");await openTaskChat("b");finish("codex://threads/a");
  expect(await first).toBe(false);
  expect(invoke).not.toHaveBeenCalledWith("open_desktop_chat",{taskId:"a",expectedLink:"codex://threads/a"});
});
