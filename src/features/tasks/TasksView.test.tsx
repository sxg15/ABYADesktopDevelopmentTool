// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { translator } from "../../i18n";
import { api } from "../../shared/api";
import type {
  CodexConversation,
  CodexWorkflowSnapshot,
  DevelopmentTask,
} from "../../shared/types";
import { TasksView } from "./TasksView";

const tasks: DevelopmentTask[] = [
  {
    id: "task-active",
    title: "Active task",
    description: "",
    status: "active",
    createdAt: "2026-08-27T00:00:00Z",
    updatedAt: "2026-08-27T00:00:00Z",
    workspacePath: "D:\\Workspaces\\active",
  },
  {
    id: "task-archived",
    title: "Archived task",
    description: "",
    status: "archived",
    createdAt: "2026-08-27T00:00:00Z",
    updatedAt: "2026-08-27T00:00:00Z",
    workspacePath: "D:\\Workspaces\\archived",
  },
];

const conversations: CodexConversation[] = [
  {
    id: "conversation-active",
    taskId: "task-active",
    title: "Active conversation",
    createdAt: "2026-08-31T00:00:00Z",
    updatedAt: "2026-08-31T00:00:00Z",
  },
  {
    id: "conversation-alternate",
    taskId: "task-active",
    title: "Alternate conversation",
    createdAt: "2026-08-30T00:00:00Z",
    updatedAt: "2026-08-30T00:00:00Z",
  },
];

const workflow: CodexWorkflowSnapshot = {
  taskId: "task-active",
  conversationId: "conversation-active",
  observability: "native",
  warning: "",
  currentTurnId: "turn-active",
  updatedAt: "2026-08-31T00:00:00Z",
  turns: [
    {
      id: "turn-active",
      status: "inProgress",
      explanation: "",
      startedAt: "2026-08-31T00:00:00Z",
      plan: [{ id: "step-active", step: "Inspect files", status: "inProgress" }],
      activities: [],
    },
  ],
};

const alternateWorkflow: CodexWorkflowSnapshot = {
  ...workflow,
  conversationId: "conversation-alternate",
  currentTurnId: "turn-alternate",
  turns: [
    {
      id: "turn-alternate",
      status: "inProgress",
      explanation: "",
      startedAt: "2026-08-31T00:00:00Z",
      plan: [
        {
          id: "step-alternate",
          step: "Run gameplay test",
          status: "inProgress",
        },
      ],
      activities: [],
    },
  ],
};

vi.mock("../../shared/api", () => ({
  api: {
    listTasks: vi.fn(async () => tasks),
    listInstances: vi.fn(async () => []),
    setTaskStatus: vi.fn(),
    deleteTask: vi.fn(),
    stopInstance: vi.fn(),
    deleteInstance: vi.fn(),
  },
  terminalApi: {
    listConversations: vi.fn(
      async (provider: string, taskId: string) =>
        provider === "codex" && taskId === "task-active" ? conversations : [],
    ),
    workflow: vi.fn(
      async (_provider: string, _taskId: string, conversationId: string) =>
        conversationId === "conversation-alternate"
          ? alternateWorkflow
          : workflow,
    ),
    availability: vi.fn(async () => ({
      available: true,
      workingDirectory: "D:\\ABYADesktopDevelopmentTool",
      reason: "",
      observabilityAvailable: true,
      observabilityReason: "",
    })),
  },
  errorMessage: (value: unknown) => String(value),
}));

vi.mock("../terminal/CodexTerminalView", () => ({
  DevelopmentTerminalView: ({
    provider,
    taskId,
    visible,
    onConversationChange,
  }: {
    provider: "codex" | "grok";
    taskId: string;
    visible: boolean;
    onConversationChange?: (conversation?: CodexConversation) => void;
  }) => (
    <div
      data-testid={`terminal-${taskId}-${provider}`}
      data-visible={String(visible)}
    >
      Terminal {taskId} {provider}
      {provider === "codex" && taskId === "task-active" && (
        <button
          onClick={() =>
            onConversationChange?.({
              id: "conversation-alternate",
              taskId,
              title: "Alternate conversation",
              createdAt: "2026-08-30T00:00:00Z",
              updatedAt: "2026-08-30T00:00:00Z",
            })
          }
        >
          Select alternate conversation
        </button>
      )}
    </div>
  ),
}));

afterEach(() => {
  cleanup();
  window.localStorage.clear();
  vi.mocked(api.deleteTask).mockReset();
  vi.mocked(api.deleteTask).mockResolvedValue(undefined);
});

describe("development task tabs", () => {
  it("defaults to game instances and keeps task terminals mounted across tabs", async () => {
    render(
      <TasksView
        t={translator("en-US")}
        notify={vi.fn()}
      />,
    );

    const instancesTab = await screen.findByRole("tab", {
      name: "Game instances",
    });
    const terminalTab = screen.getByRole("tab", { name: "Terminal" });
    expect(instancesTab.getAttribute("aria-selected")).toBe("true");
    expect(await screen.findByText("Inspect files")).toBeTruthy();

    fireEvent.click(terminalTab);
    const activeTerminal = await screen.findByTestId(
      "terminal-task-active-codex",
    );
    expect(activeTerminal.getAttribute("data-visible")).toBe("true");

    fireEvent.click(instancesTab);
    expect(activeTerminal.getAttribute("data-visible")).toBe("false");
    expect(screen.getByText("Inspect files")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: /Archived task/ }));
    fireEvent.click(screen.getByRole("tab", { name: "Terminal" }));
    const archivedTerminal = await screen.findByTestId(
      "terminal-task-archived-codex",
    );
    expect(archivedTerminal.getAttribute("data-visible")).toBe("true");

    fireEvent.click(screen.getByRole("button", { name: /Active task/ }));
    await waitFor(() => {
      expect(activeTerminal.getAttribute("data-visible")).toBe("false");
    });
    fireEvent.click(screen.getByRole("tab", { name: "Terminal" }));
    expect(activeTerminal.getAttribute("data-visible")).toBe("true");
  });

  it("lets the user explicitly switch a task between Codex and Grok", async () => {
    render(<TasksView t={translator("en-US")} notify={vi.fn()} />);

    fireEvent.click(await screen.findByRole("tab", { name: "Terminal" }));
    expect(
      (await screen.findByTestId("terminal-task-active-codex")).getAttribute(
        "data-visible",
      ),
    ).toBe("true");

    fireEvent.click(screen.getByRole("button", { name: "Grok" }));
    expect(
      (await screen.findByTestId("terminal-task-active-grok")).getAttribute(
        "data-visible",
      ),
    ).toBe("true");
    expect(
      screen
        .getByTestId("terminal-task-active-codex")
        .getAttribute("data-visible"),
    ).toBe("false");
  });

  it("keeps the workflow strip on the task's selected conversation", async () => {
    render(
      <TasksView
        t={translator("en-US")}
        notify={vi.fn()}
      />,
    );

    fireEvent.click(
      await screen.findByRole("tab", { name: "Terminal" }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Select alternate conversation" }),
    );
    expect(await screen.findByText("Run gameplay test")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: /Archived task/ }));
    fireEvent.click(screen.getByRole("button", { name: /Active task/ }));

    await waitFor(() => {
      expect(screen.getByText("Run gameplay test")).toBeTruthy();
      expect(screen.queryByText("Inspect files")).toBeNull();
    });
  });

  it("unmounts opened terminals before deleting the selected task", async () => {
    let release = () => undefined as void;
    const pending = new Promise<void>((resolve) => {
      release = resolve;
    });
    vi.mocked(api.deleteTask).mockImplementation(async () => pending);

    render(<TasksView t={translator("en-US")} notify={vi.fn()} />);
    fireEvent.click(await screen.findByRole("tab", { name: "Terminal" }));
    expect(
      await screen.findByTestId("terminal-task-active-codex"),
    ).toBeTruthy();

    fireEvent.click(screen.getByTitle("Delete"));

    await waitFor(() => {
      expect(screen.queryByTestId("terminal-task-active-codex")).toBeNull();
    });
    expect(api.deleteTask).toHaveBeenCalledWith("task-active");
    release();
    await waitFor(() => {
      expect(api.deleteTask).toHaveBeenCalledTimes(1);
    });
  });
});
