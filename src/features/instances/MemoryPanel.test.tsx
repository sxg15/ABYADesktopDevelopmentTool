// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { MemoryPanel } from "./MemoryPanel";
import { memoryApi, type MemoryStatus } from "../../shared/memory";
vi.mock("../../shared/memory",()=>({memoryApi:{status:vi.fn(),policy:vi.fn(),arm:vi.fn(),save:vi.fn(),reports:vi.fn()}}));
const status:MemoryStatus={instanceId:"instance",latest:{at:"2026-10-19",privateBytes:5*1073741824,workingSetBytes:2*1073741824,availableBytes:10*1073741824,commitBytes:20*1073741824,commitLimitBytes:40*1073741824},peakPrivateBytes:5*1073741824,pressured:true,autoStop:false,incidentId:"incident",reportPath:"report.md",error:""};
beforeEach(()=>{
  vi.mocked(memoryApi.status).mockResolvedValue(status);
  vi.mocked(memoryApi.policy).mockResolvedValue({privateLimitMib:4096,growthLimitMib:768,availableFloorMib:8192,commitLimitPercent:90});
  vi.mocked(memoryApi.arm).mockResolvedValue();vi.mocked(memoryApi.reports).mockResolvedValue();
});
afterEach(()=>{cleanup();vi.resetAllMocks();});
it("never arms stopping from pressure alone and requires a user action",async()=>{
  render(<MemoryPanel id="instance" managed running/>);
  await screen.findByText(/事件编号：/);expect(memoryApi.arm).not.toHaveBeenCalled();
  const checkbox=screen.getByRole("checkbox");expect((checkbox as HTMLInputElement).checked).toBe(false);
  fireEvent.click(checkbox);await waitFor(()=>expect(memoryApi.arm).toHaveBeenCalledWith("instance",true));
  fireEvent.click(screen.getByRole("button",{name:"打开本次报告"}));
  await waitFor(()=>expect(memoryApi.reports).toHaveBeenCalledWith("instance"));
});
it("does not grant stop control to external instances",async()=>{
  render(<MemoryPanel id="external" managed={false} running/>);
  await screen.findByText(/事件编号：/);expect((screen.getByRole("checkbox") as HTMLInputElement).disabled).toBe(true);
  expect(memoryApi.arm).not.toHaveBeenCalled();
});
it("ignores a late sample after switching instances",async()=>{
  let resolve!:(value:MemoryStatus)=>void;
  vi.mocked(memoryApi.status).mockImplementation(id=>id==="old"?new Promise(r=>{resolve=r;}):Promise.resolve({...status,incidentId:"new-incident"}));
  const view=render(<MemoryPanel id="old" managed running/>);
  view.rerender(<MemoryPanel id="new" managed running/>);await screen.findByText(/new-incident/);
  resolve({...status,incidentId:"old-incident"});await Promise.resolve();expect(screen.queryByText(/old-incident/)).toBeNull();
});
