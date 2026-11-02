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

// 未验证不等于确诊；原始问题留在技术详情中。
export function playableIssues(r: ProductionRecord, en = false) {
  const issues = (r.issues ?? []).filter(i => !["resolved", "closed"].includes(i.status) && i.kind !== "suggestion" && i.diagnosisStatus !== "excluded")
    .map(i => ({ id: i.id, confirmed: i.diagnosisStatus === "confirmed", ...issueCopy(i, en) }));
  const latest = new Map((r.selfTests ?? []).filter(t=>t.version===r.currentVersion&&t.cycle===r.cycle).map(t=>[t.id,t]));
  const names:Record<string,string>={input:"键盘、鼠标等操作",lifecycle:"暂停、返回与重开",visual:"画面与界面",lua:"游戏脚本",architecture:"玩法配置","core-loop":"完整一局游戏","save-reload":"保存与重新加载",authority:"多人状态同步"};
  for(const check of latest.values()) {
    if(!["failed","blocked"].includes(check.status)||check.blockedByIssueIds?.some(id=>issues.some(i=>i.id===id)))continue;
    issues.push({id:`check:${check.id}`,confirmed:false,title:en?"Gameplay check incomplete":`${names[check.id]??"一项玩法表现"}仍需验证`,
      description:en?"This check has not passed. AI must establish the cause and impact; details remain in the verification record.":"这项检查尚未通过，AI 还需确认原因和实际影响；具体记录保留在技术详情中。"});
  }
  return issues;
}
function issueCopy(i: ProductionIssue, en: boolean) {
  if (i.playerSummary) return { title: i.title || (en ? "Gameplay issue" : "试玩问题"), description: i.playerSummary };
  if (/escape|\besc\b/i.test(`${i.title ?? ""} ${i.description}`)) return {
    title: en ? "Esc pause and resume" : "Esc 暂停与恢复",
    description: i.diagnosisStatus === "confirmed"
      ? (en ? "Pausing with Esc or restoring controls has a confirmed issue. See technical details." : "使用 Esc 暂停或恢复操作存在已确认的问题，具体表现见技术详情。")
      : (en ? "Keyboard Esc pause, possible platform-menu overlap, and restoring controls still need verification." : "按下键盘 Esc 后，游戏能否正确暂停、是否同时弹出平台菜单，以及继续后能否正常操作，还没有完成验证。"),
  };
  return { title: i.title || (en ? "Gameplay behavior needs checking" : "有一项游戏表现需要核对"),
    description: i.executionHandling?.impact || (en ? "AI will verify the impact. The original report remains in technical details." : "AI 还需确认对试玩的具体影响，原始记录保留在技术详情中。") };
}
