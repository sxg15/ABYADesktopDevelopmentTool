// @vitest-environment jsdom
import {render,screen,cleanup} from "@testing-library/react";
import {afterEach,expect,it} from "vitest";
import {AutonomousExecution} from "./AutonomousExecution";
import type {ProductionRecord,TaskControlState} from "../../shared/production";
afterEach(cleanup);
it("does not present persisted running work as a live executor after interruption",()=>{
 const record={taskId:"task",cycle:1,workbench:{execution:{state:"running",activeUnitId:"one",units:[{id:"one",goalId:"original",cycle:1,title:"Original work",status:"running",reason:"",dependsOn:[],routes:["first"],route:0,attempts:1,resultPaths:[]}],events:[]}}} as unknown as ProductionRecord;
 render(<AutonomousExecution record={record} en={false} control={{taskId:"task",state:"interrupted"} as TaskControlState}/>);
 expect(screen.getByText("子流程被中断，工作队列已保留")).toBeTruthy();expect(screen.queryByText("AI 正在自主推进")).toBeNull();expect(screen.getByText("Original work")).toBeTruthy();
});
