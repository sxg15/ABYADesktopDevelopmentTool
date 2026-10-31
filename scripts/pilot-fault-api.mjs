// Actual candidate API rejection probes. This task is not a gameplay pilot.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {connectPilot,saveJson,hashFile} from './pilot-native-api.mjs';
const api=await connectPilot(process.argv[2]);
try{
 const root=path.join(api.root,'faults');fs.mkdirSync(root,{recursive:true});
 const title='2.11 集成故障探针（不计游戏通过）';
 const matches=(await api.invoke('list_tasks')).filter(t=>t.title===title);assert.ok(matches.length<=1);
 const task=matches[0]??await api.invoke('create_task',{input:{title,description:'隔离 API 拒绝与状态保全测试；无游戏实现，不代表原生游戏成功。'}});
 let before=await api.invoke('get_task_production',{taskId:task.id});
 if(!before.record)before=await api.invoke('update_task_production',{input:{taskId:task.id,expectedRevision:0,operation:'initialize',data:{questionMode:'no-followup',playerMode:'single'}}});
 const revision=before.record.revision;
 for(const [kind,request] of [
  ['invalid-arguments',{input:{taskId:task.id,expectedRevision:revision,operation:'__invalid_operation',data:{}}}],
  ['stale-snapshot',{input:{taskId:task.id,expectedRevision:revision-1,operation:'configure',data:{questionMode:'no-followup',playerMode:'single'}}}],
 ]){
  let error;try{await api.invoke('update_task_production',request);}catch(e){error=e.message;}
  assert.ok(error,'Injected request must be rejected');
  const after=await api.invoke('get_task_production',{taskId:task.id});
  assert.deepEqual(after.record,before.record,'Rejected operation must not change the task');
  const file=path.join(root,`${kind}.json`);
  saveJson(file,{kind,passed:true,source:'actual-candidate-public-api',appSha256:hashFile(api.build.executable),taskId:task.id,at:new Date().toISOString(),request,error,beforeRevision:revision,afterRevision:after.record.revision,gameplayEvidence:false});
  console.log(JSON.stringify({kind,passed:true,evidence:{path:`faults/${kind}.json`,sha256:hashFile(file)}}));
 }
}finally{await api.browser.close();}
