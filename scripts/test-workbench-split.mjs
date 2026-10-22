// Disposable real-component UI test. No native IPC or gameplay operations.
import fs from 'node:fs';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
const {chromium}=createRequire(import.meta.url)(process.env.ABYA_TEST_PLAYWRIGHT||'playwright');
const output='logs/workbench-split';fs.mkdirSync(output,{recursive:true});
const browser=await chromium.launch({headless:true,executablePath:process.env.ABYA_TEST_BROWSER,args:['--no-proxy-server']});
const page=await browser.newPage({viewport:{width:1440,height:900}}),errors=[];
page.on('pageerror',e=>errors.push(e.message));
const separator=page.getByRole('separator',{name:'调整成果区和回答区高度'});
const value=async()=>Number(await separator.getAttribute('aria-valuenow'));
const dimensions=()=>page.evaluate(()=>{const r=document.querySelector('.wb-split').getBoundingClientRect();return [...document.querySelector('.wb-split').children].map(e=>({top:e.getBoundingClientRect().top,bottom:e.getBoundingClientRect().bottom,height:e.getBoundingClientRect().height,containerBottom:r.bottom}));});
try {
 await page.goto('http://127.0.0.1:48736/scripts/workspace-refinement-preview.html?questions');
 await separator.waitFor();await page.getByText('是否采用首版补全方案：',{exact:false}).waitFor();
 await page.getByRole('radio',{name:'采用上述首版方案',exact:true}).check();
 const note=page.getByRole('textbox',{name:/补充说明$/});await note.fill('拖动前写好的回答必须保留');
 await page.screenshot({path:`${output}/before.png`,fullPage:true});
 const before=await dimensions(),handle=await separator.boundingBox();
 await page.mouse.move(handle.x+handle.width/2,handle.y+handle.height/2);await page.mouse.down();
 await page.mouse.move(handle.x+handle.width/2,handle.y-190,{steps:16});await page.mouse.up();
 const after=await dimensions();assert.ok(after[2].height>before[2].height+65,'Dragging upward must enlarge the answer pane');
 assert.equal(await note.inputValue(),'拖动前写好的回答必须保留');
 await page.getByRole('button',{name:'第 2 题',exact:true}).click();
 await page.getByRole('button',{name:/第 1 题/}).click();assert.equal(await note.inputValue(),'拖动前写好的回答必须保留');
 const persisted=await value();await page.getByRole('tab',{name:'Skill 库',exact:true}).click();
 await page.getByRole('tab',{name:/制作流程/}).click();assert.equal(await value(),persisted);
 await page.reload();await separator.waitFor();assert.equal(await value(),persisted);
 assert.equal(await note.inputValue(),'拖动前写好的回答必须保留');
 await page.screenshot({path:`${output}/expanded.png`,fullPage:true});
 await separator.focus();await page.keyboard.press('ArrowDown');assert.ok(await value()>persisted);
 await page.keyboard.press('Home');assert.equal(await value(),Number(await separator.getAttribute('aria-valuemin')));
 const original=await value(),box=await separator.boundingBox();
 await page.mouse.move(box.x+box.width/2,box.y+12);await page.mouse.down();await page.mouse.move(box.x+100,box.y+80);
 await page.keyboard.press('Escape');await page.mouse.up();assert.equal(await value(),original);
 await separator.dblclick();assert.equal(await value(),52);
 await page.setViewportSize({width:1024,height:650});await separator.focus();await page.keyboard.press('Home');
 await page.screenshot({path:`${output}/short-window.png`,fullPage:true});
 for(const d of await dimensions()){assert.ok(d.height>0);assert.ok(d.bottom<=d.containerBottom+1);}
 await page.setViewportSize({width:700,height:800});await separator.focus();await page.keyboard.press('End');
 for(const d of await dimensions()){assert.ok(d.height>0);assert.ok(d.bottom<=d.containerBottom+1);}
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
 assert.deepEqual(errors,[]);
 console.log(JSON.stringify({passed:true,checks:['pointer resizing','pointer capture and Escape','keyboard bounds and reset','question drafts and navigation','tab switching','reload persistence','short and narrow windows'],before:before[2].height,after:after[2].height}));
} finally {await browser.close();}
