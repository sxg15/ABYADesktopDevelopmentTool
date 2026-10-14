// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { attentionApi, type TaskAttentionSnapshot } from "./notifications";
import { useTaskAttention } from "./useTaskAttention";
vi.mock("@tauri-apps/api/event",()=>({listen:vi.fn(async()=>vi.fn())}));
vi.mock("./notifications",()=>({attentionApi:{get:vi.fn()}}));
afterEach(()=>{cleanup();vi.clearAllMocks();});
function Page({id}:{id:string}) {const state=useTaskAttention(id);return <div>{state?.taskId ?? "loading"}</div>;}
it("ignores a late response after switching tasks",async()=>{
  let finish!:(value:TaskAttentionSnapshot)=>void;
  vi.mocked(attentionApi.get).mockReturnValueOnce(new Promise(resolve=>{finish=resolve;}))
    .mockResolvedValue({taskId:"b",unavailable:false,items:[]});
  const page=render(<Page id="a"/>);page.rerender(<Page id="b"/>);
  await screen.findByText("b");await act(async()=>finish({taskId:"a",items:[],unavailable:false}));
  expect(screen.queryByText("a")).toBeNull();expect(screen.getByText("b")).toBeTruthy();
  await waitFor(()=>expect(attentionApi.get).toHaveBeenCalledTimes(2));
});
