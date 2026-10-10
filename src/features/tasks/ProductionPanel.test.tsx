// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProductionPanel } from "./ProductionPanel";
import { SkillLibrary } from "./SkillLibrary";
import { productionApi, type ProductionView } from "../../shared/production";
vi.mock("../../shared/production", () => ({ productionApi: { get: vi.fn(), update: vi.fn(), decide: vi.fn(), upgrade: vi.fn(), skills: vi.fn(), readSkill: vi.fn(), open: vi.fn(), revealDocument: vi.fn(), revealReport: vi.fn(), answer: vi.fn() } }));
const fixture = (): ProductionView => ({
  policy: { workflowVersion: "1.1.0", stages: [{ id: "requirements", name: "需求", approval: "requirements" }], dimensions: [], questions: [] },
  warnings: [], reportPaths: [], availableUpdate: false,
  record: { taskId: "a", revision: 7, workflowVersion: "1.1.0", questionMode: "ask", currentStage: "requirements", currentRound: 0, cycle: 1,
    stages: { requirements: "awaiting-confirmation" }, documents: { requirements: { kind: "requirements", revision: 2, path: "req.md", sha256: "hash-2", content: "Review this exact version", submittedAt: "now" } },
    approvals: [], versionDetails: {}, issues: [], evidence: [], rounds: [], milestones: {}, knowledge: [], skillPins: [], updatedAt: "now" },
});
afterEach(() => { cleanup(); vi.clearAllMocks(); sessionStorage.clear(); });
describe("Production decisions", () => {
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
    fireEvent.click(screen.getByRole("button", { name: "Accept and continue" }));
    await waitFor(() => expect(productionApi.decide).toHaveBeenCalledWith("a", 7, "requirements", "hash-2", true, ""));
    await waitFor(() => expect(resume).toHaveBeenCalledOnce());
  });
  it("shows stale-document warnings and cannot approve the stale version", async () => {
    const value = fixture(); value.warnings = ["requirements 文件已变化"];
    vi.mocked(productionApi.get).mockResolvedValue(value);
    render(<ProductionPanel taskId="a" active locale="en-US" />);
    const button = await screen.findByRole("button", { name: "Accept and continue" });
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
