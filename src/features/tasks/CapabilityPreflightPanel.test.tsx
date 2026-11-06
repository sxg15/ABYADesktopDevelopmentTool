// @vitest-environment jsdom
import {render,screen,waitFor,cleanup,fireEvent} from "@testing-library/react";
import {afterEach,it,expect,vi} from "vitest";
import {CapabilityPreflightPanel} from "./CapabilityPreflightPanel";
import {productionApi,type ProductionRecord} from "../../shared/production";
import {programmerIssue} from "./programmerIssue";
vi.mock("../../shared/production",()=>({productionApi:{preflight:vi.fn()}}));
vi.mock("../../shared/api",()=>({errorMessage:String}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
function record():ProductionRecord{return {taskId:"task",cycle:1,currentStage:"plan",policy:{capabilityPreflightVersion:1},workbench:{execution:{capabilityPreflight:{runId:"run",state:"failed",phase:"runtime-connect",checks:[],cleanup:{state:"completed"},error:{code:"timeout",message:"timeout"}}}}} as unknown as ProductionRecord;}
it("automatically checks once in plan and does not loop on polling",async()=>{
 const r=record();const view=render(<CapabilityPreflightPanel record={r} active onUpdated={()=>{}}/>);
 await waitFor(()=>expect(productionApi.preflight).toHaveBeenCalledTimes(1));
 view.rerender(<CapabilityPreflightPanel record={{...r,revision:4}} active onUpdated={()=>{}}/>);
 expect(productionApi.preflight).toHaveBeenCalledTimes(1);
 fireEvent.click(screen.getByRole("button",{name:"验证复用本任务已有存档"}));
 await waitFor(()=>expect(productionApi.preflight).toHaveBeenLastCalledWith("task",true,true));
});
it("shows pending cleanup and disables retries while files may still be in use",()=>{
 const r=record();r.policy!.capabilityPreflightVersion=0;r.workbench!.execution!.capabilityPreflight!.cleanup={state:"pending",message:"进程尚未退出"};
 render(<CapabilityPreflightPanel record={r} active onUpdated={()=>{}}/>);
 expect(screen.getByRole("status").textContent).toContain("待清理");
 expect((screen.getByRole("button",{name:"重新预检"}) as HTMLButtonElement).disabled).toBe(true);
});
it("marks confirmed package problems but not a connection timeout",()=>{
 const i={id:"preflight:basic",stage:"plan",kind:"blocker",status:"open",description:"failure",preflightRunId:"run",repairTarget:"other" as const};
 expect(programmerIssue(i)).toBeUndefined();
 expect(programmerIssue({...i,repairTarget:"abya-build",diagnosisStatus:"confirmed"})?.label).toBe("ABYA-PB 包兼容问题");
});

it("allows review revalidation using the existing archive without auto-running on view",async()=>{
 const r=record();r.currentStage="review";r.workbench!.execution!.capabilityPreflight=undefined;
 render(<CapabilityPreflightPanel record={r} active onUpdated={()=>{}}/>);
 expect(productionApi.preflight).not.toHaveBeenCalled();
 expect(screen.getByText(/可在当前阶段验证修改环境/)).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"重新预检"}));
 await waitFor(()=>expect(productionApi.preflight).toHaveBeenCalledWith("task",true,true));
});

it("shows existing archive verification without requesting a new plan approval",()=>{
 const r=record();r.currentStage="review";const p=r.workbench!.execution!.capabilityPreflight!;
 p.state="existing-verified";p.reportPath="artifacts/preflight.json";p.error=undefined;
 render(<CapabilityPreflightPanel record={r} active onUpdated={()=>{}}/>);
 expect(screen.getByText("已有存档的加载、保存及冷加载已验证")).toBeTruthy();
 expect(screen.queryByText(/方案需引用/)).toBeNull();
 expect(productionApi.preflight).not.toHaveBeenCalled();
});
