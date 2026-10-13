import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import readline from 'node:readline';
import fs from 'node:fs';
import path from 'node:path';

// Opt-in, no inference: only a fresh native thread under the supplied test workspace.
const executable = process.env.ABYA_TEST_CODEX;
const cwd = process.env.ABYA_TEST_WORKSPACE;
if (!executable || !cwd || !fs.statSync(cwd).isDirectory()) throw new Error('Set ABYA_TEST_CODEX and ABYA_TEST_WORKSPACE');
function backend() {
  const child = spawn(executable, ['app-server'], { stdio:['pipe','pipe','pipe'], windowsHide:true });
  child.stderr.resume();
  let sequence = 0;
  const pending = new Map();
  readline.createInterface({input:child.stdout}).on('line', line => {
    let value; try { value=JSON.parse(line); } catch { return; }
    if (value.method || !pending.has(value.id)) return;
    const {resolve,reject,timer}=pending.get(value.id); pending.delete(value.id); clearTimeout(timer);
    if(value.error) reject(value.error); else resolve(value.result);
  });
  const call=(method,params={})=>new Promise((resolve,reject)=>{
    const id=++sequence;
    const timer=setTimeout(()=>{pending.delete(id);reject(new Error(`${method} timeout`));},15000);
    pending.set(id,{resolve,reject,timer});
    child.stdin.write(JSON.stringify({id,method,params})+'\n');
  });
  return {call,child,stop(){
    if(child.exitCode===null) spawnSync('taskkill',['/PID',String(child.pid),'/T','/F'],{windowsHide:true,stdio:'ignore'});
    for(const p of pending.values()){clearTimeout(p.timer);p.reject(new Error('test backend stopped'));} pending.clear();
  }};
}
const a=backend(), b=backend();
let threadId, archived=false;
try {
  for(const server of [a,b]) {
    await server.call('initialize',{clientInfo:{name:'ABYA writer recovery validation',version:'1'},capabilities:{experimentalApi:true}});
    server.child.stdin.write(JSON.stringify({method:'initialized',params:{}})+'\n');
  }
  threadId=(await a.call('thread/start',{cwd})).thread.id;
  await a.call('thread/inject_items',{threadId,items:[{type:'message',role:'user',content:[{type:'input_text',text:'Temporary writer-conflict recovery test. Do not run a model turn.'}]}]});
  const before=(await b.call('thread/read',{threadId,includeTurns:true})).thread;
  assert.equal(path.resolve(before.cwd).toLowerCase(),path.resolve(cwd).toLowerCase());
  let conflict=false;
  try { await b.call('thread/resume',{threadId,cwd,excludeTurns:true}); }
  catch(error) { assert.equal(error.code,-32600); assert.match(error.message,/active writer/i); conflict=true; }
  assert.ok(conflict,'A second writer unexpectedly acquired the owned thread');
  // Release only the first backend created above; never kill a user client.
  a.stop();
  const resumed=(await b.call('thread/resume',{threadId,cwd,excludeTurns:true})).thread;
  assert.equal(resumed.id,threadId);
  assert.equal(path.resolve(resumed.cwd).toLowerCase(),path.resolve(cwd).toLowerCase());
  const after=(await b.call('thread/read',{threadId,includeTurns:true})).thread;
  assert.deepEqual(after.turns,before.turns,'Reconnect must not start or replay a turn');
  await b.call('thread/archive',{threadId}); archived=true;
  console.log(JSON.stringify({writerConflict:true,originalIdentity:true,workspacePreserved:true,noModelTurn:true,testHistoryArchived:true}));
} finally {
  if(threadId&&!archived) {
    const owner=a.child.exitCode===null?a:b;
    await owner.call('thread/archive',{threadId}).catch(()=>undefined);
  }
  a.stop();b.stop();
}
