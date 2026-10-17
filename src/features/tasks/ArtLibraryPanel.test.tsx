// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ArtLibraryPanel } from "./ArtLibraryPanel";
import { artCaseFixture, artViewFixture } from "../../shared/artDesign.test-fixtures";
import { artLibraryApi } from "../../shared/artDesign";
vi.mock("@tauri-apps/plugin-dialog",()=>({open:vi.fn()}));
vi.mock("../../shared/production",()=>({productionApi:{media:vi.fn(async()=>"data:image/png;base64,AA=="),revealDocument:vi.fn()}}));
vi.mock("../../shared/artDesign",async original=>{
  const actual=await original<typeof import("../../shared/artDesign")>();
  return {...actual,artLibraryApi:{list:vi.fn(async()=>[]),media:vi.fn(async()=>"data:image/png;base64,AA=="),action:vi.fn(),get:vi.fn(),request:vi.fn()}};
});
afterEach(()=>{cleanup();vi.clearAllMocks();});
it("keeps imported satisfaction separate from local approval and does not invent missing limits",async()=>{
  vi.mocked(artLibraryApi.list).mockResolvedValue([{...artCaseFixture(),provenance:"imported",limitations:""}]);
  render(<ArtLibraryPanel taskId="task" view={artViewFixture()} active embedded/>);
  fireEvent.click(await screen.findByRole("button",{name:/用物品轮廓表达剩余次数/}));
  expect(screen.getByRole("heading",{name:"来源方声明的认可范围（未在本机复验）"})).toBeTruthy();
  expect(screen.queryByRole("heading",{name:"策划满意的具体范围"})).toBeNull();
  expect(screen.getByText("尚未记录，不能据此推定没有限制。")).toBeTruthy();
  expect(artLibraryApi.action).not.toHaveBeenCalled();
});
it("requires an explicit satisfaction scope before collecting a reviewed draft",async()=>{
  vi.mocked(artLibraryApi.action).mockResolvedValue(artCaseFixture());
  render(<ArtLibraryPanel taskId="task" view={artViewFixture()} active onClose={vi.fn()}/>);
  fireEvent.click(screen.getByRole("button",{name:"待审阅草稿 · 1"}));
  fireEvent.click(screen.getByRole("button",{name:/用物品轮廓表达剩余次数/}));
  const collect=screen.getByRole("button",{name:"满意并收录此案例"});
  expect((collect as HTMLButtonElement).disabled).toBe(true);expect(artLibraryApi.action).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("我满意的具体部分与原因"),{target:{value:"只认可库存表达"}});
  fireEvent.click(collect);
  await waitFor(()=>expect(artLibraryApi.action).toHaveBeenCalledWith("task",4,"collect",{draftId:"draft-1",satisfaction:"只认可库存表达"}));
});
it("does not offer collection for a stale draft",()=>{
  const view=artViewFixture();view.artDesignStatus!.designHash="new-design";
  render(<ArtLibraryPanel taskId="task" view={view} active onClose={vi.fn()}/>);
  fireEvent.click(screen.getByRole("button",{name:"待审阅草稿 · 1"}));fireEvent.click(screen.getByRole("button",{name:/用物品轮廓表达剩余次数/}));
  fireEvent.change(screen.getByLabelText("我满意的具体部分与原因"),{target:{value:"喜欢"}});
  expect((screen.getByRole("button",{name:"满意并收录此案例"}) as HTMLButtonElement).disabled).toBe(true);
  expect(screen.getByText(/草稿对应的要求或设计已变化/)).toBeTruthy();
});
it("asks AI to inspect applicability without silently applying or approving a case",async()=>{
  vi.mocked(artLibraryApi.request).mockResolvedValue({revision:5,prompt:"评估 case-fixture 第 1 版；当前策划要求优先"});
  vi.mocked(artLibraryApi.list).mockResolvedValue([artCaseFixture()]);const next=vi.fn().mockResolvedValue(undefined);
  render(<ArtLibraryPanel taskId="task" view={artViewFixture()} active onClose={vi.fn()} onContinue={next}/>);
  fireEvent.click(await screen.findByRole("button",{name:/用物品轮廓表达剩余次数/}));
  fireEvent.click(screen.getByRole("button",{name:"让 AI 评估并参考"}));
  await waitFor(()=>expect(next).toHaveBeenCalledWith(expect.stringContaining("case-fixture 第 1 版"),5));
  expect(artLibraryApi.request).toHaveBeenCalledWith("task",4,"reference","case-fixture",1);
  expect(artLibraryApi.action).not.toHaveBeenCalled();expect(next.mock.calls[0][0]).toContain("当前策划要求优先");
});
it("does not continue when saving the concrete case request fails",async()=>{
  vi.mocked(artLibraryApi.list).mockResolvedValue([artCaseFixture()]);
  vi.mocked(artLibraryApi.request).mockRejectedValue(new Error("版本已变化"));
  const next=vi.fn();render(<ArtLibraryPanel taskId="task" view={artViewFixture()} active embedded onContinue={next}/>);
  fireEvent.click(await screen.findByRole("button",{name:/用物品轮廓表达剩余次数/}));
  fireEvent.click(screen.getByRole("button",{name:"让 AI 评估并参考"}));
  await screen.findByText("版本已变化");expect(next).not.toHaveBeenCalled();expect(artLibraryApi.action).not.toHaveBeenCalled();
});
