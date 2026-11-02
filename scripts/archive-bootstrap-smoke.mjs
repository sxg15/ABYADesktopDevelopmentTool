import fs from 'node:fs';import path from 'node:path';import crypto from 'node:crypto';import assert from 'node:assert/strict';import {spawn} from 'node:child_process';
import {connectPilot,readJson,saveJson} from './pilot-native-api.mjs';
if(!process.env.ABYA_ARCHIVE_QA_ROOT)throw Error('Set ABYA_ARCHIVE_QA_ROOT to an initialized isolated QA root.');
const root=path.resolve(process.env.ABYA_ARCHIVE_QA_ROOT),output=path.join(root,'evidence');fs.mkdirSync(output,{recursive:true});
const api=await connectPilot(root),cli=path.join(path.dirname(api.build.executable),'abya-desktop.exe');
const stateFile=path.join(root,'probe.json');let state=fs.existsSync(stateFile)?readJson(stateFile):{creationId:crypto.randomUUID(),tasks:{}};
function command(task,group,action,input){return new Promise((resolve,reject)=>{
 const child=spawn(cli,[group,action,'--input-file','-','--json'],{windowsHide:true,env:{...process.env,LOCALAPPDATA:path.join(root,'Local'),ABYA_TEST_MODE:'1',ABYA_DEVELOPMENT_TASK_ID:task.id,ABYA_DEVELOPMENT_CONVERSATION_ID:task.conversationId,ABYA_DEVELOPMENT_PROVIDER:'codex'},stdio:['pipe','pipe','pipe']});
 let stdout='',stderr='';child.stdout.on('data',d=>stdout+=d);child.stderr.on('data',d=>stderr=(stderr+d).slice(-4000));child.on('error',reject);child.on('close',()=>{try{resolve(JSON.parse(stdout));}catch{reject(Error('No JSON response: '+stderr));}});child.stdin.end(JSON.stringify(input));
});}
const record=(name,value)=>saveJson(path.join(output,name+'.json'),value);
try{
 if(process.argv[2]==='setup'){
  for(const key of ['A','B']){
   if(state.tasks[key])continue;
   const task=await api.invoke('create_task',{input:{title:`通用空白初始化验证 ${key}`,description:'隔离工具测试：创建本任务空白存档，验证任务隔离、稳定ID、读回与正常冷启动，不制作游戏。'}});
   const conv=await api.invoke('create_codex_conversation',{taskId:task.id,title:'工具契约验证（无模型推理）'});
   state.tasks[key]={...task,conversationId:conv.id};saveJson(stateFile,state);
  }console.log('Two independent ordinary tasks prepared.');
 }else if(process.argv[2]==='create'){
  const key=process.argv[3],task=state.tasks[key],input={creationId:state.creationId,name:'通用空白项目',author:'QA Team / Local'};
  record(key+'-request',input);const result=await command(task,'archive','create',input);record(key+'-create',result);
  console.log(JSON.stringify({case:key,transportSuccess:result.success,status:result.data?.status,error:result.data?.error,instanceId:result.data?.instanceId,archive:result.data?.archive}));
  assert.equal(result.data?.status,'ready');assert.equal(result.data.created,true);assert.equal(result.data.loaded,true);state.tasks[key].created=result.data;saveJson(stateFile,state);
 }else if(process.argv[2]==='verify'){
  const a=state.tasks.A,b=state.tasks.B;
  assert.notEqual(a.created.archive.archiveGuid,b.created.archive.archiveGuid);assert.notEqual(a.created.archive.archivePath,b.created.archive.archivePath);
  const count=fs.readdirSync(path.dirname(a.created.archive.archivePath)).length;
  const again=await command(a,'archive','create',{creationId:state.creationId,name:'通用空白项目',author:'QA Team / Local'});record('duplicate',again);assert.equal(again.data.instanceId,a.created.instanceId);assert.equal(fs.readdirSync(path.dirname(a.created.archive.archivePath)).length,count);
  const mismatch=await command(a,'archive','create',{creationId:state.creationId,name:'Changed name',author:'QA Team / Local'});record('conflicting-input',mismatch);assert.equal(mismatch.success,false);
  const cross=await command(a,'archive','creation-status',{taskId:b.id,creationId:state.creationId});record('cross-task-rejected',cross);assert.equal(cross.success,false);
  for(const [key,task] of Object.entries(state.tasks)){
   const status=await command(task,'archive','creation-status',{creationId:state.creationId});record(key+'-status',status);assert.equal(status.data.instanceId,task.created.instanceId);
   const live=await command(task,'runtime','run',{instanceId:task.created.instanceId,toolName:'editor_get_archive',arguments:{}});record(key+'-readback',live);assert.equal(live.success,true);assert.equal(live.data.guid,task.created.archive.archiveGuid);assert.equal(live.data.currentLevelGuid,task.created.archive.levelGuid);assert.equal(live.data.archiveAndLevelLoaded,true);
  }
  record('contract-summary',{passed:true,independentTasks:2,sameCreationIdAcrossTasksIsolated:true,duplicateDidNotCreate:true,conflictingInputRejected:true,crossTaskRejected:true,liveEditorIdentitiesMatched:true,gameplayAccepted:false});console.log('Generic creation/isolation/idempotency/readback checks passed.');
 }else if(process.argv[2]==='cold'){
  const task=state.tasks.A;record('A-stop',await command(task,'instance','stop',{instanceId:task.created.instanceId}));
  const launched=await command(task,'instance','launch',{taskId:task.id,name:'通用存档普通冷加载',executablePath:api.marker.player,mode:'editor',visibilityMode:'background',width:1280,height:720,exitOnFailure:true,archive:task.created.archive});record('A-cold-launch',launched);assert.equal(launched.success,true);
  state.coldInstance=launched.data.id;saveJson(stateFile,state);console.log(JSON.stringify({coldInstance:state.coldInstance}));
 }else if(process.argv[2]==='cold-check'){
  const task=state.tasks.A;record('A-cold-cli',await command(task,'runtime','wait',{instanceId:state.coldInstance,timeoutMs:60000}));
  // CLI 连通不等于关卡就绪；只轮询只读上下文，保留实际切换记录。
  const observations=[],deadline=Date.now()+120000;
  while(true){
   const context=await command(task,'runtime','run',{instanceId:state.coldInstance,toolName:'runtime_get_game_state',arguments:{}});observations.push({at:new Date().toISOString(),result:context});record('A-cold-context',observations);
   if(context.data?.context?.mode==='CreatorEditor'&&context.data.context.isTransitioning===false)break;
   assert.ok(Date.now()<deadline,'Cold launch did not reach the editor');await new Promise(resolve=>setTimeout(resolve,1000));
  }
  const live=await command(task,'runtime','run',{instanceId:state.coldInstance,toolName:'editor_get_archive',arguments:{}});record('A-cold-readback',live);assert.equal(live.data.guid,task.created.archive.archiveGuid);assert.equal(live.data.archiveAndLevelLoaded,true);console.log('Ordinary cold launch loaded the same archive and level.');
 }else if(process.argv[2]==='cleanup'){
  for(const [key,task] of Object.entries(state.tasks)){if(task.created)record(key+'-cleanup',await command(task,'instance','stop',{instanceId:task.created.instanceId}));}
  if(state.coldInstance)record('cold-cleanup',await command(state.tasks.A,'instance','stop',{instanceId:state.coldInstance}));
 }
}finally{await api.browser.close();}
