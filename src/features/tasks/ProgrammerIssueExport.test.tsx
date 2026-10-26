// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ProgrammerIssueExport } from "./ProgrammerIssueExport";
import { productionApi } from "../../shared/production";
vi.mock("../../shared/production", () => ({productionApi:{exportProgrammerIssues:vi.fn(),revealDocument:vi.fn()}}));
afterEach(() => {cleanup();vi.resetAllMocks();});
it("hides programmer export actions when only ordinary issues exist",()=>{
  render(<ProgrammerIssueExport taskId="task" revision={7} issueIds={[]}/>);
  expect(screen.queryByRole("button")).toBeNull();
});

it("exports the exact issue snapshot once and reveals the saved Markdown", async () => {
  let finish!: (path:string)=>void;
  vi.mocked(productionApi.exportProgrammerIssues).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
  render(<ProgrammerIssueExport taskId="task" revision={7} issueIds={["bug"]} single/>);
  const button=screen.getByRole("button",{name:"导出此问题（MD）"});
  fireEvent.click(button);fireEvent.click(button);
  expect(productionApi.exportProgrammerIssues).toHaveBeenCalledExactlyOnceWith("task",7,["bug"]);
  finish("artifacts/game-development/reports/issue.md");
  await screen.findByRole("status");
  expect(productionApi.revealDocument).toHaveBeenCalledExactlyOnceWith("task","artifacts/game-development/reports/issue.md");
});

it("retains the saved path and supports reveal retry after a file-manager failure", async()=>{
  vi.mocked(productionApi.exportProgrammerIssues).mockResolvedValue("reports/saved.md");
  vi.mocked(productionApi.revealDocument).mockRejectedValueOnce(new Error("Explorer unavailable"));
  render(<ProgrammerIssueExport taskId="task" revision={7} issueIds={["a","b"]}/>);
  fireEvent.click(screen.getByRole("button",{name:"导出程序问题汇总（MD）"}));
  expect((await screen.findByRole("alert")).textContent).toContain("文档已保存");
  expect(screen.getByRole("status").textContent).toContain("reports/saved.md");
  fireEvent.click(screen.getByRole("button",{name:"在文件夹中显示"}));
  await waitFor(()=>expect(productionApi.revealDocument).toHaveBeenCalledTimes(2));
  expect(productionApi.exportProgrammerIssues).toHaveBeenCalledOnce();
});

it("shows export failures without reporting a saved file",async()=>{
  vi.mocked(productionApi.exportProgrammerIssues).mockRejectedValue(new Error("任务记录已变化，请刷新后再操作。"));
  render(<ProgrammerIssueExport taskId="task" revision={7} issueIds={["bug"]}/>);
  fireEvent.click(screen.getByRole("button",{name:"导出程序问题汇总（MD）"}));
  expect((await screen.findByRole("alert")).textContent).toContain("任务记录已变化");
  expect(screen.queryByRole("status")).toBeNull();
  expect(productionApi.revealDocument).not.toHaveBeenCalled();
});

it("ignores a previous task's late export result",async()=>{
  let finish!: (path:string)=>void;
  vi.mocked(productionApi.exportProgrammerIssues).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
  const page=render(<ProgrammerIssueExport taskId="a" revision={1} issueIds={["bug"]}/>);
  fireEvent.click(screen.getByRole("button",{name:"导出程序问题汇总（MD）"}));
  page.rerender(<ProgrammerIssueExport taskId="b" revision={2} issueIds={["other"]}/>);
  finish("a/report.md");
  await waitFor(()=>expect((screen.getByRole("button",{name:"导出程序问题汇总（MD）"}) as HTMLButtonElement).disabled).toBe(false));
  expect(productionApi.revealDocument).not.toHaveBeenCalled();
  expect(screen.queryByRole("status")).toBeNull();
});
