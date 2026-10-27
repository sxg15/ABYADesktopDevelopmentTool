// @vitest-environment jsdom
import {cleanup,render,screen,fireEvent,waitFor} from "@testing-library/react";
import {afterEach,it,expect,vi} from "vitest";
import {PlaytestIssues} from "./PlaytestIssues";
import {productionApi,type ProductionRecord} from "../../shared/production";
vi.mock("../../shared/production",()=>({productionApi:{exportDeveloperPackage:vi.fn(),revealDocument:vi.fn()}}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
function record(){return {taskId:"task",revision:7,cycle:2,currentVersion:"v5",versionDetails:{playerBuildHash:"build"},
  evidence:[{id:"proof",version:"v5",cycle:2,reviewed:true}],issues:[{id:"engine",title:"焦点事件未分发",stage:"implementation",kind:"defect",status:"open",
    repairTarget:"abya-source",diagnosisStatus:"confirmed",evidenceIds:["proof"],description:"窗口切回后输入未恢复",
    diagnosis:{reproduction:"切换窗口后返回",comparison:"修复构建对照通过",rootCause:"引擎分发缺失",playerBuildHash:"build"},
    executionHandling:{impact:"影响窗口切换",reason:"重新进入可恢复"}}]} as unknown as ProductionRecord;}
it("shows Lua source defect in review without expanding details and exports only its evidence",async()=>{
  vi.mocked(productionApi.exportDeveloperPackage).mockResolvedValue("reports/defect.zip");
  render(<PlaytestIssues record={record()} en={false}/>);
  expect(screen.getByRole("heading",{name:"焦点事件未分发"})).toBeTruthy();
  expect(screen.getByText("影响窗口切换")).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"导出诊断资料（ZIP）"}));
  await waitFor(()=>expect(productionApi.exportDeveloperPackage).toHaveBeenCalledExactlyOnceWith("task",7,["engine"]));
  await waitFor(()=>expect(productionApi.revealDocument).toHaveBeenCalledWith("task","reports/defect.zip"));
});
it("does not label suspected, Lua or stale build records as confirmed source defects",()=>{
  const r=record();r.issues[0].diagnosisStatus="suspected";
  const v=render(<PlaytestIssues record={r} en={false}/>);
  expect(screen.queryByRole("button",{name:"导出诊断资料（ZIP）"})).toBeNull();
  r.issues[0].diagnosisStatus="confirmed";r.issues[0].diagnosis!.playerBuildHash="old";v.rerender(<PlaytestIssues record={r} en={false}/>);
  expect(screen.queryByRole("heading",{name:/已知 Abya-PB/})).toBeNull();
  expect(screen.getByText(/仍有问题需要 AI/)).toBeTruthy();
  r.issues[0].status="resolved";v.rerender(<PlaytestIssues record={r} en={false}/>);
  expect(screen.queryByLabelText("试玩问题")).toBeNull();
});
