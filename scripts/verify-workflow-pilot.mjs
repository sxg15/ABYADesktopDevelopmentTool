import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';
const sha=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const json=file=>JSON.parse(fs.readFileSync(file,'utf8').replace(/^\uFEFF/,''));
function owned(root,name){
 const base=fs.realpathSync(root),target=fs.realpathSync(path.resolve(base,name));
 const relative=path.relative(base,target);
 if(!relative||relative.startsWith('..')||path.isAbsolute(relative))throw Error('Pilot evidence escapes its root');
 return target;
}
export function verifyPilot(reportPath,executable){
 const report=json(reportPath),root=path.dirname(path.resolve(reportPath));
 if(report.schema!=='abya.workflow-pilot/v1'||report.workflowVersion!=='2.11.0')throw Error('Expected workflow 2.11 native pilot evidence');
 if(report.appSha256!==sha(executable))throw Error('Pilot executable does not match the candidate');
 if(report.packageInventorySha256!==sha(path.join(path.dirname(executable),'package-files.json')))throw Error('Pilot resources do not match the candidate package');
 if(!Array.isArray(report.runs)||report.runs.length<6)throw Error('At least three game cases, repeated independently, are required');
 const tasks=new Set(),nativeIds=new Set(),cases=new Map();
 for(const run of report.runs){
  if(run.appSha256!==report.appSha256||run.packageInventorySha256!==report.packageInventorySha256)throw Error('A run started on a different candidate or resource package');
  if(run.result!=='passed'||run.mode!=='native-gameplay'||run.externalImplementation!==false||!Array.isArray(run.interventions)||run.interventions.length)
   throw Error('Manual repairs, interventions and unsuccessful runs cannot qualify autonomous completion');
  if(!run.taskId||tasks.has(run.taskId)||!run.nativeSessionId||nativeIds.has(run.nativeSessionId))throw Error('Each repetition needs independent task and native identities');
  tasks.add(run.taskId);nativeIds.add(run.nativeSessionId);
  const start=Date.parse(run.startedAt),end=Date.parse(run.completedAt),first=Date.parse(run.firstPlayableAt);
  if(!Number.isFinite(start)||!Number.isFinite(end)||!Number.isFinite(first)||first<start||end<first||end-start>4*3600_000)
   throw Error('Invalid timing or the four-hour measured pilot target was exceeded');
  const snapshotPath=owned(root,run.production.path),workflowPath=owned(root,run.workflow.path);
  if(sha(snapshotPath)!==run.production.sha256||sha(workflowPath)!==run.workflow.sha256)throw Error('Pilot evidence fingerprint changed');
  const view=json(snapshotPath),record=view.record??view;
  if(record.taskId!==run.taskId||record.workflowVersion!==report.workflowVersion||!['review','delivery','closeout'].includes(record.currentStage))throw Error('Native task did not reach formal review');
  const checks=new Map((record.selfTests??[]).filter(c=>c.version===record.currentVersion&&c.cycle===record.cycle).map(c=>[c.id,c]));
  for(const id of record.policy?.selfTestChecks??[]){
   const check=checks.get(id);
   if(check?.status!=='passed'&&!(id==='authority'&&record.playerMode==='single'&&check?.status==='not-applicable'))throw Error(`Required check remains open: ${id}`);
  }
  if(!checks.size)throw Error('No native gameplay checks');
  const pack=record.workbench?.execution?.verificationPackage;
  if(!pack||pack.version!==record.currentVersion||pack.status!=='core-verified')throw Error('No current verified package');
  if(sha(owned(root,run.packagePath))!==pack.sha256)throw Error('Current package is missing or changed');
  const workflow=json(workflowPath);
  if(workflow.taskId!==run.taskId||workflow.observability!=='native'||!workflow.turns?.some(t=>t.activities?.some(a=>a.source==='desktopCli'&&a.status==='completed')))
   throw Error('Native execution evidence is missing');
  if(!run.caseId||!run.requirementsSha256||!run.planSha256||record.documents?.requirements?.sha256!==run.requirementsSha256||record.documents?.plan?.sha256!==run.planSha256)
   throw Error('Pilot scope does not match its approved documents');
  cases.set(run.caseId,(cases.get(run.caseId)??0)+1);
 }
 if(cases.size<3||[...cases.values()].some(count=>count<2))throw Error('Three gameplay families must each have at least two independent repetitions');
 const faults=new Set(report.faultInjections?.filter(x=>x.passed===true&&x.evidence?.path&&sha(owned(root,x.evidence.path))===x.evidence.sha256).map(x=>x.kind));
 for(const kind of ['disconnect','timeout','invalid-arguments','stale-snapshot','false-blocker'])if(!faults.has(kind))throw Error(`Missing observed fault-injection evidence: ${kind}`);
 return {passed:true,runs:report.runs.length,cases:cases.size,appSha256:report.appSha256,note:'Evidence structure, matching identities and measured outcomes verified; semantic gameplay quality still requires review.'};
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){
 try{console.log(JSON.stringify(verifyPilot(process.argv[2],process.argv[3])));}
 catch(error){console.error(error.message);process.exitCode=1;}
}
