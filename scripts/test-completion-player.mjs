// Controlled real-Player probes on a copied archive; no original task mutation or acceptance.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {createRequire} from 'node:module';
import {execFile,execFileSync} from 'node:child_process';
const [action,directory,inputFile]=process.argv.slice(2),root=path.resolve(directory);
assert.equal(JSON.parse(fs.readFileSync(path.join(root,'memory-fixture.json'))).schema,'abya-memory-fixture/v1');
const {chromium}=createRequire(import.meta.url)(process.env.ABYA_TEST_PLAYWRIGHT||'playwright');
const browser=await chromium.connectOverCDP('http://127.0.0.1:9256');
const page=browser.contexts()[0].pages()[0];await page.waitForFunction(()=>!!window.__TAURI_INTERNALS__?.invoke);
const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});
const paths=await invoke('get_app_paths'),build=await invoke('get_build_info');
assert.equal(path.resolve(paths.dataDir).toLowerCase(),path.join(root,'Local','ABYA Desktop Development Tool','Data').toLowerCase());
assert.equal(build.testEnvironment,true);assert.equal(build.verification,'verified');
const contextPath=path.join(root,'run-context.json');let context;
const normalize=p=>p.startsWith(String.fromCharCode(92,92,63,92))?p.slice(4):p;
const unbox=v=>v?.data!==undefined?unbox(v.data):v?.content?.[0]?.text?(()=>{try{return unbox(JSON.parse(v.content[0].text));}catch{return v;}})():v;
async function cli(group,verb,data,timeout=120000){
 const env={...process.env,LOCALAPPDATA:path.join(root,'Local'),ABYA_TEST_MODE:'1',ABYA_DEVELOPMENT_TASK_ID:context.task.id,ABYA_DEVELOPMENT_CONVERSATION_ID:context.conversation.id,ABYA_DEVELOPMENT_PROVIDER:'codex'};
 for(const k of ['ABYA_DESKTOP_PIPE','ABYA_DESKTOP_SESSION_TOKEN','ABYA_DESKTOP_SESSION_CAPABILITY','ABYA_CLI_TOKEN','ABYA_CLI_INSTANCE_DIR'])delete env[k];
 return new Promise((resolve,reject)=>{const child=execFile(context.cli,[group,verb,'--json','--input-file','-'],{env,windowsHide:true,timeout,maxBuffer:20*1024*1024},(error,stdout)=>{try{resolve(JSON.parse(stdout));}catch{reject(Error(error?.message||'CLI did not return JSON'));}});child.stdin.end(JSON.stringify(data));});
}
async function call(tool,args={}){
 const result=await cli('runtime','run',{instanceId:context.instance.id,toolName:tool,arguments:args});
 const file=path.join(root,'captures',`${tool}-${crypto.randomUUID()}.json`);fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,JSON.stringify(result,null,2));
 if(!result.success||result.data?.isError)throw Error(JSON.stringify({tool,file,error:result.error??result.data}));
 console.log(JSON.stringify({tool,file,data:unbox(result)}).slice(0,4500));return result;
}
async function coreFlow(fromResults){
 const attempt=path.join(root,'core-flow-attempt.json');assert.ok(!fs.existsSync(attempt),'Do not replay an existing UI input attempt');
 fs.writeFileSync(attempt,JSON.stringify({status:'started',at:new Date().toISOString()}));
 const wait=ms=>new Promise(r=>setTimeout(r,ms)),scopeId='completion-v5-core';
 const click=(x,y)=>call('runtime_input_mouse_click',{x,y,origin:'topLeft',scopeId});
 const lua=async(name,body)=>unbox(await call('lua_run',{side:'client',script:'--[[ABYA-LUA\n'+JSON.stringify({schemaVersion:1,apiVersion:'3.0',name,description:'Observe UI-driven gameplay; only requests read-only state via hello',context:'automation',side:'client',entry:'main',requires:[],parameters:[],returns:[]})+'\nABYA-LUA]]\nfunction main(input)\n'+body+'\nend'}));
 const poll=async id=>{for(let n=0;n<30;n++){const s=unbox(await call('lua_status',{run_id:id}));if(s.state!=='Running')return s;await wait(200);}throw Error('Observer did not complete');};
 try{
  if(fromResults){await click(640,628);await wait(300);}
  const numbers=await lua('completion-first-pair','local s=abya.network.listen_client("size-memory.state");abya.network.send_to_server("size-memory.action",{kind="hello"});local first=nil;for i=1,2000 do local p=s:wait();if type(p)=="table" and p.phase=="playing" then if p.stage=="memory" then first=p.current elseif p.stage=="answer" and first then s:dispose();return first,p.current,p.matchId end end end;s:dispose();error("no first pair")');
  const observer=await lua('completion-ui-lifecycle','local s=abya.network.listen_client("size-memory.state");abya.network.send_to_server("size-memory.action",{kind="hello"});local played=false;local accepted=false;local ended=false;local score=0;local match=0;for i=1,2000 do local p=s:wait();if type(p)=="table" then if p.phase=="playing" then played=true;match=p.matchId end;if played and (p.accepted or 0)>=1 then accepted=true end;if played and p.phase=="results" then ended=true;score=p.score end;if ended and p.phase=="lobby" then s:dispose();return played,accepted,match,score,p.phase,p.ready,p.score end end end;s:dispose();error("no lifecycle end")');
  await click(486,628);await wait(300);await click(792,628);
  const pair=await poll(numbers.runId);assert.equal(pair.state,'Completed');assert.equal(pair.error,null);
  const [first,second,match]=pair.values;assert.ok(first>=1&&first<=99&&second>=1&&second<=99&&first!==second);
  await call('runtime_input_mouse_set_position',{x:640,y:400,origin:'topLeft',scopeId});
  await call('runtime_input_mouse_press',{button:0,scopeId});await wait(100);
  await call('runtime_input_mouse_set_position',{x:640,y:second>first?275:525,origin:'topLeft',scopeId});await wait(100);
  await call('runtime_input_mouse_release',{button:0,scopeId});await wait(100);
  await call('ui_capture_screenshot',{maxWidth:1280});await wait(31000);
  await call('ui_capture_screenshot',{maxWidth:1280});await click(640,628);await wait(400);
  const result=await poll(observer.runId);assert.equal(result.state,'Completed');assert.equal(result.error,null);
  assert.deepEqual(result.values,[true,true,match,5,'lobby',false,0]);
  await call('ui_capture_screenshot',{maxWidth:1280});
  const proof={passed:true,sourceVersion:context.sourceVersion,archiveHash:context.sourceArchiveHash,playerBuildHash:context.playerBuildHash,instanceId:context.instance.id,first,second,match,observer:result,method:'real virtual pointer UI ready/start/drag/return; observer only hello/read; no direct answer or score mutation',humanAccepted:false};
  fs.writeFileSync(path.join(root,'core-flow-result.json'),JSON.stringify(proof,null,2));console.log('CORE FLOW VERIFIED');
 }finally{await call('runtime_input_reset',{scopeId});}
}
try{
 if(action==='prepare'){
  assert.ok(!fs.existsSync(contextPath));
  const receipt=JSON.parse(fs.readFileSync(inputFile,'utf8').replace(/^\uFEFF/,''));
  const record=receipt.data.record,version=record.versionDetails;
  const task=await invoke('create_task',{input:{title:'大小记忆 v5 核心可玩性隔离复验',description:'仅使用独立复制的冻结存档及匹配 Player 验证，不修改原任务或批准。'}});
  const conversation=await invoke('create_codex_conversation',{taskId:task.id,title:'隔离运行验证'});
  const archive=path.join(paths.dataDir,'IsolatedGameData','Archives',task.id);
  fs.mkdirSync(path.dirname(archive),{recursive:true});console.log('Copying frozen archive to owned fixture');
  execFileSync('C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe',['-NoProfile','-Command',"$ErrorActionPreference='Stop'; Copy-Item -LiteralPath $env:ABYA_COPY_SOURCE -Destination $env:ABYA_COPY_TARGET -Recurse"],{windowsHide:true,timeout:60000,env:{...process.env,ABYA_COPY_SOURCE:normalize(version.archiveSource),ABYA_COPY_TARGET:archive}});
  const instance=await invoke('launch_instance',{input:{taskId:task.id,name:'v5 core verification',executablePath:normalize(version.playerSource),profile:{mode:'lan-host',windowMode:'windowed',visibilityMode:'background',width:1280,height:720,archive:{archivePath:archive,archiveGuid:version.archiveGuid,archiveName:'v5 verification copy',levelGuid:version.levelGuid,levelName:'verification'},hostInstanceId:null,exitOnFailure:true,language:'zh-CN'}}});
  context={task,conversation,instance,cli:path.join(path.dirname(build.executable),'abya-desktop.exe'),sourceVersion:record.currentVersion,archive,playerBuildHash:version.playerBuildHash,sourceArchiveHash:version.archiveHash};
  fs.writeFileSync(contextPath,JSON.stringify(context,null,2));
  let ready=false;
  for(let n=0;n<90;n++){
   const state=await invoke('get_instance',{id:instance.id});
   if(!state.processAlive)throw Error('Isolated Player exited before readiness');
   let disk;try{disk=JSON.parse(fs.readFileSync(path.join(paths.reportsDir,instance.id+'.json'),'utf8'));}catch{}
   if(disk?.phase==='failed')throw Error(disk.error);
   if(disk?.phase==='ready'&&disk.success){ready=true;break;}
   await new Promise(r=>setTimeout(r,1000));
  }
  assert.ok(ready,'Launch did not reach ready');
  await call('read_me_first');await call('runtime_get_game_state');
  const catalog=await cli('runtime','list',{instanceId:instance.id});fs.writeFileSync(path.join(root,'catalog.json'),JSON.stringify(catalog,null,2));
  await call('ui_capture_screenshot',{maxWidth:1280});console.log('READY');
 }else{
  context=JSON.parse(fs.readFileSync(contextPath));const state=await invoke('get_instance',{id:context.instance.id});assert.equal(state.instance.taskId,context.task.id);
  if(action==='core-flow'){await coreFlow(JSON.parse(fs.readFileSync(inputFile,'utf8')).fromResults===true);}
  else if(action==='scenario'){const plan=JSON.parse(fs.readFileSync(inputFile,'utf8'));assert.ok(plan.steps.length<=64);for(const step of plan.steps){await call(step.toolName,step.arguments??{});if(step.waitAfterMs)await new Promise(r=>setTimeout(r,Math.min(45000,step.waitAfterMs)));}}
  else if(action==='call'){const input=JSON.parse(fs.readFileSync(inputFile,'utf8'));await call(input.toolName,input.arguments??{});}
  else if(action==='stop'){const result=await invoke('stop_instance',{id:context.instance.id});assert.equal(result.processAlive,false);console.log('Owned instance stopped');}
  else throw Error('Use prepare, call or stop');
 }
}catch(error){console.error(JSON.stringify({error:String(error),stack:error?.stack}));process.exitCode=1;}finally{await browser.close();}
