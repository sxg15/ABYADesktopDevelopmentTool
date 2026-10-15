// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProductionPanel } from "./ProductionPanel";
import { SkillLibrary } from "./SkillLibrary";
import { productionApi, type ProductionView } from "../../shared/production";
vi.mock("../../shared/production", () => ({ productionApi: { get: vi.fn(), update: vi.fn(), decide: vi.fn(), upgrade: vi.fn(), skills: vi.fn(), readSkill: vi.fn(), open: vi.fn(), revealDocument: vi.fn(), revealReport: vi.fn(), answer: vi.fn(), acceptanceStatus:vi.fn(async()=>({status:"not-started"})),startAcceptance:vi.fn() } }));
const fixture = (): ProductionView => ({
  actions: { review: { requirements: "" }, approve: { requirements: "" }, feedback: { gameplay: "尚无作品" } },
  policy: { workflowVersion: "1.1.0", stages: [{ id: "requirements", name: "需求", approval: "requirements" }], dimensions: [], questions: [] },
  warnings: [], reportPaths: [], availableUpdate: false,
  record: { taskId: "a", revision: 7, workflowVersion: "1.1.0", questionMode: "ask", currentStage: "requirements", currentRound: 0, cycle: 1,
    stages: { requirements: "awaiting-confirmation" }, documents: { requirements: { kind: "requirements", revision: 2, path: "req.md", sha256: "hash-2", content: "Review this exact version", submittedAt: "now" } },
    approvals: [], versionDetails: {}, issues: [], evidence: [], rounds: [], milestones: {}, knowledge: [], skillPins: [], updatedAt: "now" },
});
afterEach(() => { cleanup(); vi.clearAllMocks(); sessionStorage.clear(); });
describe("Production decisions", () => {
  it("keeps exactly one acceptance card through stage switches, refreshes and remounts",async()=>{
    const value=fixture();value.policy.iterationMode="human-feedback";
    value.policy.stages=[{id:'resources',name:'Resources',approval:'resources'},{id:'review',name:'Review',approval:null},{id:'delivery',name:'Delivery',approval:'delivery'}];
    value.record!.currentStage='review';value.record!.documents={};value.record!.stages={resources:'passed',review:'awaiting-feedback',delivery:'not-started'};
    vi.mocked(productionApi.get).mockResolvedValue(value);
    const page=render(<ProductionPanel taskId="a" active locale="en-US"/>);
    await screen.findByRole('button',{name:'Start acceptance'});
    for(const stage of ['resources','review','delivery','resources','review','delivery']){
      value.record={...value.record!,currentStage:stage,revision:value.record!.revision+1};
      vi.mocked(productionApi.get).mockResolvedValue({...value});
      const calls=vi.mocked(productionApi.get).mock.calls.length;
      fireEvent.click(screen.getByRole('button',{name:'Refresh'}));
      await waitFor(()=>expect(productionApi.get).toHaveBeenCalledTimes(calls+1));
      await waitFor(()=>expect(screen.queryAllByRole('button',{name:'Start acceptance'})).toHaveLength(stage==='resources'?0:1));
    }
    expect(productionApi.startAcceptance).not.toHaveBeenCalled();
    page.unmount();expect(document.querySelectorAll('.acceptance-launcher')).toHaveLength(0);
    render(<ProductionPanel taskId="a" active locale="en-US"/>);
    await screen.findByRole('button',{name:'Start acceptance'});
    expect(document.querySelectorAll('.acceptance-launcher')).toHaveLength(1);
  });
  it("submits the selected ordinary issue once and does not pretend it is resolved",async()=>{
    const value=fixture();value.record!.stages.requirements="in-progress";
    value.record!.issues=[{id:"lua-bug",kind:"defect",status:"open",stage:"requirements",description:"Wrong Lua callback",repairTarget:"lua"},
      {id:"engine",kind:"blocker",status:"open",stage:"requirements",description:"Engine defect",repairTarget:"abya-source"},
      {id:"done",kind:"defect",status:"resolved",stage:"requirements",description:"Resolved issue"}];
    vi.mocked(productionApi.get).mockResolvedValue(value);
    let finish!:(message:string)=>void;
    const repair=vi.fn(()=>new Promise<string>(resolve=>{finish=resolve;}));
    render(<ProductionPanel taskId="a" active onRepairIssue={repair}/>);
    const button=await screen.findByRole('button',{name:'让 AI 自动修复'});
    expect(screen.getAllByRole('button',{name:'让 AI 自动修复'})).toHaveLength(1);
    fireEvent.click(button);fireEvent.click(button);
    expect(repair).toHaveBeenCalledExactlyOnceWith('lua-bug',7);
    expect((screen.getByRole('button',{name:'正在提交…'}) as HTMLButtonElement).disabled).toBe(true);
    finish('此问题已排队，当前回复结束后处理。');
    await screen.findByText('此问题已排队，当前回复结束后处理。');
    expect(value.record!.issues[0].status).toBe('open');
    expect(productionApi.update).not.toHaveBeenCalled();expect(productionApi.decide).not.toHaveBeenCalled();
  });
  it("keeps an ordinary issue visible after a repair request fails",async()=>{
    const value=fixture();value.record!.issues=[{id:'bug',stage:'requirements',kind:'blocker',status:'open',description:'Missing configuration'}];
    vi.mocked(productionApi.get).mockResolvedValue(value);
    const repair=vi.fn().mockRejectedValue(new Error('请先连接原会话'));
    render(<ProductionPanel taskId="a" active onRepairIssue={repair}/>);
    fireEvent.click(await screen.findByRole('button',{name:'让 AI 自动修复'}));
    await screen.findByText('请先连接原会话');
    expect((screen.getByRole('button',{name:'让 AI 自动修复'}) as HTMLButtonElement).disabled).toBe(false);
  });
  it("highlights source/build problems while keeping resolved records out of the red alert",async()=>{
    const value=fixture();value.record!.currentStage="implementation";value.record!.stages.implementation="blocked";
    value.policy.stages.push({id:"implementation",name:"Lua",approval:null});
    value.record!.issues=[{id:"engine",kind:"blocker",status:"open",stage:"implementation",description:"Input dispatch defect",repairTarget:"abya-source"},
      {id:"old",kind:"defect",status:"resolved",stage:"implementation",description:"Old build issue",repairTarget:"abya-build"}];
    vi.mocked(productionApi.get).mockResolvedValue(value);
    render(<ProductionPanel taskId="a" active/>);
    expect((await screen.findByRole("alert")).textContent).toContain("1 项未解决问题");
    expect(screen.getAllByText("Input dispatch defect")[0].closest("article")?.classList.contains("programmer-issue")).toBe(true);
    expect(screen.getByText(/ABYA 游戏源码缺陷 · 需要程序修复/)).toBeTruthy();
    expect(screen.getAllByText("Old build issue")[0].closest("article")?.classList.contains("programmer-issue")).toBe(false);
    expect(screen.getAllByRole("button",{name:"导出此问题（MD）"})).toHaveLength(2);
    expect(screen.getByRole("button",{name:"导出程序问题汇总（MD）"})).toBeTruthy();
  });
  it("opens a requested review after loading the production tab and leaves the decision unselected", async () => {
    const view=fixture(), opened=vi.fn(); vi.mocked(productionApi.get).mockResolvedValue(view);
    render(<ProductionPanel taskId="a" active locale="en-US" requestedReview={view.record!.documents.requirements} onReviewOpened={opened} />);
    await screen.findByRole("dialog"); expect(screen.getByText("Review this exact version")).toBeTruthy();
    expect(opened).toHaveBeenCalledOnce(); expect(productionApi.decide).not.toHaveBeenCalled();
    expect((screen.getByRole("button",{name:"Confirm this version and continue"}) as HTMLButtonElement).disabled).toBe(true);
  });
  it("filters by selected stage and keeps selection across refreshes without changing production", async () => {
    const value=fixture();value.policy.stages.push({id:"plan",name:"方案",approval:"plan"});
    value.record!.currentStage="plan";value.record!.stages.plan="blocked";
    value.artifacts=[{id:"draft",stage:"plan",title:"Execution draft",path:"plan.md",status:"draft",exists:true}];
    value.record!.issues=[{id:"recording",stage:"plan",kind:"blocker",status:"open",title:"Wrong recording",description:"Wrong window",affectedStages:["plan"]}];
    vi.mocked(productionApi.get).mockResolvedValue(value);
    render(<ProductionPanel taskId="a" active locale="en-US"/>);
    await screen.findByText("Execution draft");expect(screen.queryByText(/Requirements · Version/)).toBeNull();
    fireEvent.click(screen.getByRole("button",{name:/1 Requirements/}));
    expect(screen.queryByText("Execution draft")).toBeNull();expect(screen.queryByText("Wrong recording")).toBeNull();
    fireEvent.click(screen.getByRole("button",{name:"Refresh"}));
    await waitFor(()=>expect(screen.getByRole("button",{name:/1 Requirements/}).getAttribute("aria-pressed")).toBe("true"));
    expect(productionApi.update).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button",{name:/2 Plan/}));
    fireEvent.click(screen.getByRole("button",{name:"Show in File Explorer"}));
    expect(productionApi.revealDocument).toHaveBeenCalledWith("a","plan.md");
  });
  it("submits only the displayed revision and hash after the user's click, then continues", async () => {
    const initial = fixture(); const accepted = fixture();
    accepted.record!.stages.requirements = "passed";
    accepted.record!.approvals = [{ kind: "requirements", documentHash: "hash-2", documentRevision: 2, decision: "accepted", feedback: "", decidedAt: "now" }];
    vi.mocked(productionApi.get).mockResolvedValue(initial);
    vi.mocked(productionApi.decide).mockImplementation(async () => { vi.mocked(productionApi.get).mockResolvedValue(accepted); return accepted; });
    const resume = vi.fn().mockResolvedValue(undefined);
    render(<ProductionPanel taskId="a" active locale="en-US" onContinue={resume} />);
    expect(await screen.findByRole("button", {name:"Show in File Explorer"})).toBeTruthy();
    expect(screen.queryByText("Review this exact version")).toBeNull();
    expect(productionApi.decide).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "View and review" }));
    expect(screen.getByText("Review this exact version")).toBeTruthy();
    expect((screen.getByRole("button", {name:"Confirm this version and continue"}) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByLabelText("Approve this version"));
    fireEvent.click(screen.getByRole("button", { name: "Confirm this version and continue" }));
    await waitFor(() => expect(productionApi.decide).toHaveBeenCalledWith("a", 7, "requirements", "hash-2", true, ""));
    await waitFor(() => expect(resume).toHaveBeenCalledOnce());
  });
  it("shows stale-document warnings and cannot approve the stale version", async () => {
    const value = fixture(); value.warnings = ["requirements 文件已变化"];
    value.actions!.review.requirements = "文件已变化";
    vi.mocked(productionApi.get).mockResolvedValue(value);
    render(<ProductionPanel taskId="a" active locale="en-US" />);
    fireEvent.click(await screen.findByRole("button", { name: "View and review" }));
    const button = screen.getByRole("button", { name: "Confirm this version and continue" });
    expect((button as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(button); expect(productionApi.decide).not.toHaveBeenCalled();
  });
  it("ignores a late response after switching tasks", async () => {
    let resolveOld!: (v: ProductionView) => void;
    vi.mocked(productionApi.get).mockImplementation(id => id === "a" ? new Promise(resolve => { resolveOld = resolve; }) : Promise.resolve({ ...fixture(), record: undefined }));
    const page = render(<ProductionPanel taskId="a" active locale="en-US" />);
    page.rerender(<ProductionPanel taskId="b" active locale="en-US" />);
    await screen.findByText("Production is not enabled");
    resolveOld(fixture());
    await waitFor(() => expect(screen.queryByText("Review this exact version")).toBeNull());
  });
});
it("Skill browsing reads only the selected body and keeps templates explicit", async () => {
  vi.mocked(productionApi.get).mockResolvedValue(fixture());
  vi.mocked(productionApi.skills).mockResolvedValue([{ id: "abya-task-template-demo", name: "Demo template", kind: "task", provider: "codex", description: "A selected workflow", path: ".codex/skills/demo", sha256: "hash" }]);
  vi.mocked(productionApi.readSkill).mockResolvedValue("Selected body");
  render(<SkillLibrary taskId="a" active locale="en-US" />);
  await screen.findByText("Demo template"); expect(productionApi.readSkill).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Read Skill" }));
  await screen.findByText("Selected body");
  expect(productionApi.readSkill).toHaveBeenCalledWith("a", "codex", "abya-task-template-demo");
  expect(productionApi.update).not.toHaveBeenCalled();
});
