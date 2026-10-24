import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {runScenario,validate} from '../.codex/skills/abya-game-development-task/scripts/runtime-scenario.mjs';
const scenario=()=>({schema:'abya.runtime-scenario/v1',version:'v1',buildHash:'build',archiveHash:'archive',capabilities:['known'],steps:[{id:'score',instanceId:'owned',toolName:'known',arguments:{},assertions:[{path:['score'],equals:5}]}]});
test('scenario assertion failures preserve evidence and never repeat writes',async t=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'abya-scenario-'));t.after(()=>fs.rmSync(dir,{recursive:true,force:true}));
 let calls=0;const call=async()=>{calls++;return {score:0};};
 await assert.rejects(runScenario(scenario(),dir,call),/Assertion failed/);
 await assert.rejects(runScenario(scenario(),dir,call),/reconciliation/);assert.equal(calls,1);
 const receipt=JSON.parse(fs.readFileSync(path.join(dir,'receipt.json')));assert.equal(receipt.steps[0].observations[0].passed,false);
});
test('completed exact runs reuse receipts; changed versions cannot reuse them',async t=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'abya-scenario-'));t.after(()=>fs.rmSync(dir,{recursive:true,force:true}));
 let calls=0;const call=async()=>{calls++;return {score:5};};
 assert.equal((await runScenario(scenario(),dir,call)).complete,true);
 await runScenario(scenario(),dir,call);assert.equal(calls,1);
 await assert.rejects(runScenario({...scenario(),version:'v2'},dir,call),/changed/);
});
test('unknown dispatch is durable and read polling is bounded',async t=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'abya-scenario-'));t.after(()=>fs.rmSync(dir,{recursive:true,force:true}));
 await assert.rejects(runScenario(scenario(),dir,async()=>{throw Error('connection lost');}));
 assert.equal(JSON.parse(fs.readFileSync(path.join(dir,'receipt.json'))).steps[0].status,'needsReconciliation');
 const s=scenario();s.steps[0].repeat=2;assert.throws(()=>validate(s),/read-only/);
 s.steps[0].readOnly=true;s.steps[0].intervalMs=100;let calls=0;
 await runScenario(s,path.join(dir,'poll'),async()=>({score:++calls===2?5:0}));assert.equal(calls,2);
});

import {summarize} from '../.codex/skills/abya-game-development-task/scripts/summarize-runtime-log.mjs';
test('log summaries retain first occurrence and collapse storms without claiming causality',()=>{
 const report=summarize('IndexOutOfRangeException: first\n  at Camera.Update\n\nIndexOutOfRangeException: first\n  at Camera.Update\n\n[StartupLaunch] failed');
 assert.equal(report.groups[0].count,2);assert.equal(report.groups[0].firstLine,1);assert.equal(report.groups[0].lastLine,4);
 assert.equal(report.groups[1].firstLine,7);assert.match(report.note,/not proof/);
});
