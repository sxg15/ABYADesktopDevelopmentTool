// Isolated native bridge smoke. Never opens model sessions or launches a game.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {spawnSync} from 'node:child_process';
import {DatabaseSync} from 'node:sqlite';
const [action,directory]=process.argv.slice(2);
if(!['init','run'].includes(action)||!directory)throw new Error('Usage: init|run <fresh owned fixture directory>');
const root=path.resolve(directory),marker=path.join(root,'art-design-native-fixture.json');
const data=path.join(root,'Local','ABYA Desktop Development Tool','Data');
if(action==='init'){
  if(fs.existsSync(root))throw new Error('Use a fresh fixture directory');
  fs.mkdirSync(data,{recursive:true});
  fs.writeFileSync(marker,JSON.stringify({schema:'abya-art-design-native/v1'}));
  const db=new DatabaseSync(path.join(data,'app.db'));
  db.exec('CREATE TABLE settings(key TEXT PRIMARY KEY,value_json TEXT NOT NULL,updated_at TEXT NOT NULL)');
  const insert=db.prepare('INSERT INTO settings VALUES(?,?,?)');
  insert.run('application',JSON.stringify({locale:'zh-CN',gameExecutablePath:'',workspaceRootPath:path.join(root,'Workspaces'),desktopCliPort:43921,gameGatewayPort:43922,lanBroadcastEnabled:false}),new Date().toISOString());
  insert.run('taskNotifications',JSON.stringify({enabled:false}),new Date().toISOString());db.close();
  console.log(JSON.stringify({root,localAppData:path.join(root,'Local'),webView:path.join(root,'WebView')}));
}else{
  if(!fs.existsSync(marker)||JSON.parse(fs.readFileSync(marker,'utf8')).schema!=='abya-art-design-native/v1')throw new Error('Not an owned fixture');
  const require=createRequire(import.meta.url);
  const {chromium}=require(process.env.ABYA_TEST_PLAYWRIGHT||'playwright');
  const browser=await chromium.connectOverCDP('http://127.0.0.1:9246');
  const page=browser.contexts()[0].pages().find(p=>p.url().includes('tauri.localhost'))??browser.contexts()[0].pages()[0];
  assert.ok(page,'Native WebView page required');
  await page.waitForFunction(()=>!!window.__TAURI_INTERNALS__?.invoke);
  const invoke=async(command,args={})=>{const result=await page.evaluate(async({command,args})=>{
    try{return {ok:true,value:await window.__TAURI_INTERNALS__.invoke(command,args)};}
    catch(e){return {ok:false,error:{code:e?.code,message:e?.message??String(e),detail:e?.detail}};}
  },{command,args});if(!result.ok)throw new Error(`${command}: ${JSON.stringify(result.error)}`);return result.value;};
  const paths=await invoke('get_app_paths');
  assert.equal(path.resolve(paths.dataDir).toLowerCase(),data.toLowerCase(),'Refuse writes outside isolated native data');
  const build=await invoke('get_build_info');assert.equal(build.verification,'verified');assert.equal(build.testEnvironment,true);
  assert.equal((await invoke('list_tasks')).length,0,'Fresh native fixture required');
  const task=await invoke('create_task',{input:{title:'美术流程 2.2 原生隔离验收',description:'不要用数字显示库存，直接显示剩余物品。'}});
  assert.ok(path.resolve(task.workspacePath).toLowerCase().startsWith(path.join(root,'Workspaces').toLowerCase()+path.sep));
  let view;
  const update=async(operation,values)=>view=await invoke('update_task_production',{input:{taskId:task.id,expectedRevision:view?.record?.revision??0,operation,data:values}});
  await update('initialize',{questionMode:'no-followup'});assert.equal(view.record.workflowVersion,'2.2.3');
  const catalog=await invoke('get_task_skills',{taskId:task.id});
  assert.equal(catalog.filter(skill=>skill.kind==='design').length,3);
  assert.ok(catalog.every(skill=>skill.provider==='codex'));
  assert.equal(fs.existsSync(path.join(task.workspacePath,'.grok')),false);
  await assert.rejects(invoke('get_grok_terminal_availability'),/not found|unknown|未找到/i);
  const interfaceSkill=catalog.find(skill=>skill.provider==='codex'&&skill.name==='product-interface-design');
  const calibrationRoot=path.join(task.workspacePath,'.codex/skills',interfaceSkill.id);
  for(const relative of ['references/reference-to-design.md','references/implementation-calibration.md','references/calibration-tools.md','assets/calibration.template.json','scripts/compare-calibration.mjs']) {
    assert.ok(fs.statSync(path.join(calibrationRoot,relative)).size>0,'Calibration resources must reach the task workspace');
  }
  const collage=catalog.find(skill=>skill.id==='abya-art-template-scrapbook-collage');
  assert.ok(collage,'Optional collage template must appear in the existing catalogue');
  assert.ok(!view.record.artTemplate,'Deploying a candidate must not select it');
  for(const relative of ['DESIGN.md','assets/specimen.json','assets/design-tokens.json','assets/png/paper.png','references/verification.md']){
    assert.ok(fs.statSync(path.join(task.workspacePath,'.codex/skills',collage.id,relative)).size>0);
  }
  assert.ok((await invoke('read_task_skill',{taskId:task.id,provider:'codex',skillId:interfaceSkill.id})).includes('从产品推导界面'));
  const source=view.record.artDesign.sources[0];
  const gameUi=catalog.find(skill=>skill.provider==='codex'&&skill.name==='game-ui-design');
  assert.ok(gameUi,'Game UI method is visible without selecting a template');
  const uiRoot=path.join(task.workspacePath,'.codex/skills',gameUi.id);
  for(const relative of ['SKILL.md','references/ui-design-handoff.md','references/ui-reference-library.md','assets/ui-design-section.template.md','assets/ui-reference-library.json','scripts/query-ui-references.mjs']){
    assert.ok(fs.statSync(path.join(uiRoot,relative)).size>0,'Complete game UI resources must deploy');
  }
  assert.ok((await invoke('read_task_skill',{taskId:task.id,provider:'codex',skillId:gameUi.id})).includes('query-ui-references.mjs'));
  const beforeQuery=JSON.stringify(view.record);
  const lookup=spawnSync(process.execPath,[path.join(uiRoot,'scripts/query-ui-references.mjs')],{
    input:JSON.stringify({uiKinds:['hud'],inputs:['pointer','keyboard'],density:'high',scope:'full',brandLocked:false}),encoding:'utf8',windowsHide:true});
  assert.equal(lookup.status,0,lookup.stderr);
  const lookupResult=JSON.parse(lookup.stdout);
  assert.ok(lookupResult.candidates.some(c=>c.id==='authored-hud-action-zones'));
  assert.ok(lookupResult.candidates.every(c=>!['paper-cut','scrapbook-collage'].includes(c.id)));
  assert.ok(lookupResult.candidates.every(c=>c.humanApproved===false&&c.runtimeVerified===false));
  for(const c of lookupResult.candidates.filter(c=>c.source.previewPath)){
    const preview=path.resolve(uiRoot,c.source.previewPath);
    assert.ok(preview.startsWith(uiRoot+path.sep));assert.ok(fs.statSync(preview).size>0);
  }
  assert.equal(JSON.stringify((await invoke('get_task_production',{taskId:task.id})).record),beforeQuery,'Reference lookup must not adopt or approve');
  const write=(file,text)=>{fs.mkdirSync(path.dirname(path.join(task.workspacePath,file)),{recursive:true});fs.writeFileSync(path.join(task.workspacePath,file),text);};
  for(const provider of ['codex']){
    const design=catalog.find(skill=>skill.provider===provider&&skill.name==='product-interface-design');
    assert.ok((await invoke('read_task_skill',{taskId:task.id,provider,skillId:design.id})).includes('visual-refinement'));
    const helper=path.join(task.workspacePath,`.${provider}/skills/${design.id}/scripts/visual-tools.mjs`);
    assert.ok(fs.existsSync(helper),'Portable visual helper must be deployed with its references');
    write('artifacts/contrast-fixture.json',JSON.stringify({pairs:[{fg:'#888',bg:'#fff',fontSizePx:18}]}));
    const check=spawnSync(process.execPath,[helper,'contrast',path.join(task.workspacePath,'artifacts/contrast-fixture.json')],{encoding:'utf8',windowsHide:true});
    assert.equal(check.status,1,check.stderr);assert.equal(JSON.parse(check.stdout).results[0].threshold,4.5);
  }
  write('artifacts/requirements.md','# 隔离测试需求\n以物品显示库存。');
  await update('submit-document',{kind:'requirements',path:'artifacts/requirements.md'});
  view=await invoke('decide_task_production',{input:{taskId:task.id,expectedRevision:view.record.revision,kind:'requirements',documentHash:view.record.documents.requirements.sha256,accepted:true,feedback:'Synthetic fixture only'}});
  const requirement={id:'ART-1',sourceId:source.id,sourceHash:source.hash,quote:source.text,kind:'must',interpretation:'逐件显示剩余物品',scope:'HUD',acceptance:'观察数量与实际状态'};
  await update('save-art-requirement',requirement);
  let downgradeRejected=false;try{await update('save-art-requirement',{...requirement,kind:'preference'});}catch{downgradeRejected=true;}assert.ok(downgradeRejected);
  await update('review-art-source',{sourceId:source.id,sourceHash:source.hash,requirementIds:['ART-1'],reason:'已拆解完整要求'});
  await update('save-art-direction',{playerExperience:'快速读懂库存',tasksAndInformation:'瞄准、发射与剩余次数',platformAndCamera:'桌面固定镜头',visualRules:'轮廓清楚',representativeScene:'进行中和最后一枚',assetProduction:'独立物品图层',feasibility:'隔离数据，不代表游戏实机',acceptance:'核对原话与图像',librarySearch:'隔离库为空'});
  const image=await page.evaluate(()=>{const c=document.createElement('canvas');c.width=640;c.height=360;const x=c.getContext('2d');x.fillStyle='#f0eee5';x.fillRect(0,0,640,360);x.fillStyle='#273d35';x.font='24px sans-serif';x.fillText('原生隔离测试 · 非游戏成果',30,50);for(let i=0;i<5;i++)x.fillRect(180+i*50,180,12,70);return c.toDataURL('image/png').split(',')[1];});
  write('artifacts/layout.png',Buffer.from(image,'base64'));
  await update('register-visual',{stage:'resources',path:'artifacts/layout.png',title:'隔离库存示意',sourceType:'mockup',roles:['asset-board','layout','states'],groupId:'layout',visualVersion:'1'});
  const artifactId=view.record.artifacts[0].id;
  write('artifacts/art.md','# 隔离测试美术\nnew art：物品库存');
  let omissionRejected=false;try{await update('submit-document',{kind:'resources',path:'artifacts/art.md',artScope:'new-art',artScopeReason:'new art'});}catch{omissionRejected=true;}assert.ok(omissionRejected);
  await update('save-art-coverage',{requirementId:'ART-1',artifactIds:[artifactId],designLocation:'库存物品区',status:'addressed'});
  await update('submit-document',{kind:'resources',path:'artifacts/art.md',artScope:'new-art',artScopeReason:'new art'});
  await update('save-art-case-draft',{id:'draft',title:'原生隔离案例',artifactIds:[artifactId],tags:['桌面','库存'],context:'固定镜头',plannerRequirements:'用物品表达库存',visualRules:'清楚轮廓',decisions:'靠近操作区',production:'独立图层',applicability:'少量库存',limitations:'仅为隔离测试',sources:'程序绘制测试示意'});
  const caseEntry=await invoke('mutate_art_library',{input:{taskId:task.id,expectedRevision:view.record.revision,operation:'collect',data:{draftId:'draft',satisfaction:'隔离测试模拟用户认可，仅验证收录入口'}}});
  assert.equal(caseEntry.runtimeVerified,false);assert.equal(caseEntry.designApproved,true);
  const requestHash=view.artDesignStatus.designHash,requestApprovals=view.record.approvals.length;
  const caseRequest=await invoke('mutate_art_library',{input:{taskId:task.id,expectedRevision:view.record.revision,operation:'request',data:{kind:'reference',caseId:caseEntry.id,version:1}}});
  view=await invoke('get_task_production',{taskId:task.id});
  assert.equal(caseRequest.revision,view.record.revision);assert.equal(view.record.artDesign.caseRequests.length,1);
  assert.equal(view.artDesignStatus.designHash,requestHash);assert.equal(view.record.approvals.length,requestApprovals);assert.equal(view.record.artDesign.caseUses.length,0);
  const media=await invoke('read_art_case_media',{id:caseEntry.id,version:1,index:0});assert.ok(media.startsWith('data:image/png;base64,'));
  const before=view.record.approvals.length;view=await invoke('get_task_production',{taskId:task.id});assert.equal(view.record.approvals.length,before);
  const exported=await invoke('mutate_art_library',{input:{taskId:task.id,expectedRevision:view.record.revision,operation:'export',data:{id:caseEntry.id,version:1}}});
  assert.ok(fs.statSync(path.join(task.workspacePath,exported.path)).size>0);
  const designBeforeInspection=view.artDesignStatus.designHash;
  await update('inspect-art-case',{caseId:caseEntry.id,version:1});assert.equal(view.record.artDesign.caseUses.length,0);assert.equal(view.artDesignStatus.designHash,designBeforeInspection);
  await update('use-art-case',{caseId:caseEntry.id,version:1,borrow:'物品表达',adapt:'按本任务镜头重新布局',avoid:'直接复制原布局'});
  assert.equal(view.record.artDesign.caseUses.length,1);
  view=await invoke('submit_production_feedback',{input:{taskId:task.id,expectedRevision:view.record.revision,operation:'add-art-note',data:{text:'最后一个物品也要显示。'}}});
  assert.equal(view.record.stages.resources,'in-progress');
  const report={nativeBridge:true,manifestVerified:true,codexOnlyCatalog:true,noRetiredProviderDeployment:true,retiredProviderCommandRejected:true,workflowVersion:'2.2.3',optionalCollageNotSelected:true,gameUiResources:true,offlineCandidateQueryWithoutAdoption:true,nestedDesignSkills:true,requestPersistedWithoutAdoption:true,downgradeRejected,omissionRejected,explicitCollection:true,conceptNotRuntimeVerified:true,exported:true,frozenCaseReference:true,newInputInvalidated:true,taskId:task.id};
  fs.writeFileSync(path.join(root,'result.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));
  await page.evaluate(taskId=>localStorage.setItem('abya.taskTerminalProviders',JSON.stringify({[taskId]:'grok'})),task.id);
  await page.reload();await page.waitForLoadState('domcontentloaded');
  assert.equal(await page.getByRole('button',{name:'Grok',exact:true}).count(),0);
  await page.getByText('制作流程',{exact:true}).click();
  await page.getByRole('tab',{name:/要求与参考/}).click();
  await page.getByText('策划要求与落实位置',{exact:true}).waitFor();
  await page.screenshot({path:path.join(root,'native-requirements.png')});
  await page.getByRole('button',{name:'美术设计库',exact:true}).click();
  await page.getByRole('button',{name:/原生隔离案例/}).click();
  await page.getByRole('dialog').locator('img').first().waitFor({state:'visible'});
  await page.screenshot({path:path.join(root,'native-library.png')});
  await page.getByRole('dialog').locator('.modal-header .icon-button').click();
  await page.getByRole('tab',{name:'Skill 库',exact:true}).click();
  await page.getByRole('heading',{name:'游戏美术设计',exact:true}).waitFor();
  await page.getByRole('heading',{name:'产品界面设计',exact:true}).waitFor();
  await page.getByRole('heading',{name:'游戏 UI 设计',exact:true}).waitFor();
  await page.screenshot({path:path.join(root,'native-skills.png')});
  const uiCard=page.locator('article.skill-card').filter({has:page.getByRole('heading',{name:'游戏 UI 设计',exact:true})});
  await uiCard.getByRole('button',{name:'查看正文',exact:true}).click();
  await page.locator('.skill-reading').getByRole('heading',{name:'游戏 UI 设计',exact:true}).waitFor();
  await page.locator('.skill-reading .skill-design-note').filter({hasText:'本地 UI 候选参考索引'}).waitFor();
  await page.screenshot({path:path.join(root,'native-game-ui-design.png')});
  await page.locator('.skill-reading button').first().click();
  await page.getByRole('tab',{name:'美术设计库',exact:true}).click();
  await page.getByRole('button',{name:/原生隔离案例/}).click();
  await page.getByRole('button',{name:'让 AI 评估并参考',exact:true}).click();
  await page.getByText(/请求已保存，继续处理需要注意/).waitFor();
  const saved=await invoke('get_task_production',{taskId:task.id});assert.ok(saved.record.artDesign.caseRequests.length>=2);
  await page.screenshot({path:path.join(root,'native-library-embedded.png')});
  await page.getByRole('tab',{name:'终端',exact:true}).click();
  await page.getByText('Codex 对话',{exact:true}).waitFor();
  assert.equal(await page.getByRole('button',{name:'Grok',exact:true}).count(),0);
  await page.screenshot({path:path.join(root,'native-codex-only-terminal.png')});
  await browser.close();
}
