// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {translator} from "../../i18n";
import {TasksView} from "./TasksView";
import {openTaskChat} from "./desktopChat";
vi.mock("./desktopChat",()=>({openTaskChat:vi.fn(async()=>{}),briefPrompt:"prepare"}));
vi.mock("../../shared/api",()=>({api:{listTasks:vi.fn(async()=>["A","B"].map(id=>({id,title:id,workspacePath:"D:/"+id,status:"active"})))},errorMessage:String}));
vi.mock("./StageReader",()=>({StageReader:({task}:{task:{id:string}})=><div>{"流程 "+task.id}</div>}));
afterEach(()=>{cleanup();vi.clearAllMocks();localStorage.clear();});
it("opens the selected project once and switches without a terminal or collapsing the task list",async()=>{
 const props={t:translator("zh-CN"),notify:vi.fn()};
 const page=render(<TasksView {...props}/>);
 await screen.findByText("流程 A");
 await waitFor(()=>expect(openTaskChat).toHaveBeenCalledWith("A"));
 fireEvent.click(screen.getByRole("button",{name:/B/}));
 await waitFor(()=>expect(openTaskChat).toHaveBeenCalledWith("B"));
 expect(screen.getByRole("button",{name:/A/})).toBeTruthy();
 expect(screen.queryByRole("tab",{name:/终端/})).toBeNull();
 page.rerender(<TasksView {...props}/>);
 expect(openTaskChat).toHaveBeenCalledTimes(2);
});

it("retries the same task after an automatic open failure",async()=>{
 vi.mocked(openTaskChat).mockRejectedValueOnce(new Error("connection unavailable")).mockResolvedValue(true);
 const notify=vi.fn();
 render(<TasksView t={translator("zh-CN")} notify={notify}/>);
 await waitFor(()=>expect(notify).toHaveBeenCalled());
 fireEvent.click(screen.getByRole("button",{name:/^A/}));
 await waitFor(()=>expect(openTaskChat).toHaveBeenCalledTimes(2));
 fireEvent.click(screen.getByRole("button",{name:/^A/}));
 expect(openTaskChat).toHaveBeenCalledTimes(2);
});
