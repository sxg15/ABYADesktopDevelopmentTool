import { TriangleAlert } from "lucide-react";
import type { ProductionRecord } from "../../shared/production";
import { ProgrammerIssueExport } from "./ProgrammerIssueExport";
import "./playtestIssues.css";

export function confirmedSourceDefect(r: ProductionRecord, id: string) {
  const i = r.issues.find(i => i.id === id);
  return !!i && ["defect","blocker"].includes(i.kind) && i.status !== "resolved" && i.repairTarget === "abya-source" && i.diagnosisStatus === "confirmed"
    && !!i.diagnosis?.playerBuildHash && i.diagnosis.playerBuildHash === r.versionDetails?.playerBuildHash
    && [i.diagnosis.reproduction,i.diagnosis.comparison,i.diagnosis.rootCause].every(s => !!s?.trim())
    && !!i.evidenceIds?.length && i.evidenceIds.every(id => r.evidence.some(e => e.id === id && e.reviewed && e.version === r.currentVersion && e.cycle === r.cycle));
}

export function PlaytestIssues({record:r,en}:{record:ProductionRecord;en:boolean}) {
  const issues = r.issues.filter(i => i.kind !== "suggestion" && confirmedSourceDefect(r,i.id));
  const pending = r.issues.filter(i => i.status !== "resolved" && i.kind !== "suggestion" && !confirmedSourceDefect(r,i.id));
  if (!issues.length && !pending.length) return null;
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
    {!!pending.length && <div className="playtest-repair-note" role="status"><strong>{en?"Implementation still needs AI repair":"仍有问题需要 AI 在 Lua 阶段处理"} · {pending.length}</strong>
      <p>{en?"These records are not confirmed source defects. Upgrade this older task to recheck its handoff; do not treat them as accepted playtest gaps.":"这些记录尚不符合源码缺陷条件。旧任务升级流程后将重新核对并返回 Lua 修复；不能将它们视为已获准带入试玩的缺口。"}</p>
    </div>}
  </section>;
}
