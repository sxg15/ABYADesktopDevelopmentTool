import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {DatabaseSync} from 'node:sqlite';
import {connectPilot,readJson,saveJson,hashFile,flushConnection} from './pilot-native-api.mjs';
const [action,directory,arg,repeatText='1']=process.argv.slice(2),root=path.resolve(directory);
if(action==='init'){
 if(fs.existsSync(root))throw Error('Use a fresh isolated pilot root');
 const player=fs.realpathSync(arg);assert.ok(player.toLowerCase().endsWith('.exe'));
 const data=path.join(root,'Local','ABYA Desktop Development Tool','Data');fs.mkdirSync(data,{recursive:true});
 saveJson(path.join(root,'pilot-root.json'),{schema:'abya.workflow-pilot-root/v1',debugPort:9254,player,createdAt:new Date().toISOString()});
 const db=new DatabaseSync(path.join(data,'app.db'));
 db.exec('CREATE TABLE settings(key TEXT PRIMARY KEY,value_json TEXT NOT NULL,updated_at TEXT NOT NULL)');
 const put=db.prepare('INSERT INTO settings VALUES(?,?,?)');
 put.run('application',JSON.stringify({locale:'zh-CN',gameExecutablePath:player,workspaceRootPath:path.join(root,'Workspaces'),desktopCliPort:44061,gameGatewayPort:44062,lanBroadcastEnabled:false}),new Date().toISOString());
 put.run('taskNotifications',JSON.stringify({enabled:false}),new Date().toISOString());db.close();
 console.log(JSON.stringify({initialized:root,player}));
}else if(action==='create'){
 const spec=readJson(arg),repeat=Number(repeatText);
 assert.match(spec.id,/^[a-z][a-z0-9-]{1,40}$/);assert.ok(Number.isInteger(repeat)&&repeat>0&&repeat<10);
 assert.ok(['single','multiplayer'].includes(spec.playerMode));
 const api=await connectPilot(root);
 try{
  const name=`${spec.id}-${repeat}`,out=path.join(root,'runs',name);fs.mkdirSync(out,{recursive:true});
  const stateFile=path.join(out,'run.json');let state=fs.existsSync(stateFile)?readJson(stateFile):{caseId:spec.id,repeat,mode:'native-gameplay',interventions:[],approvals:[],externalImplementation:false};
  const appHash=hashFile(api.build.executable),inventoryHash=hashFile(path.join(path.dirname(api.build.executable),'package-files.json'));
  if(state.appSha256)assert.equal(state.appSha256,appHash,'Candidate changed; use an independent qualification run');
  if(state.packageInventorySha256)assert.equal(state.packageInventorySha256,inventoryHash,'Candidate resources changed');
  state.appSha256=appHash;state.packageInventorySha256=inventoryHash;
  if(state.specSha256&&state.specSha256!==hashFile(arg))throw Error('Pilot specification changed; keep the existing run and start an independent repetition');
  state.specSha256=hashFile(arg);saveJson(stateFile,state);
  const title=`2.11 隔离实测 · ${spec.title} · ${repeat}`;
  if(!state.task){
   const matches=(await api.invoke('list_tasks')).filter(t=>t.title===title);assert.ok(matches.length<=1);
   state.task=matches[0]??await api.invoke('create_task',{input:{title,description:`这是已授权的工作流原生隔离测试，非用户正式项目。固定规格通过输入附件提供，分别是玩法要求、美术参考和旧方案参考；旧文件中的历史流程/旧版本状态不覆盖当前2.11策略。不要删减玩法范围，不复制已有游戏实现，不访问其他任务存档。可复用 inputs/assets 的合法素材，必须在本任务通过托管CLI实际制作、运行验证与产包。按当前七阶段流程提交成果；需求、美术和计划将在隔离APP按固定规格审查，不自行批准。${spec.description??''}`}});
   saveJson(stateFile,state);
  }
  assert.ok(path.resolve(state.task.workspacePath).toLowerCase().startsWith(path.join(root,'Workspaces').toLowerCase()+path.sep));
  const taskId=state.task.id;let view=await api.invoke('get_task_production',{taskId});
  if(!view.record)view=await api.invoke('update_task_production',{input:{taskId,expectedRevision:0,operation:'initialize',data:{questionMode:'no-followup',playerMode:spec.playerMode}}});
  assert.equal(view.record.workflowVersion,'2.11.0');
  state.inputs??=[];
  for(const [kind,file] of Object.entries(spec.documents)){
   const saved=state.inputs.find(i=>i.kind===kind);
   if(saved){assert.equal(saved.sha256,hashFile(file),'Fixed pilot input changed');continue;}
   const data=fs.readFileSync(file),name=`${kind}-fixed-source.md`;
   const existing=view.record.artifacts.filter(a=>a.plannerInput&&a.title===name);assert.ok(existing.length<=1);
   if(existing.length){
    const original=path.resolve(state.task.workspacePath,path.dirname(existing[0].path),'original.md');
    assert.ok(original.toLowerCase().startsWith(path.resolve(state.task.workspacePath).toLowerCase()+path.sep));
    assert.equal(hashFile(original),hashFile(file));
   }else view=await api.invoke('mutate_production_workbench',{input:{taskId,expectedRevision:view.record.revision,operation:'attach',data:{name,base64:data.toString('base64')}}});
   state.inputs.push({kind,name,sha256:hashFile(file)});saveJson(stateFile,state);
  }
  const assets=path.join(state.task.workspacePath,'inputs','assets');fs.mkdirSync(assets,{recursive:true});
  for(const file of spec.assets??[]){assert.match(file,/\.(png|jpg|jpeg|webp|ttf|otf)$/i);fs.copyFileSync(file,path.join(assets,path.basename(file)));}
  if(!state.conversation){
   const existing=await api.invoke('list_codex_conversations',{taskId});
   state.conversation=existing.find(c=>c.title==='原生制作与验收')??await api.invoke('create_codex_conversation',{taskId,title:'原生制作与验收'});saveJson(stateFile,state);
  }
  if(state.startReceipt){console.log(JSON.stringify({case:name,alreadyStarted:true,control:await api.invoke('get_codex_task_control',{taskId,conversationId:state.conversation.id})}));process.exitCode=0;}
  else {
  await api.invoke('open_codex_terminal',{taskId,conversationId:state.conversation.id,columns:120,rows:30});
  // 沿用用户原任务的 Astra/high/完全同意设置，仅设置本次隔离测试会话。
  await api.invoke('update_codex_execution_settings',{taskId,conversationId:state.conversation.id,input:{model:'gpt-6-astra',effort:'high',permissionMode:'full-access'}});
  if(!state.intent){state.intent=crypto.randomUUID();state.startedAt=new Date().toISOString();saveJson(stateFile,state);}
  const response=await api.invoke('continue_codex_task',{taskId,conversationId:state.conversation.id,resumeIntentId:state.intent});
  state.startReceipt=response;state.nativeSessionId=response.nativeSessionId;saveJson(stateFile,state);
  state.startReceipt=await flushConnection(api,taskId,state.conversation.id,response);saveJson(stateFile,state);
  console.log(JSON.stringify({case:name,taskId,status:response.status,nativeSessionId:state.nativeSessionId}));
  }
 }finally{await api.browser.close();}
}else throw Error('Use init ROOT PLAYER or create ROOT SPEC_JSON REPEAT');
