// Real React surface, explicit synthetic data; not gameplay acceptance.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
const require=createRequire(import.meta.url);
const {chromium}=require(process.env.ABYA_TEST_PLAYWRIGHT||'playwright');
const output=path.resolve('logs/goal-workbench-ui');fs.mkdirSync(output,{recursive:true});
const browser=await chromium.launch({headless:true,executablePath:process.env.ABYA_TEST_BROWSER,args:['--no-proxy-server']});
const page=await browser.newPage({viewport:{width:1440,height:1000},locale:'zh-CN'});
const errors=[];page.on('pageerror',e=>errors.push(e.message));
const results=[];
const check=(name,value)=>{assert.ok(value,name);results.push(name);};
try{
  await page.goto('http://127.0.0.1:48732/scripts/workspace-refinement-preview.html',{waitUntil:'domcontentloaded'});
  await page.locator('.production-workbench').waitFor();
  check('Seven stages',await page.locator('.wb-stages button').count()===7);
  check('Four task tabs',await page.getByRole('tab').count()===4);
  check('Guide remains visible',await page.locator('.task-guidance').isVisible());
  check('No approval preselected',await page.locator('.workbench-review input:checked').count()===0);
  await page.getByRole('button',{name:'添加审查意见',exact:true}).click();
  await page.locator('.workbench-review textarea').fill('保留底部操作，调整成果区域');
  await page.getByRole('tab',{name:'Skill 库',exact:true}).click();
  await page.getByRole('tab',{name:/制作流程/}).click();
  check('Review draft survives tab switch',await page.locator('.workbench-review textarea').inputValue()==='保留底部操作，调整成果区域');
  await page.locator('.wb-goals button').filter({hasText:'确认视觉方向'}).click();
  await page.getByText('历史快照 · 只读',{exact:true}).waitFor();
  check('History cannot approve',await page.locator('.workbench-review').count()===0);
  await page.getByRole('button',{name:'回到当前工作',exact:true}).click();
  check('Review draft survives history',await page.locator('.workbench-review textarea').inputValue()==='保留底部操作，调整成果区域');
  await page.getByRole('button',{name:'切换任务',exact:true}).click();
  check('Task picker reachable',await page.locator('.task-list-pane').isVisible());
  await page.locator('.task-row').first().click();
  check('Task picker releases result space',!await page.locator('.task-list-pane').isVisible());
  await page.getByRole('button',{name:'反馈与复验',exact:true}).click();
  await page.getByRole('button',{name:'查看试玩版本并反馈',exact:true}).click();
  check('Feedback composes inline',await page.locator('.workbench-feedback-compose').isVisible() && await page.getByRole('dialog').count()===0);
  await page.locator('.workbench-feedback-compose textarea').fill('反馈草稿必须保留');
  await page.getByRole('button',{name:'返回输入',exact:true}).click();
  await page.getByRole('button',{name:'反馈与复验',exact:true}).click();
  await page.getByRole('button',{name:'查看试玩版本并反馈',exact:true}).click();
  check('Feedback draft survives interaction mode',await page.locator('.workbench-feedback-compose textarea').inputValue()==='反馈草稿必须保留');
  await page.getByRole('button',{name:'返回输入',exact:true}).click();
  for(const [width,height] of [[1440,1000],[1280,800],[960,720],[640,800]]){
    await page.setViewportSize({width,height});
    await page.screenshot({path:path.join(output,`workbench-${width}.png`)});
    check(`No horizontal page overflow ${width}`,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
    check(`Interaction reachable ${width}`,await page.locator('.wb-interaction').isVisible());
  }
  await page.setViewportSize({width:1440,height:1000});
  await page.getByRole('button',{name:'全部资料与开发详情',exact:true}).click();
  check('Original tools remain reachable',await page.getByRole('dialog').locator('.production-panel').isVisible());
  await page.keyboard.press('Escape');
  for(const name of ['Skill 库','游戏实例','终端']){
    await page.getByRole('tab',{name,exact:true}).click();
    check(`${name} selection`,await page.getByRole('tab',{name,exact:true}).getAttribute('aria-selected')==='true');
  }
  await page.getByRole('tab',{name:/制作流程/}).click();
  check('Charcoal shell',await page.locator('.app-sidebar').evaluate(e=>getComputedStyle(e).backgroundColor)==='rgb(31, 33, 32)');
  check('No page errors',errors.length===0);
  fs.writeFileSync(path.join(output,'results.json'),JSON.stringify({results,errors,scope:'Synthetic adapter data; real components; no native AI or Player'},null,2));
  console.log(JSON.stringify({passed:results.length,output}));
}finally{await browser.close();}
