import fs from 'node:fs';
import path from 'node:path';
import {connectPilot,readJson,saveJson,hashFile} from './pilot-native-api.mjs';
import {verifyPilot} from './verify-workflow-pilot.mjs';
const api=await connectPilot(process.argv[2]);
try{
 const runsPath=path.join(api.root,'runs'),faultsPath=path.join(api.root,'faults');
 const runs=fs.existsSync(runsPath)?fs.readdirSync(runsPath).map(name=>readJson(path.join(runsPath,name,'run.json'))):[];
 const faultInjections=fs.existsSync(faultsPath)?fs.readdirSync(faultsPath).filter(name=>name.endsWith('.json')).map(name=>{
  const file=path.join(faultsPath,name),fault=readJson(file);
  return {kind:fault.kind,passed:fault.passed===true&&fault.appSha256===hashFile(api.build.executable),evidence:{path:`faults/${name}`,sha256:hashFile(file)}};
 }):[];
 const report={schema:'abya.workflow-pilot/v1',workflowVersion:'2.11.0',createdAt:new Date().toISOString(),appSha256:hashFile(api.build.executable),
  packageInventorySha256:hashFile(path.join(path.dirname(api.build.executable),'package-files.json')),runs,faultInjections};
 const file=path.join(api.root,'pilot.json');saveJson(file,report);
 let qualification;try{qualification=verifyPilot(file,api.build.executable);}catch(error){qualification={passed:false,reason:error.message};}
 saveJson(path.join(api.root,'qualification.json'),qualification);
 console.log(JSON.stringify({qualification,runs:runs.map(r=>({case:r.caseId,repeat:r.repeat,result:r.result??'incomplete',firstPlayableAt:r.firstPlayableAt,timing:r.timing})),faults:faultInjections.map(f=>({kind:f.kind,passed:f.passed}))}));
}finally{await api.browser.close();}
