import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
export function summarize(text) {
 const lines=text.split(/\r?\n/),groups=new Map();
 const safe=s=>s.replace(/(Bearer\s+)[\w.+/=-]+/gi,'$1[redacted]').replace(/((?:token|password|secret)\s*[:=]\s*)[^\s,;]+/gi,'$1[redacted]');
 for(let i=0;i<lines.length;i++){
  const message=lines[i];
  if(/^\s+at |^(UnityEngine\.|DG\.Tweening\.|ABYA\.)/.test(message)||!/(?:Exception:|\[StartupLaunch\]|startup failed|Unable to load)/i.test(message))continue;
  const key=safe(message).slice(0,600),old=groups.get(key);
  if(old){old.count++;old.lastLine=i+1;continue;}
  groups.set(key,{message:key,count:1,firstLine:i+1,lastLine:i+1,context:lines.slice(i+1,i+9).map(safe).join('\n').slice(0,2000)});
 }
 return {schema:'abya.runtime-log-summary/v1',lines:lines.length,distinctGroups:groups.size,
  groups:[...groups.values()].slice(0,30),omittedGroups:Math.max(0,groups.size-30),
  note:'Counts describe this supplied file only. First exceptions and exit-time errors are separate observations, not proof of root cause. Retain the original file.'};
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){
 const file=process.argv[2];if(!file)throw Error('Usage: node summarize-runtime-log.mjs ORIGINAL.log');
 const size=fs.statSync(file).size;if(size>64*1024*1024)throw Error('Log exceeds 64 MiB; select a bounded original segment and retain coverage information');
 console.log(JSON.stringify(summarize(fs.readFileSync(file,'utf8')),null,2));
}
