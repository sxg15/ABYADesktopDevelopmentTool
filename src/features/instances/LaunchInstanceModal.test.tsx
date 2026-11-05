// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { LaunchInstanceModal } from "./LaunchInstanceModal";
import { taskLaunchApi, type TaskLaunchSources } from "../../shared/taskLaunch";
import { api } from "../../shared/api";
vi.mock("../../shared/taskLaunch", () => ({ taskLaunchApi: { sources: vi.fn(), adopt: vi.fn(), export: vi.fn() } }));
vi.mock("../../shared/api", () => ({ api: { launchInstance: vi.fn() }, errorMessage: String }));
vi.mock("../../shared/production", () => ({ productionApi: { startPreview: vi.fn() } }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
function sources(): TaskLaunchSources {
  const archive = { archiveGuid: "same-guid", archiveName: "任务存档", author: "QA", levels: [{ levelGuid: "level", levelName: "开始", isStart: true }] };
  return { entries: [
    { id: "task:path", origin: "task", archive: { ...archive, archivePath: "D:/Work/.abya-authoring/task/Archives/game" } },
    { id: "global:path", origin: "global", archive: { ...archive, archivePath: "C:/Global/game" } },
  ], candidate: null, warnings: [], globalRoot: "C:/Global", player: { selectedPath: "D:/App/ABYA_PB.exe", selectedHash: "hash", previous: null, matches: true } };
}
const props = { taskId: "task", executablePath: "obsolete.exe", taskInstances: [], t: (key: string) => key, onClose: vi.fn(), onLaunched: vi.fn() };
it("launches the selected private archive even when a global copy has the same GUID", async () => {
  vi.mocked(taskLaunchApi.sources).mockResolvedValue(sources());
  vi.mocked(api.launchInstance).mockResolvedValue({} as never);
  render(<LaunchInstanceModal {...props} />);
  await screen.findByText("本任务编辑存档 · 任务存档");
  fireEvent.change(screen.getByRole("textbox"), { target: { value: "私有存档试玩" } });
  fireEvent.change(screen.getByLabelText("存档来源"), { target: { value: "task:path" } });
  fireEvent.click(screen.getByRole("button", { name: "launch" }));
  await waitFor(() => expect(api.launchInstance).toHaveBeenCalledWith("task", "私有存档试玩", "D:/App/ABYA_PB.exe", expect.objectContaining({ archive: expect.objectContaining({ archivePath: "D:/Work/.abya-authoring/task/Archives/game", archiveGuid: "same-guid", levelGuid: "level" }) })));
});
it("blocks launch until the APP selection difference is explicitly acknowledged", async () => {
  const value = sources(); value.player.matches = false;
  vi.mocked(taskLaunchApi.sources).mockResolvedValue(value);
  render(<LaunchInstanceModal {...props} />);
  fireEvent.click(await screen.findByRole("button", { name: "确认使用 APP 当前构建包" }));
  await waitFor(() => expect(taskLaunchApi.adopt).toHaveBeenCalledWith("task", "hash"));
  expect(api.launchInstance).not.toHaveBeenCalled();
});
it("discards a delayed response after switching tasks", async () => {
  let resolve!: (value: TaskLaunchSources) => void;
  vi.mocked(taskLaunchApi.sources).mockImplementationOnce(() => new Promise(r => { resolve = r; })).mockResolvedValue({ ...sources(), entries: [] });
  const view = render(<LaunchInstanceModal {...props} />);
  view.rerender(<LaunchInstanceModal {...props} taskId="other" />);
  await screen.findByText(/当前 APP 构建包/);
  resolve(sources());
  await waitFor(() => expect(screen.queryByText("本任务编辑存档 · 任务存档")).toBeNull());
});
