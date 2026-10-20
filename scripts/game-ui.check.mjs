import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import crypto from 'node:crypto';
import {loadLibrary,queryUiReferences} from '../.codex/skills/abya-game-development-task/references/game-art-design/references/game-ui-design/scripts/query-ui-references.mjs';
const root=path.resolve('.codex/skills/abya-game-development-task/references/game-art-design/references/game-ui-design');
const script=path.join(root,'scripts/query-ui-references.mjs'),bank=loadLibrary();
const hud={uiKinds:['hud'],inputs:['pointer','keyboard'],density:'high',scope:'full',brandLocked:false};
const menu={uiKinds:['menu'],inputs:['gamepad'],density:'medium',scope:'full',brandLocked:false};
const ids=result=>result.candidates.map(c=>c.id);
test('missing context and malformed input never turn into a default style recommendation',()=>{
  const missing=queryUiReferences({query:'paper'},bank);
  assert.equal(missing.status,'requires-context');assert.equal(missing.missingContext.length,5);assert.deepEqual(missing.candidates,[]);
  for(const change of [{density:'ultra'},{inputs:[]},{uiKinds:['website']},{brandLocked:'false'},{templateExplicit:'yes'},{scope:'anything'},{scores:true},{preferredSourceUrls:['javascript:alert(1)']}]){
    const result=queryUiReferences({...hud,...change},bank);
    assert.equal(result.status,'invalid-request');assert.deepEqual(result.candidates,[]);
  }
});
test('dense combat HUD excludes whole paper styles even when a supplied source is preferred',()=>{
  const result=queryUiReferences({...hud,query:'paper',templateExplicit:true,preferredSourceUrls:['https://chrislemke.github.io/website_designs/designs/Paper_Cut.html']},bank);
  assert.equal(result.status,'ok');assert.ok(ids(result).includes('authored-hud-action-zones'));
  assert.ok(!ids(result).some(id=>['paper-cut','scrapbook-collage'].includes(id)));
  assert.ok(result.excluded.some(c=>c.id==='paper-cut'&&c.reasons.length));
});
test('material-only paper borrowing remains possible on a dense branded touch HUD',()=>{
  const result=queryUiReferences({...hud,inputs:['touch'],scope:'material-only',brandLocked:true},bank);
  assert.deepEqual(ids(result),['paper-cut','scrapbook-collage']);
  for(const c of result.candidates){
    assert.equal(c.category,'style');assert.equal(c.scopeConstraints.materialOnly,true);
    assert.equal(c.scopeConstraints.preserveBrand,true);assert.equal(c.scopeConstraints.preserveLayout,true);
    assert.deepEqual(c.allowedBorrow,bank.entries.find(e=>e.id===c.id).materialBorrow);
  }
});
test('partial focus and layout work never falls back to whole-screen borrowing',()=>{
  for(const brandLocked of [true,false]){
    const result=queryUiReferences({...menu,uiKinds:['inventory'],density:'high',scope:'partial',brandLocked,categories:['layout','state-feedback']},bank);
    assert.ok(result.candidates.length);
    for(const c of result.candidates){assert.equal(c.scopeConstraints.preserveLayout,true);assert.deepEqual(c.allowedBorrow,bank.entries.find(e=>e.id===c.id).partialBorrow);}
  }
  const modified=structuredClone(bank);delete modified.entries.find(e=>e.id==='authored-inventory-detail').partialBorrow;
  assert.ok(!ids(queryUiReferences({...menu,uiKinds:['inventory'],scope:'partial'},modified)).includes('authored-inventory-detail'));
});
test('input mismatches are excluded; supplied sources rank only within viable candidates',()=>{
  const modified=structuredClone(bank);modified.entries.find(e=>e.id==='authored-hud-action-zones').inputs=['pointer'];
  assert.ok(!ids(queryUiReferences({...hud,inputs:['touch']},modified)).includes('authored-hud-action-zones'));
  const source='https://chrislemke.github.io/website_designs/designs/Paper_Cut.html';
  assert.equal(ids(queryUiReferences({...menu,scope:'partial',categories:['style'],preferredSourceUrls:[source]},bank))[0],'paper-cut');
});
test('candidates remain unapproved and querying has no adoption or input mutation',()=>{
  const before=structuredClone(bank),request={...menu,templateExplicit:false},saved=structuredClone(request);
  const result=queryUiReferences(request,bank);
  assert.ok(result.candidates.length);
  for(const c of result.candidates){assert.equal(c.humanApproved,false);assert.equal(c.runtimeVerified,false);assert.ok(!Object.hasOwn(c,'adoption'));}
  assert.ok(!Object.hasOwn(result,'artTemplate'));assert.ok(!Object.hasOwn(result,'selectedArtTemplate'));
  assert.deepEqual(bank,before);assert.deepEqual(request,saved);assert.deepEqual(queryUiReferences(request,bank),result);
  for(const flag of ['humanApproved','runtimeVerified']){const invalid=structuredClone(bank);invalid.entries[0][flag]=true;assert.throws(()=>queryUiReferences(menu,invalid),/unapproved|unverified/i);}
});
test('CLI treats query as literal data and validates request files without creating output artifacts',t=>{
  const temp=fs.mkdtempSync(path.join(os.tmpdir(),'abya-game-ui-'));
  t.after(()=>{assert.equal(path.dirname(temp),path.resolve(os.tmpdir()));fs.rmSync(temp,{recursive:true});});
  const input=path.join(temp,'request.json');fs.writeFileSync(input,JSON.stringify({...menu,query:'.* $(do-not-execute)'}));
  const run=(args,stdin)=>spawnSync(process.execPath,[script,...args],{input:stdin,encoding:'utf8',windowsHide:true});
  const result=run(['--input',input,'--limit','2']);assert.equal(result.status,0,result.stderr);
  const value=JSON.parse(result.stdout);assert.equal(value.candidates.length,2);
  assert.ok(value.candidates.every(c=>!c.matchReasons.some(r=>r.includes('literal query'))));
  assert.deepEqual(fs.readdirSync(temp),['request.json']);
  for(const [args,stdin] of [[['--limit','0'],'{}'],[['--limit','x'],'{}'],[['--input','--limit'],'{}'],[[],'{not-json']]){
    const invalid=run(args,stdin);assert.equal(invalid.status,2);assert.equal(JSON.parse(invalid.stdout).status,'invalid-request');
  }
  const imported=spawnSync(process.execPath,['--input-type=module','-e',`await import(${JSON.stringify(new URL('../.codex/skills/abya-game-development-task/references/game-art-design/references/game-ui-design/scripts/query-ui-references.mjs',import.meta.url).href)});`],{encoding:'utf8',windowsHide:true});
  assert.equal(imported.status,0,imported.stderr);assert.equal(imported.stdout,'');
});
test('curated source identities and portable relative resources remain complete',()=>{
  assert.equal(bank.entries.length,7);assert.equal(new Set(bank.entries.map(e=>e.id)).size,7);
  assert.deepEqual([...new Set(bank.entries.map(e=>e.category))].sort(),['layout','state-feedback','style']);
  for(const e of bank.entries){
    assert.ok(e.source.reviewScope);assert.ok(e.source.reviewedAt);
    if(e.source.type==='authored-pattern'){
      assert.equal(e.source.url,null);
      const preview=path.resolve(root,e.source.previewPath);assert.ok(preview.startsWith(root+path.sep));
      const bytes=fs.readFileSync(preview);
      assert.equal(crypto.createHash('sha256').update(bytes).digest('hex'),e.source.previewSha256);
      assert.equal(bytes.readUInt32BE(16),960);assert.equal(bytes.readUInt32BE(20),600);
      assert.ok(e.source.reviewScope.includes('不是实际游戏截图'));
    }
    else assert.ok(new URL(e.source.url).protocol==='https:');
  }
  const walk=dir=>{for(const e of fs.readdirSync(dir,{withFileTypes:true})){
    const file=path.join(dir,e.name);if(e.isDirectory()){walk(file);continue;}
    if(!file.endsWith('.md'))continue;
    for(const [,link] of fs.readFileSync(file,'utf8').matchAll(/\]\(([^)#]+)(?:#[^)]*)?\)/g)){
      if(/^[a-z]+:/i.test(link))continue;const target=path.resolve(path.dirname(file),link);
      assert.ok(target.startsWith(root+path.sep),file);assert.ok(fs.statSync(target).isFile(),link);
    }
  }};walk(root);
});
