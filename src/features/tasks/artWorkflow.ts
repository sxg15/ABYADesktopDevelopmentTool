import type { ProductionArtifact, ProductionFeedback, ProductionRecord, ProductionView } from "../../shared/production";

export const isArtFeedback = (f: ProductionFeedback) => f.stage === "resources" || !!f.artifactId;
export function artVisuals(record: ProductionRecord, artifacts = record.artifacts ?? []) {
  const all = artifacts.filter(a => a.stage === "resources" && a.mediaType && !a.plannerReference && a.sourceType !== "feedback");
  const groups = new Map<string, ProductionArtifact>();
  for (const a of all) if (a.cycle === undefined || a.cycle === record.cycle) groups.set(a.groupId || a.id, a);
  const current = [...groups.values()];
  return { current, history: all.filter(a => !current.some(v => v.id === a.id)).reverse() };
}
export function artRecheckReason(record: ProductionRecord, f: ProductionFeedback, actions?: Record<string,string>, en = false) {
  if (f.status !== "awaiting-recheck") return en ? "Wait for AI to submit the fix and recheck instructions." : "AI 尚未提交修复结果，请等待修改及复验说明。";
  if (actions && f.id in actions) return actions[f.id];
  const original = record.artifacts?.find(a => a.id === f.artifactId);
  const candidates = record.artifacts?.filter(a => original && a.groupId === original.groupId && a.cycle === record.cycle && !a.plannerReference && a.sourceType !== "feedback") ?? [];
  const latest = candidates[candidates.length - 1];
  if (!latest || latest.id !== f.candidateArtifactId) return en ? "AI must link this feedback to the latest revision before recheck." : "这条反馈尚未关联最新方案，请让 AI 更新复验结果后再处理。";
  if (latest.exists === false) return en ? "Recheck image is unavailable. Ask AI to restore it." : "复验图片不可用，请让 AI 补齐材料。";
  return "";
}
export function artDocument(view: ProductionView) {
  return view.record?.documents.resources ?? (!view.policy.stages.some(s => s.approval === "resources") ? view.record?.documents.plan : undefined);
}
