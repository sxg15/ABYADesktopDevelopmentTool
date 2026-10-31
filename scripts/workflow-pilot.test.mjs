import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import {verifyPilot} from './verify-workflow-pilot.mjs';
const hash=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const put=(file,value)=>fs.writeFileSync(file,JSON.stringify(value));
function fixture(t){
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'abya-pilot-'));
 t.after(()=>{assert.ok(path.resolve(root).startsWith(path.resolve(os.tmpdir())+path.sep+'abya-pilot-'));fs.rmSync(root,{recursive:true,force:true});});
 const exe=path.join(root,'candidate.exe');fs.writeFileSync(exe,'synthetic validation fixture, not an actual APP');
 fs.writeFileSync(path.join(root,'package-files.json'),'synthetic package fingerprint');
 const report={schema:'abya.workflow-pilot/v1',workflowVersion:'2.11.0',appSha256:hash(exe),packageInventorySha256:hash(path.join(root,'package-files.json')),runs:[],faultInjections:[]};
 for(let index=0;index<6;index++){
  const taskId=`synthetic-${index}`,pkg=`${index}.zip`,production=`${index}.json`,workflow=`${index}-workflow.json`;
  fs.writeFileSync(path.join(root,pkg),'synthetic package bytes');
  put(path.join(root,production),{taskId,workflowVersion:'2.11.0',currentStage:'review',currentVersion:'v1',cycle:1,
   policy:{selfTestChecks:['lua']},documents:{requirements:{sha256:'req'},plan:{sha256:'plan'}},
   selfTests:[{id:'lua',status:'passed',version:'v1',cycle:1}],
   workbench:{execution:{verificationPackage:{version:'v1',status:'core-verified',sha256:hash(path.join(root,pkg))}}}});
  put(path.join(root,workflow),{taskId,observability:'native',turns:[{activities:[{source:'desktopCli',status:'completed'}]}]});
  report.runs.push({appSha256:report.appSha256,packageInventorySha256:report.packageInventorySha256,caseId:`case-${Math.floor(index/2)}`,taskId,nativeSessionId:`native-${index}`,mode:'native-gameplay',result:'passed',externalImplementation:false,interventions:[],
   startedAt:'2026-10-30T00:00:00Z',firstPlayableAt:'2026-10-30T00:30:00Z',completedAt:'2026-10-30T01:00:00Z',requirementsSha256:'req',planSha256:'plan',
   production:{path:production,sha256:hash(path.join(root,production))},workflow:{path:workflow,sha256:hash(path.join(root,workflow))},packagePath:pkg});
 }
 for(const kind of ['disconnect','timeout','invalid-arguments','stale-snapshot','false-blocker']){
  const name=`${kind}.json`;put(path.join(root,name),{synthetic:true});
  report.faultInjections.push({kind,passed:true,evidence:{path:name,sha256:hash(path.join(root,name))}});
 }
 const file=path.join(root,'pilot.json');put(file,report);
 return {root,exe,file,report,save:()=>put(file,report)};
}
test('pilot gate checks repetitions and retained evidence, not just a passed summary',t=>{
 const f=fixture(t);assert.equal(verifyPilot(f.file,f.exe).runs,6);
 f.report.runs[0].externalImplementation=true;f.save();assert.throws(()=>verifyPilot(f.file,f.exe),/Manual/);
 f.report.runs[0].externalImplementation=false;f.report.runs[1].nativeSessionId=f.report.runs[0].nativeSessionId;f.save();assert.throws(()=>verifyPilot(f.file,f.exe),/independent/);
});
test('changed resources, missing injections and exceeded time reject promotion',t=>{
 const f=fixture(t);f.report.packageInventorySha256='wrong';f.save();assert.throws(()=>verifyPilot(f.file,f.exe),/resources/);
 f.report.packageInventorySha256=hash(path.join(f.root,'package-files.json'));f.report.faultInjections.pop();f.save();assert.throws(()=>verifyPilot(f.file,f.exe),/Missing observed/);
 f.report.runs[0].completedAt='2026-10-30T08:00:00Z';f.save();assert.throws(()=>verifyPilot(f.file,f.exe),/four-hour/);
});
