// 读取用户选定 Player 的真实基础能力；结果只作为接入证据，不提升为完整游戏验收。
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawn} from 'node:child_process';
import {connectPilot,saveJson,hashFile} from './pilot-native-api.mjs';
const [root,player]=process.argv.slice(2),api=await connectPilot(root);
const cli=path.join(path.dirname(api.build.executable),'abya-desktop.exe');
const call=(task,conversation,group,action,input)=>new Promise((resolve,reject)=>{
 const child=spawn(cli,[group,action,'--input-file','-','--json'],{windowsHide:true,env:{...process.env,
  LOCALAPPDATA:path.join(api.root,'Local'),ABYA_TEST_MODE:'1',ABYA_DEVELOPMENT_TASK_ID:task,
  ABYA_DEVELOPMENT_CONVERSATION_ID:conversation,ABYA_DEVELOPMENT_PROVIDER:'codex'},stdio:['pipe','pipe','pipe']});
 let stdout='';child.stdout.on('data',data=>stdout+=data);child.stderr.resume();child.on('error',reject);
 child.on('close',()=>{try{resolve(JSON.parse(stdout));}catch{reject(Error('CLI returned no JSON receipt'));}});
 child.stdin.end(JSON.stringify(input));
});
try{
 await api.invoke('update_settings',{input:{gameExecutablePath:fs.realpathSync(player),workspaceRootPath:path.join(api.root,'Workspaces'),
  locale:'zh-CN',gameGatewayPort:43922,preferredAdapterId:'',lanBroadcastEnabled:false}});
 const task=await api.invoke('create_task',{input:{title:'当前Player基础能力核对',description:'隔离空白存档与能力读取，不制作或验收游戏。'}});
 const conversation=await api.invoke('create_codex_conversation',{taskId:task.id,title:'能力核对，无模型推理'});
 const creationId=crypto.randomUUID(),out=path.join(api.root,'player-probe');fs.mkdirSync(out,{recursive:true});
 saveJson(path.join(out,'request.json'),{taskId:task.id,conversationId:conversation.id,creationId,player:fs.realpathSync(player)});
 const created=await call(task.id,conversation.id,'archive','create',{creationId,name:'工作流能力验证',author:'ABYA QA'});
 saveJson(path.join(out,'creation.json'),created);
 const report={schema:'abya.workflow-player-probe/v1',appSha256:hashFile(api.build.executable),creationStatus:created.data?.status??'unavailable',
  error:created.error??created.data?.error??null,capabilities:[],nativeGameplayTested:false};
 const instanceId=created.data?.instanceId;
 if(instanceId&&created.data?.status==='ready'){
  try{
   for(const capabilityId of ['runtime_get_game_state','runtime_input_key_press','runtime_input_mouse_click','runtime_input_reset','runtime_test_begin','runtime_test_assert','runtime_test_capture','runtime_test_end','custom_ui_runtime_get_tree']){
    const result=await call(task.id,conversation.id,'runtime','describe',{instanceId,capabilityId});
    saveJson(path.join(out,capabilityId+'.json'),result);
    report.capabilities.push({id:capabilityId,available:result.success===true&&result.data?.available===true,error:result.error??null});
   }
  }finally{saveJson(path.join(out,'stop.json'),await call(task.id,conversation.id,'instance','stop',{instanceId}));}
 }
 saveJson(path.join(out,'report.json'),report);console.log(JSON.stringify(report));
}finally{await api.browser.close();}
