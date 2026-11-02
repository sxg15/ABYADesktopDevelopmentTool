import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import test from 'node:test';
import assert from 'node:assert/strict';
import { verifyPackage } from './portable-package.mjs';
import {spawnSync} from 'node:child_process';

function fixture(t) {
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'abya-portable-'));
  t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
  const files=['ABYA Desktop Development Tool MCP.exe',
    'tools/ffmpeg/source.json','tools/ffmpeg/runtime/bin/ffmpeg.exe','tools/ffmpeg/runtime/bin/ffprobe.exe',
    'tools/ffmpeg/runtime/LICENSE.txt','.codex/skills/abya-game-development-task/SKILL.md'];
  for(const file of files){fs.mkdirSync(path.dirname(path.join(root,file)),{recursive:true});fs.writeFileSync(path.join(root,file),'fixture');}
  const sha=crypto.createHash('sha256').update('fixture').digest('hex');
  fs.writeFileSync(path.join(root,'build-manifest.json'),JSON.stringify({edition:'mcp',toolTransport:'mcp',releaseLabel:'fixture',executable:files[0],sha256:sha}));
  return root;
}
test('complete runtime verifies after relocation, without a system Node installation',async t=>{
  const original=fixture(t);await verifyPackage(original,{write:true});
  const moved=original+' 策划 (clone)';fs.renameSync(original,moved);t.after(()=>fs.rmSync(moved,{recursive:true,force:true}));
  assert.equal((await verifyPackage(moved)).files,7);
});
test('missing bundled runtime is rejected before launch',async t=>{
  const root=fixture(t);await verifyPackage(root,{write:true});fs.unlinkSync(path.join(root,'tools/ffmpeg/runtime/bin/ffmpeg.exe'));
  await assert.rejects(verifyPackage(root,{quick:true}),/Missing runtime/);
});

test('retired provider resources cannot enter a Codex-only package',async t=>{
  const root=fixture(t),file=path.join(root,'.grok/skills/legacy/SKILL.md');
  fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,'retired fixture');
  await assert.rejects(verifyPackage(root,{write:true}),/Unexpected package content/);
  assert.equal(fs.existsSync(path.join(root,'package-files.json')),false);
});
test('corrupt executable is rejected even by quick checks',async t=>{
  const root=fixture(t);await verifyPackage(root,{write:true});fs.writeFileSync(path.join(root,'ABYA Desktop Development Tool MCP.exe'),'changed');
  await assert.rejects(verifyPackage(root,{quick:true}),/hash mismatch/);
});
test('changed ancillary runtime content fails full byte verification',async t=>{
  const root=fixture(t);await verifyPackage(root,{write:true});fs.writeFileSync(path.join(root,'tools/ffmpeg/runtime/LICENSE.txt'),'changed');
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
  const profile=path.join(root,'ABYA Desktop Development Tool MCP.exe.WebView2');fs.mkdirSync(profile);fs.writeFileSync(path.join(profile,'auth.json'),'local only');
  await verifyPackage(root,{quick:true});
  await assert.rejects(verifyPackage(root,{write:true}),/Private data/);
});

test('native PowerShell launcher verifies complete inventory and refuses an extra CLI executable', {skip:process.platform!=='win32'}, async t=>{
  const packageRoot=fixture(t);await verifyPackage(packageRoot,{write:true});
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'abya-mcp-launch-'));
  t.after(()=>{assert.ok(root.startsWith(path.join(os.tmpdir(),'abya-mcp-launch-')));fs.rmSync(root,{recursive:true,force:true});});
  fs.mkdirSync(path.join(root,'scripts'));
  fs.copyFileSync(new URL('./launch-mcp.ps1',import.meta.url),path.join(root,'scripts','launch-mcp.ps1'));
  fs.cpSync(packageRoot,path.join(root,'Publish-MCP'),{recursive:true});
  const check=()=>spawnSync('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(root,'scripts','launch-mcp.ps1'),'-VerifyOnly'],{encoding:'utf8',windowsHide:true,timeout:20000});
  const valid=check();assert.equal(valid.status,0,valid.stderr);
  fs.writeFileSync(path.join(root,'Publish-MCP','abya-desktop.exe'),'unexpected');
  assert.notEqual(check().status,0);
});
