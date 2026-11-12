import type { ProductionIssue, ProductionRecord } from "../../shared/production";

export function issueObservation(issue: ProductionIssue): string {
  return issue.playerSummary?.trim() || issue.description?.trim() ||
    issue.diagnosis?.reproduction?.trim() || issue.executionHandling?.impact?.trim() || "";
}

export function outstandingChecks(record: ProductionRecord, shownIssues: ReadonlySet<string>) {
  const latest = new Map<string, NonNullable<ProductionRecord["selfTests"]>[number]>();
  if (!record.currentVersion) return [];
  for (const check of record.selfTests ?? []) {
    if (check.version === record.currentVersion && check.cycle === record.cycle) {
      latest.set(check.id, check);
    }
  }
  return Array.from(latest.values()).filter(check =>
    (check.status === "failed" || check.status === "blocked") &&
    !check.blockedByIssueIds?.some(id => shownIssues.has(id))
  );
}
