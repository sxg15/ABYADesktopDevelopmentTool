import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
const [origin,out]=process.argv.slice(2);
if(new URL(origin).hostname!=='127.0.0.1')throw Error('Local fixture only');
fs.mkdirSync(out,{recursive:true});
const {chromium}=createRequire(import.meta.url)(process.env.ABYA_TEST_PLAYWRIGHT||'playwright');
const browser=await chromium.launch({channel:'msedge',headless:true});
const page=await browser.newPage({viewport:{width:1280,height:900}}),errors=[];
page.on('pageerror',error=>errors.push(error.message));
try{
 await page.goto(origin+'/scripts/art-recipes-preview.html');
 for(const title of ['按钮按压与恢复','选择高亮','面板进入与退出','资源到账轨迹','命中冲击','持续状态光环']){
  await page.getByRole('button',{name:new RegExp(title)}).click();
  const frame=page.frameLocator('iframe[title="表现配方交互预览"]');
  await frame.locator('#target').waitFor();
  await frame.getByRole('button',{name:'播放',exact:true}).click();
  await page.waitForFunction(()=>true);
  await frame.locator('#seek').evaluate(async element=>{await new Promise(resolve=>setTimeout(resolve,150));if(Number(element.value)<=0)throw Error('Preview did not animate');});
  await frame.getByRole('button',{name:'暂停',exact:true}).click();
  const progress=await frame.locator('#seek').inputValue();
  await frame.locator('#seek').evaluate(async(element,expected)=>{await new Promise(resolve=>setTimeout(resolve,100));if(element.value!==expected)throw Error('Pause did not hold');},progress);
  await page.screenshot({path:path.join(out,title+'.png'),fullPage:true});
  await frame.getByRole('button',{name:'还原',exact:true}).click();
  assert.equal(await frame.locator('#seek').inputValue(),'0');
  assert.equal(await frame.locator('#target').evaluate(element=>element.style.transform),'');
  assert.equal(await frame.locator('#target').evaluate(element=>element.style.opacity),'');
 }
 await page.setViewportSize({width:960,height:720});
 await page.locator('.art-case-detail').scrollIntoViewIfNeeded();
 assert.ok(await page.frameLocator('iframe[title="表现配方交互预览"]').getByRole('button',{name:'播放',exact:true}).isVisible());
 assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
 await page.screenshot({path:path.join(out,'narrow.png'),fullPage:true});
 assert.deepEqual(errors,[]);fs.writeFileSync(path.join(out,'report.json'),JSON.stringify({passed:true,recipes:6,scope:'Web UI and lifecycle; not native Player evidence'}));
}finally{await browser.close();}
