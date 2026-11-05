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
