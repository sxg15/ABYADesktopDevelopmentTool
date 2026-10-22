// Tests APP IPC on an owned, empty fixture; never starts Codex or sends terminal bytes.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
const root=path.resolve(process.argv[2]);
assert.equal(JSON.parse(fs.readFileSync(path.join(root,'art-design-native-fixture.json'),'utf8')).schema,'abya-art-design-native/v1');
const {chromium}=createRequire(import.meta.url)(process.env.ABYA_TEST_PLAYWRIGHT||'playwright');
const browser=await chromium.connectOverCDP('http://127.0.0.1:9248');
try {
  const page=browser.contexts()[0].pages()[0];await page.waitForFunction(()=>!!window.__TAURI_INTERNALS__?.invoke);
  const invoke=async(command,args={})=>{const r=await page.evaluate(async({command,args})=>{try{return {ok:true,value:await window.__TAURI_INTERNALS__.invoke(command,args)};}catch(e){return {ok:false,message:e?.message??String(e)};}},{command,args});if(!r.ok)throw new Error(r.message);return r.value;};
  const paths=await invoke('get_app_paths');
  assert.equal(path.resolve(paths.dataDir).toLowerCase(),path.join(root,'Local','ABYA Desktop Development Tool','Data').toLowerCase());
  assert.equal((await invoke('get_build_info')).testEnvironment,true);
  assert.equal((await invoke('list_tasks')).length,0);
  const task=await invoke('create_task',{input:{title:'终端恢复 IPC 隔离测试',description:'No real model, terminal or game process.'}});
  const conversation=await invoke('create_codex_conversation',{taskId:task.id,title:'显式命名的未启动测试会话'});
  assert.ok(conversation.nativeSessionId==null);
  const metadata=path.join(task.workspacePath,'conversations',conversation.id,'conversation.json');
  const before=fs.readFileSync(metadata,'utf8');
  await invoke('rebuild_codex_terminal',{taskId:task.id,conversationId:conversation.id});
  assert.equal(fs.readFileSync(metadata,'utf8'),before);
  await assert.rejects(invoke('rebuild_codex_terminal',{taskId:'foreign-task',conversationId:conversation.id}));
  await assert.rejects(invoke('write_codex_terminal',{conversationId:conversation.id,data:'not sent'}));
  await assert.rejects(invoke('write_codex_terminal',{conversationId:conversation.id,inputSessionId:'stale',data:'not sent'}));
  assert.equal(fs.readFileSync(metadata,'utf8'),before);
  fs.writeFileSync(path.join(root,'recovery-native-result.json'),JSON.stringify({passed:true,scope:'Tauri command identity, missing/stale input session rejection, preserved inactive metadata. Live writer and RPC stalls are covered by deterministic tests.'},null,2));
  console.log(JSON.stringify({passed:true,root}));
} finally {await browser.close();}
