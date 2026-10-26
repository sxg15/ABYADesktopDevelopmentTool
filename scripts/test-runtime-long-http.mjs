// Opt-in wall-clock regression: a local operation intentionally returns after five minutes.
import http from 'node:http';import assert from 'node:assert/strict';import fs from 'node:fs';
import {loopbackFetch} from '../tools/abya/src/loopback-http.mjs';
if(process.env.ABYA_LONG_TRANSPORT_TEST!=='1')throw Error('Set ABYA_LONG_TRANSPORT_TEST=1 for the 305-second localhost regression');
let calls=0;const server=http.createServer((req,res)=>{calls++;setTimeout(()=>res.end('{"success":true}'),305000);});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const start=Date.now(),url=`http://127.0.0.1:${server.address().port}/invoke`;
try{
 const result=await loopbackFetch(url,{body:'{}',signal:AbortSignal.timeout(330000)});
 assert.equal((await result.json()).success,true);assert.equal(calls,1);
 const report={passed:true,elapsedMs:Date.now()-start,calls,transport:'node:http',note:'Actual delayed-header response >300 seconds; one dispatch only, localhost fixture, not gameplay acceptance'};
 fs.writeFileSync('logs/runtime-long-http-20261026.json',JSON.stringify(report,null,2));console.log(JSON.stringify(report));
}finally{server.closeAllConnections();server.close();}
