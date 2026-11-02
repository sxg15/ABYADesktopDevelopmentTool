import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {connectPilot,readJson,saveJson,hashFile} from './pilot-native-api.mjs';
const api=await connectPilot(process.argv[2]);
try{
 const runs=fs.readdirSync(path.join(api.root,'runs'));
 const checked=[];
 for(const name of runs){
  const file=path.join(api.root,'runs',name,'run.json'),state=readJson(file);
  const taskId=state.task.id,conversationId=state.conversation.id;
  const control=await api.invoke('get_codex_task_control',{taskId,conversationId});
  const workflow=await api.invoke('get_codex_workflow',{taskId,conversationId});
  assert.equal(control.state,'waitingForApproval','Preserve only an idle review checkpoint');
  assert.equal(workflow.turns.at(-1)?.status,'completed','Native turn must be finished');
  checked.push({name,file,state,workflow,production:await api.invoke('get_task_production',{taskId})});
 }
 for(const {name,file,state,workflow,production} of checked){
  saveJson(path.join(path.dirname(file),'production.json'),production);
  saveJson(path.join(path.dirname(file),'workflow.json'),workflow);
  state.result='exploratory-only';state.stoppedAt=new Date().toISOString();
  state.appSha256=hashFile(api.build.executable);
  state.note='Requirements/resources exercised on the first candidate; newer source and Skill corrections require fresh qualification runs. Lua and final gameplay were not tested in this run.';
  saveJson(file,state);console.log(JSON.stringify({name,retained:true,stage:production.record.currentStage,qualifiesRelease:false}));
 }
 saveJson(path.join(api.root,'exploratory-checkpoint.json'),{at:new Date().toISOString(),processId:readJson(path.join(api.root,'launch.json')).processId,allIdle:true,runs:checked.map(x=>x.name),appSha256:hashFile(api.build.executable)});
}finally{await api.browser.close();}
