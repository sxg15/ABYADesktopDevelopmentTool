// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ArtTracePanel } from "./ArtTracePanel";
import { artViewFixture } from "../../shared/artDesign.test-fixtures";
import { productionApi } from "../../shared/production";
vi.mock("../../shared/production",()=>({productionApi:{feedback:vi.fn(),media:vi.fn()}}));
afterEach(()=>{cleanup();vi.clearAllMocks();});
it("keeps original wording, interpretations and incomplete coverage distinct",()=>{
  render(<ArtTracePanel taskId="task" view={artViewFixture()} active onUpdated={vi.fn()}/>);
  expect(screen.getByText("不要用数字显示库存，直接显示剩余物品。")).toBeTruthy();
  expect(screen.getByText("已关联设计")).toBeTruthy();expect(screen.getByText("待落实到当前设计")).toBeTruthy();
  expect(screen.getByText(/图片不代表已满足要求/)).toBeTruthy();
  expect(productionApi.feedback).not.toHaveBeenCalled();
});
it("saves a raw planner note with the viewed revision and ignores duplicate submit",async()=>{
  const view=artViewFixture(),updated=vi.fn();let resolve!:(v:typeof view)=>void;
  vi.mocked(productionApi.feedback).mockReturnValue(new Promise(r=>{resolve=r;}));
  render(<ArtTracePanel taskId="task" view={view} active onUpdated={updated}/>);
  fireEvent.change(screen.getByLabelText("补充策划要求"),{target:{value:"最后一个物品仍需可见"}});
  fireEvent.click(screen.getByRole("button",{name:"保存要求"}));fireEvent.click(screen.getByRole("button",{name:"保存要求"}));
  expect(productionApi.feedback).toHaveBeenCalledTimes(1);
  expect(productionApi.feedback).toHaveBeenCalledWith("task",4,"add-art-note",{text:"最后一个物品仍需可见"});
  resolve({...view,record:{...view.record!,revision:5}});await waitFor(()=>expect(updated).toHaveBeenCalledTimes(1));
});
it("retains planner text on a stale-write failure and never starts AI implicitly",async()=>{
  vi.mocked(productionApi.feedback).mockRejectedValue(new Error("来源已变化"));const next=vi.fn();
  render(<ArtTracePanel taskId="task" view={artViewFixture()} active onUpdated={vi.fn()} onContinue={next}/>);
  fireEvent.change(screen.getByLabelText("补充策划要求"),{target:{value:"不要删掉身份色"}});
  fireEvent.click(screen.getByRole("button",{name:"保存要求"}));
  await waitFor(()=>expect(screen.getByRole("alert").textContent).toContain("来源已变化"));
  expect((screen.getByLabelText("补充策划要求") as HTMLTextAreaElement).value).toBe("不要删掉身份色");expect(next).not.toHaveBeenCalled();
});
