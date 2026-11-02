import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {sourceFingerprint} from './source-fingerprint.mjs';

test('source fingerprint describes bytes rather than staged deletion status',()=>{
 const previous=process.cwd(),root=fs.mkdtempSync(path.join(os.tmpdir(),'abya-source-'));
 try{
  process.chdir(root);execFileSync('git',['init','-q']);
  fs.mkdirSync('src');fs.writeFileSync('src/kept.ts','kept');fs.writeFileSync('src/retired.ts','old transport');
  execFileSync('git',['add','src']);
  const original=sourceFingerprint();fs.unlinkSync('src/retired.ts');
  const before=sourceFingerprint();assert.notEqual(before,original);
  execFileSync('git',['add','-u']);assert.equal(sourceFingerprint(),before);
  fs.writeFileSync('src/kept.ts','changed');assert.notEqual(sourceFingerprint(),before);
 }finally{process.chdir(previous);assert.ok(root.startsWith(path.join(os.tmpdir(),'abya-source-')));fs.rmSync(root,{recursive:true,force:true});}
});
