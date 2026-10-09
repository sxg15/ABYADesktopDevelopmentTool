// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { ExecutionSettings } from "./ExecutionSettings";
import { terminalApi } from "../../shared/api";
vi.mock("../../shared/api", () => ({
  terminalApi: { executionSettings: vi.fn(), updateExecutionSettings: vi.fn() },
  errorMessage: (e: Error) => e.message,
}));
const fixture = () => ({
  settings: {
    model: "test-a",
    effort: "high",
    approvalPolicy: "on-request",
    approvalsReviewer: "auto_review",
    sandboxPolicy: { type: "workspaceWrite" },
  },
  models: [
    {
      model: "test-a",
      displayName: "Test A",
      defaultReasoningEffort: "high",
      supportedReasoningEfforts: [{ reasoningEffort: "high", description: "" }],
    },
    {
      model: "test-b",
      displayName: "Test B",
      defaultReasoningEffort: "medium",
      supportedReasoningEfforts: [
        { reasoningEffort: "medium", description: "" },
      ],
    },
  ],
  running: true,
  connected: true,
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
it("uses advertised effort and applies future settings only after native success", async () => {
  vi.mocked(terminalApi.executionSettings).mockResolvedValue(fixture());
  const updated = fixture();
  updated.settings.model = "test-b";
  updated.settings.effort = "medium";
  vi.mocked(terminalApi.updateExecutionSettings).mockResolvedValue(updated);
  render(
    <ExecutionSettings
      taskId="task"
      conversationId="conversation"
      connected
      onResume={vi.fn()}
    />,
  );
  await screen.findByText("Test B");
  fireEvent.change(screen.getByLabelText("模型"), {
    target: { value: "test-b" },
  });
  expect((screen.getByLabelText("思考强度") as HTMLSelectElement).value).toBe(
    "medium",
  );
  fireEvent.click(screen.getByText("下一轮应用"));
  await waitFor(() =>
    expect(terminalApi.updateExecutionSettings).toHaveBeenCalledWith(
      "task",
      "conversation",
      { model: "test-b", effort: "medium", applyNow: false },
    ),
  );
  await screen.findByText(/将用于下一轮/);
});
it("reports a rejected permission change without claiming it took effect", async () => {
  vi.mocked(terminalApi.executionSettings).mockResolvedValue(fixture());
  vi.mocked(terminalApi.updateExecutionSettings).mockRejectedValue(
    new Error("Managed policy denied this setting"),
  );
  const resume = vi.fn();
  render(
    <ExecutionSettings
      taskId="task"
      conversationId="conversation"
      connected
      onResume={resume}
    />,
  );
  await screen.findByText("Test A");
  fireEvent.change(screen.getByLabelText("权限"), {
    target: { value: "full-access" },
  });
  fireEvent.click(screen.getByText("暂停并应用"));
  await screen.findByRole("alert");
  expect(screen.queryByText("执行设置已生效。")).toBeNull();
  expect(resume).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("取消"));
  expect((screen.getByLabelText("权限") as HTMLSelectElement).value).toBe(
    "auto-review",
  );
});
