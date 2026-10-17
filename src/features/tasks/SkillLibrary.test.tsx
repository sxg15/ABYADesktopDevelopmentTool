// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SkillLibrary } from "./SkillLibrary";
import { productionApi } from "../../shared/production";
import { artLibraryApi } from "../../shared/artDesign";
import { artCaseFixture, artViewFixture } from "../../shared/artDesign.test-fixtures";
vi.mock("@tauri-apps/plugin-dialog", () => ({open:vi.fn()}));
vi.mock("../../shared/production", () => ({productionApi:{get:vi.fn(),skills:vi.fn(),readSkill:vi.fn(),update:vi.fn(),media:vi.fn()}}));
vi.mock("../../shared/artDesign", async original => ({...await original<typeof import("../../shared/artDesign")>(),artLibraryApi:{list:vi.fn(),media:vi.fn(),action:vi.fn()}}));
const skill = {id:"abya-game-development-task/references/game-art-design",name:"game-art-design",kind:"design",provider:"codex",description:"Game art method",path:".codex/skills/design",sha256:"design-hash"};
beforeEach(() => {
  vi.mocked(productionApi.get).mockResolvedValue(artViewFixture()); vi.mocked(productionApi.skills).mockResolvedValue([skill]);
  vi.mocked(artLibraryApi.list).mockResolvedValue([artCaseFixture()]); vi.mocked(artLibraryApi.media).mockResolvedValue("data:image/png;base64,AA==");
  vi.mocked(productionApi.media).mockResolvedValue("data:image/png;base64,AA==");
});
afterEach(() => {cleanup();vi.resetAllMocks();});
it("exposes embedded design content without treating it as a selected template", async () => {
  vi.mocked(productionApi.readSkill).mockResolvedValue("Actual pinned design content");
  render(<SkillLibrary taskId="task" active locale="en-US"/>);
  await screen.findByRole("heading",{name:"Game art design"}); expect(productionApi.readSkill).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"Read Skill"}));
  await screen.findByText("Actual pinned design content");
  expect(screen.queryByText(/This version includes visual refinement/)).toBeNull();
  expect(productionApi.readSkill).toHaveBeenCalledWith("task","codex",skill.id); expect(productionApi.update).not.toHaveBeenCalled();
});
it("describes refinement only when present in the task's actually read design version", async () => {
  vi.mocked(productionApi.readSkill).mockResolvedValue("Read references/visual-refinement.md for scoped review.");
  render(<SkillLibrary taskId="task" active locale="en-US"/>);
  await screen.findByRole("heading",{name:"Game art design"});
  fireEvent.click(screen.getByRole("button",{name:"Read Skill"}));
  await screen.findByText(/This version includes visual refinement/);
  expect(productionApi.update).not.toHaveBeenCalled();
});
it("retains unsubmitted case notes across library section changes", async () => {
  render(<SkillLibrary taskId="task" active/>);await screen.findByRole("heading",{name:"游戏美术设计"});
  fireEvent.click(screen.getByRole("tab",{name:"美术设计库"}));
  fireEvent.click(await screen.findByRole("button",{name:"待审阅草稿 · 1"}));
  fireEvent.click(screen.getByRole("button",{name:/用物品轮廓表达剩余次数/}));
  fireEvent.change(screen.getByLabelText("我满意的具体部分与原因"),{target:{value:"只认可库存表达，尚未提交"}});
  fireEvent.click(screen.getByRole("tab",{name:"技能与模板"}));fireEvent.click(screen.getByRole("tab",{name:"美术设计库"}));
  expect((screen.getByLabelText("我满意的具体部分与原因") as HTMLTextAreaElement).value).toContain("尚未提交");
  expect(artLibraryApi.action).not.toHaveBeenCalled();expect(productionApi.update).not.toHaveBeenCalled();
});
it("discards a selected-body response after the task changes", async () => {
  let finish!: (text:string) => void; vi.mocked(productionApi.readSkill).mockImplementation(() => new Promise(resolve => {finish=resolve;}));
  const page=render(<SkillLibrary taskId="task" active locale="en-US"/>);await screen.findByRole("heading",{name:"Game art design"});
  fireEvent.click(screen.getByRole("button",{name:"Read Skill"}));
  page.rerender(<SkillLibrary taskId="other" active locale="en-US"/>);
  finish("OLD TASK PRIVATE BODY");await waitFor(()=>expect(screen.queryByText("Loading this task's skills…")).toBeNull());
  expect(screen.queryByText("OLD TASK PRIVATE BODY")).toBeNull();
});
