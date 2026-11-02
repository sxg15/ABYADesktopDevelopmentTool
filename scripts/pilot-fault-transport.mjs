// Maintainer-only native Rust transport faults, never an ABYA business CLI.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {sourceFingerprint} from './source-fingerprint.mjs';
import {connectPilot,saveJson,readJson,hashFile} from './pilot-native-api.mjs';
const api=await connectPilot(process.argv[2]);
try {
 const manifest=readJson(path.join(path.dirname(api.build.executable),'build-manifest.json'));
 assert.equal(manifest.sourceFingerprint,sourceFingerprint(),'Fault tests must match candidate source');
 const root=path.join(api.root,'faults');fs.mkdirSync(root,{recursive:true});
 const output=path.join(root,'native-transport-tests.log');const fd=fs.openSync(output,'w');
 let result;
 try {result=spawnSync('cargo',['test','--manifest-path','src-tauri/Cargo.toml','mcp_transport_fault_','--','--nocapture'],{env:{...process.env,CARGO_TARGET_DIR:path.resolve('target-mcp')},windowsHide:true,stdio:['ignore',fd,fd],timeout:180000});}
 finally {fs.closeSync(fd);}
 assert.equal(result.status,0,'Native fault regression failed; inspect log');
 const log=fs.readFileSync(output,'utf8');assert.match(log,/2 passed; 0 failed/);
 for(const kind of ['disconnect','timeout']) {
  const file=path.join(root,`${kind}.json`);
  saveJson(file,{kind,passed:true,source:'native-rust-mcp-transport-fault-regression',at:new Date().toISOString(),appSha256:hashFile(api.build.executable),sourceFingerprint:manifest.sourceFingerprint,outcome:'outcome_unknown',blindReplay:false,gameplayEvidence:false,log:{path:'native-transport-tests.log',sha256:hashFile(output)}});
  console.log(JSON.stringify({kind,passed:true,evidence:{path:`faults/${kind}.json`,sha256:hashFile(file)}}));
 }
} finally {await api.browser.close();}
