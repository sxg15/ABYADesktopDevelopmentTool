// @vitest-environment jsdom
import {render,screen,cleanup} from "@testing-library/react";
import {afterEach,it,expect,vi} from "vitest";
import type {ReactNode} from "react";
import {ProductionWorkbench} from "./ProductionWorkbench";
import {productionApi,type ProductionView} from "../../shared/production";
vi.mock("../../shared/production",()=>({productionApi:{get:vi.fn(),reconcile:vi.fn(),preflight:vi.fn()}}));
vi.mock("../../shared/api",()=>({errorMessage:String}));
vi.mock("./ProductionPanel",()=>({ProductionPanel:()=>null}));
vi.mock("./TaskIntakeDialog",()=>({TaskIntakeDialog:()=>null}));
vi.mock("./AutonomousExecution",()=>({AutonomousExecution:()=>null}));
vi.mock("./VisualGallery",()=>({VisualGallery:()=>null}));
vi.mock("./WorkbenchSplit",()=>({WorkbenchSplit:({children}:{children:ReactNode})=><>{children}</>}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
function fixture(platform:boolean){
 const policy={capabilityPreflightVersion:1,stages:[{id:"plan",name:"方案预检"}],workflowVersion:"2.11.0"};
 const view={policy,warnings:[],availableUpdate:false,record:{taskId:"task",revision:2,cycle:1,workflowVersion:"2.11.0",policy,currentStage:"plan",stages:{plan:"in-progress"},documents:{},artifacts:[],
  issues:[{id:"preflight:basic",title:"基础能力预检未通过",description:"真实失败原因",stage:"plan",status:"open",kind:"blocker",preflightRunId:"run",repairTarget:platform?"abya-build":"other",diagnosisStatus:platform?"confirmed":"suspected"}],
  workbench:{goals:[],drafts:{},attempts:[],deferrals:[],execution:{capabilityPreflight:{runId:"run",state:"failed",phase:"create",checks:[],cleanup:{state:"completed"}}}}}} as unknown as ProductionView;
 vi.mocked(productionApi.get).mockResolvedValue(view);vi.mocked(productionApi.reconcile).mockResolvedValue(view);
 render(<ProductionWorkbench taskId="task" active visible onTerminal={()=>{}} onEnsureConnection={vi.fn()}/>);
}
it("shows preflight and an expanded platform card on the default workbench",async()=>{
 fixture(true);
 expect(await screen.findByRole("region",{name:"基础能力预检"})).toBeTruthy();
 const reason=await screen.findByText("真实失败原因");
 expect((reason.closest("details") as HTMLDetailsElement).open).toBe(true);
 expect(reason.closest(".programmer-issue")).not.toBeNull();
 expect(screen.getByText(/ABYA-PB 包兼容问题/)).toBeTruthy();
});
it("keeps ordinary preflight failures visible without platform styling",async()=>{
 fixture(false);const reason=await screen.findByText("真实失败原因");
 expect((reason.closest("details") as HTMLDetailsElement).open).toBe(true);
 expect(reason.closest(".programmer-issue")).toBeNull();
});
