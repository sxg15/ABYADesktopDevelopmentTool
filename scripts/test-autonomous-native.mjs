// Synthetic native orchestration test; no game, user task or real approvals are modified.
import fs from 'node:fs';import path from 'node:path';import assert from 'node:assert/strict';import {createRequire} from 'node:module';
const [action,directory]=process.argv.slice(2),root=path.resolve(directory);
assert.equal(JSON.parse(fs.readFileSync(path.join(root,'art-design-native-fixture.json'),'utf8')).schema,'abya-art-design-native/v1');
const {chromium}=createRequire(import.meta.url)(process.env.ABYA_TEST_PLAYWRIGHT||'playwright');
const browser=await chromium.connectOverCDP('http://127.0.0.1:9248');
try{
 const page=browser.contexts()[0].pages()[0];await page.waitForFunction(()=>!!window.__TAURI_INTERNALS__?.invoke);
 const invoke=async(command,args={})=>{const r=await page.evaluate(async({command,args})=>{try{return{ok:true,value:await window.__TAURI_INTERNALS__.invoke(command,args)};}catch(e){return{ok:false,error:e};}},{command,args});if(!r.ok)throw Error(JSON.stringify({command,error:r.error}));return r.value;};
 const paths=await invoke('get_app_paths');assert.equal(path.resolve(paths.dataDir).toLowerCase(),path.join(root,'Local','ABYA Desktop Development Tool','Data').toLowerCase());
 const build=await invoke('get_build_info');assert.equal(build.testEnvironment,true);assert.equal(build.verification,'verified');assert.equal(build.release,'workflow-continuity-20261026');
 const file=path.join(root,'autonomous-native.json');let state=fs.existsSync(file)?JSON.parse(fs.readFileSync(file,'utf8')):{};
 const save=()=>fs.writeFileSync(file,JSON.stringify(state,null,2));
 if(action==='prepare'){
  assert.equal((await invoke('list_tasks')).length,0);
  state.task=await invoke('create_task',{input:{title:'Lua 自主调度隔离验收',description:'这是APP自主调度的合成测试，不是游戏开发。只处理已登记工作项：在本任务 artifacts 下写简短测试文本并通过 production update 报告当前工作项完成。不要启动游戏、联网、生成美术或操作其他目录。PB问题是故障注入数据，保留它及其依赖，完成其余可做项即可。'}});
  const view=await invoke('update_task_production',{input:{taskId:state.task.id,expectedRevision:0,operation:'initialize',data:{questionMode:'no-followup',playerMode:'single'}}});assert.equal(view.record.workflowVersion,'2.6.0');
  state.conversation=await invoke('create_codex_conversation',{taskId:state.task.id,title:'自主调度测试原会话'});save();console.log(JSON.stringify({prepared:true,task:state.task.id}));
 }else if(action==='run'){
  let view=await invoke('get_task_production',{taskId:state.task.id});const goal=view.record.workbench.activeGoalId;
  const update=async(operation,data)=>{view=await invoke('update_task_production',{input:{taskId:state.task.id,expectedRevision:view.record.revision,operation,data}});};
  await update('save-issue',{id:'injected-platform',kind:'blocker',status:'open',stage:'implementation',description:'故障注入：本隔离用例没有游戏运行实例；不应尝试启动或修复游戏。',repairTarget:'abya-source',resumeWhen:'本测试不提供游戏实例，保留问题即可'});
  for(const [id,title,deps] of [['platform','等待本测试不提供的平台能力',[]],['write-a','在 artifacts/proof-a.txt 写入 synthetic A，并 report-work-unit 本项 succeeded，resultPaths 引用该文件',[]],['write-b','在 artifacts/proof-b.txt 写入 synthetic B，并 report-work-unit 本项 succeeded，resultPaths 引用该文件',[]],['dependent','依赖缺失平台的工作，不要执行',['platform']]]){
   await update('save-work-unit',{id,goalId:goal,title,dependsOn:deps,issueIds:id==='platform'?['injected-platform']:[],routes:[title]});
  }
  await update('record-issue-handling',{issueId:'injected-platform',mode:'awaiting-condition',reason:'隔离测试未提供游戏实例，不能调用游戏能力',impact:'平台项及依赖保留；两个文本工作项可以独立完成',affectedUnitIds:['platform']});
  state.goal=goal;state.beforeRevision=view.record.revision;save();
  const channel=await page.evaluate(()=>`__CHANNEL__:${window.__TAURI_INTERNALS__.transformCallback(()=>{},false)}`);
  await invoke('open_codex_terminal',{taskId:state.task.id,conversationId:state.conversation.id,columns:100,rows:30,onEvent:channel});
  await invoke('update_codex_execution_settings',{taskId:state.task.id,conversationId:state.conversation.id,input:{permissionMode:'auto-review'}});
  state.permissionMode='auto-review (owned synthetic fixture only)';
  state.first=await invoke('continue_codex_task',{taskId:state.task.id,conversationId:state.conversation.id,resumeIntentId:crypto.randomUUID()});save();
  assert.equal(state.first.status,'started');
  const deadline=Date.now()+600000;let last='';
  while(Date.now()<deadline){
   view=await invoke('get_task_production',{taskId:state.task.id});const execution=view.record.workbench.execution;
   const summary=JSON.stringify({state:execution.state,units:execution.units.map(u=>[u.id,u.status]),attempts:view.record.workbench.attempts.length});
   if(summary!==last){console.log(summary);last=summary;}
   if(execution.units.filter(u=>['write-a','write-b'].includes(u.id)).every(u=>u.status==='succeeded')&&execution.state==='settledWithGaps')break;
   await new Promise(r=>setTimeout(r,2000));
  }
  const execution=view.record.workbench.execution;state.execution=execution;state.attempts=view.record.workbench.attempts;save();
  assert.equal(execution.units.find(u=>u.id==='write-a').status,'succeeded');assert.equal(execution.units.find(u=>u.id==='write-b').status,'succeeded');
  assert.equal(view.record.issues.find(i=>i.id==='injected-platform').status,'open');assert.equal(execution.units.find(u=>u.id==='dependent').status,'deferred');
  assert.equal(execution.state,'settledWithGaps');assert.equal(view.record.stages.implementation,'in-progress');assert.ok(state.attempts.length>=2);
  await page.reload();await page.getByRole('region',{name:'Lua 自主执行'}).waitFor();await page.screenshot({path:path.join(root,'autonomous-native.png'),fullPage:true});
  state.passed=true;save();console.log(JSON.stringify({passed:true,task:state.task.id,attempts:state.attempts.length}));
 }else throw Error('Use prepare or run');
}finally{await browser.close();}
