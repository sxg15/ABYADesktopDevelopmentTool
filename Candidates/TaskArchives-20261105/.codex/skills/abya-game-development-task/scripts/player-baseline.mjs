import {spawnSync} from 'node:child_process';
import {decodeResult} from './scenario-errors.mjs';
const [action,instanceId,...testIds]=process.argv.slice(2);
try {
 const cli=process.env.ABYA_DESKTOP_CLI,taskId=process.env.ABYA_DEVELOPMENT_TASK_ID;
 if(!cli||!taskId)throw Error('Run in the task managed terminal');
 if(!['read','record'].includes(action))throw Error('Usage: player-baseline.mjs read | record INSTANCE_ID TEST_ID...');
 if(action==='record'&&(!instanceId||testIds.length<1||testIds.length>16))throw Error('Use actual completed native runtime test IDs');
 const command=action==='read'?'baseline':'qualify-player';
 const input=action==='read'?{taskId,query:[instanceId,...testIds].filter(Boolean).join(' ')}:{taskId,instanceId,testIds};
 const result=spawnSync(cli,['runtime',command,'--input-file','-','--json'],{
  input:JSON.stringify(input),encoding:'utf8',windowsHide:true,timeout:180000,maxBuffer:4*1024*1024});
 console.log(JSON.stringify(decodeResult(result)));
} catch(error){console.error(error.message);process.exitCode=1;}
