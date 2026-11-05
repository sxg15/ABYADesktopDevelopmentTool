// @vitest-environment jsdom
import {cleanup,render,screen,fireEvent,waitFor} from "@testing-library/react";
import {afterEach,it,expect,vi} from "vitest";
import {VerificationPackage} from "./VerificationPackage";
import {playableState,playableIssues} from "./playableState";
import {productionApi,type ProductionRecord} from "../../shared/production";
vi.mock("../../shared/production",()=>({productionApi:{revealDocument:vi.fn(),startPreview:vi.fn(),workbench:vi.fn()}}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
it.each([1,4,8])("passes %i multiplayer windows with the current review version",async seats=>{
 const r=record();r.currentStage="review";r.currentVersion="feedback-v4";r.playerMode="multiplayer";
 vi.mocked(productionApi.startPreview).mockResolvedValue({status:"ready"});
 render(<VerificationPackage record={r} active en={false}/>);
 fireEvent.change(screen.getByRole("combobox",{name:"试玩人数（窗口数）"}),{target:{value:String(seats)}});
 fireEvent.click(screen.getByRole("button",{name:"启动当前修改版试玩"}));
 await waitFor(()=>expect(productionApi.startPreview).toHaveBeenCalledExactlyOnceWith("task","feedback-v4",seats));
});
function record(){return {taskId:"task",revision:3,currentStage:"implementation",currentVersion:"v1",cycle:1,issues:[],selfTests:["lua","input","core-loop","save-reload"].map(id=>({id,status:"passed",version:"v1",cycle:1})),workbench:{execution:{verificationPackage:{status:"core-verified",version:"v1",path:"artifacts/playable-packages/package.zip"}}}} as unknown as ProductionRecord;}
it("reviews the latest candidate even when only the old package exists",async()=>{
 const r=record();r.currentStage="review";r.currentVersion="feedback-v4";
 vi.mocked(productionApi.startPreview).mockResolvedValue({status:"ready",version:"feedback-v4"});
 render(<VerificationPackage record={r} active en={false}/>);
 expect(screen.getByText("feedback-v4")).toBeTruthy();
 expect(screen.queryByRole("button",{name:"启动此版本试玩"})).toBeNull();
 fireEvent.click(screen.getByRole("button",{name:"启动当前修改版试玩"}));
 await waitFor(()=>expect(productionApi.startPreview).toHaveBeenCalledExactlyOnceWith("task","feedback-v4",undefined));
});
it("offers current review without a package and surfaces rejection without fallback",async()=>{
 const r=record();r.currentStage="review";r.currentVersion="feedback-v4";
 r.workbench!.execution!.verificationPackage=undefined;
 vi.mocked(productionApi.startPreview).mockRejectedValue(new Error("当前版本已改变"));
 render(<VerificationPackage record={r} active en={false}/>);
 fireEvent.click(screen.getByRole("button",{name:"启动当前修改版试玩"}));
 await waitFor(()=>expect(screen.getByRole("alert").textContent).toContain("当前版本已改变"));
 expect(productionApi.startPreview).toHaveBeenCalledTimes(1);
});
it("groups package, isolated playtest and repair actions for older tasks",async()=>{
 const repair=vi.fn().mockResolvedValue({});vi.mocked(productionApi.startPreview).mockResolvedValue({status:"ready"});
 render(<VerificationPackage record={record()} active en={false} onRepair={repair}/>);
 fireEvent.click(screen.getByRole("button",{name:"启动此版本试玩"}));await waitFor(()=>expect(productionApi.startPreview).toHaveBeenCalledExactlyOnceWith("task","v1",undefined));
 await waitFor(()=>expect((screen.getByRole("button",{name:"定位试玩包"}) as HTMLButtonElement).disabled).toBe(false));
 fireEvent.click(screen.getByRole("button",{name:"定位试玩包"}));await waitFor(()=>expect(productionApi.revealDocument).toHaveBeenCalledExactlyOnceWith("task","artifacts/playable-packages/package.zip"));
 await waitFor(()=>expect((screen.getByRole("button",{name:"诊断并修复上述缺陷"}) as HTMLButtonElement).disabled).toBe(false));
 fireEvent.click(screen.getByRole("button",{name:"诊断并修复上述缺陷"}));await waitFor(()=>expect(repair).toHaveBeenCalledOnce());
});
it("does not launch a package contradicted by a newer core failure",()=>{
 const r=record();r.selfTests!.push({id:"input",status:"failed",version:"v1",cycle:1,observations:"broken",recordedAt:"now"});
 render(<VerificationPackage record={r} active en={false}/>);
 expect(playableState(r).available).toBe(false);
 expect((screen.getByRole("button",{name:"启动此版本试玩"}) as HTMLButtonElement).disabled).toBe(true);
});
it("labels a historical checkpoint and prevents using it to advance the current candidate",()=>{
 const r=record();r.currentVersion="v2";r.workbench!.execution!.verificationPackage!.sourceRevision=12;
 render(<VerificationPackage record={r} active en={false}/>);
 expect(screen.getByText(/这是之前的可试玩版本/)).toBeTruthy();
 expect((screen.getByRole("button",{name:"进入试玩与修改阶段 →"}) as HTMLButtonElement).disabled).toBe(true);
});
it("requires explicit user handoff and preserves failed checks in the submitted record",async()=>{
 const r=record();r.selfTests!.push({id:"lifecycle",status:"failed",version:"v1",cycle:1,observations:"unverified",recordedAt:"now"});
 const updated=vi.fn();vi.mocked(productionApi.workbench).mockResolvedValue({record:{...r,currentStage:"review"}} as never);
 render(<VerificationPackage record={r} active en={false} onUpdated={updated}/>);
 fireEvent.click(screen.getByRole("button",{name:"进入试玩与修改阶段 →"}));expect(productionApi.workbench).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"带着待处理项进入"}));
 await waitFor(()=>expect(productionApi.workbench).toHaveBeenCalledExactlyOnceWith("task",3,"enter-playtest",{version:"v1"}));
 expect(r.selfTests![r.selfTests!.length-1]?.status).toBe("failed");expect(updated).toHaveBeenCalledOnce();
});
it("does not present uncertain Esc observations as confirmed defects",()=>{
 const r=record();r.issues=[{id:"any-game",kind:"defect",status:"open",stage:"implementation",description:"physical Escape not tested",diagnosisStatus:"suspected"}];
 expect(playableIssues(r)[0]).toMatchObject({confirmed:false,title:"Esc 暂停与恢复"});
});
