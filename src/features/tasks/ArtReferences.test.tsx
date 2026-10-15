// @vitest-environment jsdom
import { useState } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ArtReferences } from "./ArtReferences";
import { productionApi, type ProductionRecord, type ProductionView } from "../../shared/production";
vi.mock("../../shared/production", () => ({ productionApi: { feedback: vi.fn(), media: vi.fn() } }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const initial = (): ProductionRecord => ({taskId:"a",revision:2,workflowVersion:"2.1.0",questionMode:"ask",currentStage:"resources",currentRound:0,cycle:1,stages:{resources:"in-progress"},documents:{},approvals:[],versionDetails:{},issues:[],evidence:[],rounds:[],milestones:{},knowledge:[],skillPins:[],updatedAt:"now",artifacts:[]});
const view = (record:ProductionRecord):ProductionView => ({record,policy:{workflowVersion:"2.1.0",stages:[],dimensions:[],questions:[]},warnings:[],reportPaths:[],availableUpdate:false});
const file = (name="reference.png") => new File(["fixture"],name,{type:"image/png"});
const paste = (files:File[]) => fireEvent.paste(screen.getByRole("group",{name:"Image paste area"}),{clipboardData:{items:files.map(f=>({kind:"file",getAsFile:()=>f}))}});
function Harness({onContinue=vi.fn(),active=true}:{onContinue?:(s:string,n:number)=>Promise<void>;active?:boolean}) {
  const [record,setRecord]=useState(initial());
  return <ArtReferences taskId="a" record={record} active={active} en onUpdated={v=>setRecord(v.record!)} onContinue={onContinue}/>;
}
function mockUploads() {
  let r=initial();
  vi.mocked(productionApi.media).mockResolvedValue("data:image/png;base64,AA==");
  vi.mocked(productionApi.feedback).mockImplementation(async(_task,revision,operation,data)=>{
    expect(revision).toBe(r.revision);
    const d=data as {name?:string;summary?:string;id?:string};
    r={...r,revision:r.revision+1,artifacts:operation==="remove-art-reference"?r.artifacts!.map(a=>a.id===d.id?{...a,status:"withdrawn"}:a):[...r.artifacts!,{id:String(r.revision),title:d.name!,path:"artifacts/ref.png",stage:"resources",status:"reference",plannerReference:true,mediaType:"image/png",summary:d.summary}]};
    return view(r);
  });
}
it("accepts clipboard images before any results, serializes revisions and continues only on click",async()=>{
  mockUploads(); const next=vi.fn().mockResolvedValue(undefined); render(<Harness onContinue={next}/>);
  fireEvent.change(screen.getByRole("textbox"),{target:{value:"palette only"}});
  paste([file("first.png"),file("second.png")]);
  // A repeated paste during an in-flight read must not duplicate the batch.
  paste([file("duplicate.png")]);
  await screen.findByText("2 saved. Continue when you want AI to use them.");
  expect(productionApi.feedback).toHaveBeenCalledTimes(2);
  expect(productionApi.feedback).toHaveBeenNthCalledWith(2,"a",3,"attach-art-reference",expect.objectContaining({summary:"palette only",name:"second.png"}));
  expect(next).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"Preview first.png"}));
  expect(screen.getByRole("dialog")).toBeTruthy();
  fireEvent.keyDown(screen.getByRole("dialog"),{key:"Escape"});
  fireEvent.click(screen.getByRole("button",{name:"Let AI use references and continue"}));
  await waitFor(()=>expect(next).toHaveBeenCalledWith(expect.stringContaining("plannerReference"),4));
});
it("supports file selection and removal with task history retained by the service",async()=>{
  mockUploads(); render(<Harness/>);
  fireEvent.change(screen.getByLabelText("Reference image files"),{target:{files:[file()]}});
  await screen.findByRole("button",{name:"Preview reference.png"});
  fireEvent.click(screen.getByRole("button",{name:"Remove reference"}));
  await screen.findByText("Removed from references; history retained.");
  expect(screen.queryByRole("button",{name:"Preview reference.png"})).toBeNull();
});
it("ignores text paste, rejects unsupported files and too many files, and disables inactive tasks",async()=>{
  const page=render(<Harness/>);
  fireEvent.paste(screen.getByRole("group"),{clipboardData:{items:[{kind:"string"}]}});
  paste([new File(["<svg/>"],"bad.svg",{type:"image/svg+xml"})]);
  expect(screen.getByRole("alert").textContent).toContain("PNG/JPEG");
  paste(Array.from({length:9},()=>file()));
  expect(screen.getByRole("alert").textContent).toContain("up to 8");
  expect(productionApi.feedback).not.toHaveBeenCalled();
  page.rerender(<Harness active={false}/>); paste([file()]);
  expect(productionApi.feedback).not.toHaveBeenCalled();
  expect((screen.getByRole("button",{name:"Upload reference images"}) as HTMLButtonElement).disabled).toBe(true);
});
it("retains completed uploads on revision conflict and does not pretend the remaining images saved",async()=>{
  mockUploads();
  const implementation=vi.mocked(productionApi.feedback).getMockImplementation()!;
  vi.mocked(productionApi.feedback).mockImplementationOnce(implementation).mockRejectedValueOnce(new Error("Revision conflict"));
  render(<Harness/>); paste([file("saved.png"),file("failed.png")]);
  await screen.findByText("Revision conflict");
  expect(screen.getByText("1 saved; remaining images were not saved.")).toBeTruthy();
  expect(screen.getByRole("button",{name:"Preview saved.png"})).toBeTruthy();
  expect(screen.queryByRole("button",{name:"Preview failed.png"})).toBeNull();
});
it("does not apply an old upload result or continue its batch after switching tasks",async()=>{
  let finish!:(v:ProductionView)=>void;
  vi.mocked(productionApi.feedback).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
  const updated=vi.fn();
  const page=render(<ArtReferences taskId="a" record={initial()} active en onUpdated={updated}/>);
  paste([file("first.png"),file("second.png")]);
  await waitFor(()=>expect(productionApi.feedback).toHaveBeenCalledTimes(1));
  page.rerender(<ArtReferences taskId="b" record={{...initial(),taskId:"b"}} active en onUpdated={updated}/>);
  await act(async()=>finish(view({...initial(),revision:3})));
  expect(updated).not.toHaveBeenCalled();
  expect(productionApi.feedback).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("status")).toBeNull();
});
