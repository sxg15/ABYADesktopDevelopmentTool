import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import test from 'node:test';
import assert from 'node:assert/strict';
import {buildScene} from '../.codex/skills/abya-art-template-scrapbook-collage/scripts/specimen-model.mjs';
import {makeAssets} from '../.codex/skills/abya-art-template-scrapbook-collage/scripts/make-assets.mjs';
import {stateScript} from '../.codex/skills/abya-art-template-scrapbook-collage/scripts/specimen-lua.mjs';
const root=path.resolve('.codex/skills/abya-art-template-scrapbook-collage');
const read=p=>JSON.parse(fs.readFileSync(path.join(root,p),'utf8').replace(/^\uFEFF/,''));
const spec=read('assets/specimen.json'),tokens=read('assets/design-tokens.json');
const evidence=read('evidence/index.json');
const sha=p=>crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
test('both density modes preserve numeric targets, stable hit areas and unclipped text containers',()=>{
  for(const mode of ['3','4'])for(const state of spec.states){
    const scene=buildScene(spec,tokens,mode,state),ids=scene.map(n=>n.id);
    assert.equal(new Set(ids).size,ids.length);
    assert.equal(scene.filter(n=>n.kind==='Button').length,7);
    for(const n of scene){
      assert.ok(n.rect.x>=0&&n.rect.y>=0&&n.rect.width>0&&n.rect.height>0,n.id);
      assert.ok(n.rect.x+n.rect.width<=spec.canvas.width&&n.rect.y+n.rect.height<=spec.canvas.height,n.id);
      if(n.asset)assert.ok(fs.existsSync(path.join(root,'assets/png',n.asset+'.png')));
    }
    assert.equal(spec.selectedCells.reduce((sum,i)=>sum+spec.grids[mode].values[i],0),Number(spec.grids[mode].target));
    for(const b of scene.filter(n=>n.kind==='Button')){
      assert.deepEqual(b.rect,buildScene(spec,tokens,mode).find(n=>n.id===b.id).rect);
    }
  }
});
test('original SVG sources are deterministic and all raster sizes retain the source dimensions',()=>{
  const generated=makeAssets(tokens);
  assert.deepEqual(generated,makeAssets(tokens));
  for(const [name,svg] of Object.entries(generated)){
    assert.equal(fs.readFileSync(path.join(root,'assets/source',name),'utf8'),svg);
    const [,w,h]=svg.match(/width="(\d+)" height="(\d+)"/);
    const png=fs.readFileSync(path.join(root,'assets/png',name.replace('.svg','.png')));
    assert.equal(png.readUInt32BE(16),Number(w));assert.equal(png.readUInt32BE(20),Number(h));
  }
});
test('portable emitter reproduces all seven actually exercised native bindings byte-for-byte',()=>{
  const native=read('evidence/native-bindings.json');
  assert.equal(native.bindings.length,7);
  for(const b of native.bindings)assert.equal(stateScript(spec,tokens,native.nodeMap,native.adapter,b.command),b.script);
});
test('evidence cannot pass with missing captures, changed parameters or failed native events',()=>{
  assert.equal(evidence.humanApproved,false);assert.equal(evidence.multiplayerVerified,false);
  assert.equal(evidence.status,'candidate-with-differences');
  assert.equal(evidence.configuration.specimenSha256,sha(path.join(root,'assets/specimen.json')));
  assert.equal(evidence.configuration.tokensSha256,sha(path.join(root,'assets/design-tokens.json')));
  assert.equal(evidence.samples.length,20);
  assert.equal(new Set(evidence.samples.map(s=>`${s.width}/${s.mode}/${s.state}`)).size,20);
  let residuals=0;
  for(const s of evidence.samples){
    for(const [kind,relative] of Object.entries(s.images))assert.equal(sha(path.join(root,'evidence',relative)),s.hashes[kind]);
    const m=read('evidence/'+s.measurementPath);
    assert.equal(s.event.state,'Completed');assert.equal(s.event.eventName,'OnCustomUIButtonClick');
    assert.equal(s.event.scriptName,'scrapbook-'+s.state);
    assert.ok(m.geometry.every(g=>g.maxError<=spec.calibration.edgePx));
    assert.ok(m.gaps.every(g=>g.error<=spec.calibration.gapPx));
    assert.ok(m.pixelBorders.every(g=>g.within2Px));
    residuals+=m.glyphs.filter(g=>!g.withinInitial5Percent).length;
  }
  assert.ok(residuals>0,'Retain measured small-text differences; never invent blanket acceptance');
});
test('template identity and relative documentation stay portable',()=>{
  assert.equal(read('abya-art-template.json').id,'abya-art-template-scrapbook-collage');
  function walk(dir){
    for(const entry of fs.readdirSync(dir,{withFileTypes:true})){
      const file=path.join(dir,entry.name);if(entry.isDirectory()){walk(file);continue;}
      if(!file.endsWith('.md'))continue;
      for(const [,link] of fs.readFileSync(file,'utf8').matchAll(/\]\(([^)#]+)(?:#[^)]*)?\)/g)){
        if(/^[a-z]+:/i.test(link))continue;
        const target=path.resolve(path.dirname(file),link);
        assert.ok(target.startsWith(root+path.sep),file);assert.ok(fs.statSync(target).isFile(),link);
      }
    }
  }
  walk(root);
});
