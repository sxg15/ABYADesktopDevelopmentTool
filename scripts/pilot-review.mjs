// 只读取隔离任务；approve 是固定测试规格的 APP 审查动作，不是正式用户作品验收。
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {DatabaseSync} from 'node:sqlite';
import {connectPilot,readJson,saveJson,hashFile} from './pilot-native-api.mjs';
const [action,directory,name,kind,expectedHash,feedbackPath]=process.argv.slice(2);
assert.match(name,/^[a-z0-9-]+$/);
const api=await connectPilot(directory),out=path.join(api.root,'runs',name),file=path.join(out,'run.json');
try{
 const state=readJson(file),taskId=state.task.id,conversationId=state.conversation.id;
 if(state.appSha256)assert.equal(state.appSha256,hashFile(api.build.executable),'Mixed-candidate runs cannot qualify');
 if(state.packageInventorySha256)assert.equal(state.packageInventorySha256,hashFile(path.join(path.dirname(api.build.executable),'package-files.json')));
 const view=await api.invoke('get_task_production',{taskId});
 const control=await api.invoke('get_codex_task_control',{taskId,conversationId});
 const workflow=await api.invoke('get_codex_workflow',{taskId,conversationId});
 saveJson(path.join(out,'production.json'),view);saveJson(path.join(out,'workflow.json'),workflow);
 const r=view.record,pack=r.workbench?.execution?.verificationPackage;
 if(!state.firstPlayableAt){
  const db=new DatabaseSync(path.join(api.paths.dataDir,'app.db'),{readOnly:true});
  try{
   const first=db.prepare("SELECT json_extract(body,'$.workbench.execution.verificationPackage.createdAt') AS at FROM task_production_history WHERE task_id=? AND json_extract(body,'$.workbench.execution.verificationPackage.status')='core-verified' ORDER BY revision LIMIT 1").get(taskId);
   if(first?.at)state.firstPlayableAt=first.at;
  }finally{db.close();}
 }
 state.observedStates??=[];
 const last=state.observedStates.at(-1);
 if(last?.stage!==r.currentStage||last?.control!==control.state)state.observedStates.push({at:new Date().toISOString(),stage:r.currentStage,control:control.state});
 state.timing={nativeActiveMs:workflow.turns.reduce((sum,t)=>sum+(t.completedAt?Math.max(0,Date.parse(t.completedAt)-Date.parse(t.startedAt)):0),0),
  unitActiveMs:(r.workbench.execution.units??[]).reduce((sum,u)=>sum+(u.elapsedMs??0),0),
  note:'Native completed-turn duration and APP unit accounting are separate measurements; sampled state transitions are not exact per-category durations.'};
 if(action==='status'){
  saveJson(file,state);
  console.log(JSON.stringify({name,taskId,stage:r.currentStage,control:control.state,execution:r.workbench?.execution?.state,
   active:r.workbench?.execution?.units?.find(u=>u.id===r.workbench.execution.activeUnitId)?.title,
   version:r.currentVersion,package:pack?.version,awaiting:Object.entries(r.documents).filter(([stage])=>r.stages[stage]==='awaiting-confirmation').map(([stage,d])=>({stage,path:path.join(state.task.workspacePath,d.path),sha256:d.sha256})),
   errors:control.lastError,technicalQueue:r.workbench?.execution?.technicalQueue}));
 }else if(action==='approve'||action==='reject'){
  assert.ok(['requirements','resources','plan'].includes(kind),'No automatic delivery/human acceptance');
  assert.equal(r.stages[kind],'awaiting-confirmation');assert.equal(r.documents[kind].sha256,expectedHash,'Review the exact submitted document first');
  assert.equal(view.actions.approve[kind],'');
  const accepted=action==='approve';
  const feedback=accepted?'隔离原生测试：按预先固定的测试规格审查此文档；不是正式用户游戏验收。':fs.readFileSync(feedbackPath,'utf8').trim();
  assert.ok(feedback);
  const approved=await api.invoke('decide_task_production',{input:{taskId,expectedRevision:r.revision,kind,documentHash:expectedHash,accepted,feedback}});
  const intent=crypto.randomUUID();
  state.approvals.push({kind,accepted,documentHash:expectedHash,feedback,at:new Date().toISOString(),fixtureReview:true});saveJson(file,state);
  const response=await api.invoke('continue_codex_task',{taskId,conversationId,afterRevision:approved.record.revision,resumeIntentId:intent});
  saveJson(path.join(out,`approval-${kind}.json`),{approval:state.approvals.at(-1),continuation:response});
  console.log(JSON.stringify({document:kind,accepted,status:response.status}));
 }else if(action==='export'){
  assert.ok(['review','delivery','closeout'].includes(r.currentStage));
  assert.equal(pack?.version,r.currentVersion);assert.equal(pack.status,'core-verified');
  const checks=new Map((r.selfTests??[]).filter(c=>c.version===r.currentVersion&&c.cycle===r.cycle).map(c=>[c.id,c]));
  for(const id of r.policy.selfTestChecks){
   const check=checks.get(id);
   assert.ok(check?.status==='passed'||(id==='authority'&&r.playerMode==='single'&&check?.status==='not-applicable'),`Required check remains open: ${id}`);
  }
  const source=path.resolve(state.task.workspacePath,pack.path);
  assert.ok(source.toLowerCase().startsWith(path.resolve(state.task.workspacePath).toLowerCase()+path.sep));
  assert.equal(hashFile(source),pack.sha256);
  fs.copyFileSync(source,path.join(out,'package.zip'));
  state.result='passed';state.completedAt=r.updatedAt;state.requirementsSha256=r.documents.requirements.sha256;state.planSha256=r.documents.plan.sha256;
  state.production={path:`runs/${name}/production.json`,sha256:hashFile(path.join(out,'production.json'))};
  state.workflow={path:`runs/${name}/workflow.json`,sha256:hashFile(path.join(out,'workflow.json'))};
  state.packagePath=`runs/${name}/package.zip`;state.taskId=taskId;
  state.nativeSessionId=control.nativeSessionId??state.nativeSessionId;
  state.completedAt=new Date().toISOString();saveJson(file,state);
  console.log(JSON.stringify({exported:name,wallMs:Date.parse(state.completedAt)-Date.parse(state.startedAt),firstPlayableMs:Date.parse(state.firstPlayableAt)-Date.parse(state.startedAt)}));
 }else throw Error('Use status, approve, reject or export');
}finally{await api.browser.close();}
