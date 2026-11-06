// @vitest-environment jsdom
import {render,screen,cleanup,fireEvent,waitFor,within} from "@testing-library/react";
import {afterEach,it,expect,vi} from "vitest";
import type {ReactNode} from "react";
import {ProductionWorkbench} from "./ProductionWorkbench";
import {productionApi,type ProductionView} from "../../shared/production";
vi.mock("../../shared/production",()=>({productionApi:{get:vi.fn(),reconcile:vi.fn(),workbench:vi.fn(),snapshot:vi.fn(),startPreview:vi.fn()}}));
vi.mock("./ProductionPanel",()=>({ProductionPanel:()=>null}));
vi.mock("./TaskIntakeDialog",()=>({TaskIntakeDialog:()=>null}));
vi.mock("./AutonomousExecution",()=>({AutonomousExecution:()=>null}));
vi.mock("./VisualGallery",()=>({VisualGallery:()=>null}));
vi.mock("./WorkbenchSplit",()=>({WorkbenchSplit:({children}:{children:ReactNode})=><>{children}</>}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
function fixture(active="new"){
 const policy={stages:[{id:"review",name:"试玩与修改"}]};
 return {policy,warnings:[],availableUpdate:false,record:{taskId:"task",revision:8,cycle:1,policy,currentStage:"review",currentVersion:"previous-saved",stages:{review:"awaiting-feedback"},documents:{},artifacts:[],issues:[],
  workbench:{activeGoalId:active,goals:["old","new"].map((id,i)=>({id,number:i+1,title:id==="old"?"旧中断意见":"新插针修改",stage:"review",cycle:1,sourceType:"request",status:"pending",recordRevision:7,summary:id})),
   drafts:{},attempts:[{id:"a",goalId:"old",status:"interrupted"},{id:"b",goalId:"new",status:"finished"}],execution:{state:"paused"}}}} as unknown as ProductionView;
}
it("keeps the finished current request editable and returns from old history to that request",async()=>{
 const view=fixture();vi.mocked(productionApi.get).mockResolvedValue(view);vi.mocked(productionApi.reconcile).mockResolvedValue(view);
 vi.mocked(productionApi.snapshot).mockResolvedValue(view.record!);vi.mocked(productionApi.startPreview).mockResolvedValue({status:"ready"});
 const resume=vi.fn().mockResolvedValue({status:"resumed"});
 render(<ProductionWorkbench taskId="task" active visible onTerminal={()=>{}} onEnsureConnection={vi.fn()} onResumeGoal={resume}/>);
 const area=await screen.findByRole("region",{name:"任务交互"});
 expect(within(area).getByRole("button",{name:"启动当前修改版试玩"})).toBeTruthy();
 fireEvent.change(within(area).getByRole("textbox",{name:"补充要求或试玩反馈"}),{target:{value:"保留我的草稿"}});
 fireEvent.click(screen.getByRole("button",{name:/旧中断意见/}));
 expect(await screen.findByText("历史快照 · 只读")).toBeTruthy();
 expect(screen.queryByRole("button",{name:"启动当前修改版试玩"})).toBeNull();
 fireEvent.click(screen.getByRole("button",{name:"回到当前工作"}));
 expect((within(area).getByRole("textbox",{name:"补充要求或试玩反馈"}) as HTMLTextAreaElement).value).toBe("保留我的草稿");
 fireEvent.click(within(area).getByRole("button",{name:"启动当前修改版试玩"}));
 await waitFor(()=>expect(productionApi.startPreview).toHaveBeenCalledWith("task","previous-saved",undefined));
 fireEvent.click(within(area).getByRole("button",{name:"继续此子流程"}));
 await waitFor(()=>expect(resume).toHaveBeenCalledWith("new",8,expect.any(String),undefined));
});
it("follows a newly submitted request even if the prior current goal was explicitly selected",async()=>{
 const old=fixture("old"),next=fixture();next.record!.revision=9;
 vi.mocked(productionApi.get).mockResolvedValue(old);vi.mocked(productionApi.reconcile).mockResolvedValue(old);
 vi.mocked(productionApi.workbench).mockResolvedValue(next);
 const continuation=vi.fn().mockResolvedValue(undefined);
 render(<ProductionWorkbench taskId="task" active visible onTerminal={()=>{}} onEnsureConnection={vi.fn()} onContinue={continuation}/>);
 fireEvent.click(await screen.findByRole("button",{name:/旧中断意见/}));
 fireEvent.change(screen.getByRole("textbox",{name:"补充要求或试玩反馈"}),{target:{value:"插入的针要保留"}});
 fireEvent.click(screen.getByRole("button",{name:"提交并继续"}));
 await waitFor(()=>expect(continuation).toHaveBeenCalledWith("",undefined,9));
 expect(screen.queryByText("历史快照 · 只读")).toBeNull();
 expect(screen.getByRole("button",{name:"启动当前修改版试玩"})).toBeTruthy();
 expect(screen.getByRole("button",{name:/新插针修改/}).className).toBe("selected");
});
