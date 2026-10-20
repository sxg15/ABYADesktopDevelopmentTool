// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { translator } from "../../i18n";
import { productionApi, type ProductionView } from "../../shared/production";
import { attentionFixture } from "../../shared/notifications.test-fixtures";
import type { NotificationActivation } from "../../shared/notifications";
import type { TerminalConnectionRequest } from "../terminal/terminalConnection";
import { TasksView } from "./TasksView";
import { useEffect } from "react";
vi.mock("../../shared/api",()=>({api:{
  listTasks:vi.fn(async()=>["first","target"].map(id=>({id,title:id,status:"active",workspacePath:"D:/fixture/"+id,createdAt:"now",updatedAt:"now"}))),
  listInstances:vi.fn(async()=>[]),
},terminalApi:{availability:vi.fn(async()=>({available:true})),listConversations:vi.fn(async()=>[])},errorMessage:String}));
vi.mock("../../shared/production",()=>({productionApi:{get:vi.fn(),continueTask:vi.fn(),answer:vi.fn(),decide:vi.fn()},continuationMessages:{}}));
vi.mock("../terminal/useTaskControl",()=>({useTaskControl:()=>undefined}));
vi.mock("./TaskGuidance",()=>({TaskGuidance:()=>null}));
vi.mock("./CodexWorkflowBar",()=>({CodexWorkflowBar:()=>null}));
vi.mock("./TaskIntakeDialog",()=>({TaskIntakeDialog:({requestedGroup}:{requestedGroup?:{groupId:string}})=><div>{requestedGroup ? "question:"+requestedGroup.groupId : ""}</div>}));
vi.mock("./ProductionPanel",()=>({ProductionPanel:({taskId,requestedReview,requestedFeedback}:{taskId:string;requestedReview?:{sha256:string};requestedFeedback?:{id:string}})=><div>{"stage:"+taskId}{requestedReview ? " review:"+requestedReview.sha256 : ""}{requestedFeedback ? " feedback:"+requestedFeedback.id : ""}</div>}));
vi.mock("../terminal/CodexTerminalView",()=>({DevelopmentTerminalView:({connectionRequest}:{connectionRequest?:TerminalConnectionRequest})=>{
  useEffect(()=>{connectionRequest?.finish();},[connectionRequest?.id]);
  return <div>{connectionRequest ? "terminal:"+connectionRequest.provider+":"+connectionRequest.conversationId : ""}</div>;
}}));
const view=()=>({policy:{stages:[]},actions:{review:{resources:""}},record:{taskId:"target",currentStage:"resources",
  documents:{resources:{kind:"resources",revision:1,sha256:"old"}},questionGroups:[{id:"q",stage:"resources",status:"pending"}]
}} as unknown as ProductionView);
const nav=(kind:"question"|"review"|"terminal"):NotificationActivation=>({id:kind,status:"current",target:attentionFixture(kind,"target").items[0].target});
beforeEach(()=>vi.mocked(productionApi.get).mockResolvedValue(view()));
afterEach(()=>{cleanup();vi.clearAllMocks();localStorage.clear();sessionStorage.clear();});
it.each(["question","review"] as const)("opens the exact %s entry on an unselected task without submitting",async(kind)=>{
  const handled=vi.fn();render(<TasksView t={translator("en-US")} notify={vi.fn()} notificationNavigation={nav(kind)} onNotificationHandled={handled}/>);
  await screen.findByText(kind==="question" ? "question:q" : "stage:target review:old");
  expect(productionApi.continueTask).not.toHaveBeenCalled();expect(productionApi.answer).not.toHaveBeenCalled();expect(productionApi.decide).not.toHaveBeenCalled();
  await waitFor(()=>expect(handled).toHaveBeenCalledWith(kind));
});
it("does not open the approval dialog when the document version changed",async()=>{
  const next=view();next.record!.documents.resources.sha256="changed";vi.mocked(productionApi.get).mockResolvedValue(next);
  const notify=vi.fn(),handled=vi.fn();render(<TasksView t={translator("en-US")} notify={notify} notificationNavigation={nav("review")} onNotificationHandled={handled}/>);
  await waitFor(()=>expect(handled).toHaveBeenCalled());
  expect(screen.getByText("stage:target")).toBeTruthy();expect(screen.queryByText(/review:/)).toBeNull();expect(notify).toHaveBeenCalled();
  expect(productionApi.decide).not.toHaveBeenCalled();
});
it("requests attachment to the original conversation without continuing",async()=>{
  const handled=vi.fn();render(<TasksView t={translator("en-US")} notify={vi.fn()} notificationNavigation={nav("terminal")} onNotificationHandled={handled}/>);
  await waitFor(()=>expect(handled).toHaveBeenCalledWith("terminal"));
  expect(screen.getByRole("tab",{name:"Terminal"}).getAttribute("aria-selected")).toBe("true");
  expect(productionApi.continueTask).not.toHaveBeenCalled();
});
it("discards stale version reads after a newer notification navigation",async()=>{
  let finish!:(value:ProductionView)=>void;
  vi.mocked(productionApi.get).mockReturnValueOnce(new Promise(resolve=>{finish=resolve;})).mockResolvedValue(view());
  const props={t:translator("en-US"),notify:vi.fn(),onNotificationHandled:vi.fn()};
  const page=render(<TasksView {...props} notificationNavigation={nav("review")}/>);
  await waitFor(()=>expect(productionApi.get).toHaveBeenCalled());
  page.rerender(<TasksView {...props} notificationNavigation={nav("question")}/>);await screen.findByText("question:q");
  await act(async()=>finish(view()));expect(screen.queryByText(/review:old/)).toBeNull();
});

it.each([false,true])("routes recheck notification to its exact candidate and rejects stale=%s",async(stale)=>{
  const current=view();current.actions!.recheck={issue:""};
  current.record!.feedback=[{id:"issue",stage:"resources",status:"awaiting-recheck",artifactId:"original",candidateArtifactId:stale?"changed":"current",description:"Check pins",createdAt:"now",updatedAt:"now"}];
  vi.mocked(productionApi.get).mockResolvedValue(current);
  const activation=nav("review");activation.id="recheck";
  activation.target={...activation.target!,action:"recheck",feedbackId:"issue",candidateArtifactId:"current"};
  const handled=vi.fn();render(<TasksView t={translator("en-US")} notify={vi.fn()} notificationNavigation={activation} onNotificationHandled={handled}/>);
  await waitFor(()=>expect(handled).toHaveBeenCalledWith("recheck"));
  expect(screen.queryByText("stage:target feedback:issue") !== null).toBe(!stale);
  expect(productionApi.continueTask).not.toHaveBeenCalled();expect(productionApi.decide).not.toHaveBeenCalled();
});
