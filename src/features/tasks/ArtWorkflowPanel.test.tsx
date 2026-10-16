// @vitest-environment jsdom
import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ArtWorkflowPanel } from "./ArtWorkflowPanel";
import { DocumentReview } from "./DocumentReview";
import { productionApi, type ProductionView } from "../../shared/production";
vi.mock("../../shared/production",()=>({productionApi:{media:vi.fn(async()=>"data:image/png;base64,AA=="),feedback:vi.fn(),revealDocument:vi.fn(),revealReport:vi.fn()}}));
afterEach(()=>{cleanup();vi.clearAllMocks();});
const fixture = ():ProductionView => ({
  policy:{workflowVersion:"2.1.0",stages:[{id:"resources",name:"Art",approval:"resources"}],dimensions:[],questions:[]},warnings:[],reportPaths:[],availableUpdate:false,
  actions:{review:{resources:""},approve:{resources:"1 art feedback item needs recheck"},feedback:{new:"",old:"Not current"},recheck:{fix:""}},
  record:{taskId:"task",revision:7,workflowVersion:"2.1.0",questionMode:"ask",currentStage:"resources",currentRound:0,cycle:1,stages:{resources:"awaiting-confirmation"},
    documents:{resources:{kind:"resources",revision:4,path:"art.md",sha256:"doc",content:"# Current art",submittedAt:"now",artifactBindings:{"new.png":"new-hash"}}},
    artifacts:[{id:"old",path:"old.png",title:"Old proposal",stage:"resources",status:"draft",cycle:1,groupId:"layout",sourceType:"mockup",mediaType:"image/png",sha256:"old-hash"},
      {id:"new",path:"new.png",title:"Latest proposal",stage:"resources",status:"draft",cycle:1,groupId:"layout",sourceType:"mockup",mediaType:"image/png",sha256:"new-hash"}],
    feedback:[{id:"fix",stage:"resources",description:"Change the spacing",artifactId:"old",candidateArtifactId:"new",status:"awaiting-recheck",createdAt:"now",updatedAt:"now",fix:"Adjusted spacing",recheck:"Compare images"}],
    approvals:[],versionDetails:{},issues:[],evidence:[],rounds:[],milestones:{},knowledge:[],skillPins:[],updatedAt:"now"}
});
it("shows latest images only, keeps references optional and has no gameplay feedback entry",async()=>{
  render(<ArtWorkflowPanel taskId="task" view={fixture()} active en onUpdated={vi.fn()} onReview={vi.fn()}/>);
  expect(screen.getByRole("button",{name:"Preview Latest proposal"})).toBeTruthy();
  expect(screen.queryByRole("button",{name:"Preview Old proposal"})).toBeNull();
  expect(screen.queryByText("Review playable version / feedback")).toBeNull();
  await waitFor(()=>expect(productionApi.media).toHaveBeenCalledWith("task","new"));
  expect(productionApi.media).not.toHaveBeenCalledWith("task","old");
  expect(screen.queryByRole("textbox")).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"References (optional) · 0"}));
  expect(screen.getByRole("dialog")).toBeTruthy();
  expect(screen.getByRole("group",{name:"Image paste area"})).toBeTruthy();
});
it("requires explicit fix verification before opening final approval and never auto-approves",async()=>{
  const initial=fixture(), review=vi.fn();
  const resolved=structuredClone(initial);resolved.record!.revision++;resolved.record!.feedback![0].status="resolved";resolved.actions!.approve.resources="";resolved.actions!.recheck={fix:"Already closed"};
  vi.mocked(productionApi.feedback).mockResolvedValue(resolved);
  function Harness(){const[v,setV]=useState(initial);return <ArtWorkflowPanel taskId="task" view={v} active en onUpdated={setV} onReview={review}/>;}
  render(<Harness/>);
  fireEvent.click(screen.getByRole("button",{name:"Recheck fixes"}));
  fireEvent.click(screen.getByRole("button",{name:"Review fix"}));
  expect(productionApi.feedback).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"Verified, close this issue"}));
  await waitFor(()=>expect(screen.queryByRole("dialog")).toBeNull());
  expect(productionApi.feedback).toHaveBeenCalledWith("task",7,"resolve",{id:"fix"});
  expect(review).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"Continue to proposal approval"}));
  expect(review).toHaveBeenCalledWith(resolved.record!.documents.resources);
});
it("explains outdated candidates and prevents approving when checks are missing",()=>{
  const v=fixture();v.actions!.recheck={fix:"AI must update the latest candidate"};
  const rendered=render(<ArtWorkflowPanel taskId="task" view={v} active en onUpdated={vi.fn()} onReview={vi.fn()} feedbackRequest={1}/>);
  expect(screen.getByText("AI must update the latest candidate")).toBeTruthy();
  expect((screen.getByRole("button",{name:"Review fix"}) as HTMLButtonElement).disabled).toBe(true);
  rendered.unmount(); v.actions!.approve={}; const route=vi.fn(), decide=vi.fn();
  render(<DocumentReview taskId="task" document={v.record!.documents.resources} view={v} active busy={false} error="" en onClose={vi.fn()} onDecide={decide} onArtFeedback={route}/>);
  fireEvent.click(screen.getByLabelText("Approve this version"));
  expect(screen.getByText("Approval checks are unavailable. Refresh before approving.")).toBeTruthy();
  expect((screen.getByRole("button",{name:"Confirm this version and continue"}) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button",{name:"Go to changes and rechecks"}));expect(route).toHaveBeenCalled();expect(decide).not.toHaveBeenCalled();
});
it("reopens stale recheck material with a saved reason before requesting AI continuation",async()=>{
  const v=fixture(), continuation=vi.fn().mockResolvedValue(undefined), review=vi.fn();
  v.actions!.recheck={fix:"AI must update the latest candidate"};
  const reopened=structuredClone(v);reopened.record!.revision=8;reopened.record!.feedback![0].status="open";
  reopened.record!.stages.resources="in-progress";reopened.actions!.review.resources="Wait for AI";
  vi.mocked(productionApi.feedback).mockResolvedValue(reopened);
  function Harness(){const [value,setValue]=useState(v);return <ArtWorkflowPanel taskId="task" view={value} active en onUpdated={setValue} onReview={review} onContinue={continuation} feedbackRequest={1}/>;}
  render(<Harness/>);
  fireEvent.click(screen.getByRole("button",{name:"Ask AI to update recheck material"}));
  await waitFor(()=>expect(continuation).toHaveBeenCalledWith(expect.any(String),8));
  expect(productionApi.feedback).toHaveBeenCalledWith("task",7,"reopen",expect.objectContaining({id:"fix",reason:expect.stringContaining("当前最新版方案")}));
  expect(review).not.toHaveBeenCalled();
  expect(screen.queryByRole("button",{name:"Ask AI to update recheck material"})).toBeNull();
});
