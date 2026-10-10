// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { IntakeQuestions } from "./IntakeQuestions";
import { productionApi, type IntakeQuestionGroup, type ProductionView } from "../../shared/production";
vi.mock("../../shared/production", async importOriginal => ({ ...await importOriginal<typeof import("../../shared/production")>(), productionApi: { answer:vi.fn(),submitAnswers:vi.fn(),continueQuestions:vi.fn(),flushContinuation:vi.fn() } }));
const group: IntakeQuestionGroup = { id: "rules", title: "玩法需求", provider: "codex", conversationId: "chat",
  nativeSessionId: "native", status: "pending", publishedAt: "now", updatedAt: "now", draft: {}, answers: [],
  questions: [{ id: "a", text: "玩法人数？", options: ["单人", "多人"], optional: false },
    { id: "b", text: "如何计分？", options: [], optional: false }] };
beforeEach(() => { vi.clearAllMocks(); localStorage.clear(); });
afterEach(cleanup);
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
  expect((screen.getByLabelText("玩法人数？") as HTMLTextAreaElement).value).toBe("单人");
  page.unmount(); render(<IntakeQuestions {...props} />);
  expect((screen.getByLabelText("玩法人数？") as HTMLTextAreaElement).value).toBe("单人");
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
  fireEvent.click(screen.getByLabelText("单人")); fireEvent.click(screen.getByText("保存草稿"));
  await screen.findByRole("alert");
  expect(localStorage.getItem("abya-intake:task:rules")).toContain("单人");
});
