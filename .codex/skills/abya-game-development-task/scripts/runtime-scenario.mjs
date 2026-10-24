// Declarative, task-owned runtime scenarios. Never retries a possibly executed write.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const hash=v=>crypto.createHash('sha256').update(JSON.stringify(v)).digest('hex');
const get=(v,keys)=>keys.reduce((x,k)=>x?.[k],v);
const atomic=(file,v)=>{const tmp=`${file}.${crypto.randomUUID()}.tmp`;fs.writeFileSync(tmp,JSON.stringify(v,null,2));fs.renameSync(tmp,file);};
export function validate(s) {
 if(s.schema!=='abya.runtime-scenario/v1'||!s.version||!s.buildHash||!s.archiveHash)throw Error('Version/build/archive bindings required');
 if(!Array.isArray(s.steps)||!s.steps.length||s.steps.length>256)throw Error('Expected 1–256 steps');
 const ids=new Set();
 for(const step of s.steps){
  if(!/^[a-z0-9_-]{1,80}$/i.test(step.id)||ids.has(step.id))throw Error('Unique step ID required');ids.add(step.id);
  if(!step.instanceId||!step.toolName||!s.capabilities?.includes(step.toolName))throw Error('Bind a discovered capability and owned instance');
  if(!step.arguments||typeof step.arguments!=='object'||Array.isArray(step.arguments))throw Error('Object arguments required');
  if(!Array.isArray(step.assertions)||step.assertions.some(a=>!Array.isArray(a.path)||a.path.some(k=>typeof k!=='string'&&typeof k!=='number')||!Object.hasOwn(a,'equals')))throw Error('Explicit assertions required');
  if(step.repeat!==undefined&&(!Number.isInteger(step.repeat)||step.repeat<1||step.repeat>20||step.readOnly!==true))throw Error('Only explicit read-only polling may repeat (1–20)');
 }
 return s;
}
function invoke(step,requestId,cli) {
 const r=spawnSync(cli,['runtime','run','--input-file','-','--request-id',requestId,'--json'],{
  input:JSON.stringify({instanceId:step.instanceId,toolName:step.toolName,arguments:step.arguments}),
  encoding:'utf8',windowsHide:true,timeout:Math.min(step.timeoutMs??60000,120000),maxBuffer:16*1024*1024});
 if(r.error||r.status!==0)throw Error('Runtime result uncertain or rejected; inspect the operation receipt before retrying');
 const value=JSON.parse(r.stdout);
 if(value.success!==true||value.isError||value.data?.success===false||value.data?.isError)throw Error('Runtime operation did not succeed');
 return value.data??value;
}
export async function runScenario(s,directory,call=invoke,cli=process.env.ABYA_DESKTOP_CLI) {
 validate(s);fs.mkdirSync(directory,{recursive:true});
 const lock=path.join(directory,'running.lock');const fd=fs.openSync(lock,'wx');
 try{
  const stateFile=path.join(directory,'receipt.json'),digest=hash(s);
  const state=fs.existsSync(stateFile)?JSON.parse(fs.readFileSync(stateFile,'utf8')):{schema:'abya.runtime-scenario-receipt/v1',inputHash:digest,version:s.version,buildHash:s.buildHash,archiveHash:s.archiveHash,steps:[]};
  if(state.inputHash!==digest)throw Error('Scenario changed: use a new run directory, retain previous evidence');
  for(const step of s.steps){
   const previous=state.steps.find(x=>x.id===step.id);
   if(previous){if(previous.status==='completed')continue;throw Error(`Step ${step.id} requires reconciliation; no automatic replay`);}
   const receipt={id:step.id,requestId:crypto.randomUUID(),status:'pending',startedAt:new Date().toISOString(),observations:[]};
   state.steps.push(receipt);atomic(stateFile,state);
   try{
    for(let i=0;i<(step.repeat??1);i++){
     // Each bounded read poll has a distinct identity. Writes are dispatched exactly once.
     const requestId=i?crypto.randomUUID():receipt.requestId;
     receipt.pollRequestId=requestId;atomic(stateFile,state);
     const value=await call(step,requestId,cli);
     receipt.resultHash=hash(value);
     receipt.observations=step.assertions.map(a=>({path:a.path,expected:a.equals,actual:get(value,a.path)??null,passed:JSON.stringify(get(value,a.path))===JSON.stringify(a.equals)}));
     if(receipt.observations.every(a=>a.passed))break;
     if(i+1<(step.repeat??1))await new Promise(r=>setTimeout(r,Math.max(100,Math.min(step.intervalMs??500,2000))));
    }
    receipt.status=receipt.observations.every(a=>a.passed)?'completed':'assertionFailed';
   }catch(e){receipt.status='needsReconciliation';throw e;}
   finally{receipt.completedAt=new Date().toISOString();atomic(stateFile,state);}
   if(receipt.status!=='completed')throw Error(`Assertion failed: ${step.id}`);
  }
  return {...state,complete:true,note:'Scenario assertions only; not human acceptance or automatic production approval'};
 }finally{fs.closeSync(fd);fs.unlinkSync(lock);}
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){
 try{
  const [file,out]=process.argv.slice(2);if(!file||!out||!process.env.ABYA_DESKTOP_CLI)throw Error('Usage: node runtime-scenario.mjs SCENARIO.json NEW_TASK_RUN_DIRECTORY');
  const workspace=process.env.ABYA_DEVELOPMENT_WORKSPACE;if(!workspace)throw Error('Run inside the task managed terminal');
  const relative=path.relative(fs.realpathSync(workspace),path.resolve(out));
  if(!relative||relative.startsWith('..')||path.isAbsolute(relative))throw Error('Run directory must be inside the current task');
  console.log(JSON.stringify(await runScenario(JSON.parse(fs.readFileSync(file,'utf8')),out)));
 }catch(e){console.error(e.message);process.exitCode=1;}
}
