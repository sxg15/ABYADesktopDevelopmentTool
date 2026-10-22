// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { openUrl } from "@tauri-apps/plugin-opener";
import { productionApi } from "../../shared/production";
import { ReadableText } from "./ReadableText";
vi.mock("@tauri-apps/plugin-opener",()=>({openUrl:vi.fn().mockResolvedValue(undefined)}));
vi.mock("../../shared/production",()=>({productionApi:{media:vi.fn().mockResolvedValue("data:image/png;base64,aGVsbG8="),open:vi.fn()}}));
afterEach(()=>{cleanup();vi.clearAllMocks();});
it("renders GFM headings, nested lists, tables, checklists and emphasis",()=>{
 const {container}=render(<ReadableText text={"# 玩法说明\n\n1. **得分**\n   - 每次加分\n\n| 项目 | 数值 |\n| --- | --- |\n| 回合 | 3 |\n\n- [x] 已完成\n- [ ] 待检查\n\n> 说明\n\n~~旧规则~~"}/>);
 expect(screen.getByRole("heading",{name:"玩法说明",level:2})).toBeTruthy();
 expect(container.querySelector("ol ul")).toBeTruthy();expect(screen.getByRole("table")).toBeTruthy();
 expect(screen.getAllByRole("checkbox").map(e=>(e as HTMLInputElement).checked)).toEqual([true,false]);
 expect(container.querySelector("blockquote")).toBeTruthy();expect(container.querySelector("del")?.textContent).toBe("旧规则");
});
it("keeps HTML and dangerous URLs inert and does not auto-fetch remote images",()=>{
 const {container}=render(<ReadableText text={'<script>alert(1)</script>\n\n[危险](javascript:alert(1))\n\n<img src="private-path" onerror="bad()">\n\n![外部图片](https://example.com/image.png)'}/>);
 expect(container.querySelector("script,img,iframe")).toBeNull();expect(container.querySelector('a[href^="javascript"]')).toBeNull();expect(openUrl).not.toHaveBeenCalled();
});
it("renders fenced Lua literally without interpreting markdown inside code",()=>{
 const {container}=render(<ReadableText text={'```lua\nlocal x = "**literal**"\nprint(x < 2)\n```\n## 正文'}/>);
 expect(container.querySelector("pre code.language-lua")?.textContent).toBe('local x = "**literal**"\nprint(x < 2)\n');
 expect(container.querySelector("strong")).toBeNull();expect(screen.getByRole("heading",{name:"正文",level:3})).toBeTruthy();
});
it("opens an HTTP link only after an explicit click",async()=>{
 render(<ReadableText text="[参考](https://example.com/docs)"/>);fireEvent.click(screen.getByRole("link",{name:"参考"}));
 await waitFor(()=>expect(openUrl).toHaveBeenCalledWith("https://example.com/docs"));
});

it("resolves only registered task images from document-relative or workspace paths",async()=>{
 render(<ReadableText taskId="task" basePath="artifacts/report.md" artifacts={[{id:"image",path:"artifacts/image.png",title:"已登记图",stage:"resources",status:"registered",mediaType:"image/png"}]} text="![图](image.png)\n\n![图](artifacts/image.png)\n\n![未登记](private.png)"/>);
 await waitFor(()=>expect(screen.getAllByRole("img")).toHaveLength(2));
 expect(productionApi.media).toHaveBeenCalledWith("task","image");
 expect(screen.getByText("未登记（未登记或不可用）")).toBeTruthy();
});
