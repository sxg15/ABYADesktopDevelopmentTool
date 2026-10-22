// Owned native UI smoke; only fixture tasks and a local layout preference are changed.
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
 const build=await invoke('get_build_info');assert.equal(build.testEnvironment,true);assert.equal(build.verification,'verified');assert.equal(build.release,'workbench-resize-20261022');
 assert.equal((await invoke('list_tasks')).length,0);
 const task=await invoke('create_task',{input:{title:'面板高度调整隔离验收',description:'仅验证桌面布局，不启动 AI 或游戏。'}});
 await invoke('update_task_production',{input:{taskId:task.id,expectedRevision:0,operation:'initialize',data:{questionMode:'no-followup',playerMode:'single'}}});
 await page.reload();
 const split=page.getByRole('separator',{name:'调整成果区和回答区高度'});await split.waitFor();
 const before=await page.locator('.wb-interaction').boundingBox(),grip=await split.boundingBox();
 await page.mouse.move(grip.x+grip.width/2,grip.y+12);await page.mouse.down();await page.mouse.move(grip.x+grip.width/2,grip.y-80,{steps:12});await page.mouse.up();
 const after=await page.locator('.wb-interaction').boundingBox();assert.ok(after.height>before.height+30);
 const ratio=await split.getAttribute('aria-valuenow');
 await page.screenshot({path:path.join(root,'native-expanded.png'),fullPage:true});
 await page.reload();await split.waitFor();assert.equal(await split.getAttribute('aria-valuenow'),ratio);
 const saved=await invoke('get_task_production',{taskId:task.id});assert.equal(saved.record.revision,1);
 assert.equal(saved.record.workbench.requests.length,0);
 fs.writeFileSync(path.join(root,'split-result.json'),JSON.stringify({passed:true,release:build.release,ratio,before:before.height,after:after.height,productionRevision:1},null,2));
 console.log(JSON.stringify({passed:true,ratio,before:before.height,after:after.height}));
}finally{await browser.close();}
