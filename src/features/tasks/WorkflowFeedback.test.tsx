// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { AcceptanceLauncher } from "./AcceptanceLauncher";
import { VisualGallery } from "./VisualGallery";
import { FeedbackPanel } from "./FeedbackPanel";
import { productionApi, type ProductionRecord } from "../../shared/production";
vi.mock("../../shared/production", () => ({ productionApi: {
  acceptanceStatus: vi.fn(), startAcceptance: vi.fn(), media: vi.fn(), feedback: vi.fn(),
} }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const record = (): ProductionRecord => ({
  taskId:"task",revision:3,workflowVersion:"2.0.0",questionMode:"ask",currentStage:"review",currentRound:0,
  cycle:1,stages:{review:"awaiting-feedback"},documents:{},approvals:[],currentVersion:"v2",versionDetails:{},
  issues:[],evidence:[],rounds:[],milestones:{},knowledge:[],skillPins:[],updatedAt:"now",feedback:[],
});

it("opens only on an explicit click and prevents duplicate launch while pending", async () => {
  vi.mocked(productionApi.acceptanceStatus).mockResolvedValue({status:"not-started"});
  let finish!: (x: {status:string;message:string}) => void;
  vi.mocked(productionApi.startAcceptance).mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  render(<AcceptanceLauncher taskId="task" active en />);
  expect(productionApi.startAcceptance).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", {name:"Start acceptance"}));
  expect((screen.getByRole("button", {name:"Opening…"}) as HTMLButtonElement).disabled).toBe(true);
  finish({status:"ready",message:"Ready for human play"});
  await screen.findByText("Ready for human play");
  expect(productionApi.startAcceptance).toHaveBeenCalledTimes(1);
});

it("does not display a prior task's late readiness result", async () => {
  vi.mocked(productionApi.acceptanceStatus).mockResolvedValue({status:"not-started"});
  let finish!: (x: {status:string;message:string}) => void;
  vi.mocked(productionApi.startAcceptance).mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const page=render(<AcceptanceLauncher taskId="old" active en />);
  fireEvent.click(screen.getByRole("button",{name:"Start acceptance"}));
  page.rerender(<AcceptanceLauncher taskId="new" active en />);
  finish({status:"ready",message:"OLD READY"});
  await waitFor(() => expect(screen.queryByText("OLD READY")).toBeNull());
});

it("shows source/version labels and reports a media hash failure without displaying the file", async () => {
  vi.mocked(productionApi.media).mockRejectedValue(new Error("Image changed"));
  render(<VisualGallery taskId="task" en artifacts={[{id:"image",path:"artifacts/preview.png",stage:"resources",
    title:"Board design",status:"draft",mediaType:"image/png",sourceType:"mockup",visualVersion:"v1",temporary:true}]} />);
  expect(screen.getByText(/mockup · v1/)).toBeTruthy();
  await screen.findByText(/Image changed/);
  expect(screen.queryByRole("img")).toBeNull();
});

it("saves feedback before continuation and never silently closes a stale candidate", async () => {
  const r=record();r.feedback=[{id:"f",description:"Earlier problem",status:"awaiting-recheck",stage:"review",
    candidateVersion:"v1",createdAt:"now",updatedAt:"now"}];
  vi.mocked(productionApi.feedback).mockResolvedValue({record:r,policy:{workflowVersion:"2.0.0",stages:[],dimensions:[],questions:[]},warnings:[],reportPaths:[],availableUpdate:false});
  const next=vi.fn().mockRejectedValue(new Error("Conversation disconnected"));
  render(<FeedbackPanel taskId="task" record={r} active en onUpdated={vi.fn()} onContinue={next} />);
  expect((screen.getByRole("button",{name:"Verified, close"}) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(screen.getByRole("textbox"),{target:{value:"The score is wrong"}});
  fireEvent.click(screen.getByRole("button",{name:"Submit and continue"}));
  await screen.findByText(/Feedback saved.*Conversation disconnected/);
  expect(productionApi.feedback).toHaveBeenCalledWith("task",3,"add",{description:"The score is wrong",artifactId:undefined,attachmentIds:[]});
  expect(next).toHaveBeenCalledTimes(1);
});
