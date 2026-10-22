// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { translator } from "../../i18n";
import type { CodexConversation, CodexTerminalState } from "../../shared/types";
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
  readClipboard: vi.fn(async () => ({ kind: "text", text: "pasted" })),
  copyHistory: vi.fn(async () => true),
  projectWorkspace: vi.fn(async () => "D:\\Workspaces\\task-1"),
  setArchived: vi.fn(async (_task: string, _id: string, archived: boolean) => ({ ...conversation, archived })),
  workflow: vi.fn(async () => ({
    taskId: "task-1",
    conversationId: "conversation-1",
    observability: "native",
    warning: "",
    turns: [],
    updatedAt: "2026-08-31T01:00:00Z",
  })),
  open: vi.fn(async ():Promise<CodexTerminalState> => ({
    inputSessionId:"input-session-test",
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
  interrupt: vi.fn(async () => undefined),
  forceStop: vi.fn(async () => undefined),
  rebuild: vi.fn(async () => undefined),
  executionSettings: vi.fn(async () => ({connected:true,running:false,models:[],settings:{model:'fixture'}})),
}));

const terminalGrid = vi.hoisted(() => ({ cols: 80, rows: 24 }));
const productionApiMock=vi.hoisted(()=>({
  control:vi.fn(),continueTask:vi.fn(),flushContinuation:vi.fn(),
}));
vi.mock("../../shared/production",async importOriginal=>({
  ...await importOriginal<typeof import("../../shared/production")>(),
  productionApi:productionApiMock,
}));
beforeEach(()=>{
  productionApiMock.control.mockResolvedValue({taskId:"task-1",conversationId:"conversation-1",
    nativeSessionId:"native-1",state:"paused",connection:"connected",queued:false,lastEventAt:"",updatedAt:""});
  productionApiMock.continueTask.mockResolvedValue({status:"needsConnection",requestId:"request-1",conversationId:"conversation-1",nativeSessionId:"native-1"});
  productionApiMock.flushContinuation.mockResolvedValue({status:"started",turnId:"turn-2",conversationId:"conversation-1",nativeSessionId:"native-1"});
});
const terminalEvents = vi.hoisted(() => ({ data:undefined as undefined|((data:string)=>void), resize: undefined as undefined | ((size:{cols:number;rows:number})=>void), focus:vi.fn() }));
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
    modes = { bracketedPasteMode: true };
    attachCustomKeyEventHandler() {}
    paste() {}
    buffer = { active: { baseY: 0, viewportY: 0 } };
    get cols() {
      return terminalGrid.cols;
    }
    get rows() {
      return terminalGrid.rows;
    }
    loadAddon() {}
    open() {}
    onData(callback:(data:string)=>void) { terminalEvents.data=callback;
      return { dispose() {terminalEvents.data=undefined;} };
    }
    onResize(callback:(size:{cols:number;rows:number})=>void) {
      terminalEvents.resize=callback;
      return { dispose() { terminalEvents.resize=undefined; } };
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
    focus() { terminalEvents.focus(); }
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

it("keeps a failed resume actionable and retries only on an explicit reconnect", async () => {
  terminalApiMock.open.mockRejectedValueOnce(new Error("原对话被其他 Codex 客户端占用；诊断编号：test-resume"));
  renderView();
  await waitFor(() => expect(document.querySelector('.terminal-host')).toBeTruthy());
  giveHostLayout();
  act(notifyResize);
  const alert=await screen.findByRole('alert');
  expect(alert.textContent).toContain('其他 Codex 客户端占用');
  expect(alert.textContent).toContain('test-resume');
  act(notifyResize);
  expect(terminalApiMock.open).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole('button',{name:'接回原对话'}));
  await waitFor(()=>expect(terminalApiMock.open).toHaveBeenCalledTimes(2));
  expect(terminalApiMock.open).toHaveBeenNthCalledWith(2,'codex','task-1','conversation-1',80,24,expect.anything());
  expect(terminalApiMock.createConversation).not.toHaveBeenCalled();
  expect(terminalApiMock.setArchived).not.toHaveBeenCalled();
  expect(productionApiMock.continueTask).not.toHaveBeenCalled();
  expect(productionApiMock.flushContinuation).not.toHaveBeenCalled();
  await waitFor(()=>expect(screen.queryByRole('alert')).toBeNull());
});

it("continues a stopped terminal through same-request attachment and execution without extra input",async()=>{
  terminalApiMock.open.mockResolvedValueOnce({taskId:"task-1",conversationId:"conversation-1",
    status:"exited",pid:0,workingDirectory:"D:\\Workspaces\\task-1",lastError:""});
  renderView();await screen.findAllByText("Original title");giveHostLayout();act(notifyResize);
  await screen.findByTitle("Reconnect terminal");
  let finishOpen!:(value:Awaited<ReturnType<typeof terminalApiMock.open>>)=>void;
  terminalApiMock.open.mockImplementationOnce(()=>new Promise(resolve=>{finishOpen=resolve;}));
  const button=screen.getByRole("button",{name:"继续任务"});
  expect((button as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(button);
  await waitFor(()=>expect(terminalApiMock.open).toHaveBeenCalledTimes(2));
  fireEvent.click(button);
  expect(productionApiMock.continueTask).toHaveBeenCalledTimes(1);
  expect(productionApiMock.flushContinuation).not.toHaveBeenCalled();
  let finishFlush!:(value:unknown)=>void;
  productionApiMock.flushContinuation.mockImplementationOnce(()=>new Promise(resolve=>{finishFlush=resolve;}));
  await act(async()=>finishOpen({taskId:"task-1",conversationId:"conversation-1",status:"running",
    pid:10,workingDirectory:"D:\\Workspaces\\task-1",lastError:""}));
  expect(productionApiMock.flushContinuation).toHaveBeenCalledExactlyOnceWith("task-1","conversation-1","request-1");
  expect(screen.queryByText("运行中")).toBeNull();
  fireEvent.click(button);
  expect(productionApiMock.continueTask).toHaveBeenCalledTimes(1);
  productionApiMock.control.mockResolvedValue({taskId:"task-1",conversationId:"conversation-1",
    nativeSessionId:"native-1",state:"running",connection:"connected",lastEventAt:""});
  await act(async()=>finishFlush({status:"started",conversationId:"conversation-1",nativeSessionId:"native-1",turnId:"turn-2"}));
  await screen.findByText("运行中");
  expect(terminalApiMock.open).toHaveBeenLastCalledWith("codex","task-1","conversation-1",80,24,expect.anything());
  expect(terminalApiMock.write).not.toHaveBeenCalled();
  expect(terminalApiMock.createConversation).not.toHaveBeenCalled();
});

it("does not dispatch after failed continuation attachment and allows an explicit retry",async()=>{
  terminalApiMock.open.mockRejectedValueOnce(new Error("原对话被占用"));
  renderView();await screen.findAllByText("Original title");giveHostLayout();act(notifyResize);
  await screen.findByRole("alert");
  terminalApiMock.open.mockRejectedValueOnce(new Error("原对话仍被占用"));
  fireEvent.click(screen.getByRole("button",{name:"继续任务"}));
  await screen.findByText("Error: 原对话仍被占用");
  expect(productionApiMock.flushContinuation).not.toHaveBeenCalled();
  act(notifyResize);
  expect(terminalApiMock.open).toHaveBeenCalledTimes(2);
  fireEvent.click(screen.getByRole("button",{name:"继续任务"}));
  await waitFor(()=>expect(productionApiMock.flushContinuation).toHaveBeenCalledExactlyOnceWith("task-1","conversation-1","request-1"));
  expect(terminalApiMock.createConversation).not.toHaveBeenCalled();
});

it.each(["waitingForAnswers","waitingForApproval","needsReview","running"])("preserves %s when Continue rechecks the task",async status=>{
  productionApiMock.continueTask.mockResolvedValueOnce({status,conversationId:"conversation-1",nativeSessionId:"native-1"});
  renderView();await screen.findAllByText("Original title");giveHostLayout();act(notifyResize);
  await waitFor(()=>expect(terminalApiMock.open).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByRole("button",{name:"继续任务"}));
  await waitFor(()=>expect(productionApiMock.continueTask).toHaveBeenCalledExactlyOnceWith("task-1","conversation-1"));
  await waitFor(()=>expect((screen.getByRole("button",{name:"继续任务"}) as HTMLButtonElement).disabled).toBe(false));
  expect(terminalApiMock.open).toHaveBeenCalledTimes(1);
  expect(productionApiMock.flushContinuation).not.toHaveBeenCalled();
  expect(terminalApiMock.write).not.toHaveBeenCalled();
});

it("continues an already connected terminal without reopening it",async()=>{
  productionApiMock.continueTask.mockResolvedValueOnce({status:"started",conversationId:"conversation-1",nativeSessionId:"native-1",turnId:"turn-2"});
  renderView();await screen.findAllByText("Original title");giveHostLayout();act(notifyResize);
  await waitFor(()=>expect(terminalApiMock.open).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByRole("button",{name:"继续任务"}));
  await waitFor(()=>expect(productionApiMock.continueTask).toHaveBeenCalledTimes(1));
  expect(productionApiMock.flushContinuation).not.toHaveBeenCalled();
  expect(terminalApiMock.open).toHaveBeenCalledTimes(1);
  expect(terminalApiMock.write).not.toHaveBeenCalled();
});

it("does not send a pending continuation when the terminal unmounts before attachment finishes",async()=>{
  terminalApiMock.open.mockRejectedValueOnce(new Error("原对话未连接"));
  const view=renderView();await screen.findAllByText("Original title");giveHostLayout();act(notifyResize);
  await screen.findByRole("alert");
  let finishOpen!:(value:Awaited<ReturnType<typeof terminalApiMock.open>>)=>void;
  terminalApiMock.open.mockImplementationOnce(()=>new Promise(resolve=>{finishOpen=resolve;}));
  fireEvent.click(screen.getByRole("button",{name:"继续任务"}));
  await waitFor(()=>expect(terminalApiMock.open).toHaveBeenCalledTimes(2));
  view.unmount();
  await act(async()=>finishOpen({taskId:"task-1",conversationId:"conversation-1",status:"running",
    pid:10,workingDirectory:"D:\\Workspaces\\task-1",lastError:""}));
  expect(productionApiMock.flushContinuation).not.toHaveBeenCalled();
});

describe("Codex conversation list", () => {
  it("canceling force stop sends nothing; rejected force stop keeps the session available",async()=>{
    terminalApiMock.stop.mockRejectedValueOnce(new Error("暂停失败"));
    terminalApiMock.forceStop.mockRejectedValueOnce(new Error("退出未确认"));
    renderView();await screen.findAllByText("Original title");giveHostLayout();act(notifyResize);
    await waitFor(()=>expect(terminalApiMock.open).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByTitle("Pause task"));
    fireEvent.click(await screen.findByRole("button",{name:"强制停止此会话"}));
    fireEvent.click(screen.getByRole("button",{name:"取消"}));
    expect(terminalApiMock.forceStop).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button",{name:"强制停止此会话"}));
    fireEvent.click(screen.getByRole("button",{name:"确认强制停止"}));
    await screen.findByText(/退出未确认/);
    expect(screen.queryByText(/此会话执行器已停止/)).toBeNull();
    expect(screen.getByRole("button",{name:"强制停止此会话"})).toBeTruthy();
    expect(productionApiMock.continueTask).not.toHaveBeenCalled();
  });
  it("offers scoped force stop after rejection and waits for explicit confirmation",async()=>{
    terminalApiMock.interrupt.mockRejectedValueOnce(new Error("中断被拒绝"));
    renderView();await screen.findAllByText("Original title");giveHostLayout();act(notifyResize);
    await waitFor(()=>expect(terminalApiMock.open).toHaveBeenCalledTimes(1));
    await waitFor(()=>expect((screen.getByRole("button",{name:"中断执行（Stop）"}) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole("button",{name:"中断执行（Stop）"}));
    fireEvent.click(await screen.findByRole("button",{name:"强制停止此会话"}));
    expect(terminalApiMock.forceStop).not.toHaveBeenCalled();
    expect(screen.getByText(/已完成的文件或外部操作不会撤销/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button",{name:"确认强制停止"}));
    await waitFor(()=>expect(terminalApiMock.forceStop).toHaveBeenCalledExactlyOnceWith("codex","task-1","conversation-1"));
    expect(terminalApiMock.stop).not.toHaveBeenCalled();
    expect(productionApiMock.continueTask).not.toHaveBeenCalled();
    expect(terminalApiMock.open).toHaveBeenCalledTimes(1);
  });
  it("interrupts once, keeps the terminal mounted and focuses new input after native confirmation",async()=>{
    renderView();await screen.findAllByText("Original title");giveHostLayout();act(notifyResize);
    await waitFor(()=>expect(terminalApiMock.open).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByTitle("Output history"));
    let confirmInterrupt!:()=>void;
    terminalApiMock.interrupt.mockImplementationOnce(()=>new Promise(resolve=>{confirmInterrupt=()=>resolve(undefined);}));
    const button=screen.getByRole("button",{name:"中断执行（Stop）"});
    await waitFor(()=>expect((button as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(button);fireEvent.click(button);
    expect(terminalApiMock.interrupt).toHaveBeenCalledExactlyOnceWith("codex","task-1","conversation-1");
    expect(screen.queryByText(/当前执行已中断，可直接输入/)).toBeNull();
    terminalEvents.focus.mockClear();
    await act(async()=>confirmInterrupt());
    await screen.findByText(/当前执行已中断，可直接输入/);
    expect((document.querySelector(".terminal-host") as HTMLElement).hidden).toBe(false);
    expect(terminalEvents.focus).toHaveBeenCalled();
    expect(terminalApiMock.open).toHaveBeenCalledTimes(1);
    expect(terminalApiMock.stop).not.toHaveBeenCalled();
    expect(terminalApiMock.write).not.toHaveBeenCalled();
    expect(productionApiMock.continueTask).not.toHaveBeenCalled();
    expect(productionApiMock.flushContinuation).not.toHaveBeenCalled();
  });
  it("shows failed interruption without closing the terminal or claiming success",async()=>{
    terminalApiMock.interrupt.mockRejectedValueOnce(new Error("Codex 尚未确认中断"));
    renderView();await screen.findAllByText("Original title");giveHostLayout();act(notifyResize);
    await waitFor(()=>expect(terminalApiMock.open).toHaveBeenCalledTimes(1));
    await waitFor(()=>expect((screen.getByRole("button",{name:"中断执行（Stop）"}) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole("button",{name:"中断执行（Stop）"}));
    await screen.findByText(/Codex 尚未确认中断/);
    expect(screen.queryByText(/当前执行已中断，可直接输入/)).toBeNull();
    expect(terminalApiMock.stop).not.toHaveBeenCalled();
    expect(terminalApiMock.open).toHaveBeenCalledTimes(1);
  });
  it("serializes deletion and creation, ignoring focus refresh and old list responses", async () => {
    vi.spyOn(window,'confirm').mockReturnValue(true);
    let finishDelete!:()=>void;
    terminalApiMock.deleteConversation.mockImplementationOnce(()=>new Promise<void>(resolve=>{finishDelete=resolve;}));
    const replacement={...conversation,id:'conversation-2',title:'New conversation'};
    terminalApiMock.createConversation.mockResolvedValueOnce(replacement);
    renderView();await screen.findAllByText('Original title');
    fireEvent.click(screen.getByTitle('Delete conversation'));
    fireEvent(window,new Event('focus'));
    expect((screen.getByTitle('New conversation') as HTMLButtonElement).disabled).toBe(true);
    expect(terminalApiMock.listConversations).toHaveBeenCalledTimes(1);
    expect(terminalApiMock.createConversation).not.toHaveBeenCalled();
    await act(async()=>finishDelete());
    fireEvent.click(screen.getByTitle('New conversation'));
    await screen.findAllByText('New conversation');
    expect(terminalApiMock.createConversation).toHaveBeenCalledTimes(1);
    expect(screen.queryByText('Original title')).toBeNull();
    vi.mocked(window.confirm).mockRestore();
  });
  it("keeps native terminal controls available when workspace probe reports a warning",async()=>{
    renderView();await screen.findAllByText('Original title');giveHostLayout();notifyResize();
    await waitFor(()=>expect(terminalApiMock.open).toHaveBeenCalledTimes(1));
    await act(async()=>terminalChannels[terminalChannels.length-1].onmessage?.({type:'workflow',workflow:{taskId:'task-1',conversationId:'conversation-1',observability:'native',warning:'工作区沙箱启动检查失败；APP 服务正常，权限仍由原生沙箱控制',turns:[],updatedAt:'now'}}));
    expect((await screen.findByText(/工作区沙箱启动检查失败；APP 服务正常/)).getAttribute('role')).toBe('alert');
    expect(screen.getByTitle('Pause task')).toBeTruthy();
    expect((screen.getByLabelText('权限') as HTMLSelectElement).disabled).toBe(false);
    expect(screen.queryByText('原对话尚未接回')).toBeNull();
  });
  it("copies persisted history while the terminal is stopped and shows success", async () => {
    terminalApiMock.open.mockResolvedValueOnce({
      taskId: "task-1", conversationId: "conversation-1", status: "exited",
      pid: 0, workingDirectory: "D:\\Workspaces\\task-1", lastError: "",
    });
    renderView();
    await screen.findAllByText("Original title");
    giveHostLayout();
    notifyResize();
    await screen.findByTitle("Reconnect terminal");
    fireEvent.click(await screen.findByRole("button", { name: "Copy all conversation history" }));
    await screen.findByText("All conversation history copied");
    expect(terminalApiMock.copyHistory).toHaveBeenCalledWith("codex", "task-1", "conversation-1");
    expect(terminalApiMock.open).toHaveBeenCalledTimes(1);
    expect(terminalApiMock.write).not.toHaveBeenCalled();
  });

  it("disables repeat copies, reports errors, and allows retry from history view", async () => {
    let rejectCopy!: (error: Error) => void;
    terminalApiMock.copyHistory.mockImplementationOnce(() => new Promise((_resolve, reject) => { rejectCopy = reject; }));
    renderView();
    const button = await screen.findByRole("button", { name: "Copy all conversation history" });
    fireEvent.click(button);
    expect((button as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(button);
    expect(terminalApiMock.copyHistory).toHaveBeenCalledTimes(1);
    rejectCopy(new Error("Clipboard is busy"));
    await screen.findByText(/Could not copy conversation history/);
    expect((button as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByTitle(translator("en-US")("terminalHistory")));
    fireEvent.click(button);
    await screen.findByText("All conversation history copied");
  });

  it("reports empty Codex history without success", async () => {
    terminalApiMock.copyHistory.mockResolvedValueOnce(false);
    render(<DevelopmentTerminalView provider="codex" taskId="task-1" visible
      availability={availability} t={translator("en-US")} onRefreshAvailability={vi.fn()} />);
    fireEvent.click(await screen.findByRole("button", { name: "Copy all conversation history" }));
    await screen.findByText("This conversation has no history to copy yet.");
    expect(terminalApiMock.copyHistory).toHaveBeenCalledWith("codex", "task-1", "conversation-1");
    expect(screen.queryByText("All conversation history copied")).toBeNull();
  });

  it("shows a read-only task folder for desktop project compatibility", async () => {
    renderView();
    fireEvent.click(await screen.findByTitle("Codex project grouping"));
    const field = await screen.findByRole("textbox", { name: "Task folder" });
    expect((field as HTMLInputElement).value).toBe("D:\\Workspaces\\task-1");
    expect((field as HTMLInputElement).readOnly).toBe(true);
    expect(terminalApiMock.projectWorkspace).toHaveBeenCalledWith("task-1");
  });
  it("archives without recreating a conversation and restores without a new native thread", async () => {
    renderView();
    fireEvent.click(await screen.findByTitle("Archive conversation and stop terminal"));
    await waitFor(() => expect(terminalApiMock.setArchived).toHaveBeenCalledWith("task-1", "conversation-1", true));
    await waitFor(() => expect(screen.queryByTitle("Archive conversation and stop terminal")).toBeNull());
    expect(terminalApiMock.createConversation).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTitle("Toggle archived conversations"));
    fireEvent.click(await screen.findByTitle("Restore conversation"));
    await waitFor(() => expect(terminalApiMock.setArchived).toHaveBeenCalledWith("task-1", "conversation-1", false));
    await screen.findByTitle("Archive conversation and stop terminal");
    expect(terminalApiMock.createConversation).not.toHaveBeenCalled();
  });

  it("does not automatically open or replace archived conversations on reload", async () => {
    terminalApiMock.listConversations.mockResolvedValueOnce([{ ...conversation, archived: true }]);
    renderView();
    await screen.findAllByText("No conversations");
    expect(terminalApiMock.createConversation).not.toHaveBeenCalled();
    expect(terminalApiMock.open).not.toHaveBeenCalled();
  });

  it("preserves the active conversation when native archival fails", async () => {
    terminalApiMock.setArchived.mockRejectedValueOnce(new Error("Native archive failed"));
    renderView();
    fireEvent.click(await screen.findByTitle("Archive conversation and stop terminal"));
    await screen.findByText(/Native archive failed/);
    expect(screen.getByTitle("Archive conversation and stop terminal")).toBeTruthy();
    expect(terminalApiMock.deleteConversation).not.toHaveBeenCalled();
  });
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
  it("defers resize until attach completes and restores input focus", async () => {
    let finish!:(state:Awaited<ReturnType<typeof terminalApiMock.open>>)=>void;
    terminalApiMock.open.mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));
    renderView();await screen.findAllByText("Original title");giveHostLayout();notifyResize();
    await waitFor(()=>expect(terminalApiMock.open).toHaveBeenCalledTimes(1));
    terminalGrid.cols=110;terminalGrid.rows=35;
    terminalEvents.resize?.({cols:110,rows:35});
    expect(terminalApiMock.resize).not.toHaveBeenCalled();
    terminalEvents.focus.mockClear();
    finish({taskId:"task-1",conversationId:"conversation-1",status:"running",pid:10,workingDirectory:"D:\\Workspaces\\task-1",lastError:""});
    await waitFor(()=>expect(terminalApiMock.resize).toHaveBeenCalledWith("codex","conversation-1",110,35));
    expect(terminalEvents.focus).toHaveBeenCalled();
  });
  it("automatically connects the exact requested conversation and acknowledges only after open", async () => {
    const other = { ...conversation, id: "other", title: "Other conversation" };
    terminalApiMock.listConversations.mockResolvedValueOnce([conversation, other]);
    const finish = vi.fn();
    render(<DevelopmentTerminalView provider="codex" taskId="task-1" visible availability={availability}
      t={translator("en-US")} onRefreshAvailability={vi.fn()}
      connectionRequest={{ id:"request",taskId:"task-1",provider:"codex",conversationId:"other",finish }} />);
    await screen.findAllByText("Other conversation");
    expect(finish).not.toHaveBeenCalled(); giveHostLayout(); notifyResize();
    await waitFor(() => expect(finish).toHaveBeenCalledWith(undefined));
    expect(terminalApiMock.open).toHaveBeenCalledWith("codex", "task-1", "other", 80, 24, expect.anything());
    expect(terminalApiMock.createConversation).not.toHaveBeenCalled();
  });
  it("does not steal focus when attachment completes behind an active dialog", async () => {
    let finish!: (value: Awaited<ReturnType<typeof terminalApiMock.open>>) => void;
    terminalApiMock.open.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    renderView(); await screen.findAllByText("Original title");
    giveHostLayout(); notifyResize();
    await waitFor(() => expect(terminalApiMock.open).toHaveBeenCalled());
    const dialog=document.createElement("section"); dialog.setAttribute("aria-modal","true");
    const input=document.createElement("textarea"); dialog.append(input); document.body.append(dialog); input.focus();
    terminalEvents.focus.mockClear();
    try {
      await act(async () => {finish({taskId:"task-1",conversationId:"conversation-1",status:"running",pid:10,workingDirectory:"D:\\Workspaces\\task-1",lastError:""});});
      expect(terminalEvents.focus).not.toHaveBeenCalled(); expect(document.activeElement).toBe(input);
    } finally { dialog.remove(); }
  });
  it("does not create a replacement when the requested conversation is missing", async () => {
    terminalApiMock.listConversations.mockResolvedValueOnce([]); const finish=vi.fn();
    render(<DevelopmentTerminalView provider="codex" taskId="task-1" visible availability={availability}
      t={translator("en-US")} onRefreshAvailability={vi.fn()}
      connectionRequest={{id:"request",taskId:"task-1",provider:"codex",conversationId:"missing",finish}} />);
    await waitFor(() => expect(finish).toHaveBeenCalled());
    expect(terminalApiMock.createConversation).not.toHaveBeenCalled();
    expect(terminalApiMock.open).not.toHaveBeenCalled();
  });
  it("preserves visible history when continuing the original conversation fails", async () => {
    renderView(); await screen.findAllByText("Original title");
    giveHostLayout(); notifyResize();
    await waitFor(() => expect(terminalApiMock.open).toHaveBeenCalled());
    terminalChannels[terminalChannels.length - 1].onmessage?.({ type: "output", data: "Preserved original history\r\n" });
    fireEvent.click(screen.getByTitle("Pause task"));
    await screen.findByTitle("Reconnect terminal");
    terminalApiMock.open.mockRejectedValueOnce(new Error("Native resume failed"));
    fireEvent.click(screen.getByTitle("Reconnect terminal"));
    await screen.findByText(/Native resume failed/);
    fireEvent.click(screen.getByTitle("Output history"));
    expect(screen.getByText(/Preserved original history/)).toBeTruthy();
    expect(terminalApiMock.createConversation).not.toHaveBeenCalled();
  });
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

it("rebuilds the same idle terminal without replaying input or continuing inference",async()=>{
 renderView();await screen.findAllByText("Original title");giveHostLayout();notifyResize();
 await waitFor(()=>expect(terminalApiMock.open).toHaveBeenCalledTimes(1));
 fireEvent.click(screen.getByRole("button",{name:"重建终端连接"}));
 await waitFor(()=>expect(terminalApiMock.rebuild).toHaveBeenCalledWith("task-1","conversation-1"));
 await waitFor(()=>expect(terminalApiMock.open).toHaveBeenCalledTimes(2));
 expect(terminalApiMock.write).not.toHaveBeenCalled();expect(productionApiMock.continueTask).not.toHaveBeenCalled();
});

it("binds typed input to the exact attached PTY session",async()=>{
 renderView();await screen.findAllByText("Original title");giveHostLayout();notifyResize();
 await waitFor(()=>expect(terminalApiMock.open).toHaveBeenCalledTimes(1));
 await act(async()=>{terminalEvents.data?.("中文输入");});
 expect(terminalApiMock.write).toHaveBeenCalledWith("codex","conversation-1","中文输入","input-session-test");
});
