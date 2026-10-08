const zh = {
  revealDocument: "在文件资源管理器中显示", revealDocumentHint: "打开文档所在文件夹，并选中这份文档",
  openReportFolder: "打开报告所在文件夹，并选中对应报告文件",
  title: "制作流程", skills: "Skill 库", enable: "启用完整制作流程", enableHint: "完整玩法和较大改动使用此流程；局部任务可以继续使用终端。",
  ask: "允许需求追问", noAsk: "不做需求追问", questionMode: "需求提问", refresh: "刷新", loading: "正在读取…",
  version: "流程版本", round: "当前轮次", revision: "记录版本", cycle: "需求周期", noRecord: "尚未启用制作流程",
  confirm: "确认并继续", reject: "退回修改", feedback: "反馈（退回时请说明需要修改的内容）", document: "待确认文档",
  approved: "已确认此版本", rejected: "已退回", changed: "文档或条件已变化，请先刷新处理提示", reports: "阶段报告",
  planReport: "需求与方案", reviewReport: "开发与迭代", closeoutReport: "交付与复盘", issues: "问题与阻塞",
  noIssues: "暂无已登记问题", rounds: "迭代记录", noRounds: "尚未开始整体迭代", evidence: "证据", noEvidence: "暂无已登记证据",
  open: "打开", search: "搜索名称、用途或阶段", readSkill: "查看正文", select: "采用此模板", selected: "已选", scope: "当前任务固定版本",
  upgrade: "升级流程版本", upgradeConfirm: "升级会保留历史和旧 Skill 副本，重新确认适用需求与计划。请先停止本任务终端。继续升级？",
  newVersion: "有新的流程版本可用，当前任务仍使用原版本。", limit: "资料完整不等于实际体验通过；运行和策划验收仍需分别完成。",
  noSkills: "没有匹配的 Skill", back: "返回列表", source: "来源", verified: "已记录审阅", unreviewed: "未审阅", readonly: "只读查看",
  completed: "已完成", none: "未提供", continueHint: "决定已保存，请在此任务终端继续。", close: "关闭", fullTask: "完整制作 / 较大改动", scopedTask: "局部任务",
  taskMode: "任务范围", method: "制作阶段", details: "查看详情", knowledge: "知识与工具建议", acceptance: "确认历史",
};
const en: Record<keyof typeof zh, string> = {
  revealDocument: "Show in File Explorer", revealDocumentHint: "Open the document folder and select this document",
  openReportFolder: "Open the report folder and select the corresponding report file",
  title: "Production", skills: "Skills", enable: "Enable full production", enableHint: "Use for complete gameplay or major changes. Scoped tasks can continue in the terminal.",
  ask: "Allow requirements questions", noAsk: "No requirements follow-ups", questionMode: "Questions", refresh: "Refresh", loading: "Loading…",
  version: "Workflow version", round: "Current round", revision: "Record revision", cycle: "Requirements cycle", noRecord: "Production is not enabled",
  confirm: "Accept and continue", reject: "Request changes", feedback: "Feedback (describe changes when returning)", document: "Document for review",
  approved: "This version accepted", rejected: "Changes requested", changed: "Document or prerequisites changed. Resolve the warnings first.", reports: "Stage reports",
  planReport: "Requirements and plan", reviewReport: "Development and review", closeoutReport: "Delivery and learning", issues: "Issues and blockers",
  noIssues: "No recorded issues", rounds: "Review rounds", noRounds: "Whole-experience review has not started", evidence: "Evidence", noEvidence: "No recorded evidence",
  open: "Open", search: "Search name, purpose or stage", readSkill: "Read Skill", select: "Select template", selected: "Selected", scope: "Pinned task version",
  upgrade: "Upgrade workflow", upgradeConfirm: "Upgrade preserves history and a Skill backup, then requires updated requirements and plan approval. Stop task terminals first. Continue?",
  newVersion: "A new workflow is available. This task still uses its pinned version.", limit: "Complete records do not prove gameplay quality. Runtime checks and user acceptance remain separate.",
  noSkills: "No matching Skills", back: "Back to list", source: "Source", verified: "Review recorded", unreviewed: "Not reviewed", readonly: "Read only",
  completed: "Completed", none: "Not provided", continueHint: "Decision saved. Continue in this task's terminal.", close: "Close", fullTask: "Complete gameplay / major change", scopedTask: "Scoped task",
  taskMode: "Task scope", method: "Production stages", details: "Details", knowledge: "Knowledge and tooling", acceptance: "Decision history",
};
export const productionText = (locale?: string) => locale === "en-US" ? en : zh;
const stages: Record<string, string> = { requirements: "Requirements", resources: "Resources", plan: "Plan", implementation: "Implementation", review: "8+4 review", delivery: "Delivery", closeout: "Closeout" };
const statuses: Record<string, string> = { "not-started": "未开始", "in-progress": "进行中", "awaiting-confirmation": "待确认", blocked: "阻塞", passed: "已通过", closed: "已关闭", stopped: "已停止", open: "未关闭", resolved: "已解决", rejected: "已退回", accepted: "已确认", "not-applicable": "不适用" };
export const stageName = (id: string, fallback: string, locale?: string) => locale === "en-US" ? stages[id] ?? id : fallback;
export const statusName = (id: string, locale?: string) => locale === "en-US" ? id.replace(/-/g, " ") : statuses[id] ?? id;
