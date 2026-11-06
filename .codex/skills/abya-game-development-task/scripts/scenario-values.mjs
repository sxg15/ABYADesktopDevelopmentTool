import {isDeepStrictEqual} from 'node:util';

const operations=['equals','notEquals','greaterThan','atLeast','lessThan','atMost','exists'];
export function at(value,path){
 for(const key of path){
  if(value===null||typeof value!=='object'||!Object.hasOwn(value,key))return {exists:false};
  value=value[key];
 }
 return {exists:true,value};
}
// 引用只读取此前成功步骤的实际返回值，不执行表达式。
export function resolve(value,results){
 if(Array.isArray(value))return value.map(x=>resolve(x,results));
 if(value&&typeof value==='object'){
  if(Object.hasOwn(value,'$ref')){
   if(Object.keys(value).length!==1||!Array.isArray(value.$ref)||value.$ref.length<2)throw Error('Expected $ref: [stepId, ...path]');
   const [id,...path]=value.$ref;
   if(!Object.hasOwn(results,id))throw Error(`Reference needs a completed step: ${id}`);
   const found=at(results[id],path);if(!found.exists)throw Error(`Reference path is missing: ${id}`);
   return found.value;
  }
  return Object.fromEntries(Object.entries(value).map(([k,v])=>[k,resolve(v,results)]));
 }
 return value;
}
export function validateAssertion(a){
 if(!a||!Array.isArray(a.path)||a.path.some(k=>typeof k!=='string'&&(!Number.isInteger(k)||k<0)))throw Error('Assertion path required');
 const ops=operations.filter(k=>Object.hasOwn(a,k));
 if(ops.length!==1)throw Error('Exactly one assertion operator required');
 if(ops[0]==='exists'&&typeof a.exists!=='boolean')throw Error('exists requires a boolean');
}
export function evaluate(a,value,results){
 validateAssertion(a);
 const operation=operations.find(k=>Object.hasOwn(a,k)),expected=resolve(a[operation],results),actual=at(value,a.path);
 let passed=false;
 if(operation==='exists')passed=actual.exists===expected;
 else if(actual.exists){
  if(operation==='equals')passed=isDeepStrictEqual(actual.value,expected);
  else if(operation==='notEquals')passed=!isDeepStrictEqual(actual.value,expected);
  else if(typeof actual.value==='number'&&Number.isFinite(actual.value)&&typeof expected==='number'&&Number.isFinite(expected)){
   passed=({greaterThan:()=>actual.value>expected,atLeast:()=>actual.value>=expected,
    lessThan:()=>actual.value<expected,atMost:()=>actual.value<=expected})[operation]();
  }
 }
 return {path:a.path,operation,expected,actual:actual.exists?actual.value:null,exists:actual.exists,passed};
}
