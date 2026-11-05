import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {connectPilot,saveJson} from './pilot-native-api.mjs';
const root=path.resolve(process.argv[2]);
const fixture=path.resolve(process.argv[3]);
const api=await connectPilot(root);
const evidence=path.join(root,'evidence');fs.mkdirSync(evidence,{recursive:true});
let instance;
try {
 const task=await api.invoke('create_task',{input:{title:'任务私有存档启动回归',description:'隔离 QA：已有真实游戏的副本，测试存档路径与包唯一性，不修改用户原件。'}});
 const owned=path.join(path.dirname(task.workspacePath),'.abya-authoring',task.id);
 const source=path.join(owned,'Archives',path.basename(fixture));
 fs.mkdirSync(owned,{recursive:true});saveJson(path.join(owned,'owner.json'),{taskId:task.id,purpose:'authoring'});
 function copyTree(from,to){fs.mkdirSync(to,{recursive:true});for(const e of fs.readdirSync(from,{withFileTypes:true})){assert.ok(!e.isSymbolicLink());const a=path.join(from,e.name),b=path.join(to,e.name);if(e.isDirectory())copyTree(a,b);else fs.copyFileSync(a,b,fs.constants.COPYFILE_EXCL);}} copyTree(fixture,source);
 const list=await api.invoke('task_launch_sources',{taskId:task.id});
 assert.equal(list.entries.filter(e=>e.archive.archiveGuid===list.entries[0].archive.archiveGuid).length,2);
 saveJson(path.join(evidence,'sources.json'),list);
 const option=list.entries.find(e=>e.origin==='task');assert.ok(option);
 assert.equal(fs.realpathSync.native(option.archive.archivePath),fs.realpathSync.native(source));
 const level=option.archive.levels.find(l=>l.isStart)||option.archive.levels[0];assert.ok(level);
 const input={taskId:task.id,name:'私有存档真实启动',executablePath:list.player.selectedPath,profile:{mode:'editor',windowMode:'windowed',visibilityMode:'visible',width:1280,height:720,exitOnFailure:true,language:'',archive:{...option.archive,levelGuid:level.levelGuid,levelName:level.levelName}}};
 // 即使路径是另一个真实 EXE，也不能覆盖 APP 设置。
 const bad={...input,executablePath:process.execPath};
 await assert.rejects(api.invoke('launch_instance',{input:bad}),/app_player_mismatch/);
 const runs=[];
 for(const mode of ['editor','offline']){
  input.profile.mode=mode;instance=await api.invoke('launch_instance',{input});
  saveJson(path.join(evidence,mode+'-instance.json'),instance);
  let report;
  const reportPath=path.join(api.paths.reportsDir,instance.id+'.json');
  for(let i=0;i<90;i++){
   if(fs.existsSync(reportPath)){try{report=JSON.parse(fs.readFileSync(reportPath,'utf8').replace(/^\uFEFF/,''));}catch{}}
   if(report?.success===true||report?.phase==='failed')break;
   await new Promise(r=>setTimeout(r,1000));
  }
  saveJson(path.join(evidence,mode+'-report.json'),report??{});
  assert.equal(report?.success,true,JSON.stringify(report)); assert.equal(report.archiveGuid,option.archive.archiveGuid); assert.equal(report.levelGuid,level.levelGuid);
  const info=await api.invoke('get_instance',{id:instance.id});assert.equal(info.processAlive,true);
  const log=fs.readFileSync(instance.logFilePath,'utf8');
  fs.writeFileSync(path.join(evidence,mode+'-path-log.txt'),log.split(/\r?\n/).filter(l=>l.includes('.abya-authoring')||l.includes('ArchivePath')).join('\n'));
  assert.ok(log.includes('.abya-authoring'), 'runtime log must show private archive root');
  runs.push({mode,instanceId:instance.id,success:true,archivePath:source,report});
  await api.invoke('stop_instance',{id:instance.id});instance=null;
 }
 saveJson(path.join(evidence,'result.json'),{taskId:task.id,player:list.player,runs,foreignExecutableRejected:true,app:api.build});
 console.log(JSON.stringify({success:true,taskId:task.id,runs:runs.map(r=>({mode:r.mode,success:r.success})),evidence}));
}catch(error){ console.error("SMOKE FAILURE",error); throw error; }finally{
 if(instance)await api.invoke('stop_instance',{id:instance.id}).catch(()=>{});
 await api.browser.close();
}
