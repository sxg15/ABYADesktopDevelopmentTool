// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { TaskDescription } from "./TaskDescription";
afterEach(() => { cleanup(); sessionStorage.clear(); vi.restoreAllMocks(); });
const text = "第一段：玩法规则\n第二段：美术说明\n<script>literal text</script>";
const page = (id: string, description = text) => <TaskDescription key={id} taskId={id} description={description} workspacePath={`D:/任务/${id}`} />;

it("defaults collapsed and exposes complete literal text without changing it", () => {
  render(page("a"));
  const button = screen.getByRole("button", { name:"展开任务说明" });
  expect(button.getAttribute("aria-expanded")).toBe("false");
  expect(screen.queryByRole("region")).toBeNull();
  fireEvent.click(button);
  const region = screen.getByRole("region", { name:"完整任务说明与工作区" });
  expect(region.querySelector(".task-description-copy")?.textContent).toBe(text);
  expect(region.querySelector("script")).toBeNull();
  expect(region.textContent).toContain("D:/任务/a");
  fireEvent.click(screen.getByRole("button", { name:"收起任务说明" }));
  expect(screen.queryByRole("region")).toBeNull();
});

it("preserves disclosure through polling and remembers each task independently", () => {
  const rendered = render(page("a"));
  fireEvent.click(screen.getByRole("button"));
  rendered.rerender(page("a", "更新后的说明"));
  expect(screen.getByRole("region").textContent).toContain("更新后的说明");
  rendered.rerender(page("b"));
  expect(screen.queryByRole("region")).toBeNull();
  rendered.rerender(page("a"));
  expect(screen.getByRole("button").getAttribute("aria-expanded")).toBe("true");
  fireEvent.click(screen.getByRole("button"));
  rendered.unmount(); render(page("a"));
  expect(screen.getByRole("button").getAttribute("aria-expanded")).toBe("false");
});

it("still toggles when storage is unavailable and provides an empty-description fallback", () => {
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new Error("unavailable"); });
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("unavailable"); });
  render(<TaskDescription taskId="empty" description="" workspacePath="D:/work" en />);
  fireEvent.click(screen.getByRole("button", { name:"Expand task description" }));
  expect(screen.getByRole("region").textContent).toContain("No task description.");
});
