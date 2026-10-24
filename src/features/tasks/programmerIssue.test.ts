import {expect,it} from "vitest";
import type {ProductionIssue} from "../../shared/production";
import {activeProgrammerIssues,programmerIssue} from "./programmerIssue";
const issue:ProductionIssue={id:"engine",kind:"blocker",status:"open",stage:"implementation",description:"底层事件缺陷",resumeWhen:"等待程序修复后复验"};
it("marks confirmed source/build separately and retains uncertainty for old records",()=>{
  expect(programmerIssue({...issue,repairTarget:"abya-source"})).toEqual({label:"运行异常 · 原因待确认",suspected:true});
  expect(programmerIssue({...issue,repairTarget:"abya-build"})?.label).toBe("运行异常 · 原因待确认");
  expect(programmerIssue({...issue,repairTarget:"abya-build",diagnosisStatus:"confirmed"})?.suspected).toBe(false);
  expect(programmerIssue(issue)?.suspected).toBe(true);
  expect(programmerIssue({...issue,repairTarget:"lua"})).toBeUndefined();
  expect(programmerIssue({...issue,description:"Lua 空引用",resumeWhen:"修复脚本"})).toBeUndefined();
  expect(programmerIssue({...issue,resumeWhen:"无需程序修复"})).toBeUndefined();
});
it("does not keep resolved or unrelated-stage problems in the active Lua alert",()=>{
  expect(activeProgrammerIssues([issue,{...issue,status:"resolved"},{...issue,affectedStages:["plan"]}])).toEqual([issue]);
});
