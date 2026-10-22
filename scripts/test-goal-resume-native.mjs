// Owned native APP fault injection. Uses a real Codex turn in a disposable task.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
const [action,directory]=process.argv.slice(2),root=path.resolve(directory);
assert.ok(['start','resume','inspect'].includes(action));
assert.equal(JSON.parse(fs.readFileSync(path.join(root,'art-design-native-fixture.json'),'utf8')).schema,'abya-art-design-native/v1');
const {chromium}=createRequire(import.meta.url)(process.env.ABYA_TEST_PLAYWRIGHT||'playwright');
const browser=await chromium.connectOverCDP('http://127.0.0.1:9248');
try {
 const page=browser.contexts()[0].pages()[0];await page.waitForFunction(()=>!!window.__TAURI_INTERNALS__?.invoke);
 const invoke=async(command,args={})=>{
  const r=await page.evaluate(async({command,args})=>{try{return {ok:true,value:await window.__TAURI_INTERNALS__.invoke(command,args)};}catch(e){return {ok:false,error:e};}},{command,args});
  if(!r.ok)throw new Error(`${command}: ${JSON.stringify(r.error)}`);return r.value;
 };
 const paths=await invoke('get_app_paths');assert.equal(path.resolve(paths.dataDir).toLowerCase(),path.join(root,'Local','ABYA Desktop Development Tool','Data').toLowerCase());
 const build=await invoke('get_build_info');assert.equal(build.testEnvironment,true);assert.equal(build.verification,'verified');assert.equal(build.release,'task-isolation-goal-resume-20261022');
 const filename=path.join(root,'goal-resume-native.json');
 let state=fs.existsSync(filename)?JSON.parse(fs.readFileSync(filename,'utf8')):{};
 const save=()=>fs.writeFileSync(filename,JSON.stringify(state,null,2));
 const open=async()=>{
  const channel=await page.evaluate(()=>`__CHANNEL__:${window.__TAURI_INTERNALS__.transformCallback(()=>{},false)}`);
  return invoke('open_codex_terminal',{taskId:state.task.id,conversationId:state.conversation.id,columns:100,rows:30,onEvent:channel});
 };
 if(action==='start'){
  assert.equal((await invoke('list_tasks')).length,0);
  state.task=await invoke('create_task',{input:{title:'原子流程故障恢复隔离验收',description:'这是桌面APP隔离故障注入测试，没有游戏需求。仅思考并说明任务恢复的幂等原则，不要调用工具、联网、修改文件或启动游戏；所有人工审批保持原样。'}});
  state.conversation=await invoke('create_codex_conversation',{taskId:state.task.id,title:'故障恢复原会话'});save();
  let view=await invoke('update_task_production',{input:{taskId:state.task.id,expectedRevision:0,operation:'initialize',data:{questionMode:'no-followup',playerMode:'single'}}});
  state.goalId=view.record.workbench.activeGoalId;state.goalCount=view.record.workbench.goals.length;
  state.terminal=await open();state.conversation=(await invoke('list_codex_conversations',{taskId:state.task.id})).find(c=>c.id===state.conversation.id);save();
  view=await invoke('get_task_production',{taskId:state.task.id});
  state.intent=crypto.randomUUID();save();
  state.first=await invoke('resume_workbench_goal',{taskId:state.task.id,goalId:state.goalId,conversationId:state.conversation.id,expectedRevision:view.record.revision,resumeIntentId:state.intent});
  assert.equal(state.first.status,'started');
  const after=await invoke('get_task_production',{taskId:state.task.id});
  state.beforeCrash={goalIds:after.record.workbench.goals.map(g=>g.id),attempts:after.record.workbench.attempts.map(a=>({id:a.id,goalId:a.goalId,status:a.status,turnId:a.turnId})),revision:after.record.revision};save();
  const owner=after.record.workbench.attempts.find(a=>a.id===state.intent).ownerPid;
  assert.equal(owner,Number(fs.readFileSync(path.join(root,'owned-app.pid'),'utf8').trim()));
  assert.equal(path.resolve(build.executable).toLowerCase(),path.resolve('Publish-Staging-Validation/ABYA Desktop Development Tool.exe').toLowerCase());
  process.kill(owner);
  console.log(JSON.stringify({crashedOwnedApp:true,owner,goal:state.goalId,attempt:state.intent}));
 } else {
  const view=await invoke('reconcile_task_execution',{taskId:state.task.id});
  state.reconciled=view.record.workbench.attempts.map(a=>({id:a.id,status:a.status,goalId:a.goalId,reason:a.reason}));save();
  if(action==='inspect'){console.log(JSON.stringify(state.reconciled));}
  else {
   assert.equal(state.reconciled.find(a=>a.id===state.intent)?.status,'interrupted');
   assert.deepEqual(view.record.workbench.goals.map(g=>g.id),state.beforeCrash.goalIds);
   await page.getByRole('button',{name:'继续此子流程',exact:true}).waitFor();
   await page.screenshot({path:path.join(root,'interrupted-native.png'),fullPage:true});
   await page.getByRole('button',{name:'继续此子流程',exact:true}).click();
   let resumedAttempt;
   const deadline=Date.now()+45000;
   while(Date.now()<deadline){
    const next=await invoke('get_task_production',{taskId:state.task.id});
    resumedAttempt=next.record.workbench.attempts.find(a=>a.id!==state.intent&&a.goalId===state.goalId&&a.turnId);
    if(resumedAttempt)break;
    await new Promise(r=>setTimeout(r,300));
   }
   assert.ok(resumedAttempt,`UI resume did not dispatch: ${(await page.locator('body').innerText()).slice(-1800)}`);
   state.resumed={status:'started',nativeSessionId:resumedAttempt.nativeId,turnId:resumedAttempt.turnId,requestId:resumedAttempt.id};
   const latest=await invoke('get_task_production',{taskId:state.task.id});
   const args={taskId:state.task.id,goalId:state.goalId,conversationId:state.conversation.id,expectedRevision:latest.record.revision,resumeIntentId:resumedAttempt.id};
   state.duplicate=await invoke('resume_workbench_goal',args);assert.ok(['running','completed'].includes(state.duplicate.status));
   assert.equal(state.resumed.nativeSessionId,state.first.nativeSessionId);
   const current=await invoke('get_task_production',{taskId:state.task.id});
   assert.equal(current.record.workbench.attempts.length,2);assert.deepEqual(current.record.workbench.goals.map(g=>g.id),state.beforeCrash.goalIds);
   assert.equal(current.record.workbench.attempts[1].predecessorId,state.intent);
   state.afterResume={goals:current.record.workbench.goals.map(g=>g.id),attempts:current.record.workbench.attempts.map(a=>({id:a.id,goalId:a.goalId,status:a.status,predecessorId:a.predecessorId}))};
   const control=await invoke('get_codex_task_control',{taskId:state.task.id,conversationId:state.conversation.id});
   await invoke('force_stop_codex_conversation',{taskId:state.task.id,conversationId:state.conversation.id,expectedGeneration:control.generation});
   await assert.rejects(invoke('force_stop_codex_conversation',{taskId:state.task.id,conversationId:state.conversation.id,expectedGeneration:control.generation}));
   state.passed=true;save();console.log(JSON.stringify({passed:true,goal:state.goalId,attempts:2,nativeId:state.first.nativeSessionId}));
  }
 }
}finally{await browser.close().catch(()=>{});}
