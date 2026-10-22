// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
const control=vi.hoisted(()=>vi.fn());
vi.mock("../../shared/production",()=>({productionApi:{control}}));
import { useTaskControl } from "./useTaskControl";
afterEach(()=>{cleanup();vi.useRealTimers();vi.clearAllMocks();});
it("shares pending reads across remounts without blocking another task or accepting stale state",async()=>{
  vi.useFakeTimers();let release!:(value:unknown)=>void;
  control.mockImplementation((task:string)=>task==="a"?new Promise(r=>{release=r;}):Promise.resolve({taskId:"b",conversationId:"cb",state:"ready"}));
  const first=renderHook(()=>useTaskControl("a","ca","codex"));
  await act(async()=>{vi.advanceTimersByTime(10000);});
  expect(control).toHaveBeenCalledTimes(1);first.unmount();
  const second=renderHook(({task,chat})=>useTaskControl(task,chat,"codex"),{initialProps:{task:"a",chat:"ca"}});
  expect(control).toHaveBeenCalledTimes(1);
  await act(async()=>second.rerender({task:"b",chat:"cb"}));
  expect(second.result.current?.conversationId).toBe("cb");
  await act(async()=>release({taskId:"a",conversationId:"ca",state:"running"}));
  expect(second.result.current?.conversationId).toBe("cb");
});
