// Isolated native rendering check. Does not start an AI session or approve a document.
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
  const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});
  const paths=await invoke('get_app_paths');
  assert.equal(path.resolve(paths.dataDir).toLowerCase(),path.join(root,'Local','ABYA Desktop Development Tool','Data').toLowerCase());
  assert.equal((await invoke('get_build_info')).testEnvironment,true);
  assert.equal((await invoke('list_tasks')).length,0);
  const task=await invoke('create_task',{input:{title:'Markdown 原生隔离检查',description:'Synthetic UI validation only; no inference or gameplay acceptance.'}});
  let view=await invoke('update_task_production',{input:{taskId:task.id,expectedRevision:0,operation:'initialize',data:{questionMode:'no-followup'}}});
  const relative='artifacts/game-development/markdown-native.md';
  const content='# 需求确认单\n\n| 内容 | 结果 |\n| --- | --- |\n| 表格 | 正常渲染 |\n\n- [x] 准备文档\n- [ ] 人工确认\n\n> 仅测试渲染，不代表验收。\n\n```lua\nlocal score = 5\nprint(score)\n```\n\n<script>window.__mdExecuted = true</script>';
  fs.mkdirSync(path.dirname(path.join(task.workspacePath,relative)),{recursive:true});
  fs.writeFileSync(path.join(task.workspacePath,relative),content);
  view=await invoke('update_task_production',{input:{taskId:task.id,expectedRevision:view.record.revision,operation:'submit-document',data:{kind:'requirements',path:relative}}});
  const revision=view.record.revision;
  await page.reload();await page.locator('.production-workbench').waitFor();
  await page.locator('.wb-document table').waitFor();
  assert.equal(await page.locator('.wb-document pre code.language-lua').textContent(),'local score = 5\nprint(score)\n');
  assert.equal(await page.locator('.wb-document input[type=checkbox]:disabled').count(),2);
  assert.equal(await page.locator('.workbench-review input[type=radio]:checked').count(),0);
  assert.equal(await page.evaluate(()=>window.__mdExecuted),undefined);
  const after=await invoke('get_task_production',{taskId:task.id});
  assert.equal(after.record.revision,revision);assert.equal(after.record.approvals.length,0);
  assert.equal(after.record.documents.requirements.content,content);
  await page.screenshot({path:path.join(root,'markdown-native.png')});
  fs.writeFileSync(path.join(root,'result.json'),JSON.stringify({passed:true,revision,scope:'Native Markdown rendering and unchanged approval/source; no real AI inference'},null,2));
  console.log(JSON.stringify({passed:true,root}));
} finally {await browser.close();}
