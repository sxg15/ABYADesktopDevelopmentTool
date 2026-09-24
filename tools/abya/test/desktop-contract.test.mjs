import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const cli = fileURLToPath(new URL('../bin/abya.mjs', import.meta.url));
async function fixture(t, reply) {
  const requests=[];
  const server=createServer(async(req,res)=>{
    let raw=''; for await(const chunk of req) raw+=chunk;
    const body=JSON.parse(raw); requests.push({url:req.url,...body});
    const content=req.url.endsWith('/status') ? [{type:'text',text:JSON.stringify({
      processId:process.pid,target:'Player',projectPath:process.cwd(),desktopInstanceId:'managed-1'})}] : reply(body,req.url);
    res.end(JSON.stringify({version:1,requestId:body.requestId,success:true,content}));
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  t.after(()=>new Promise(resolve=>server.close(resolve)));
  const folder=await mkdtemp(path.join(tmpdir(),'abya 中文 output '));
  t.after(()=>rm(folder,{recursive:true,force:true}));
  async function run(args,env={}){
    const child=spawn(process.execPath,[cli,...args,'--endpoint',
      'http://127.0.0.1:'+server.address().port,'--output-dir',folder,'--json'],{
      env:{...process.env,ABYA_CLI_TOKEN:'test-token',ABYA_CLI_SESSION_ID:'conversation-1',...env}});
    const exited=new Promise(resolve=>child.once('close',resolve));
    child.stdin.end('{}'); let raw='';for await(const chunk of child.stdout)raw+=chunk;
    return {code:await exited,value:JSON.parse(raw)};
  }
  return {run,requests,folder};
}
test('desktop identity mismatch prevents capability execution',async t=>{
  const f=await fixture(t,()=>[]);
  const r=await f.run(['capability','run','read','--input-file','-'],{ABYA_CLI_DESKTOP_INSTANCE_ID:'wrong'});
  assert.equal(r.code,3);assert.equal(f.requests.length,1);
});
test('separate CLI processes preserve session and carry operation identity',async t=>{
  const f=await fixture(t,()=>[{type:'text',text:'{}'}]);
  for(const id of ['operation-1','operation-2']){
    const r=await f.run(['capability','run','read','--input-file','-'],{ABYA_CLI_REQUEST_ID:id});
    assert.equal(r.code,0);assert.equal(r.value.requestId,id);
  }
  const invoked=f.requests.filter(x=>x.url.endsWith('/invoke'));
  assert.deepEqual(invoked.map(x=>x.sessionId),['conversation-1','conversation-1']);
  assert.deepEqual(invoked.map(x=>x.requestId),['operation-1','operation-2']);
});
test('all image files and mixed text survive CLI output',async t=>{
  const bytes=Buffer.from('image-test');
  const f=await fixture(t,()=>[{type:'text',text:'{"label":"画面"}'},
    {type:'image',mimeType:'image/png',data:bytes.toString('base64')},
    {type:'text',text:'second text'},{type:'image',mimeType:'image/jpeg',data:bytes.toString('base64')}]);
  const r=await f.run(['capability','run','capture','--input-file','-']);
  assert.equal(r.code,0);assert.equal(r.value.content.length,4);
  for(const block of r.value.content.filter(x=>x.type==='image')){
    assert.ok(block.path.startsWith(f.folder));assert.deepEqual(await readFile(block.path),bytes);assert.equal(block.data,undefined);
  }
});
test('approval denial cannot be reported as completed execution',async t=>{
  const f=await fixture(t,()=>[{type:'text',text:'{"approval":"denied","executed":false}'}]);
  const r=await f.run(['capability','run','write','--input-file','-']);
  assert.equal(r.code,6);assert.equal(r.value.success,false);assert.equal(r.value.executionState,'not_executed');
});
test('cancel targets the original operation within the same session',async t=>{
  const f=await fixture(t,()=>[{type:'text',text:'{"cancelRequested":true}'}]);
  const r=await f.run(['cancel','operation-1']);
  assert.equal(r.code,0);const cancel=f.requests.find(x=>x.url.endsWith('/cancel'));
  assert.equal(cancel.targetRequestId,'operation-1');assert.equal(cancel.sessionId,'conversation-1');
});
