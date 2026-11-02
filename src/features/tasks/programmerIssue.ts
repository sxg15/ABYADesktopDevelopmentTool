import type {ProductionIssue} from "../../shared/production";

export function affectsLua(issue: ProductionIssue) {
  return issue.affectedStages ? issue.affectedStages.includes("implementation")
    : issue.stage === "implementation" || issue.kind === "blocker";
}

export function programmerIssue(issue: ProductionIssue, en = false) {
  if(issue.diagnosisStatus==="excluded")return undefined;
  if (issue.repairTarget === "abya-source" || issue.repairTarget === "abya-build") {
    const confirmed=issue.diagnosisStatus==="confirmed";
    return {suspected:!confirmed,label:confirmed?(en?"Confirmed platform issue":"已确认平台问题"):(en?"Runtime issue · cause unconfirmed":"运行异常 · 原因待确认")};
  }
  if (issue.repairTarget) return undefined;
  // Older pinned tasks have no structured target. Only explicit programmer handoff
  // language plus engine/build context qualifies, and it remains labelled suspected.
  const text = [issue.title,issue.description,issue.fix,issue.resumeWhen].filter(Boolean).join("\n");
  const context = /源码|构建包|引擎|底层|\b(?:ABYA|Player|engine|build)\b/i.test(text);
  const handoff = /(?:需要|需|等待|交由|请).{0,12}(?:程序|开发).{0,12}(?:修复|排查)|(?:programmer|developer).{0,16}(?:fix|investigat)/i.test(text);
  const negated = /(?:无需|不需要|不必).{0,6}(?:程序|开发)/.test(text);
  if (context && handoff && !negated) return {suspected:true,label:en ? "Suspected ABYA source/build defect" : "疑似 ABYA 源码／构建包缺陷"};
  return undefined;
}

export function activeProgrammerIssues(issues: ProductionIssue[]) {
  return issues.filter(i => i.status !== "resolved" && affectsLua(i) && programmerIssue(i));
}
