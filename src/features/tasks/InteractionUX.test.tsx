// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { DocumentReview } from "./DocumentReview";
import { VisualGallery } from "./VisualGallery";
import { FeedbackPanel } from "./FeedbackPanel";
import { TaskGuidance } from "./TaskGuidance";
import { productionApi, type ProductionView, type TaskControlState } from "../../shared/production";
import { useTaskAttention } from "../../shared/useTaskAttention";
import { attentionFixture } from "../../shared/notifications.test-fixtures";
vi.mock("../../shared/useTaskAttention", () => ({useTaskAttention:vi.fn()}));
vi.mock("../../shared/production", () => ({productionApi:{media:vi.fn(),get:vi.fn(),feedback:vi.fn()}}));
beforeEach(() => vi.mocked(useTaskAttention).mockReturnValue(attentionFixture()));
afterEach(() => {cleanup(); vi.resetAllMocks();});
const fixture = (): ProductionView => ({
  policy:{workflowVersion:"2.1.0",stages:[{id:"resources",name:"美术需求",approval:"resources"}],dimensions:[],questions:[]},
  actions:{review:{resources:""},approve:{resources:""},feedback:{gameplay:"尚无作品"}},
  record:{taskId:"task",revision:2,workflowVersion:"2.1.0",questionMode:"ask",currentStage:"resources",currentRound:0,cycle:1,
    stages:{resources:"awaiting-confirmation"},documents:{resources:{kind:"resources",revision:1,path:"art.md",sha256:"old",content:"# 美术需求确认单\n复用现有资源",submittedAt:"now"}},
    approvals:[],versionDetails:{},issues:[],evidence:[],rounds:[],milestones:{},knowledge:[],skillPins:[],updatedAt:"now"},
  warnings:[],reportPaths:[],availableUpdate:false,
});

it("keeps approval unselected, preserves feedback on version change, and requires rereading", () => {
  const view=fixture(), decide=vi.fn();
  const props={taskId:"task",document:view.record!.documents.resources,view,active:true,busy:false,error:"",onClose:vi.fn(),onDecide:decide};
  const page=render(<DocumentReview {...props} />);
  const button=screen.getByRole("button",{name:"确认此版本并继续"}) as HTMLButtonElement;
  expect(button.disabled).toBe(true);
  fireEvent.click(screen.getByLabelText("需要修改"));
  fireEvent.change(screen.getByRole("textbox"),{target:{value:"调整颜色"}});
  const next=structuredClone(view); next.record!.documents.resources.sha256="new"; next.record!.documents.resources.revision=2;
  page.rerender(<DocumentReview {...props} view={next} />);
  expect((screen.getByRole("button",{name:"提交修改意见"}) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.keyDown(screen.getByRole("textbox"),{key:"Enter"});
  expect(decide).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("阅读最新版本（保留意见）"));
  expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("调整颜色");
  expect((screen.getByLabelText("需要修改") as HTMLInputElement).checked).toBe(false);
});

it("clicking the image opens a zoomable preview and Escape returns focus without feedback", async () => {
  vi.mocked(productionApi.media).mockResolvedValue("data:image/png;base64,aGVsbG8=");
  render(<VisualGallery taskId="task" artifacts={[{id:"a",title:"概念图",path:"a.png",stage:"resources",status:"draft",mediaType:"image/png",visualVersion:"v1"}]} />);
  const thumbnail=screen.getByRole("button",{name:"查看大图 概念图"}); thumbnail.focus();
  fireEvent.click(thumbnail);
  await screen.findByRole("dialog");
  fireEvent.click(screen.getByRole("button",{name:"放大"}));
  expect(screen.getByText("125%")).toBeTruthy();
  fireEvent.keyDown(document,{key:"Escape"});
  expect(screen.queryByRole("dialog")).toBeNull(); expect(document.activeElement).toBe(thumbnail);
  expect(productionApi.feedback).not.toHaveBeenCalled();
});

it("does not expose a feedback editor without a submitted artifact or playable version", () => {
  render(<FeedbackPanel taskId="task" record={fixture().record!} active actions={{gameplay:"尚无可验收作品"}} onUpdated={vi.fn()} />);
  expect(screen.queryByRole("textbox")).toBeNull();
  expect((screen.getByText("查看试玩版本并反馈") as HTMLButtonElement).disabled).toBe(true);
  expect(productionApi.feedback).not.toHaveBeenCalled();
});

it("gives human questions priority over running AI and explains waiting when none exist", async () => {
  vi.mocked(useTaskAttention).mockReturnValue({taskId:"task",items:[],unavailable:false});
  const view=fixture(); view.actions!.review={};
  vi.mocked(productionApi.get).mockResolvedValue(view);
  const props={taskId:"task",active:true,control:{state:"running"} as TaskControlState,onStage:vi.fn(),onTerminal:vi.fn()};
  const page=render(<TaskGuidance {...props} />);
  await screen.findByRole("heading",{name:"AI 正在执行"});
  expect(screen.getByText("当前无需操作，请等待结果。")).toBeTruthy();
  view.record!.questionGroups=[{id:"q",title:"玩法",provider:"codex",conversationId:"c",status:"pending",publishedAt:"now",updatedAt:"now",draft:{},answers:[],questions:[{id:"a",text:"请选择",options:[],optional:false}]}];
  vi.mocked(useTaskAttention).mockReturnValue(attentionFixture("question"));
  page.unmount(); render(<TaskGuidance {...props} />);
  await screen.findByText(/需要你回答 1 个问题/);
  expect(screen.getByText("开始回答")).toBeTruthy();
});

it("directs an observed native approval to the terminal", async () => {
  vi.mocked(useTaskAttention).mockReturnValue(attentionFixture("terminal"));
  vi.mocked(productionApi.get).mockResolvedValue(fixture()); const terminal=vi.fn();
  render(<TaskGuidance taskId="task" active control={{state:"waitingForTerminal"} as TaskControlState} onStage={vi.fn()} onTerminal={terminal} />);
  fireEvent.click(await screen.findByText("前往终端处理")); await waitFor(() => expect(terminal).toHaveBeenCalledOnce());
});

it("opens the exact review snapshot and reports pending badges without an approval mutation", async () => {
  const view=fixture(), open=vi.fn(), pending=vi.fn(); vi.mocked(productionApi.get).mockResolvedValue(view);
  render(<TaskGuidance taskId="task" active onStage={vi.fn()} onTerminal={vi.fn()} onReview={open} onPendingChange={pending} />);
  fireEvent.click(await screen.findByRole("button",{name:"查看并审查"}));
  expect(open).toHaveBeenCalledWith(view.record!.documents.resources);
  expect(pending).toHaveBeenLastCalledWith({taskId:"task",questions:0,reviews:1,terminal:0});
  expect(screen.getByText("处理位置：制作流程")).toBeTruthy();
});

it("clears stale human actions when a later status read fails", async () => {
  vi.mocked(productionApi.get).mockResolvedValueOnce(fixture()).mockRejectedValue(new Error("offline"));
  render(<TaskGuidance taskId="task" active onStage={vi.fn()} onTerminal={vi.fn()} />);
  await screen.findByText("查看并审查");
  window.dispatchEvent(new Event("abya:control-changed"));
  await screen.findByText("正在核对任务状态"); expect(screen.queryByText("查看并审查")).toBeNull();
});
