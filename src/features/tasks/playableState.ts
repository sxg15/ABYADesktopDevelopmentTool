import type { ProductionIssue, ProductionRecord } from "../../shared/production";

// 各试玩入口共享版本判断；启动前由后端核验包及真实文件。
export function playableState(r?: ProductionRecord) {
  const pack = r?.workbench?.execution?.verificationPackage;
  if (!r || !pack?.path || pack.status !== "core-verified") return { available: false, current: false };
  const current = pack.version === r.currentVersion;
  const tests = [...(r.selfTests ?? [])].reverse().filter(t => t.version === pack.version && t.cycle === r.cycle);
  const core = ["lua", "input", "core-loop", "save-reload"].every(id =>
    tests.find(t => t.id === (id === "input" && tests.some(c => c.id === "playable-input") ? "playable-input" : id))?.status === "passed");
  return { available: current ? core : typeof pack.sourceRevision === "number", current };
}

// 显示已记录的具体事实；确认状态与描述分开，不用占位句替换原始问题。
export function playableIssues(r: ProductionRecord, en = false) {
  const issues = (r.issues ?? []).filter(i => !["resolved", "closed"].includes(i.status) && i.kind !== "suggestion" && i.diagnosisStatus !== "excluded")
    .map(i => ({ id: i.id, confirmed: i.diagnosisStatus === "confirmed", ...issueCopy(i, en) }));
  const latest = new Map((r.selfTests ?? []).filter(t=>t.version===r.currentVersion&&t.cycle===r.cycle).map(t=>[t.id,t]));
  const names:Record<string,string>={input:"键盘、鼠标等操作",lifecycle:"暂停、返回与重开",visual:"画面与界面",lua:"游戏脚本",architecture:"玩法配置","core-loop":"完整一局游戏","save-reload":"保存与重新加载",authority:"多人状态同步"};
  for(const check of latest.values()) {
    if(!["failed","blocked"].includes(check.status)||check.blockedByIssueIds?.some(id=>issues.some(i=>i.id===id)))continue;
    issues.push({id:`check:${check.id}`,confirmed:false,title:en?`Check incomplete: ${check.id}`:`${names[check.id]??check.id}仍需验证`,
      description:check.observations?.trim() || (en?`No specific observations recorded for ${check.id}. AI must add reproduction steps and the observed result.`:`检查 ${check.id} 尚未记录具体表现，需补充复现步骤和实际结果。`)});
  }
  return issues;
}
function issueCopy(i: ProductionIssue, en: boolean) {
  const description = i.playerSummary?.trim() || i.description?.trim() || i.diagnosis?.reproduction?.trim() || i.executionHandling?.impact?.trim();
  const firstLine = description?.split(/[\r\n]/).find(line=>line.trim())?.trim();
  const title = i.title?.trim() || (firstLine ? (firstLine.length > 72 ? `${firstLine.slice(0,72)}…` : firstLine) : (en ? `Issue ${i.id}` : `问题 ${i.id}`));
  const impact = i.executionHandling?.impact?.trim();
  return { title, description: description
    ? `${description}${impact && !description.includes(impact) ? `\n${en?"Impact":"影响"}：${impact}` : ""}`
    : (en ? `Issue ${i.id} has no description. AI must add reproduction steps and the observed result.` : `问题 ${i.id} 缺少具体描述，需补充复现步骤和实际结果。`) };
}
