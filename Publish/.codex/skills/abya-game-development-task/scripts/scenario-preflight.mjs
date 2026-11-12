import {spawnSync} from 'node:child_process';
import {decodeResult,ScenarioCallError} from './scenario-errors.mjs';

function request(cli,command,input){
 return decodeResult(spawnSync(cli,[...command,'--input-file','-','--json'],{
  input:JSON.stringify(input),encoding:'utf8',windowsHide:true,timeout:30000,maxBuffer:16*1024*1024}));
}
// 在任何场景写操作之前核对任务版本和所有角色所需的实际能力。
export function preflight(s,cli){
 const taskId=process.env.ABYA_DEVELOPMENT_TASK_ID;
 if(!taskId)throw new ScenarioCallError('Managed task identity required','invocation-rejected','not-executed');
 const view=request(cli,['production','get'],{taskId}),record=view.record;
 if(!record||record.currentVersion!==s.version||record.versionDetails?.archiveHash!==s.archiveHash||record.versionDetails?.playerBuildHash!==s.buildHash)
  throw new ScenarioCallError('Scenario does not match the current registered candidate','invocation-rejected','not-executed');
 const definitions=new Map();
 const probes=[...s.steps];
 for(const step of s.steps.filter(step=>step.toolName.startsWith('runtime_input_')))probes.push({...step,toolName:'runtime_input_reset',readOnly:false});
 for(const step of probes){
  const key=step.instanceId+':'+step.toolName;
  if(definitions.has(key))continue;
  const definition=request(cli,['runtime','describe'],{instanceId:step.instanceId,capabilityId:step.toolName});
  if(definition.available!==true||!definition.inputSchema)
   throw new ScenarioCallError(`Required capability unavailable: ${step.toolName}`,'invocation-rejected','not-executed');
  definitions.set(key,definition);
 }
 for(const step of s.steps){
  const definition=definitions.get(step.instanceId+':'+step.toolName);
  if(step.readOnly===true&&definition.riskLevel!=='ReadOnly')
   throw new ScenarioCallError(`Capability is not read-only: ${step.toolName}`,'invocation-rejected','not-executed');
 }
 return {taskId,version:record.currentVersion,archiveHash:s.archiveHash,buildHash:s.buildHash,
  verifiedAt:new Date().toISOString(),capabilities:[...definitions].map(([key,d])=>({key,inputSchema:d.inputSchema,riskLevel:d.riskLevel})),
  scope:'Registered candidate and owned-instance capability contracts; live archive identity must also be asserted by the suite'};
}
