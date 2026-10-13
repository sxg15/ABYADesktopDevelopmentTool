import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import test from 'node:test';
import assert from 'node:assert/strict';
import { verifyPackage } from './portable-package.mjs';

function fixture(t) {
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'abya-portable-'));
  t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
  const files=['ABYA Desktop Development Tool.exe','abya-desktop.exe','runtime/node.exe','runtime/LICENSE.txt','tools/abya/bin/abya.mjs',
    'tools/ffmpeg/source.json','tools/ffmpeg/runtime/bin/ffmpeg.exe','tools/ffmpeg/runtime/bin/ffprobe.exe',
    'tools/ffmpeg/runtime/LICENSE.txt','.codex/skills/abya-game-development-task/SKILL.md','.grok/skills/abya-game-development-task/SKILL.md'];
  for(const file of files){fs.mkdirSync(path.dirname(path.join(root,file)),{recursive:true});fs.writeFileSync(path.join(root,file),'fixture');}
  const sha=crypto.createHash('sha256').update('fixture').digest('hex');
  fs.writeFileSync(path.join(root,'build-manifest.json'),JSON.stringify({releaseLabel:'fixture',executable:files[0],sha256:sha,cliExecutable:files[1],cliSha256:sha}));
  return root;
}
test('complete runtime verifies after relocation, without a system Node installation',async t=>{
  const original=fixture(t);await verifyPackage(original,{write:true});
  const moved=original+' 策划 (clone)';fs.renameSync(original,moved);t.after(()=>fs.rmSync(moved,{recursive:true,force:true}));
  assert.equal((await verifyPackage(moved)).files,12);
});
test('missing bundled runtime is rejected before launch',async t=>{
  const root=fixture(t);await verifyPackage(root,{write:true});fs.unlinkSync(path.join(root,'runtime/node.exe'));
  await assert.rejects(verifyPackage(root,{quick:true}),/Missing runtime/);
});
test('corrupt executable is rejected even by quick checks',async t=>{
  const root=fixture(t);await verifyPackage(root,{write:true});fs.writeFileSync(path.join(root,'abya-desktop.exe'),'changed');
  await assert.rejects(verifyPackage(root,{quick:true}),/hash mismatch/);
});
test('changed ancillary runtime content fails full byte verification',async t=>{
  const root=fixture(t);await verifyPackage(root,{write:true});fs.writeFileSync(path.join(root,'tools/abya/bin/abya.mjs'),'changed');
  await assert.rejects(verifyPackage(root),/modified package file/);
});
test('private data is never included when generating a release inventory',async t=>{
  const root=fixture(t);fs.writeFileSync(path.join(root,'.codex/auth.json'),'private fixture');
  await assert.rejects(verifyPackage(root,{write:true}),/Private data/);
  assert.equal(fs.existsSync(path.join(root,'package-files.json')),false);
});
test('inventory traversal and unlisted files fail closed',async t=>{
  const root=fixture(t);await verifyPackage(root,{write:true});
  const file=path.join(root,'package-files.json'),index=JSON.parse(fs.readFileSync(file));index.files[0].path='../outside.exe';fs.writeFileSync(file,JSON.stringify(index));
  await assert.rejects(verifyPackage(root),/inventory differs/);
});
test('local WebView state does not block launch and cannot enter a new distribution',async t=>{
  const root=fixture(t);await verifyPackage(root,{write:true});
  const profile=path.join(root,'ABYA Desktop Development Tool.exe.WebView2');fs.mkdirSync(profile);fs.writeFileSync(path.join(profile,'auth.json'),'local only');
  await verifyPackage(root,{quick:true});
  await assert.rejects(verifyPackage(root,{write:true}),/Private data/);
});
