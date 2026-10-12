import { expect, it } from "vitest";
import { taskGuidanceState } from "./taskGuidanceState";
import type { ProductionView, TaskControlState } from "../../shared/production";
const view = {policy:{stages:[]},actions:{review:{},feedback:{}},record:undefined} as unknown as ProductionView;
it.each([['running','waiting'],['waitingForResponse','waiting'],['queued','waiting'],['waitingForTerminal','terminal'],['failed','error'],['disconnected','error'],['paused','neutral']])("maps %s to distinct %s guidance",(state,tone)=>{
  const result=taskGuidanceState('task',true,view,{taskId:'task',state} as TaskControlState,undefined,false);
  expect(result.tone).toBe(tone); expect(result.title).not.toBe('正在核对任务状态');
});
it("does not claim running without confirmed stage information or with another task's control",()=>{
  expect(taskGuidanceState('task',true,undefined,{state:'running'} as TaskControlState,undefined,false).tone).toBe('neutral');
  expect(taskGuidanceState('task',true,view,{taskId:'other',state:'running'} as TaskControlState,undefined,false).key).toBe('unknown');
});
it("closed tasks expose no primary action or pending badge",()=>{
  const result=taskGuidanceState('task',false,view,{state:'waitingForTerminal'} as TaskControlState,undefined,false);
  expect(result.action).toBe('none'); expect(result.pending.terminal).toBe(0);
});
