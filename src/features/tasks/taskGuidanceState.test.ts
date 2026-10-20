import { expect, it } from "vitest";
import { taskGuidanceState } from "./taskGuidanceState";
import type { ProductionView, TaskControlState } from "../../shared/production";
import { attentionFixture } from "../../shared/notifications.test-fixtures";
const view = {policy:{stages:[]},actions:{review:{},feedback:{}},record:undefined} as unknown as ProductionView;
it("distinguishes interrupted input from a paused terminal",()=>{
  const guide=taskGuidanceState('task',true,view,{taskId:'task',state:'awaitingInput',connection:'connected'} as TaskControlState,undefined,false);
  expect(guide.title).toBe('当前执行已中断，等待新指令');
  expect(guide.instruction).toContain('直接输入');
  expect(guide.button).toBe('前往终端输入');
});
it("prioritizes unresolved ABYA defects over an AI-running message and clears on resolution",()=>{
  const record={taskId:'task',currentStage:'implementation',issues:[{id:'bug',kind:'blocker',status:'open',stage:'implementation',repairTarget:'abya-build',description:'Broken build'}],documents:{}};
  const input={...view,record} as unknown as ProductionView;
  const control={taskId:'task',state:'running'} as TaskControlState;
  const guide=taskGuidanceState('task',true,input,control,undefined,false);
  expect(guide.tone).toBe('programmer');expect(guide.title).toContain('需要程序修复');expect(guide.action).toBe('stage');
  record.issues[0].status='resolved';
  expect(taskGuidanceState('task',true,input,control,undefined,false).tone).toBe('waiting');
});
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

it("prioritizes native feedback verification independently of document review",()=>{
  const attention=attentionFixture("review");
  attention.items[0].kind="recheck";
  attention.items[0].target={...attention.items[0].target,action:"recheck",stage:"resources",feedbackId:"old-feedback",candidateArtifactId:"current-image"};
  const model=taskGuidanceState("task",true,view,{taskId:"task",state:"running"} as TaskControlState,undefined,false,attention);
  expect(model.action).toBe("recheck");expect(model.target?.feedbackId).toBe("old-feedback");
  expect(model.pending.reviews).toBe(1);expect(model.tone).toBe("human");
});
