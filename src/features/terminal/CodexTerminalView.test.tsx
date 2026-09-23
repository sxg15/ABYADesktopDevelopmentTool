// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { translator } from "../../i18n";
import type { CodexConversation } from "../../shared/types";
import { DevelopmentTerminalView } from "./CodexTerminalView";

const conversation: CodexConversation = {
  id: "conversation-1",
  taskId: "task-1",
  title: "Original title",
  createdAt: "2026-08-31T01:00:00Z",
  updatedAt: "2026-08-31T01:00:00Z",
};

const terminalApiMock = vi.hoisted(() => ({
  listConversations: vi.fn(async () => [conversation]),
  createConversation: vi.fn(),
  renameConversation: vi.fn(async () => ({
    ...conversation,
    title: "Renamed conversation",
  })),
  deleteConversation: vi.fn(),
  workflow: vi.fn(async () => ({
    taskId: "task-1",
    conversationId: "conversation-1",
    observability: "native",
    warning: "",
    turns: [],
    updatedAt: "2026-08-31T01:00:00Z",
  })),
  open: vi.fn(async () => ({
    taskId: "task-1",
    conversationId: "conversation-1",
    status: "running",
    pid: 10,
    workingDirectory: "D:\\Workspaces\\task-1",
    lastError: "",
  })),
  write: vi.fn(async () => undefined),
  resize: vi.fn(async () => undefined),
  stop: vi.fn(async () => undefined),
}));

const terminalGrid = vi.hoisted(() => ({ cols: 80, rows: 24 }));
const resizeObservers = vi.hoisted(() => ({
  callbacks: [] as ResizeObserverCallback[],
}));
const terminalChannels = vi.hoisted(
  () => [] as Array<{ onmessage?: (value: unknown) => void }>,
);

vi.mock("../../shared/api", () => ({
  terminalApi: terminalApiMock,
  errorMessage: (value: unknown) => String(value),
}));

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage?: (value: unknown) => void;
    constructor() {
      terminalChannels.push(this);
    }
  },
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));

vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    buffer = { active: { baseY: 0, viewportY: 0 } };
    get cols() {
      return terminalGrid.cols;
    }
    get rows() {
      return terminalGrid.rows;
    }
    loadAddon() {}
    open() {}
    onData() {
      return { dispose() {} };
    }
    onResize() {
      return { dispose() {} };
    }
    onScroll() {
      return { dispose() {} };
    }
    reset() {}
    write(_data: string, callback?: () => void) {
      callback?.();
    }
    scrollToLine() {}
    scrollToBottom() {}
    focus() {}
    dispose() {}
  },
}));

beforeAll(() => {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      callback: ResizeObserverCallback;
      constructor(callback: ResizeObserverCallback) {
        this.callback = callback;
        resizeObservers.callbacks.push(callback);
      }
      observe() {}
      disconnect() {
        resizeObservers.callbacks = resizeObservers.callbacks.filter(
          (callback) => callback !== this.callback,
        );
      }
    },
  );
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    callback(0);
    return 1;
  });
  vi.stubGlobal("cancelAnimationFrame", () => undefined);
});

afterEach(() => {
  cleanup();
  terminalGrid.cols = 80;
  terminalGrid.rows = 24;
  resizeObservers.callbacks = [];
  terminalChannels.length = 0;
  vi.clearAllMocks();
});

const availability = {
  available: true,
  workingDirectory: "D:\\Workspaces\\task-1",
  reason: "",
  observabilityAvailable: true,
  observabilityReason: "",
};

function renderView(visible = true) {
  return render(
    <DevelopmentTerminalView
      provider="codex"
      taskId="task-1"
      visible={visible}
      availability={availability}
      t={translator("en-US")}
      onRefreshAvailability={vi.fn()}
    />,
  );
}

function giveHostLayout(width = 800, height = 480) {
  const host = document.querySelector(".terminal-host");
  expect(host).toBeTruthy();
  Object.defineProperty(host as HTMLElement, "clientWidth", {
    configurable: true,
    get: () => width,
  });
  Object.defineProperty(host as HTMLElement, "clientHeight", {
    configurable: true,
    get: () => height,
  });
}

function notifyResize() {
  for (const callback of [...resizeObservers.callbacks]) {
    callback([], {} as ResizeObserver);
  }
}

describe("Codex conversation list", () => {
  it("renames a conversation inline and persists the new title", async () => {
    renderView();

    fireEvent.click(
      await screen.findByRole("button", { name: "Rename conversation" }),
    );
    const input = screen.getByRole("textbox", { name: "Conversation name" });
    fireEvent.change(input, { target: { value: "Renamed conversation" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(terminalApiMock.renameConversation).toHaveBeenCalledWith(
        "codex",
        "task-1",
        "conversation-1",
        "Renamed conversation",
      );
    });
    expect(
      (await screen.findAllByText("Renamed conversation")).length,
    ).toBeGreaterThanOrEqual(1);
  });
});

describe("terminal open sizing", () => {
  it("does not open a PTY while the host has no layout", async () => {
    renderView();
    await screen.findAllByText("Original title");
    expect(terminalApiMock.open).not.toHaveBeenCalled();
  });

  it("opens after the host receives a laid-out size", async () => {
    renderView();
    await screen.findAllByText("Original title");
    giveHostLayout();
    notifyResize();
    await waitFor(() => {
      expect(terminalApiMock.open).toHaveBeenCalledWith(
        "codex",
        "task-1",
        "conversation-1",
        80,
        24,
        expect.anything(),
      );
    });
  });

  it("defers opening until the terminal is visible", async () => {
    const view = renderView(false);
    await screen.findAllByText("Original title");
    giveHostLayout();
    notifyResize();
    expect(terminalApiMock.open).not.toHaveBeenCalled();

    view.rerender(
      <DevelopmentTerminalView
        provider="codex"
        taskId="task-1"
        visible
        availability={availability}
        t={translator("en-US")}
        onRefreshAvailability={vi.fn()}
      />,
    );
    await waitFor(() => {
      expect(terminalApiMock.open).toHaveBeenCalledTimes(1);
    });
  });

  it("clamps undersized dimensions before opening", async () => {
    terminalGrid.cols = 2;
    terminalGrid.rows = 1;
    renderView();
    await screen.findAllByText("Original title");
    giveHostLayout(40, 20);
    notifyResize();
    await waitFor(() => {
      expect(terminalApiMock.open).toHaveBeenCalledWith(
        "codex",
        "task-1",
        "conversation-1",
        20,
        5,
        expect.anything(),
      );
    });
  });

  it("clamps oversized dimensions before opening", async () => {
    terminalGrid.cols = 800;
    terminalGrid.rows = 400;
    renderView();
    await screen.findAllByText("Original title");
    giveHostLayout(4000, 3000);
    notifyResize();
    await waitFor(() => {
      expect(terminalApiMock.open).toHaveBeenCalledWith(
        "codex",
        "task-1",
        "conversation-1",
        500,
        200,
        expect.anything(),
      );
    });
  });
});

describe("terminal output history", () => {
  it("keeps a bounded recent window when output is very large", async () => {
    renderView();
    await screen.findAllByText("Original title");
    giveHostLayout();
    notifyResize();
    await waitFor(() => expect(terminalApiMock.open).toHaveBeenCalledTimes(1));

    terminalChannels[0]?.onmessage?.({
      type: "output",
      data: `old-${"x".repeat(2 * 1024 * 1024)}-recent`,
    });
    fireEvent.click(screen.getByRole("button", { name: "Output history" }));

    const history = document.querySelector(".terminal-history pre");
    expect(history?.querySelector(".terminal-history-truncated")).toBeTruthy();
    expect(history?.textContent).toContain("-recent");
    expect(history?.textContent?.length).toBeLessThan(2 * 1024 * 1024 + 100);
  });
});
