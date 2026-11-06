import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {sample,style} from '../.codex/skills/abya-game-development-task/assets/art-library/effects.mjs';
const catalog=JSON.parse(fs.readFileSync(new URL('../.codex/skills/abya-game-development-task/assets/art-library/catalog.json',import.meta.url)));
test('all shipped recipes have deterministic finite frames and reusable parameters',()=>{
 assert.equal(new Set(catalog.entries.map(x=>x.id)).size,catalog.entries.length);
 for(const {id} of catalog.entries)for(const progress of [0,0.25,0.5,0.75,1]){
  const frame=sample(id,progress);assert.deepEqual(frame,sample(id,progress));
  assert.ok(Object.values(frame).every(Number.isFinite));assert.ok(frame.opacity>=0&&frame.opacity<=1);
  assert.ok(style(frame,'#ffffff').transform.includes('translate'));
 }
 assert.equal(sample('press',1).scale,1);assert.equal(sample('reward',1).opacity,0);
 assert.equal(sample('panel',1).y,0);assert.equal(sample('press',0.5,{strength:0}).scale,1);
});
