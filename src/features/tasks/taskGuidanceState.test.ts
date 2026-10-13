import { expect, it } from "vitest";
import { taskGuidanceState } from "./taskGuidanceState";
import type { ProductionView, TaskControlState } from "../../shared/production";
const view = {policy:{stages:[]},actions:{review:{},feedback:{}},record:undefined} as unknown as ProductionView;
it("prioritizes unresolved ABYA defects over an AI-running message and clears on resolution",()=>{
  const record={taskId:'task',currentStage:'implementation',issues:[{id:'bug',kind:'blocker',status:'open',stage:'implementation',repairTarget:'abya-build',description:'Broken build'}],documents:{}};
  const input={...view,record} as unknown as ProductionView;
  const control={taskId:'task',state:'running'} as TaskControlState;
  const guide=taskGuidanceState('task',true,input,control,undefined,false);
  expect(guide.tone).toBe('programmer');expect(guide.title).toContain('需要程序修复');expect(guide.action).toBe('stage');
  record.issues[0].status='resolved';
  expect(taskGuidanceState('task',true,input,control,undefined,false).tone).toBe('waiting');
});
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
it("directs a connected pending task to explicit continuation",()=>{
  const result=taskGuidanceState('task',true,view,{taskId:'task',state:'awaitingContinue',connection:'connected'} as TaskControlState,undefined,false);
  expect(result.title).toBe('终端已连接，任务等待继续');
  expect(result.instruction).toContain('继续任务');
  expect(result.tone).toBe('neutral');
});
