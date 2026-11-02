import fs from 'node:fs';
import crypto from 'node:crypto';
import {execFileSync} from 'node:child_process';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
export function sourceFingerprint(){
 const roots=['src','src-tauri','.codex','tools/ffmpeg/source.json','package.json','package-lock.json','index.html',
  'vite.config.ts','tsconfig.json','tsconfig.node.json','scripts/publish.ps1','scripts/promote-workflow.ps1',
  'scripts/source-fingerprint.mjs','scripts/verify-workflow-pilot.mjs','scripts/publish-mcp.ps1','scripts/promote-mcp.ps1','scripts/launch-mcp.ps1','scripts/portable-package.mjs'];
 const files=[...new Set(execFileSync('git',['ls-files','-z','--cached','--others','--exclude-standard','--',...roots],{encoding:'utf8'}).split('\0').filter(Boolean))].sort();
 const hash=crypto.createHash('sha256');
 for(const file of files){
  // The on-disk source tree must hash identically before and after staging a deletion.
  if(!fs.existsSync(file))continue;
  hash.update(file+'\0');
  hash.update(crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex')+'\0');
 }
 return hash.digest('hex');
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url))console.log(sourceFingerprint());
