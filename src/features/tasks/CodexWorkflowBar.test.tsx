// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { translator } from "../../i18n";
import type { CodexConversation, CodexWorkflowSnapshot } from "../../shared/types";
import { CodexWorkflowBar } from "./CodexWorkflowBar";

const conversation: CodexConversation = {
  id: "conversation-1",
  taskId: "task-1",
  title: "Gameplay implementation",
  createdAt: "2026-08-31T01:00:00Z",
  updatedAt: "2026-08-31T01:00:00Z",
};

const workflow: CodexWorkflowSnapshot = {
  taskId: "task-1",
  conversationId: "conversation-1",
  observability: "native",
  warning: "",
  currentTurnId: "turn-1",
  updatedAt: "2026-08-31T01:01:00Z",
  turns: [
    {
      id: "turn-1",
      status: "inProgress",
      explanation: "Inspect, implement, and verify",
      startedAt: "2026-08-31T01:00:00Z",
      plan: [
        { id: "step-1", step: "Inspect files", status: "completed" },
        { id: "step-2", step: "Run tests", status: "inProgress" },
      ],
      activities: [
        {
          id: "activity-1",
          stepId: "step-2",
          kind: "test",
          status: "started",
          summary: "Inspecting game logs",
          detail: "{\"sessionId\":\"session-1\"}",
          source: "desktopMcp",
          unplanned: false,
          startedAt: "2026-08-31T01:01:00Z",
        },
      ],
    },
  ],
};

afterEach(cleanup);

describe("Codex workflow bar", () => {
  it("shows recent step activity on hover and opens the complete timeline", () => {
    render(
      <CodexWorkflowBar
        provider="codex"
        conversation={conversation}
        workflow={workflow}
        t={translator("en-US")}
      />,
    );

    const step = screen.getByRole("button", {
      name: "Run tests: In progress",
    });
    fireEvent.mouseEnter(step);
    expect(screen.getByRole("tooltip").textContent).toContain(
      "Inspecting game logs",
    );

    fireEvent.click(step);
    expect(
      screen.getByRole("dialog", { name: "Complete execution timeline" })
        .textContent,
    ).toContain("Inspect, implement, and verify");
    expect(screen.getByText('{"sessionId":"session-1"}')).toBeTruthy();
  });

  it("shows a compatibility warning when native events are unavailable", () => {
    render(
      <CodexWorkflowBar
        provider="codex"
        conversation={conversation}
        workflow={{
          ...workflow,
          observability: "compatibility",
          warning: "Legacy Codex",
        }}
        t={translator("en-US")}
      />,
    );

    expect(
      screen.getByText(
        /This Codex version is not observable/,
      ),
    ).toBeTruthy();
  });
});
