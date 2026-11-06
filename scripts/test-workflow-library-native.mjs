// 隔离候选的真实后端验证；不会启动模型或游戏，也不作为整款游戏通过证据。
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {connectPilot,saveJson,hashFile} from './pilot-native-api.mjs';
const [directory]=process.argv.slice(2),api=await connectPilot(directory);
try{
 const task=await api.invoke('create_task',{input:{title:'工作流2.12组件验证',description:'隔离测试：检查不限时策略、配方源码和任务部署；不代表游戏验收。'}});
 assert.ok(path.resolve(task.workspacePath).toLowerCase().startsWith(path.join(api.root,'Workspaces').toLowerCase()+path.sep));
 let view=await api.invoke('update_task_production',{input:{taskId:task.id,expectedRevision:0,operation:'initialize',data:{questionMode:'no-followup',playerMode:'single'}}});
 assert.equal(view.record.policy.executionTimePolicy,'unlimited');
 assert.equal(view.record.policy.runtimeTestPlanVersion,1);
 assert.ok(!Object.keys(view.record.policy.implementationExecution).some(key=>key.includes('Minutes')));
 const skill=path.join(task.workspacePath,'.codex/skills/abya-game-development-task');
 for(const name of ['scenario-actions.mjs','scenario-values.mjs','scenario-cleanup.mjs','scenario-preflight.mjs'])assert.ok(fs.statSync(path.join(skill,'scripts',name)).size>0);
 const cases=await api.invoke('list_art_cases',{query:'',history:false});
 assert.equal(cases.filter(entry=>entry.provenance==='builtin').length,6);
 const original=await api.invoke('read_art_case',{id:'builtin.reward',version:1});
 assert.equal(original.designApproved,false);assert.equal(original.runtimeVerified,false);
 const action=(operation,data)=>api.invoke('mutate_art_library',{input:{taskId:task.id,expectedRevision:view.record.revision,operation,data}});
 const exported=await action('export',{id:original.id,version:1});
 const imported=await action('import',{path:path.join(task.workspacePath,exported.path)});
 assert.equal(imported.provenance,'imported');assert.equal(imported.runtimeVerified,false);
 view=await api.invoke('update_task_production',{input:{taskId:task.id,expectedRevision:view.record.revision,operation:'inspect-art-case',data:{caseId:imported.id,version:1}}});
 const manifest=path.join(task.workspacePath,`artifacts/art-library/${imported.id}/1/resources.json`);
 const resources=JSON.parse(fs.readFileSync(manifest,'utf8'));
 for(const item of resources.files)assert.equal(hashFile(path.join(task.workspacePath,item.path)),item.sha256);
 assert.equal(view.record.artDesign.caseUses.length,0);assert.equal(view.record.approvals.length,0);
 const request=await action('request',{kind:'reference',caseId:'builtin.reward',version:1});
 assert.ok(request.prompt.includes('inspect-art-case'));
 const report={schema:'abya.workflow-components-smoke/v1',passed:true,appSha256:hashFile(api.build.executable),
  inventorySha256:hashFile(path.join(path.dirname(api.build.executable),'package-files.json')),
  checks:['unlimited-policy','planned-tests-contract','complete-task-script-deployment','six-builtin-recipes','source-package-roundtrip','materialized-source-hashes','no-fabricated-approval','reference-request'],
  nativeGameplayTested:false,scope:'Isolated real packaged APP service calls; no model or game acceptance'};
 saveJson(path.join(api.root,'components-report.json'),report);console.log(JSON.stringify(report));
}finally{await api.browser.close();}
