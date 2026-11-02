// 传输、调用和断言是不同观察；任何分类都不直接证明平台或玩法缺陷。
export class ScenarioCallError extends Error {
 constructor(message,kind='transport-unknown',outcome='unknown') {
  super(message);this.kind=kind;this.outcome=outcome;
 }
}
export function classifyFailure(error,readOnly=false) {
 const definite=error?.outcome==='not-executed';
 return {
  kind:definite?'invocation-rejected':readOnly?'verification-unavailable':error?.kind??'transport-unknown',
  outcome:definite?'not-executed':readOnly?'read-failed':'unknown',
  needsReconciliation:!readOnly&&!definite,
  platformDiagnosis:'unconfirmed'
 };
}
export function decodeResult(processResult,expectedRequestId) {
 let value;
 try{value=JSON.parse(processResult.stdout);}catch{}
 // 原请求的完整成功回执优先于后续进程退出异常，避免把已完成写入误判为未知并重做。
 if(expectedRequestId&&value?.requestId===expectedRequestId&&value.success===true&&!value.isError&&value.data?.success!==false&&!value.data?.isError)
  return value.data??value;
 if(processResult.error){
  const notStarted=processResult.error.code==='ENOENT';
  throw new ScenarioCallError(notStarted?'CLI unavailable before dispatch':'CLI connection or process result unavailable',
   'transport-unknown',notStarted?'not-executed':'unknown');
 }
 if(!value)throw new ScenarioCallError('No valid operation receipt returned');
 if(expectedRequestId&&value.requestId!==expectedRequestId)throw new ScenarioCallError('Operation receipt identity mismatch');
 if(value.success!==true||value.isError||value.data?.success===false||value.data?.isError){
  const error=value.error??value.data??{};
  const rejected=error.detail==='not_executed'||['invalid_request','unauthorized','desktop_identity_mismatch'].includes(error.code);
  throw new ScenarioCallError('Runtime request failed; inspect the retained receipt','runtime-error',rejected?'not-executed':'unknown');
 }
 if(processResult.status!==0)throw new ScenarioCallError('CLI exited before a reliable result');
 return value.data??value;
}
