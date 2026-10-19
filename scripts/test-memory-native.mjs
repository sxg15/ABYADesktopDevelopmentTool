// 只使用 marker 所属隔离数据、复制存档和显式 Player；不启动模型，不执行复现包里的脚本。
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { createRequire } from 'node:module';
import { execFile, execFileSync } from 'node:child_process';
import { DatabaseSync } from 'node:sqlite';
const [action, directory, player, archiveSource, phase='ui'] = process.argv.slice(2);
if (!['init','run'].includes(action)||!directory) throw new Error('Usage: init root | run root Player.exe archive [baseline|ui|architecture|guard]');
const root=path.resolve(directory), marker=path.join(root,'memory-fixture.json');
if(action==='init') {
  assert.ok(!fs.existsSync(root),'Use a fresh fixture');
  fs.mkdirSync(path.join(root,'Local','ABYA Desktop Development Tool','Data'),{recursive:true});
  fs.writeFileSync(marker,JSON.stringify({schema:'abya-memory-fixture/v1'}));
  const db=new DatabaseSync(path.join(root,'Local','ABYA Desktop Development Tool','Data','app.db'));
  db.exec('CREATE TABLE settings(key TEXT PRIMARY KEY,value_json TEXT NOT NULL,updated_at TEXT NOT NULL)');
  const insert=db.prepare('INSERT INTO settings VALUES(?,?,?)');
  insert.run('application',JSON.stringify({locale:'zh-CN',gameExecutablePath:'',workspaceRootPath:path.join(root,'Workspaces'),desktopCliPort:43931,gameGatewayPort:43932,lanBroadcastEnabled:false}),new Date().toISOString());
  insert.run('taskNotifications',JSON.stringify({enabled:false}),new Date().toISOString());db.close();
  console.log(JSON.stringify({root,localAppData:path.join(root,'Local'),webView:path.join(root,'WebView')}));
} else {
  assert.equal(JSON.parse(fs.readFileSync(marker,'utf8')).schema,'abya-memory-fixture/v1');
  assert.ok(player&&archiveSource&&['baseline','ui','architecture','guard'].includes(phase));
  const {chromium}=createRequire(import.meta.url)(process.env.ABYA_TEST_PLAYWRIGHT||'playwright');
  const browser=await chromium.connectOverCDP('http://127.0.0.1:9256');
  const page=browser.contexts()[0].pages()[0];await page.waitForFunction(()=>!!window.__TAURI_INTERNALS__?.invoke);
  const invoke=async(command,args={})=>{const result=await page.evaluate(async({command,args})=>{
    try{return {ok:true,value:await window.__TAURI_INTERNALS__.invoke(command,args)};}
    catch(e){return {ok:false,error:{code:e?.code,message:e?.message??String(e)}};}
  },{command,args});if(!result.ok)throw new Error(`${command}: ${JSON.stringify(result.error)}`);return result.value;};
  const paths=await invoke('get_app_paths');
  assert.equal(path.resolve(paths.dataDir).toLowerCase(),path.join(root,'Local','ABYA Desktop Development Tool','Data').toLowerCase());
  assert.equal((await invoke('get_build_info')).testEnvironment,true);
  const task=await invoke('create_task',{input:{title:`内存隔离复现 ${phase}`,description:'Synthetic diagnostic task; copied post-fix archive, not the historical failure snapshot.'}});
  const run=path.join(root,task.id);fs.mkdirSync(run);
  const archive=path.join(root,'Local','ABYA Desktop Development Tool','Data','IsolatedGameData','Archives',task.id);
  fs.mkdirSync(path.dirname(archive),{recursive:true});
  execFileSync('C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe',['-NoProfile','-Command',"$ErrorActionPreference='Stop'; Copy-Item -LiteralPath $env:ABYA_COPY_SOURCE -Destination $env:ABYA_COPY_TARGET -Recurse"],
    {windowsHide:true,timeout:30000,env:{...process.env,ABYA_COPY_SOURCE:path.resolve(archiveSource),ABYA_COPY_TARGET:archive}});
  const conversation=crypto.randomUUID(), stamp=new Date().toISOString();
  const conversationPath=path.join(task.workspacePath,'conversations',conversation);fs.mkdirSync(conversationPath,{recursive:true});
  fs.writeFileSync(path.join(conversationPath,'conversation.json'),JSON.stringify({id:conversation,taskId:task.id,title:'合成上下文验证',createdAt:stamp,updatedAt:stamp,nativeSessionId:null}));
  fs.writeFileSync(path.join(conversationPath,'workflow.json'),JSON.stringify({taskId:task.id,conversationId:conversation,observability:'native',warning:'Synthetic fixture only',currentTurnId:'fixture-turn',updatedAt:stamp,turns:[{id:'fixture-turn',status:'inProgress',explanation:'Fixture',plan:[],startedAt:stamp,completedAt:null,activities:[{id:'fixture-operation',stepId:null,kind:'command',status:'started',summary:'$env:API_KEY=MEMORY_SECRET_SENTINEL',detail:'',source:'synthetic-fixture',unplanned:true,startedAt:stamp,completedAt:null}]}]}));
  const build=await invoke('get_build_info');
  const desktopCli=path.join(path.dirname(build.executable),'abya-desktop.exe');
  const cliEnv={...process.env,LOCALAPPDATA:path.join(root,'Local'),ABYA_TEST_MODE:'1'};
  for(const key of ['ABYA_DEVELOPMENT_TASK_ID','ABYA_DEVELOPMENT_CONVERSATION_ID','ABYA_DESKTOP_CLI','ABYA_DESKTOP_SESSION_CAPABILITY','ABYA_CLI_TOKEN','ABYA_CLI_INSTANCE_DIR']) delete cliEnv[key];
  cliEnv.ABYA_DEVELOPMENT_TASK_ID=task.id;cliEnv.ABYA_DEVELOPMENT_CONVERSATION_ID=conversation;cliEnv.ABYA_DEVELOPMENT_PROVIDER='codex';
  const cli=async(group,verb,input,timeout=95000)=>new Promise((resolve,reject)=>{
    const child=execFile(desktopCli,[group,verb,'--json','--input-file','-'],{windowsHide:true,timeout,maxBuffer:20*1024*1024,
      env:cliEnv},
      (error,stdout)=>{try{const value=JSON.parse(stdout);resolve(value);}catch{reject(new Error(error?.message||'CLI returned no JSON'));}});
    child.stdin.end(JSON.stringify(input));
  });
  const unbox=value=>{if(value?.data!==undefined)return unbox(value.data);if(value?.content?.[0]?.text){try{return unbox(JSON.parse(value.content[0].text));}catch{}}return value;};
  let instance, timer;const observations=[];
  try {
    await invoke('set_instance_memory_policy',{policy:{privateLimitMib:4096,growthLimitMib:768,availableFloorMib:8192,commitLimitPercent:90}});
    instance=await invoke('launch_instance',{input:{taskId:task.id,name:`memory-${phase}`,executablePath:path.resolve(player),profile:{mode:'editor',windowMode:'windowed',visibilityMode:'background',width:1280,height:720,archive:{archivePath:archive,archiveGuid:'b974f73a-6806-48e5-b1d1-e0b23b53665d',archiveName:'内存隔离副本',levelGuid:'37ff01bd16b948fa8e5ea40a62aa157b',levelName:'新关卡'},hostInstanceId:null,exitOnFailure:true,language:'zh-CN'}}});
    fs.writeFileSync(path.join(run,'launch.json'),JSON.stringify(instance,null,2));
    timer=setTimeout(()=>{void invoke('stop_instance',{id:instance.id});},180000);
    await invoke('arm_instance_memory_stop',{id:instance.id,enabled:true});
    const call=async(name,args={})=>{const started=Date.now();const response=await cli('runtime','run',{instanceId:instance.id,toolName:name,arguments:args});
      const file=fs.existsSync(path.join(run,`${name}.json`))?`${name}-${observations.length}.json`:`${name}.json`;
      fs.writeFileSync(path.join(run,file),JSON.stringify(response));observations.push({tool:name,file,milliseconds:Date.now()-started,transportSuccess:response.success,runtimeError:response.data?.isError??null});return unbox(response);};
    let ready=false;
    for(let i=0;i<60;i++) {const rawState=await cli('runtime','status',{instanceId:instance.id},10000);const state=unbox(rawState);
      fs.writeFileSync(path.join(run,'runtime-status-last.json'),JSON.stringify(rawState));
      let launchState;try{launchState=JSON.parse(fs.readFileSync(path.join(paths.reportsDir,`${instance.id}.json`),'utf8'));}catch{}
      if(state.connected&&launchState?.success&&launchState.archiveGuid==='b974f73a-6806-48e5-b1d1-e0b23b53665d'){ready=true;break;}
      if(!(await invoke('get_instance',{id:instance.id})).processAlive)break;await new Promise(r=>setTimeout(r,1000));}
    assert.ok(ready,'Runtime must become ready within protection budget');
    await call('read_me_first'); await call('editor_get_state');
    if(phase==='ui') {await call('custom_ui_authoring_open');await call('custom_ui_authoring_get_tree',{detail:'full',limit:400});}
    if(phase==='architecture') {
      const manifest=JSON.parse(fs.readFileSync(path.resolve('logs/memory-repro-20261019/source/Workspace/artifacts/gameplay-architecture.v1.json'),'utf8'));
      const snapshot=await call('editor_gameplay_architecture_snapshot');
      assert.equal(snapshot.archiveId,manifest.archiveId);assert.equal(snapshot.levelId,manifest.levelId);
      for(const component of manifest.components??[]) if(component.target) assert.ok(snapshot.targets.some(item=>Object.entries(component.target).every(([key,value])=>item.target?.[key]===value)),`Manifest target missing: ${component.id}`);
      const current={...manifest,snapshotRevision:snapshot.snapshotRevision};
      fs.writeFileSync(path.join(run,'current-manifest.json'),JSON.stringify(current,null,2));
      fs.writeFileSync(path.join(run,'manifest-provenance.json'),JSON.stringify({sourceRevision:manifest.snapshotRevision,currentRevision:snapshot.snapshotRevision,note:'Only copied manifest revision refreshed after checking live archive, level and component targets; no diagnostic waived.'}));
      await call('editor_gameplay_architecture_validate',{manifest:current});
      await call('editor_gameplay_architecture_lint',{manifest:current});
    }
    if(phase==='guard') await invoke('set_instance_memory_policy',{policy:{privateLimitMib:256,growthLimitMib:768,availableFloorMib:8192,commitLimitPercent:90}});
    for(let i=0;i<20;i++) {const state=await invoke('get_instance',{id:instance.id});if(!state.processAlive)break;await new Promise(r=>setTimeout(r,1000));}
  } finally {
    clearTimeout(timer);
    if(instance) {
      const beforeStop=await invoke('instance_memory_status',{id:instance.id});
      const state=await invoke('get_instance',{id:instance.id});
      const stop=state.processAlive?await invoke('stop_instance',{id:instance.id}):{processAlive:false,forcedTermination:null};assert.equal(stop.processAlive,false);
      await new Promise(r=>setTimeout(r,500));
      const report={phase,scope:'post-fix archive; synthetic Codex context; no model execution',instanceId:instance.id,observations,beforeStop,stop:{processAlive:stop.processAlive,forced:stop.forcedTermination}};
      fs.writeFileSync(path.join(run,'result.json'),JSON.stringify(report,null,2));
      if(beforeStop.reportPath) {const text=fs.readFileSync(beforeStop.reportPath,'utf8');assert.ok(!text.includes('MEMORY_SECRET_SENTINEL'));}
      if(phase==='guard') { assert.ok(beforeStop.incidentId&&beforeStop.pressured,'Threshold test must leave a memory-pressure incident');
        const context=JSON.parse(fs.readFileSync(path.join(path.dirname(beforeStop.reportPath),'codex-context.json'),'utf8'));
        assert.ok(context.conversations?.length>0,'Synthetic Codex activity must be traceable'); }
      console.log(JSON.stringify({run,phase,observations,peakPrivateBytes:beforeStop.peakPrivateBytes,incidentId:beforeStop.incidentId,processAlive:stop.processAlive}));
    }
    await browser.close();
  }
}
