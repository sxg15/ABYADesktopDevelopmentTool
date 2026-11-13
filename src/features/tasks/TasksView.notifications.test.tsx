// @vitest-environment jsdom
import {render,screen,cleanup,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {TasksView} from "./TasksView";
import {translator} from "../../i18n";
import {attentionFixture} from "../../shared/notifications.test-fixtures";
import type {NotificationActivation} from "../../shared/notifications";
import {productionApi} from "../../shared/production";
vi.mock("./desktopChat",()=>({openTaskChat:vi.fn(async()=>{}),briefPrompt:"prepare"}));
vi.mock("../../shared/api",()=>({api:{listTasks:vi.fn(async()=>["first","target"].map(id=>({id,title:id,status:"active",workspacePath:"D:/"+id})))},errorMessage:String}));
vi.mock("./StageReader",()=>({StageReader:({task,stage}:{task:{id:string};stage:string})=><div>{task.id+" / "+stage}</div>}));
vi.mock("../../shared/production",()=>({productionApi:{decide:vi.fn(),answer:vi.fn()}}));
afterEach(()=>{cleanup();vi.clearAllMocks();localStorage.clear();});
it.each(["review","question","terminal","recheck"] as const)("opens the target in the reader without submitting a %s decision",async(kind)=>{
 const target=attentionFixture(kind==="recheck"?"review":kind,"target").items[0].target;
 const activation:NotificationActivation={id:kind,status:"current",target};
 const handled=vi.fn();render(<TasksView t={translator("zh-CN")} notify={vi.fn()} notificationNavigation={activation} onNotificationHandled={handled}/>);
 await screen.findByText("target / "+target.stage);
 await waitFor(()=>expect(handled).toHaveBeenCalledWith(kind));
 expect(productionApi.decide).not.toHaveBeenCalled();expect(productionApi.answer).not.toHaveBeenCalled();
});
