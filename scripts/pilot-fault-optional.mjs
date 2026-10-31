// Inject an explicitly optional verification outage; never fabricate a game defect.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {connectPilot,readJson,saveJson,hashFile} from './pilot-native-api.mjs';
const [root,name,action='inject']=process.argv.slice(2);
assert.match(name,/^[a-z0-9-]+$/);
const api=await connectPilot(root);
try{
 const runFile=path.join(api.root,'runs',name,'run.json'),run=readJson(runFile),taskId=run.task.id;
 let view=await api.invoke('get_task_production',{taskId});
 const id='qualification-optional-monitor';
 const faults=path.join(api.root,'faults');fs.mkdirSync(faults,{recursive:true});
 const file=path.join(faults,'false-blocker.json');
 if(action==='inject'){
  assert.equal(view.record.currentStage,'implementation');assert.ok(view.record.currentVersion);
  const expected='Isolation test only: optional diagnostic monitor availability, unrelated to approved gameplay requirements';
  const contract=view.record.workbench.execution.acceptanceContract[id];
  if(contract){assert.equal(contract.expected,expected);assert.equal(contract.required,false);}
  else view=await api.invoke('update_task_production',{input:{taskId,expectedRevision:view.record.revision,operation:'register-acceptance-check',data:{id,expected,basis:'optional',required:false}}});
  view=await api.invoke('get_task_production',{taskId});
  const already=view.record.selfTests.some(c=>c.id===id&&c.version===view.record.currentVersion);
  if(!already)view=await api.invoke('update_task_production',{input:{taskId,expectedRevision:view.record.revision,operation:'save-self-test',data:{id,status:'blocked',outcome:'verification-unavailable',observations:'Controlled qualification fault injection: optional monitoring is intentionally unavailable. This does not demonstrate a game failure or PB defect and does not add an approved requirement.'}}});
  assert.equal(view.record.workbench.execution.acceptanceContract[id].required,false);
  assert.ok(view.record.workbench.execution.units.every(u=>!(u.validationFailures??[]).includes(id)));
  const prior=fs.existsSync(file)?readJson(file):null;
  if(prior)assert.equal(prior.taskId,taskId,'Do not replace another injection result');
  saveJson(file,{kind:'false-blocker',passed:false,source:'actual-candidate-optional-check-injection',appSha256:hashFile(api.build.executable),taskId,checkId:id,at:prior?.at??new Date().toISOString(),version:view.record.currentVersion,injected:true,mandatoryGateExpanded:false,awaitingNativeCompletion:true});
  run.faultInjections??=[];if(!run.faultInjections.includes(id))run.faultInjections.push(id);saveJson(runFile,run);
  console.log(JSON.stringify({injected:true,taskId,mandatoryGateExpanded:false,awaitingNativeCompletion:true}));
 }else if(action==='verify'){
  const result=readJson(file);assert.equal(result.taskId,taskId);
  assert.ok(['review','delivery','closeout'].includes(view.record.currentStage));
  assert.equal(view.record.workbench.execution.verificationPackage?.version,view.record.currentVersion);
  assert.equal(view.record.workbench.execution.verificationPackage?.status,'core-verified');
  assert.equal(view.record.workbench.execution.acceptanceContract[id].required,false);
  result.passed=true;result.awaitingNativeCompletion=false;result.completedAt=new Date().toISOString();
  result.finalVersion=view.record.currentVersion;result.nativeOutcome='Reached formal review without promoting the optional outage to a required gameplay check';
  saveJson(file,result);console.log(JSON.stringify({kind:result.kind,passed:true,evidence:{path:'faults/false-blocker.json',sha256:hashFile(file)}}));
 }else throw Error('Use inject or verify');
}finally{await api.browser.close();}
