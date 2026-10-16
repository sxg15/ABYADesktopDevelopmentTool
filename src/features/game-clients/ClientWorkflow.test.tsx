// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ClientWorkflowProvider, ClientRetentionNotice, useClientWorkflow } from "./ClientWorkflow";
import { clientApi, emptyClientSelection, type ClientInstallation, type ClientUpdate } from "../../shared/gameClients";
import type { AppSettings } from "../../shared/types";
import { translator } from "../../i18n";

const t = translator("zh-CN");
const base: AppSettings = { gameExecutablePath: "D:\\Clients\\old\\Game.exe", gameClient: { ...emptyClientSelection(), source: "remote", installationId: "old", executableRelativePath: "Game.exe", storageRoot: "D:\\Clients" }, workspaceRootPath: "D:\\Work", locale: "zh-CN", desktopCliPort: 47600, gameGatewayPort: 47610, preferredAdapterId: "adapter", lanBroadcastEnabled: false, toolId: "test" };
const newer: ClientInstallation = { id: "new", releaseId: 2, tagName: "v2", prerelease: false, publishedAt: "2026-10-16", assetId: 20, assetName: "Windows.zip", assetUpdatedAt: "2026-10-16", sha256: "fixture", digestVerified: true, directory: "D:\\Clients\\new", storageRoot: "D:\\Clients", executableCandidates: ["Game.exe"], installedAt: "2026-10-16" };
const candidate = (): ClientUpdate => ({ releaseId: 2, tagName: "v2", currentTag: "v1", key: "2:20:first", preferredAssetId: 20,
  assets: [{ id: 20, name: "Windows.zip", size: 4096, digest: null, updatedAt: "2026-10-16", state: "uploaded" }] });

function Controls({ onLocal }: { onLocal: () => void }) {
  const flow = useClientWorkflow();
  return <><p>任务工作区</p><button onClick={() => flow.download(2, 20, "D:\\Clients")}>直接下载</button>
    <button onClick={onLocal}>切到本地模式</button><button onClick={flow.checkUpdate}>检查一次</button><ClientRetentionNotice t={t} /></>;
}
function Harness({ initial = base }: { initial?: AppSettings }) {
  const [settings, setSettings] = useState(initial);
  const [page, setPage] = useState(true);
  return <ClientWorkflowProvider settings={settings} t={t} onSaved={setSettings} notify={vi.fn()}>
    <output data-testid="settings">{JSON.stringify(settings)}</output><button onClick={() => setPage(false)}>切换页面</button>
    {page && <Controls onLocal={() => setSettings({ ...settings, gameClient: { ...settings.gameClient!, source: "local" } })} />}
  </ClientWorkflowProvider>;
}
const saved = () => JSON.parse(screen.getByTestId("settings").textContent!) as AppSettings;

beforeEach(() => {
  vi.spyOn(clientApi, "checkUpdate").mockResolvedValue(candidate());
  vi.spyOn(clientApi, "maintain").mockResolvedValue({ removed: 0, deferred: [] });
  vi.spyOn(clientApi, "operation").mockResolvedValue({ active: false, kind: "", stage: "", completedBytes: 0, totalBytes: 0, errorCode: null });
  vi.spyOn(clientApi, "install").mockResolvedValue(newer);
  vi.spyOn(clientApi, "discardPending").mockResolvedValue();
  vi.spyOn(clientApi, "activate").mockImplementation(async (id, path) => ({ ...base, gameExecutablePath: `${newer.directory}\\${path}`, gameClient: { ...base.gameClient!, installationId: id, executableRelativePath: path } }));
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it("prompts in the task workspace and a declined candidate is not repeated on focus", async () => {
  render(<Harness />);
  await screen.findByRole("dialog", { name: t("gcUpdateTitle") });
  expect(screen.getByText("任务工作区")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: t("gcLater") }));
  fireEvent.focus(window);
  await waitFor(() => expect(clientApi.checkUpdate).toHaveBeenCalledTimes(2));
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(clientApi.install).not.toHaveBeenCalled(); expect(clientApi.activate).not.toHaveBeenCalled();
  expect(saved().gameClient?.installationId).toBe("old");
  vi.mocked(clientApi.checkUpdate).mockResolvedValue({ ...candidate(), key: "2:21:replacement" });
  fireEvent.focus(window);
  await screen.findByRole("dialog", { name: t("gcUpdateTitle") });
});

it("updates directly from the prompt and retains unrelated settings", async () => {
  render(<Harness />); await screen.findByRole("dialog", { name: t("gcUpdateTitle") });
  vi.mocked(clientApi.checkUpdate).mockResolvedValue(null);
  fireEvent.click(screen.getByRole("button", { name: t("gcUpdateNow") }));
  await waitFor(() => expect(saved().gameClient?.installationId).toBe("new"));
  expect(clientApi.install).toHaveBeenCalledWith(2, 20, "D:\\Clients");
  expect(clientApi.activate).toHaveBeenCalledWith("new", "Game.exe");
  expect(saved().preferredAdapterId).toBe("adapter"); expect(saved().lanBroadcastEnabled).toBe(false);
  expect(screen.queryByRole("dialog")).toBeNull();
  await waitFor(() => expect(clientApi.maintain).toHaveBeenCalledTimes(2));
});

it("requires an explicit asset choice when a new release has ambiguous packages", async () => {
  const update = candidate();
  update.assets.push({ ...update.assets[0], id: 21, name: "Windows-server.zip" }); update.preferredAssetId = null;
  vi.mocked(clientApi.checkUpdate).mockResolvedValue(update);
  render(<Harness />); await screen.findByRole("dialog", { name: t("gcUpdateTitle") });
  expect((screen.getByRole("button", { name: t("gcUpdateNow") }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(screen.getByLabelText(t("gcSelectAsset")), { target: { value: "21" } });
  vi.mocked(clientApi.checkUpdate).mockResolvedValue(null);
  fireEvent.click(screen.getByRole("button", { name: t("gcUpdateNow") }));
  await waitFor(() => expect(clientApi.install).toHaveBeenCalledWith(2, 21, "D:\\Clients"));
});

it("keeps the selected client after download failure and makes retry available", async () => {
  vi.mocked(clientApi.install).mockRejectedValue({ code: "clientIntegrity" });
  render(<Harness />); await screen.findByRole("dialog", { name: t("gcUpdateTitle") });
  fireEvent.click(screen.getByRole("button", { name: t("gcUpdateNow") }));
  await screen.findByText(t("gcErrorIntegrity"));
  expect(saved().gameClient?.installationId).toBe("old"); expect(clientApi.activate).not.toHaveBeenCalled();
  const closeButtons = screen.getAllByRole("button", { name: t("close") });
  fireEvent.click(closeButtons[closeButtons.length - 1]);
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(screen.getByRole("button", { name: /立即更新 · v2/ })).toBeTruthy();
});

it("waits for an EXE choice and cancels only the unselected download", async () => {
  vi.mocked(clientApi.install).mockResolvedValue({ ...newer, executableCandidates: ["Game.exe", "Server.exe"] });
  render(<Harness />); await screen.findByRole("dialog", { name: t("gcUpdateTitle") });
  fireEvent.click(screen.getByRole("button", { name: t("gcUpdateNow") }));
  await screen.findByRole("dialog", { name: t("gcChooseExeTitle") });
  expect((screen.getByRole("button", { name: t("gcUseNow") }) as HTMLButtonElement).disabled).toBe(true);
  expect(clientApi.activate).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: t("cancel") }));
  await waitFor(() => expect(clientApi.discardPending).toHaveBeenCalledWith("new"));
  expect(saved().gameClient?.installationId).toBe("old");
});

it("survives navigation while downloading, rejects duplicate clicks and uses the chosen EXE", async () => {
  vi.mocked(clientApi.checkUpdate).mockResolvedValue(null);
  let finish!: (value: ClientInstallation) => void;
  vi.mocked(clientApi.install).mockReturnValue(new Promise(resolve => { finish = resolve; }));
  render(<Harness />);
  fireEvent.click(screen.getByText("直接下载")); fireEvent.click(screen.getByText("直接下载"));
  fireEvent.click(screen.getByText("切换页面"));
  expect(clientApi.install).toHaveBeenCalledOnce();
  finish({ ...newer, executableCandidates: ["Game.exe", "Server.exe"] });
  await screen.findByLabelText(t("gcExe"));
  fireEvent.change(screen.getByLabelText(t("gcExe")), { target: { value: "Server.exe" } });
  fireEvent.click(screen.getByRole("button", { name: t("gcUseNow") }));
  await waitFor(() => expect(saved().gameClient?.executableRelativePath).toBe("Server.exe"));
});

it("does not prompt in local mode or apply a late check after switching to local", async () => {
  let finish!: (value: ClientUpdate) => void;
  vi.mocked(clientApi.checkUpdate).mockReturnValue(new Promise(resolve => { finish = resolve; }));
  render(<Harness />);
  fireEvent.click(screen.getByText("切到本地模式")); finish(candidate());
  await waitFor(() => expect(saved().gameClient?.source).toBe("local"));
  expect(screen.queryByRole("dialog")).toBeNull(); expect(clientApi.install).not.toHaveBeenCalled();
  cleanup(); vi.mocked(clientApi.checkUpdate).mockClear();
  render(<Harness initial={{ ...base, gameClient: { ...base.gameClient!, source: "local" } }} />);
  expect(clientApi.checkUpdate).not.toHaveBeenCalled();
});

it("shows why a task-pinned older package is retained", async () => {
  vi.mocked(clientApi.checkUpdate).mockResolvedValue(null);
  vi.mocked(clientApi.maintain).mockResolvedValue({ removed: 2, deferred: [{ installationId: "pinned", tagName: "v0", reason: "task" }] });
  render(<Harness />);
  await screen.findByText("v0 · 已有任务绑定此版本");
});
