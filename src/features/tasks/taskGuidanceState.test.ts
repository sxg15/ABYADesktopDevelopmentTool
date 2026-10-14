import { expect, it } from "vitest";
import { taskGuidanceState } from "./taskGuidanceState";
import type { ProductionView, TaskControlState } from "../../shared/production";
import { attentionFixture } from "../../shared/notifications.test-fixtures";
const view = {policy:{stages:[]},actions:{review:{},feedback:{}},record:undefined} as unknown as ProductionView;
it.each([['running','waiting'],['waitingForResponse','waiting'],['queued','waiting'],['paused','neutral']])("maps %s to distinct %s guidance",(state,tone)=>{
  const result=taskGuidanceState('task',true,view,{taskId:'task',state} as TaskControlState,undefined,false);
  expect(result.tone).toBe(tone); expect(result.title).not.toBe('正在核对任务状态');
});
it("uses the backend human projection and never guesses terminal waits from local control",()=>{
  const control={taskId:'task',state:'waitingForTerminal'} as TaskControlState;
  expect(taskGuidanceState('task',true,view,control,undefined,false).tone).toBe('neutral');
  const snapshot=attentionFixture('terminal');
  expect(taskGuidanceState('task',true,view,control,undefined,false,snapshot).target?.conversationId).toBe('original');
  snapshot.unavailable=true;
  expect(taskGuidanceState('task',true,view,control,undefined,false,snapshot).pending.terminal).toBe(0);
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
