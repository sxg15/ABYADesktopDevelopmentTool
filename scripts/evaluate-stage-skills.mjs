// Opt-in real-model interpretation probe. No game operations or approvals are executed.
import fs from 'node:fs';
import path from 'node:path';
import {spawn,spawnSync,execFileSync} from 'node:child_process';
import readline from 'node:readline';
const [codex,folder]=process.argv.slice(2);
if(!codex||!folder)throw Error('Usage: <installed codex.exe> <fresh output directory>');
const output=path.resolve(folder);
if(fs.existsSync(output))throw Error('A fresh isolated directory is required');
fs.mkdirSync(output,{recursive:true});
const file='.codex/skills/abya-game-development-task/';
const old=['SKILL.md','references/production-stages.md'].map(f=>execFileSync('git',['show',`b60ccc4:${file+f}`],{encoding:'utf8'})).join('\n');
const current=['SKILL.md','references/production-stages.md'].map(f=>fs.readFileSync(file+f,'utf8')).join('\n');
const scenarios=[
  {id:'art-buttons',state:'resources; requirements approved; plan not approved; art v2 exists and covers latest feedback; no active prototype grant; Player unavailable',request:'按钮设计保留，同时支持 W/S，先给我看美术设计。',expected:'submit-design'},
  {id:'scope-gap',state:'resources; requirements approved; plan not approved; art draft and requirement mapping complete; prior AI tried scope=customUiNode,ui,custom-ui; no allowed enum returned; no active prototype grant',request:'美术稿可以提交了吗？请继续当前阶段。',expected:'submit-design'},
  {id:'implementation-invalid',state:'implementation; current plan approved; architecture validate completed with success=true, data.valid=false, data.ready=false, component.scope error; allowedValues coordinator/entity/input/presentation',request:'继续 Lua 实现。',expected:'repair-input'}
];
const child=spawn(codex,['app-server'],{stdio:['pipe','pipe','pipe'],windowsHide:true});child.stderr.resume();
let next=0,finish,text='',pending=new Map();
readline.createInterface({input:child.stdout}).on('line',line=>{
  let m;try{m=JSON.parse(line);}catch{return;}
  if(m.id!==undefined&&!m.method){pending.get(m.id)?.(m);pending.delete(m.id);}
  if(m.method==='item/completed'&&m.params?.item?.type==='agentMessage')text+=m.params.item.text??'';
  if(m.method==='turn/completed')finish?.(m.params.turn.status);
  if(m.id!==undefined&&m.method)child.stdin.write(JSON.stringify({id:m.id,error:{code:-32601,message:'This interpretation probe does not approve or execute external operations.'}})+'\n');
});
async function call(method,params){const id=++next;const result=await Promise.race([
  new Promise(resolve=>{pending.set(id,resolve);child.stdin.write(JSON.stringify({id,method,params})+'\n');}),
  new Promise((_,reject)=>setTimeout(()=>reject(Error(`${method} timeout`)),25000).unref())]);
  if(result.error)throw Error(JSON.stringify(result.error));return result.result;}
const results=[];
try{
  await call('initialize',{clientInfo:{name:'abya-stage-skill-eval',version:'1'},capabilities:{experimentalApi:true}});
  child.stdin.write(JSON.stringify({method:'initialized',params:{}})+'\n');
  for(const variant of ['old-skill','new-skill','new-skill-with-contract']){
    const instructions=variant==='old-skill'?old:current;
    const contract=variant==='new-skill-with-contract'?'Backend contract: design submission requires intact materials and human review; technical runtime issues are tracked without blocking design. Before plan approval runtime authoring requires an active APP prototype grant. Static valid=false never permits implementation.':'';
    const thread=await call('thread/start',{cwd:output,model:'gpt-6-astra',developerInstructions:`Interpret the following ABYA Skill in a synthetic scenario. Do not use tools or modify files. This is only an interpretation evaluation, never a real approval. Respond only with a JSON array, one object per scenario: id, action, reason. Actions: submit-design, run-runtime-check, request-user-help, repair-input, perform-write.\n${instructions}\n${contract}`});
    text='';const done=new Promise(resolve=>{finish=resolve;});const start=Date.now();
    await call('turn/start',{threadId:thread.thread.id,input:[{type:'text',text:JSON.stringify(scenarios.map(({expected,...s})=>s))}]});
    const status=await Promise.race([done,new Promise((_,reject)=>setTimeout(()=>reject(Error('Model turn timed out')),240000).unref())]);
    let decisions;try{decisions=JSON.parse(text.replace(/^```json\s*|\s*```$/g,''));}catch{decisions=[];}
    const result={variant,status,elapsedMs:Date.now()-start,decisions,passed:scenarios.every(s=>decisions.find(d=>d.id===s.id)?.action===s.expected)};
    results.push(result);fs.writeFileSync(path.join(output,'results.json'),JSON.stringify({scope:'real-model interpretation; not tool execution or full-game timing',model:'gpt-6-astra',results},null,2));
    console.log(JSON.stringify({variant,status,passed:result.passed,elapsedMs:result.elapsedMs}));
    await call('thread/archive',{threadId:thread.thread.id});
  }
  if(results.slice(1).some(r=>!r.passed))process.exitCode=1;
}finally{spawnSync('taskkill',['/PID',String(child.pid),'/T','/F'],{windowsHide:true,stdio:'ignore'});}
