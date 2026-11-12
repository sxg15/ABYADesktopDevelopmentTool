// Declarative, task-owned runtime scenarios. Never retries a possibly executed write.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {decodeResult,classifyFailure,ScenarioCallError} from './scenario-errors.mjs';
import {expand} from './scenario-actions.mjs';
import {evaluate,resolve,validateAssertion} from './scenario-values.mjs';
import {preflight} from './scenario-preflight.mjs';
import {cleanupInputs} from './scenario-cleanup.mjs';
const hash=v=>crypto.createHash('sha256').update(JSON.stringify(v)).digest('hex');
const atomic=(file,v)=>{const tmp=`${file}.${crypto.randomUUID()}.tmp`;fs.writeFileSync(tmp,JSON.stringify(v,null,2));fs.renameSync(tmp,file);};
const readCapabilities=new Set();
export function validate(s) {
 s=expand(s);
 if(!['abya.runtime-scenario/v1','abya.runtime-scenario/v2','abya.runtime-scenario/v3'].includes(s.schema)||!s.version||!s.buildHash||!s.archiveHash)throw Error('Version/build/archive bindings required');
 if(!Array.isArray(s.steps)||!s.steps.length||s.steps.length>256)throw Error('Expected 1–256 steps');
 if(s.schema!=='abya.runtime-scenario/v1'&&!s.steps.some(step=>step.assertions?.length))throw Error('At least one observable assertion is required');
 if(s.schema==='abya.runtime-scenario/v3')for(const role of Object.keys(s.roles)){
  if(!s.steps.some(step=>step.role===role&&step.readOnly===true&&step.assertions?.length))throw Error(`Role ${role} needs an observation assertion`);
 }
 const ids=new Set();
 for(const step of s.steps){
  const references=value=>{
   if(!value||typeof value!=='object')return;
   if(Object.hasOwn(value,'$ref')&&(!Array.isArray(value.$ref)||!ids.has(value.$ref[0])))throw Error('References must target an earlier step');
   Object.values(value).forEach(references);
  };
  references(step.arguments);references(step.assertions);
  if(!/^[a-z0-9_-]{1,80}$/i.test(step.id)||ids.has(step.id))throw Error('Unique step ID required');ids.add(step.id);
  if(!step.instanceId||!step.toolName||!s.capabilities?.includes(step.toolName))throw Error('Bind a discovered capability and owned instance');
  if(s.schema==='abya.runtime-scenario/v3'&&step.toolName.startsWith('runtime_input_')&&!s.capabilities.includes('runtime_input_reset'))throw Error('Input scenarios require scope cleanup capability');
  if(!step.arguments||typeof step.arguments!=='object'||Array.isArray(step.arguments))throw Error('Object arguments required');
  if(!Array.isArray(step.assertions))throw Error('Explicit assertions required');
  step.assertions.forEach(validateAssertion);
  if(step.timeoutMs!==undefined&&(!Number.isInteger(step.timeoutMs)||step.timeoutMs<0))throw Error('timeoutMs must be zero (unlimited) or positive');
  if(step.repeat!==undefined&&(!Number.isInteger(step.repeat)||step.repeat<1||step.repeat>20||step.readOnly!==true))throw Error('Only explicit read-only polling may repeat (1–20)');
  if(step.readRetries!==undefined&&(!Number.isInteger(step.readRetries)||step.readRetries<0||step.readRetries>2||step.readOnly!==true))throw Error('Only reads may retry transport errors (0–2)');
 }
 return s;
}
function invoke(step,requestId,cli) {
 if(step.readOnly===true&&!readCapabilities.has(`${step.instanceId}:${step.toolName}`)){
  const probe=spawnSync(cli,['runtime','describe','--input-file','-','--json'],{
   input:JSON.stringify({instanceId:step.instanceId,capabilityId:step.toolName}),encoding:'utf8',windowsHide:true,timeout:30000,maxBuffer:2*1024*1024});
  const definition=decodeResult(probe);
  if(definition.riskLevel!=='ReadOnly')throw new ScenarioCallError('Discovered capability is not read-only','invocation-rejected','not-executed');
  readCapabilities.add(`${step.instanceId}:${step.toolName}`);
 }
 const r=spawnSync(cli,['runtime','run','--input-file','-','--request-id',requestId,'--json'],{
  input:JSON.stringify({instanceId:step.instanceId,toolName:step.toolName,arguments:step.arguments}),
  encoding:'utf8',windowsHide:true,timeout:step.timeoutMs??0,maxBuffer:16*1024*1024});
 return decodeResult(r,requestId);
}
export async function runScenario(s,directory,call=invoke,cli=process.env.ABYA_DESKTOP_CLI) {
 s=validate(s);fs.mkdirSync(directory,{recursive:true});
 const lock=path.join(directory,'running.lock');const fd=fs.openSync(lock,'wx');
 let state,started=false;
 try{
  if(s.schema==='abya.runtime-scenario/v3'&&call===invoke)atomic(path.join(directory,'preflight.json'),preflight(s,cli));
  const stateFile=path.join(directory,'receipt.json'),digest=hash(s);
  state=fs.existsSync(stateFile)?JSON.parse(fs.readFileSync(stateFile,'utf8')):{schema:'abya.runtime-scenario-receipt/v1',scopeId:crypto.randomUUID(),inputHash:digest,version:s.version,buildHash:s.buildHash,archiveHash:s.archiveHash,steps:[]};
  if(state.inputHash!==digest)throw Error('Scenario changed: use a new run directory, retain previous evidence');
  atomic(path.join(directory,'scenario.json'),s);
  started=true;
  const results=Object.create(null);
  for(const original of s.steps){
   const step={...original,arguments:resolve(original.arguments,results)};
   if(s.schema==='abya.runtime-scenario/v3'&&step.toolName.startsWith('runtime_input_'))step.arguments.scopeId=state.scopeId;
   const previous=state.steps.find(x=>x.id===step.id);
   if(previous){if(previous.status==='completed'){
    if(previous.resultPath){if(previous.resultPath!==`response-${step.id}.json`)throw Error('Invalid saved response path');const value=JSON.parse(fs.readFileSync(path.join(directory,previous.resultPath),'utf8'));if(hash(value)!==previous.resultHash)throw Error('Saved response fingerprint changed');results[step.id]=value;}
    continue;
   }throw Error(`Step ${step.id} requires reconciliation; no automatic replay`);}
   const receipt={id:step.id,role:step.role,action:step.action,instanceId:step.instanceId,inputLayer:step.inputLayer,arguments:step.arguments,
    requestId:crypto.randomUUID(),status:'pending',startedAt:new Date().toISOString(),observations:[]};
   state.steps.push(receipt);atomic(stateFile,state);
   try{
    for(let i=0;i<(step.repeat??1);i++){
     // Each bounded read poll has a distinct identity. Writes are dispatched exactly once.
     const requestId=i?crypto.randomUUID():receipt.requestId;
     receipt.pollRequestId=requestId;atomic(stateFile,state);
     let value;
     for(let retry=0;;retry++){
      const attemptId=retry?crypto.randomUUID():requestId;
      receipt.attempts??=[];
      receipt.attempts.push({requestId:attemptId,startedAt:new Date().toISOString()});atomic(stateFile,state);
      try{value=await call(step,attemptId,cli);break;}
      catch(error){
       receipt.failure=classifyFailure(error,step.readOnly===true);atomic(stateFile,state);
       if(!step.readOnly||retry>=(step.readRetries??0)||receipt.failure.outcome==='not-executed')throw error;
      }
     }
     receipt.resultHash=hash(value);
     receipt.resultPath=`response-${step.id}.json`;atomic(path.join(directory,receipt.resultPath),value);
     receipt.observations=step.assertions.map(a=>evaluate(a,value,results));
     results[step.id]=value;
     if(receipt.observations.every(a=>a.passed))break;
     if(i+1<(step.repeat??1))await new Promise(r=>setTimeout(r,Math.max(100,Math.min(step.intervalMs??500,2000))));
    }
    receipt.status=receipt.observations.every(a=>a.passed)?'completed':'assertionFailed';
    if(receipt.status==='assertionFailed')receipt.failure={kind:'verification-failed',outcome:'observed',platformDiagnosis:'unconfirmed'};
    else delete receipt.failure;
   }catch(e){receipt.failure=classifyFailure(e,step.readOnly===true);receipt.status=receipt.failure.needsReconciliation?'needsReconciliation':receipt.failure.outcome==='not-executed'?'invocationRejected':'verificationUnavailable';throw e;}
   finally{receipt.completedAt=new Date().toISOString();atomic(stateFile,state);}
   if(receipt.status!=='completed')throw Error(`Assertion failed: ${step.id}`);
  }
  return {...state,complete:true,note:'Scenario assertions only; not human acceptance or automatic production approval'};
 }finally{
  try{if(started)await cleanupInputs(s,state,directory,call,cli,atomic);}
  finally{fs.closeSync(fd);fs.unlinkSync(lock);}
 }
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
