import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {runScenario,validate} from '../.codex/skills/abya-game-development-task/scripts/runtime-scenario.mjs';
import {evaluate} from '../.codex/skills/abya-game-development-task/scripts/scenario-values.mjs';
const scenario=()=>({schema:'abya.runtime-scenario/v3',version:'v1',buildHash:'b',archiveHash:'a',roles:{Host:'owned-host',Client:'owned-client'},
 capabilities:['runtime_get_game_state','runtime_input_key_press','runtime_input_reset'],steps:[
 {id:'before',role:'Host',action:'state.read',arguments:{},assertions:[{path:['score'],atLeast:0}]},
 {id:'press',role:'Client',action:'key.down',arguments:{keyCode:'Space'},assertions:[]},
 {id:'after',role:'Host',action:'state.read',arguments:{},assertions:[{path:['score'],greaterThan:{$ref:['before','score']}}]},
 {id:'client',role:'Client',action:'state.read',arguments:{},assertions:[{path:['score'],equals:{$ref:['after','score']}}]}]});
function directory(t){const root=fs.mkdtempSync(path.join(os.tmpdir(),'abya-v3-'));t.after(()=>{assert.ok(root.startsWith(path.join(os.tmpdir(),'abya-v3-')));fs.rmSync(root,{recursive:true,force:true});});return root;}
test('roles and action library preserve full responses and support restart references',async t=>{
 const root=directory(t);let calls=0;
 const call=async step=>{calls++;return step.id==='before'?{score:2}:['after','client'].includes(step.id)?{score:3}:{accepted:true};};
 const result=await runScenario(scenario(),root,call);
 assert.equal(result.steps[1].instanceId,'owned-client');assert.equal(result.complete,true);
 await runScenario(scenario(),root,call);assert.equal(calls,5);
 fs.writeFileSync(path.join(root,'response-before.json'),JSON.stringify({score:100}));
 await assert.rejects(runScenario(scenario(),root,call),/fingerprint/);
});
test('missing data does not satisfy inequality or numeric comparisons',()=>{
 for(const operator of ['notEquals','greaterThan','atLeast','lessThan','atMost'])assert.equal(evaluate({path:['missing'],[operator]:1},{},{}).passed,false);
 assert.equal(evaluate({path:['missing'],exists:false},{},{}).passed,true);
 assert.equal(evaluate({path:['score'],equals:null},{score:null},{}).passed,true);
});
test('preflight validation rejects missing roles/actions, future references and write polling',()=>{
 let s=scenario();s.steps[1].action='network.disconnect';assert.throws(()=>validate(s),/Missing reusable action/);
 s=scenario();s.steps[0].role='Foreign';assert.throws(()=>validate(s),/Unknown role/);
 s=scenario();s.steps[0].assertions=[{path:['score'],equals:{$ref:['after','score']}}];assert.throws(()=>validate(s),/earlier/);
 s=scenario();s.steps[1].repeat=2;assert.throws(()=>validate(s),/read-only/);
});
test('assertion failure stops subsequent writes and retains observed response',async t=>{
 const s=scenario(),root=directory(t);s.steps[0].assertions=[{path:['score'],equals:5}];let writes=0;
 await assert.rejects(runScenario(s,root,async step=>{if(step.id==='press')writes++;return {score:0};}),/Assertion failed/);
 assert.equal(writes,0);assert.equal(JSON.parse(fs.readFileSync(path.join(root,'response-before.json'))).score,0);
});
