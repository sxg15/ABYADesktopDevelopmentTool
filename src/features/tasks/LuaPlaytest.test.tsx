// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {productionApi,type ProductionRecord} from "../../shared/production";
import {LuaPlaytest} from "./LuaPlaytest";

afterEach(()=>{cleanup();vi.restoreAllMocks();});
it.each([["implementation","frozen"],["review","current"]])("previews the displayed playable build during %s",async(stage,expected)=>{
  const start=vi.spyOn(productionApi,"startPreview").mockResolvedValue({status:"ready",message:"窗口已打开"});
  const r={taskId:"bird",currentStage:stage,currentVersion:"current",playerMode:"multiplayer",workbench:{execution:{verificationPackage:{version:"frozen"}}}} as ProductionRecord;
  render(<LuaPlaytest record={r} active en={false} label={v=><span>{v}</span>}/>);
  expect(screen.getByText(expected)).toBeTruthy();
  fireEvent.change(screen.getByRole("combobox"),{target:{value:"3"}});
  fireEvent.click(screen.getByRole("button",{name:"启动试玩"}));
  await waitFor(()=>expect(start).toHaveBeenCalledExactlyOnceWith("bird",expected,3));
  expect(await screen.findByRole("status")).toBeTruthy();
});
