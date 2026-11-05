import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {DatabaseSync} from 'node:sqlite';
import {connectPilot,saveJson} from './pilot-native-api.mjs';
const root=path.resolve(process.argv[2]);
const api=await connectPilot(root),out=path.join(root,'evidence');fs.mkdirSync(out,{recursive:true});
let task;
try {
 task=await api.invoke('create_task',{input:{title:'基础预检与自动清理回归',description:'隔离 QA：仅验证当前 APP 包的预检、问题卡片和清理；不制作或批准真实游戏。'}});
 let view=await api.invoke('update_task_production',{input:{taskId:task.id,expectedRevision:0,operation:'initialize',data:{questionMode:'no-followup',playerMode:'single'}}});
 const req='artifacts/preflight-qa-requirements.md';fs.mkdirSync(path.join(task.workspacePath,'artifacts'),{recursive:true});fs.writeFileSync(path.join(task.workspacePath,req),'# 隔离测试规格\n仅测试基础能力预检和清理，不修改用户存档。');
 view=await api.invoke('update_task_production',{input:{taskId:task.id,expectedRevision:view.record.revision,operation:'submit-document',data:{kind:'requirements',path:req}}});
 view=await api.invoke('decide_task_production',{input:{taskId:task.id,expectedRevision:view.record.revision,kind:'requirements',documentHash:view.record.documents.requirements.sha256,accepted:true,feedback:''}});
 // 仅在已验证为隔离 QA 的数据库准备方案阶段夹具；不宣称美术/完整工作流通过。
 const db=new DatabaseSync(path.join(api.paths.dataDir,'app.db'));
 function seed(edit){const r=JSON.parse(db.prepare('SELECT body FROM task_production WHERE task_id=?').get(task.id).body);const previous=r.revision;edit(r);r.revision++;assert.equal(db.prepare('UPDATE task_production SET revision=?,body=? WHERE task_id=? AND revision=?').run(r.revision,JSON.stringify(r),task.id,previous).changes,1);}
 seed(r=>{r.currentStage='plan';r.stages.resources='passed';r.stages.plan='in-progress';});
 const native=await api.invoke('run_task_preflight',{taskId:task.id,reuseExisting:false,retry:false});
 for(let i=0;i<60;i++){view=await api.invoke('get_task_production',{taskId:task.id});if(view.record.workbench.execution.capabilityPreflight.cleanup.state==='completed')break;await new Promise(r=>setTimeout(r,1000));}
 const p=view.record.workbench.execution.capabilityPreflight;
 assert.equal(p.cleanup.state,'completed');assert.equal(fs.existsSync(p.dataRoot),false);
 assert.ok(fs.existsSync(path.join(task.workspacePath,p.reportPath)));
 if(p.state!=='passed'){assert.ok(view.record.issues.some(i=>i.id==='preflight:basic'));}
 saveJson(path.join(out,'native-preflight.json'),{taskId:task.id,result:p,issues:view.record.issues,app:api.build});
 // 模拟本次测试进程退出后的崩溃残留，验证后台只清理，不重放创建。
 const recovery=structuredClone(p);recovery.runId=crypto.randomUUID();recovery.state='running';recovery.cleanup={state:'pending'};recovery.instances=[];recovery.error=null;
 recovery.dataRoot=path.join(path.dirname(task.workspacePath),'.abya-preflight',task.id,recovery.runId);
 recovery.reportPath=`artifacts/capability-preflight/${recovery.runId}.json`;
 fs.mkdirSync(recovery.dataRoot,{recursive:true});saveJson(path.join(recovery.dataRoot,'owner.json'),{taskId:task.id,runId:recovery.runId,purpose:'capability-preflight'});fs.writeFileSync(path.join(recovery.dataRoot,'partial.tmp'),'interrupted test fixture');
 seed(r=>{r.workbench.execution.capabilityPreflight=recovery;});
 for(let i=0;i<65;i++){view=await api.invoke('get_task_production',{taskId:task.id});if(view.record.workbench.execution.capabilityPreflight.cleanup.state==='completed')break;await new Promise(r=>setTimeout(r,1000));}
 const recovered=view.record.workbench.execution.capabilityPreflight;
 assert.equal(recovered.state,'interrupted');assert.equal(recovered.cleanup.state,'completed');assert.equal(fs.existsSync(recovery.dataRoot),false);
 saveJson(path.join(out,'cleanup-recovery.json'),{result:recovered,fixture:'simulated interrupted receipt; no fabricated native success'});
 db.close();
 console.log(JSON.stringify({taskId:task.id,nativeState:p.state,error:p.error,automaticCleanup:true,restartResidueCleanup:true,evidence:out}));
}finally{await api.browser.close();}
