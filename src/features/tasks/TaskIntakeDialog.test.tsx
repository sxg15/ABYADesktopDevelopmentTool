// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { TaskIntakeDialog } from "./TaskIntakeDialog";
import { productionApi, type ProductionView } from "../../shared/production";
vi.mock("../../shared/production", async original => ({...await original<typeof import("../../shared/production")>(), productionApi: { get:vi.fn(),answer:vi.fn(),submitAnswers:vi.fn(),flushContinuation:vi.fn() } }));
const view: ProductionView = { record: { taskId: "task", revision: 1,
  workflowVersion: "1.2.0", questionMode: "ask", currentStage: "requirements", currentRound: 0,
  cycle: 1, stages: {}, documents: {}, approvals: [], issues: [], evidence: [], rounds: [],
  milestones: {}, versionDetails: {}, knowledge: [], skillPins: [], updatedAt: "now",
  questionGroups: [{ id: "rules", title: "玩法人数",
  provider: "codex", conversationId: "chat", status: "pending", draft: {}, answers: [], publishedAt: "now", updatedAt: "now",
  questions: [{ id: "mode", text: "单人还是多人？", options: ["单人", "多人"], optional: false }] }] },
  policy: { workflowVersion: "1.2.0", stages: [], dimensions: [], questions: [] },
  warnings: [], reportPaths: [], availableUpdate: false };
beforeEach(() => { vi.clearAllMocks(); localStorage.clear(); sessionStorage.clear(); vi.mocked(productionApi.get).mockResolvedValue(view); });
afterEach(cleanup);
it("keeps submit-and-continue alive when polling observes submitted answers before the response", async () => {
  let finish!:(value:Awaited<ReturnType<typeof productionApi.submitAnswers>>)=>void;
  vi.mocked(productionApi.submitAnswers).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
  render(<TaskIntakeDialog taskId="task" active/>); await screen.findByRole("dialog");
  fireEvent.click(screen.getByLabelText("多人"));fireEvent.click(screen.getByText("提交并继续"));
  const saved=structuredClone(view);saved.record!.revision=2;
  saved.record!.questionGroups![0].status="submitted";saved.record!.questionGroups![0].answers=[{revision:1,answers:{mode:"多人"},submittedAt:"now"}];
  vi.mocked(productionApi.get).mockResolvedValue(saved);
  window.dispatchEvent(new CustomEvent("abya:open-intake",{detail:{taskId:"task",groupId:"rules"}}));
  await screen.findByText("修改已提交答案");
  finish({production:saved,continuation:{status:"queued",conversationId:"chat",nativeSessionId:"native",requestId:"request"}});
  await screen.findByText("操作已保存，当前回复结束后继续。");
  expect(productionApi.submitAnswers).toHaveBeenCalledTimes(1);expect(productionApi.answer).not.toHaveBeenCalled();
});
it("opens pending questions without requiring the production tab and keeps drafts after closing", async () => {
  render(<TaskIntakeDialog taskId="task" active />);
  await screen.findByRole("dialog", { name: "需求问答" });
  fireEvent.click(screen.getByLabelText("多人"));
  fireEvent.click(screen.getByText("稍后回答 / 关闭"));
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(productionApi.answer).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("需求有待回答问题 · 继续填写"));
  await screen.findByRole("dialog");
  expect((screen.getByLabelText("单人还是多人？") as HTMLTextAreaElement).value).toBe("多人");
});
it("Escape defers the question without submitting or cancelling it", async () => {
  render(<TaskIntakeDialog taskId="task" active />);
  await screen.findByRole("dialog");
  fireEvent.keyDown(window, { key: "Escape" });
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(productionApi.answer).not.toHaveBeenCalled();
  expect(sessionStorage.getItem("abya-intake-later:task:rules:0")).toBe("1");
});
it("does not reopen deferred questions on mounting again, but the explicit entry works", async () => {
  sessionStorage.setItem("abya-intake-later:task:rules:0", "1");
  render(<TaskIntakeDialog taskId="task" active />);
  await screen.findByText("需求有待回答问题 · 继续填写");
  expect(screen.queryByRole("dialog")).toBeNull();
  window.dispatchEvent(new CustomEvent("abya:open-intake", { detail: { taskId: "task", groupId: "rules" } }));
  await waitFor(() => expect(screen.getByRole("dialog")).toBeTruthy());
});
