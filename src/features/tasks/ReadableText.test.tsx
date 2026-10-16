// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { ReadableText } from "./ReadableText";
afterEach(cleanup);
it("presents authored headings, numbered points, labels and emphasis", () => {
  const content = "# 玩法说明\n\n3.1 基础规则：每次成功 **加 5 分**。\n- 操作：鼠标左键";
  const {container} = render(<ReadableText text={content}/>);
  expect(screen.getByRole("heading",{name:"玩法说明",level:2})).toBeTruthy();
  expect(container.querySelector(".readable-number")?.textContent).toBe("3.1 ");
  expect([...container.querySelectorAll("strong")].map(e=>e.textContent)).toEqual(["基础规则：","加 5 分","操作："]);
});
it("keeps arbitrary HTML, links and media inert and retains plain source text", () => {
  const content = '<script>alert(1)</script>\n[链接](javascript:alert(1))\n<img src="private-path" onerror="bad()">';
  const {container} = render(<ReadableText text={content}/>);
  expect(container.querySelector("script,a,img")).toBeNull();
  expect(container.textContent).toBe(content);
});
it("keeps fenced code literal and resumes formatting only after its closing fence", () => {
  const content = '```lua\n# not a heading\n**not bold**\n```\n## 正文';
  const {container} = render(<ReadableText text={content}/>);
  expect(container.querySelectorAll(".readable-code-line")).toHaveLength(4);
  expect(container.querySelector("strong")).toBeNull();
  expect(screen.getByRole("heading",{name:"正文",level:3})).toBeTruthy();
});
