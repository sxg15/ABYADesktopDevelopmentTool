import { TriangleAlert } from "lucide-react";
import type { ProductionRecord } from "../../shared/production";
import { ProgrammerIssueExport } from "./ProgrammerIssueExport";
import { issueObservation, outstandingChecks } from "./playtestObservations";
import "./playtestIssues.css";

export function confirmedSourceDefect(r: ProductionRecord, id: string) {
  const i = r.issues.find(i => i.id === id);
  return !!i && ["defect","blocker"].includes(i.kind) && !["resolved", "closed"].includes(i.status) && i.repairTarget === "abya-source" && i.diagnosisStatus === "confirmed"
    && !!i.diagnosis?.playerBuildHash && i.diagnosis.playerBuildHash === r.versionDetails?.playerBuildHash
    && [i.diagnosis.reproduction,i.diagnosis.comparison,i.diagnosis.rootCause].every(s => !!s?.trim())
    && !!i.evidenceIds?.length && i.evidenceIds.every(id => r.evidence.some(e => e.id === id && e.reviewed && e.version === r.currentVersion && e.cycle === r.cycle));
}

export function PlaytestIssues({record:r,en}:{record:ProductionRecord;en:boolean}) {
  const issues = r.issues.filter(i => i.kind !== "suggestion" && confirmedSourceDefect(r,i.id));
  const pending = r.issues.filter(i => !["resolved", "closed"].includes(i.status) && i.diagnosisStatus !== "excluded" && i.kind !== "suggestion" && !confirmedSourceDefect(r,i.id));
  const checks = outstandingChecks(r, new Set([...issues, ...pending].map(issue => issue.id)));
  if (!issues.length && !pending.length && !checks.length) return null;
  return <section className="playtest-issues" aria-label={en?"Playtest issues":"试玩问题"}>
    {!!issues.length && <><h2><TriangleAlert size={20} aria-hidden="true"/>{en?"Known Abya-PB source defects":"已知 Abya-PB 源码缺陷"} · {issues.length}</h2>
      <p>{en?"These defects remain open. Check the affected features while testing; export a diagnostic package for the developer.":"以下缺陷尚未修复。试玩时请注意影响范围，可直接导出诊断资料交给程序定位。"}</p>
      {issues.map(i => <article className="playtest-defect" key={i.id} aria-label={i.title || i.id}>
        <h3>{i.title || i.id}</h3><p>{i.description}</p>
        <dl><dt>{en?"Impact":"影响范围"}</dt><dd>{i.executionHandling?.impact || i.resumeWhen || (en?"See reproduction details below.":"详见下方复现说明。")}</dd>
          <dt>{en?"Workaround / attempted repair":"替代方案／已尝试处理"}</dt><dd>{i.executionHandling?.reason || i.fix || (en?"No verified workaround recorded.":"尚无已验证的替代方案。")}</dd></dl>
        <ProgrammerIssueExport taskId={r.taskId} revision={r.revision} issueIds={[i.id]} en={en} diagnostic/>
        <details><summary>{en?"Reproduction and diagnosis":"复现与诊断详情"}</summary><dl>
          <dt>{en?"Reproduction":"复现步骤"}</dt><dd>{i.diagnosis?.reproduction}</dd>
          <dt>{en?"Comparison":"对照结果"}</dt><dd>{i.diagnosis?.comparison}</dd>
          <dt>{en?"Source cause":"源码根因"}</dt><dd>{i.diagnosis?.rootCause}</dd>
          <dt>{en?"Version":"对应版本"}</dt><dd>{r.currentVersion} · {i.id}</dd>
        </dl></details>
      </article>)}</>}
    {!!(pending.length || checks.length) && <section className="playtest-repair-note">
      <h3>{en ? "Open playtest items" : "试玩待核对项"} · {pending.length + checks.length}</h3>
      {!!pending.length && <p>{en ? "AI can continue checking these items in the implementation stage. Older tasks can recheck the handoff after a workflow upgrade." : "AI 可在 Lua 实现阶段继续核对这些问题；旧任务升级流程后可重新核对试玩交接。"}</p>}
      <ul>{pending.map(issue => <li key={issue.id}><strong>{issue.title?.trim() || issue.id}</strong>
        <p>{issueObservation(issue) || (en ? "No observation recorded." : "尚未登记具体表现。")}</p></li>)}
      {checks.map(check => <li key={check.id}><strong>{check.id}</strong>
        <p>{check.observations?.trim() || (en ? "No observation recorded." : "尚未登记具体表现。")}</p></li>)}</ul>
    </section>}
  </section>;
}
