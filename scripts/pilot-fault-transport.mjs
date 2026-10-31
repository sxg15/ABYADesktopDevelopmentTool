// Fault-injected local peer exercises the shipped CLI binary, not a game verdict.
import fs from 'node:fs';
import path from 'node:path';
import net from 'node:net';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {connectPilot,saveJson,hashFile} from './pilot-native-api.mjs';
const api=await connectPilot(process.argv[2]);
const packageRoot=path.dirname(api.build.executable),cli=path.join(packageRoot,'abya-desktop.exe');
const {decodeResult,classifyFailure}=await import(new URL('../Publish-Staging-Workflow/.codex/skills/abya-game-development-task/scripts/scenario-errors.mjs',import.meta.url));
const execute=(args,env,timeout)=>new Promise(resolve=>{
 const child=spawn(cli,args,{env,windowsHide:true,stdio:['ignore','pipe','pipe']});
 let stdout='',stderr='',expired=false;child.stdout.on('data',d=>stdout+=d);child.stderr.on('data',d=>stderr+=d);
 const timer=setTimeout(()=>{expired=true;child.kill();},timeout);
 child.on('error',error=>{clearTimeout(timer);resolve({error,stdout,stderr,status:null});});
 child.on('close',status=>{clearTimeout(timer);resolve({status,stdout,stderr,error:expired?{code:'ETIMEDOUT'}:undefined});});
});
try{
 const root=path.join(api.root,'faults');fs.mkdirSync(root,{recursive:true});
 const cleanEnv={...process.env,ABYA_TEST_MODE:'1',LOCALAPPDATA:path.join(api.root,'Local')};
 for(const key of ['ABYA_DESKTOP_PIPE','ABYA_DESKTOP_SESSION_TOKEN','ABYA_DESKTOP_PID'])delete cleanEnv[key];
 for(const kind of ['disconnect','timeout']){
  const pipe=`\\\\.\\pipe\\abya-desktop-${crypto.randomUUID()}`,sockets=new Set();let received=0;
  const server=net.createServer(socket=>{
   sockets.add(socket);socket.on('error',()=>{});socket.on('close',()=>sockets.delete(socket));
   let bytes=Buffer.alloc(0),counted=false;
   socket.on('data',chunk=>{
    bytes=Buffer.concat([bytes,chunk]);
    if(!counted&&bytes.length>=4&&bytes.length>=4+bytes.readUInt32LE(0)){
     counted=true;received++;if(kind==='disconnect')socket.destroy();
    }
   });
  });
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(pipe,resolve);});
  try{
   const requestId=crypto.randomUUID();
   const result=await execute(['doctor','--request-id',requestId,'--json'],{...cleanEnv,ABYA_DESKTOP_PIPE:pipe,ABYA_DESKTOP_SESSION_TOKEN:'a'.repeat(64),ABYA_DESKTOP_PID:String(process.pid)},1800);
   assert.equal(received,1,'The injected peer must receive exactly one request');
   let failure;try{decodeResult(result,requestId);}catch(error){failure=classifyFailure(error,true);}
   assert.equal(failure?.kind,'verification-unavailable');assert.equal(failure.platformDiagnosis,'unconfirmed');
   const recoveryId=crypto.randomUUID();
   const recovered=decodeResult(await execute(['doctor','--request-id',recoveryId,'--json'],cleanEnv,15000),recoveryId);
   assert.equal(recovered.protocolVersion,1);assert.equal(recovered.runtimeCliInstalled,true);
   const file=path.join(root,`${kind}.json`);
   saveJson(file,{kind,passed:true,at:new Date().toISOString(),source:'shipped-cli-with-owned-fault-injection-peer',appSha256:hashFile(api.build.executable),cliSha256:hashFile(cli),requestCount:received,readFailure:failure,reconnectedToCandidate:true,gameplayEvidence:false,note:'Transport classification and subsequent real APP connectivity; does not claim AI repaired a game.'});
   console.log(JSON.stringify({kind,passed:true,evidence:{path:`faults/${kind}.json`,sha256:hashFile(file)}}));
  }finally{for(const socket of sockets)socket.destroy();await new Promise(resolve=>server.close(resolve));}
 }
}finally{await api.browser.close();}
