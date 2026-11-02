import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import crypto from 'node:crypto';
export const readJson=file=>JSON.parse(fs.readFileSync(file,'utf8').replace(/^\uFEFF/,''));
export const hashFile=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
export function saveJson(file,value){const temporary=`${file}.tmp`;fs.writeFileSync(temporary,JSON.stringify(value,null,2));fs.renameSync(temporary,file);}
export async function flushConnection(api,taskId,conversationId,receipt){
 if(receipt?.status!=='needsConnection')return receipt;
 assert.ok(receipt.requestId,'Reconnect only an identified continuation');
 await api.invoke('open_codex_terminal',{taskId,conversationId,columns:120,rows:30});
 return api.invoke('flush_codex_continuation',{taskId,conversationId,requestId:receipt.requestId});
}
export async function connectPilot(directory){
 const root=path.resolve(directory),marker=readJson(path.join(root,'pilot-root.json'));
 assert.equal(marker.schema,'abya.workflow-pilot-root/v1');
 const {chromium}=createRequire(import.meta.url)(process.env.ABYA_TEST_PLAYWRIGHT||'playwright');
 const browser=await chromium.connectOverCDP(`http://127.0.0.1:${marker.debugPort}`);
 const page=browser.contexts()[0].pages()[0];
 const invoke=async(command,args={})=>{
  const response=await page.evaluate(async({command,args})=>{
   if(command==='open_codex_terminal')args.onEvent=`__CHANNEL__:${window.__TAURI_INTERNALS__.transformCallback(()=>{},false)}`;
   try{return {success:true,data:await window.__TAURI_INTERNALS__.invoke(command,args)};}
   catch(error){return {success:false,error:{code:error?.code,message:error?.message??String(error)}};}
  },{command,args});
  if(!response.success)throw Error(`${command}: ${JSON.stringify(response.error)}`);
  return response.data;
 };
 const paths=await invoke('get_app_paths'),build=await invoke('get_build_info');
 assert.equal(path.resolve(paths.dataDir).toLowerCase(),path.join(root,'Local','ABYA Desktop Development Tool','Data').toLowerCase());
 assert.equal(build.testEnvironment,true);assert.equal(build.verification,'verified');
 assert.equal(build.release,'delivery-first-20261030');
 const manifest=readJson(path.join(path.dirname(build.executable),'build-manifest.json'));
 assert.equal(manifest.workflowVersion,'2.11.0');
 return {root,marker,browser,page,invoke,build,paths};
}
