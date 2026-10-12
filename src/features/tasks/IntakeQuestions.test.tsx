// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { IntakeQuestions } from "./IntakeQuestions";
import { productionApi, type IntakeQuestionGroup, type ProductionView } from "../../shared/production";
vi.mock("../../shared/production", async importOriginal => ({ ...await importOriginal<typeof import("../../shared/production")>(), productionApi: { answer:vi.fn(),submitAnswers:vi.fn(),continueQuestions:vi.fn(),flushContinuation:vi.fn() } }));
const group: IntakeQuestionGroup = { id: "rules", title: "玩法需求", provider: "codex", conversationId: "chat",
  nativeSessionId: "native", status: "pending", publishedAt: "now", updatedAt: "now", draft: {}, answers: [],
  questions: [{ id: "a", text: "玩法人数？", options: ["单人", "多人"], optional: false },
    { id: "b", text: "如何计分？", options: [], optional: false }] };
beforeEach(() => { vi.clearAllMocks(); localStorage.clear(); });
afterEach(() => { cleanup(); vi.useRealTimers(); });
const submitted: IntakeQuestionGroup = { ...group, status: "submitted", answers: [{revision:1, answers:{a:"多人",b:"按时间"},submittedAt:"now"}] };
const result = (status: "needsConnection" | "resumed" | "completed" | "running") => ({ status, conversationId:"chat", nativeSessionId:"native", requestId:"request" });
it("connects the originating terminal and then resumes once with an accurate message", async () => {
  vi.mocked(productionApi.continueQuestions).mockResolvedValueOnce(result("needsConnection"));
  vi.mocked(productionApi.flushContinuation).mockResolvedValueOnce(result("resumed"));
  const connect = vi.fn(async () => undefined);
  render(<IntakeQuestions taskId="task" group={submitted} revision={1} active onSaved={vi.fn()} onEnsureConnection={connect} />);
  fireEvent.click(screen.getByText("继续任务"));
  await screen.findByText("已恢复中断的执行，将先核对已完成操作。");
  expect(connect).toHaveBeenCalledWith(submitted);
  expect(productionApi.continueQuestions).toHaveBeenCalledTimes(1);
  expect(productionApi.flushContinuation).toHaveBeenCalledWith("task","chat","request");
});
it("does not claim sent or connect a terminal when the old attempt is completed", async () => {
  vi.mocked(productionApi.continueQuestions).mockResolvedValue(result("completed"));
  const connect = vi.fn();
  render(<IntakeQuestions taskId="task" group={submitted} revision={1} active onSaved={vi.fn()} onEnsureConnection={connect} />);
  fireEvent.click(screen.getByText("继续任务"));
  await screen.findByText("本组答案已处理完成，未重复发送。"); expect(connect).not.toHaveBeenCalled();
});
it("never retries sending when terminal connection fails", async () => {
  vi.mocked(productionApi.continueQuestions).mockResolvedValue(result("needsConnection"));
  render(<IntakeQuestions taskId="task" group={submitted} revision={1} active onSaved={vi.fn()}
    onEnsureConnection={async () => { throw new Error("连接失败"); }} />);
  fireEvent.click(screen.getByText("继续任务")); await screen.findByText("连接失败");
  expect(productionApi.continueQuestions).toHaveBeenCalledTimes(1);
});
it("points to the first missing answer without asking to refresh or sending a mutation", async () => {
  render(<IntakeQuestions taskId="task" group={group} revision={1} active onSaved={vi.fn()} />);
  fireEvent.click(screen.getByText("下一题"));
  fireEvent.click(screen.getByText("提交并继续"));
  expect(screen.getByRole("alert").textContent).toBe("请先回答第 1 题，再提交本组答案。");
  expect(productionApi.answer).not.toHaveBeenCalled();
});
it("keeps previous answers when navigating and restores an unsubmitted local draft", async () => {
  const props = { taskId: "task", group, revision: 1, active: true, onSaved: vi.fn() };
  const page = render(<IntakeQuestions {...props} />);
  expect(productionApi.answer).not.toHaveBeenCalled();
  fireEvent.click(screen.getByLabelText("单人"));
  fireEvent.click(screen.getByText("下一题"));
  fireEvent.change(screen.getByLabelText("如何计分？"), { target: { value: "按时间" } });
  fireEvent.click(screen.getByText("上一题"));
  expect((screen.getByLabelText("单人") as HTMLInputElement).checked).toBe(true);
  page.unmount(); render(<IntakeQuestions {...props} />);
  expect((screen.getByLabelText("单人") as HTMLInputElement).checked).toBe(true);
  fireEvent.click(screen.getByText("下一题"));
  expect((screen.getByLabelText("如何计分？") as HTMLTextAreaElement).value).toBe("按时间");
});
it("submits answers with one durable continuation request without approving documents", async () => {
  vi.mocked(productionApi.submitAnswers).mockResolvedValue({production:{ record: { revision: 2 } } as ProductionView, continuation:{...result("running"),status:"queued"}});
  const onContinue = vi.fn();
  render(<IntakeQuestions taskId="task" group={group} revision={1} active onSaved={vi.fn()} onContinue={onContinue} />);
  fireEvent.click(screen.getByLabelText("多人"));
  fireEvent.click(screen.getByText("下一题"));
  fireEvent.change(screen.getByLabelText("如何计分？"), { target: { value: "按步数" } });
  fireEvent.click(screen.getByText("提交并继续"));
  await waitFor(() => expect(productionApi.submitAnswers).toHaveBeenCalledWith("task",1,"rules",{a:"多人",b:"按步数"}));
  expect(onContinue).not.toHaveBeenCalled();
  expect(productionApi.continueQuestions).not.toHaveBeenCalled();
});
it("retains the draft when the database rejects a stale revision", async () => {
  vi.mocked(productionApi.answer).mockRejectedValue(new Error("记录已变化"));
  render(<IntakeQuestions taskId="task" group={group} revision={1} active onSaved={vi.fn()} />);
  fireEvent.click(screen.getByLabelText("单人"));
  await screen.findByText(/自动保存失败/);
  expect(localStorage.getItem("abya-intake:task:rules")).toContain("单人");
});

it("keeps selection separate from notes and retains Other text when switching choices", () => {
  render(<IntakeQuestions taskId="task" group={group} revision={1} active onSaved={vi.fn()} />);
  fireEvent.click(screen.getByLabelText("其他"));
  fireEvent.change(screen.getByLabelText("玩法人数？"), {target:{value:"三队合作"}});
  fireEvent.click(screen.getByLabelText("单人"));
  fireEvent.change(screen.getByLabelText("玩法人数？ 补充说明"), {target:{value:"可带机器人"}});
  expect((screen.getByLabelText("单人") as HTMLInputElement).checked).toBe(true);
  fireEvent.click(screen.getByLabelText("其他"));
  expect((screen.getByLabelText("玩法人数？") as HTMLTextAreaElement).value).toBe("三队合作");
  expect(screen.queryByText("保存草稿")).toBeNull();
  expect(screen.queryByText("取消本组问题")).toBeNull();
});

it("autosaves without disabling or replacing focused input, and saves edits made during the request", async () => {
  vi.useFakeTimers();
  let finish!: (v: ProductionView) => void;
  vi.mocked(productionApi.answer).mockImplementationOnce(() => new Promise(resolve => {finish=resolve;}))
    .mockResolvedValue({record:{revision:3}} as ProductionView);
  const props = {taskId:"task",group,revision:1,active:true,onSaved:vi.fn()};
  const page = render(<IntakeQuestions {...props} />);
  fireEvent.click(screen.getByLabelText("其他"));
  const input = screen.getByLabelText("玩法人数？") as HTMLTextAreaElement;
  input.focus(); fireEvent.change(input,{target:{value:"第一版"}});
  await act(async () => {vi.advanceTimersByTime(800);});
  expect(input.disabled).toBe(false); expect(input.closest("fieldset")!.disabled).toBe(false);
  expect(document.activeElement).toBe(input);
  fireEvent.change(input,{target:{value:"保存过程中继续输入"}});
  page.rerender(<IntakeQuestions {...props} revision={2} group={{...group,draft:{a:"旧快照"}}} />);
  expect(screen.getByLabelText("玩法人数？")).toBe(input);
  await act(async () => {finish({record:{revision:2}} as ProductionView);});
  expect(input.value).toBe("保存过程中继续输入"); expect(document.activeElement).toBe(input);
  await act(async () => {vi.advanceTimersByTime(800);});
  expect(productionApi.answer).toHaveBeenLastCalledWith("task",2,"save-draft",{id:"rules",answers:{a:"保存过程中继续输入",b:""}});
});

it("does not save while an IME composition is active", async () => {
  vi.useFakeTimers();
  vi.mocked(productionApi.answer).mockResolvedValue({record:{revision:2}} as ProductionView);
  render(<IntakeQuestions taskId="task" group={group} revision={1} active onSaved={vi.fn()} />);
  fireEvent.click(screen.getByLabelText("其他"));
  const input = screen.getByLabelText("玩法人数？");
  fireEvent.compositionStart(input); fireEvent.change(input,{target:{value:"zhongwen"}});
  await act(async () => {vi.advanceTimersByTime(900);});
  expect(productionApi.answer).not.toHaveBeenCalled();
  fireEvent.change(input,{target:{value:"中文"}});
  await act(async () => {fireEvent.compositionEnd(input);});
  expect(productionApi.answer).toHaveBeenCalledTimes(1);
});
