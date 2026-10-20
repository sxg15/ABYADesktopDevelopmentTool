// @vitest-environment jsdom
import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ArtWorkflowPanel } from "./ArtWorkflowPanel";
import { FeedbackPanel } from "./FeedbackPanel";
import { productionApi, type ProductionView } from "../../shared/production";
vi.mock("../../shared/production", () => ({productionApi:{media:vi.fn(async()=>"data:image/png;base64,AA=="),feedback:vi.fn(),revealDocument:vi.fn(),revealReport:vi.fn()}}));
afterEach(()=>{cleanup();vi.clearAllMocks();});

function fixture(resolved = false): ProductionView {
  return {
    policy:{workflowVersion:"2.1.0",stages:[{id:"resources",name:"Art",approval:"resources"}],dimensions:[],questions:[]},
    approvalStatuses:{resources:""},warnings:[],reportPaths:[],availableUpdate:false,
    actions:{review:{resources:"Not awaiting review"},approve:{resources:"Not awaiting review"},feedback:{},recheck:{issue:""},resume:{resources:resolved?"":"Feedback remains open"}},
    record:{taskId:"task",revision:251,workflowVersion:"2.1.0",questionMode:"ask",currentStage:"resources",currentRound:0,cycle:2,
      stages:{resources:"in-progress",plan:"not-started"},
      documents:{resources:{kind:"resources",revision:6,path:"art.md",sha256:"approved-hash",content:"Approved proposal",submittedAt:"now",artifactBindings:{"new.png":"new-hash"}}},
      artifacts:[{id:"old",path:"old.png",title:"Original",stage:"resources",status:"draft",cycle:1,groupId:"direction",visualVersion:"art-v3",sourceType:"mockup",mediaType:"image/png",sha256:"old-hash"},
        {id:"new",path:"new.png",title:"Current",stage:"resources",status:"draft",cycle:2,groupId:"direction",visualVersion:"art-v5.1",sourceType:"mockup",mediaType:"image/png",sha256:"new-hash"}],
      feedback:[{id:"issue",stage:"resources",description:"Remove inventory numbers",artifactId:"old",candidateArtifactId:"new",status:resolved?"resolved":"awaiting-recheck",createdAt:"now",updatedAt:"now"}],
      approvals:[{kind:"resources",documentRevision:6,documentHash:"approved-hash",decision:"accepted",feedback:"",decidedAt:"now"}],
      versionDetails:{},issues:[],evidence:[],rounds:[],milestones:{},knowledge:[],skillPins:[],updatedAt:"now"}
  };
}

it("keeps the approved document visible and continues after explicit feedback verification",async()=>{
  const initial=fixture(),next=fixture(true),continuation=vi.fn();
  next.record!.revision++;next.record!.currentStage="plan";next.record!.stages={resources:"passed",plan:"in-progress"};next.actions!.resume={resources:"Not needed"};
  vi.mocked(productionApi.feedback).mockResolvedValue(next);
  function Harness(){const[view,setView]=useState(initial);return <ArtWorkflowPanel taskId="task" view={view} active en onUpdated={setView} onReview={vi.fn()} onContinue={continuation}/>;}
  render(<Harness/>);
  expect(screen.getByText("Confirmed")).toBeTruthy();
  expect(document.querySelector(".art-next-action")?.classList.contains("needs-action")).toBe(true);
  fireEvent.click(screen.getByRole("button",{name:"Recheck fixes"}));
  fireEvent.click(screen.getByRole("button",{name:"Review fix"}));
  expect(productionApi.feedback).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"Verified, close this issue"}));
  await waitFor(()=>expect(continuation).toHaveBeenCalledWith(expect.stringContaining("系统已恢复计划阶段"),252));
  expect(productionApi.feedback).toHaveBeenCalledTimes(1);
  expect(productionApi.feedback).toHaveBeenCalledWith("task",251,"resolve",{id:"issue"});
  expect(screen.queryByText("Waiting for AI to submit the proposal")).toBeNull();
});

it("offers explicit legacy recovery and retains success when continuation is disconnected",async()=>{
  const initial=fixture(true),next=structuredClone(initial),continuation=vi.fn().mockRejectedValue(new Error("Connect the original conversation"));
  next.record!.revision++;next.record!.currentStage="plan";next.record!.stages.resources="passed";next.actions!.resume={resources:"Not needed"};
  vi.mocked(productionApi.feedback).mockResolvedValue(next);
  function Harness(){const[view,setView]=useState(initial);return <ArtWorkflowPanel taskId="task" view={view} active en onUpdated={setView} onReview={vi.fn()} onContinue={continuation}/>;}
  render(<Harness/>);
  fireEvent.click(screen.getByRole("button",{name:"Restore execution plan"}));
  await screen.findByText("Plan restored. Connect the original conversation");
  expect(productionApi.feedback).toHaveBeenCalledWith("task",251,"resume-art",{});
  expect(screen.queryByRole("button",{name:"Restore execution plan"})).toBeNull();
});

it("confirms art reopen impact and reason from the generic plan feedback panel before continuing AI",async()=>{
  const view=fixture(true),next=fixture(),continuation=vi.fn().mockRejectedValue(new Error("Reconnect first"));
  next.record!.revision++;next.record!.feedback![0].status="open";
  vi.mocked(productionApi.feedback).mockResolvedValue(next);
  render(<FeedbackPanel taskId="task" record={view.record!} active en onUpdated={vi.fn()} onContinue={continuation}/>);
  fireEvent.click(screen.getByRole("button",{name:"Reopen"}));
  expect(productionApi.feedback).not.toHaveBeenCalled();
  expect(screen.getByText("Original proposal：art-v3")).toBeTruthy();
  expect(screen.getByText("Current proposals：art-v5.1")).toBeTruthy();
  expect((screen.getByRole("button",{name:"Reopen and ask AI to revise"}) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
  expect(productionApi.feedback).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"Reopen"}));
  fireEvent.change(screen.getByRole("textbox"),{target:{value:"Check the current pin row"}});
  fireEvent.click(screen.getByRole("button",{name:"Reopen and ask AI to revise"}));
  await waitFor(()=>expect(continuation).toHaveBeenCalledWith(expect.stringContaining("重新打开了美术反馈"),252));
  expect(productionApi.feedback).toHaveBeenCalledWith("task",251,"reopen",{id:"issue",reason:"Check the current pin row"});
  await screen.findByText("Request saved. Reconnect first");
});

it("opens and focuses the requested recheck without changing records",async()=>{
  render(<ArtWorkflowPanel taskId="task" view={fixture()} active en onUpdated={vi.fn()} onReview={vi.fn()} feedbackRequest={1} feedbackId="issue"/>);
  await waitFor(()=>expect((document.activeElement as HTMLElement).dataset.feedbackId).toBe("issue"));
  expect((screen.getByRole("button",{name:"Review fix"}) as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(screen.getByRole("tab",{name:"Current proposal"}));
  expect(productionApi.feedback).not.toHaveBeenCalled();
});

it("does not offer recovery or current approval when backend checks report changed content",()=>{
  const view=fixture(true);view.approvalStatuses={resources:"The bound image changed"};view.actions!.resume={resources:"The bound image changed"};
  render(<ArtWorkflowPanel taskId="task" view={view} active en onUpdated={vi.fn()} onReview={vi.fn()}/>);
  expect(screen.queryByText("Confirmed")).toBeNull();
  expect(screen.queryByRole("button",{name:"Restore execution plan"})).toBeNull();
});
