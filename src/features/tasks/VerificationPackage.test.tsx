// @vitest-environment jsdom
import {cleanup,render,screen,fireEvent,waitFor} from "@testing-library/react";
import {afterEach,it,expect,vi} from "vitest";
import {VerificationPackage} from "./VerificationPackage";
import {productionApi,type ProductionRecord} from "../../shared/production";
vi.mock("../../shared/production",()=>({productionApi:{revealDocument:vi.fn(),startAcceptance:vi.fn(),startPreview:vi.fn()}}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
function record(){return {taskId:"task",currentVersion:"v1",cycle:1,selfTests:["lua","input","core-loop","save-reload"].map(id=>({id,status:"passed",version:"v1",cycle:1})),workbench:{execution:{verificationPackage:{version:"v1",path:"artifacts/playable-packages/package.zip"}}}} as unknown as ProductionRecord;}
it("exposes the exact verified package and owned playtest action",async()=>{
 render(<VerificationPackage record={record()} active en={false}/>);
 fireEvent.click(screen.getByRole("button",{name:"启动此版本试玩"}));await waitFor(()=>expect(productionApi.startAcceptance).toHaveBeenCalledExactlyOnceWith("task"));
 fireEvent.click(screen.getByRole("button",{name:"定位试玩包"}));await waitFor(()=>expect(productionApi.revealDocument).toHaveBeenCalledExactlyOnceWith("task","artifacts/playable-packages/package.zip"));
});
it("retains the old package but does not offer it as the current verified version after failed recheck",()=>{
 const r=record();r.selfTests!.push({id:"input",status:"failed",version:"v1",cycle:1,observations:"input broken",recordedAt:"now"});
 render(<VerificationPackage record={r} active en={false}/>);expect(screen.queryByRole("button",{name:"启动此版本试玩"})).toBeNull();
 expect(screen.getByRole("button",{name:"定位试玩包"})).toBeTruthy();
});
it("opens a previous checkpoint without accepting the unfinished current candidate",async()=>{
 const r=record();r.currentVersion="v2";
 r.policy={implementationExecution:{mode:"autonomous",inPlaceRecoveryVersion:1}} as ProductionRecord["policy"];
 r.workbench!.execution!.verificationPackage!.sourceRevision=12;
 render(<VerificationPackage record={r} active en={false}/>);
 fireEvent.click(screen.getByRole("button",{name:"预览可玩检查点"}));
 await waitFor(()=>expect(productionApi.startPreview).toHaveBeenCalledExactlyOnceWith("task"));
 expect(productionApi.startAcceptance).not.toHaveBeenCalled();
 expect(screen.queryByRole("button",{name:"启动此版本试玩"})).toBeNull();
});
