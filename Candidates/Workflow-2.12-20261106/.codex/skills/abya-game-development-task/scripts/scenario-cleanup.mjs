import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';

// 清理仅释放本次场景的虚拟输入；回执持久化后才发送，不重放未知清理。
export async function cleanupInputs(s,state,directory,call,cli,atomic){
 if(s.schema!=='abya.runtime-scenario/v3')return;
 const roles=new Map(s.steps.filter(step=>step.toolName.startsWith('runtime_input_')).map(step=>[step.role,step.instanceId]));
 if(!roles.size)return;
 const file=path.join(directory,'cleanup.json');
 const receipts=fs.existsSync(file)?JSON.parse(fs.readFileSync(file,'utf8')):[];
 for(const [role,instanceId] of roles){
  const prior=receipts.find(item=>item.role===role);
  if(prior){if(prior.status==='completed')continue;throw Error('Input cleanup requires reconciliation');}
  const receipt={role,instanceId,scopeId:state.scopeId,requestId:crypto.randomUUID(),status:'pending'};
  receipts.push(receipt);atomic(file,receipts);
  try{
   receipt.result=await call({id:'cleanup-'+role,instanceId,toolName:'runtime_input_reset',arguments:{scopeId:state.scopeId},readOnly:false},receipt.requestId,cli);
   receipt.status='completed';
  }catch(error){receipt.status='needsReconciliation';receipt.error=error.message;throw error;}
  finally{atomic(file,receipts);}
 }
}
