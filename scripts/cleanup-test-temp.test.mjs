import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {randomUUID} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const script=fileURLToPath(new URL('./cleanup-test-temp.mjs',import.meta.url));
const logs=path.resolve(path.dirname(script),'../logs');
const create=()=>{const d=path.join(logs,'check-temp-'+randomUUID().replaceAll('-',''));fs.mkdirSync(d,{recursive:true});return d};
const clean=p=>spawnSync(process.execPath,[script,p],{encoding:'utf8'});
test('cleanup unlinks fixture junctions without touching targets and rejects a linked root',()=>{
 const own=create(),external=create(),linked=path.join(logs,'check-temp-'+randomUUID().replaceAll('-',''));
 try{
  fs.writeFileSync(path.join(external,'keep.txt'),'external data');
  fs.symlinkSync(external,path.join(own,'fixture-link'),'junction');
  assert.equal(clean(own).status,0);
  assert.equal(fs.existsSync(own),false);
  assert.equal(fs.readFileSync(path.join(external,'keep.txt'),'utf8'),'external data');
  fs.symlinkSync(external,linked,'junction');
  assert.notEqual(clean(linked).status,0);
  assert.equal(fs.readFileSync(path.join(external,'keep.txt'),'utf8'),'external data');
  assert.notEqual(clean(logs).status,0);
 }finally{
  if(fs.existsSync(linked))fs.unlinkSync(linked);
  for(const d of [own,external])assert.equal(clean(d).status,0);
 }
});
