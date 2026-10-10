import { useEffect, useRef, useState } from "react";
import { Check, FileText, FolderOpen } from "lucide-react";
import {
  productionApi,
  type ProductionView,
  type ProductionDocument,
  type ProductionIssue,
  type IntakeQuestionGroup,
} from "../../shared/production";
import { errorMessage } from "../../shared/api";
import { productionText, stageName, statusName } from "./productionText";
import "./production.css";
import { ProductionTiming } from "./ProductionTiming";
import { AcceptanceLauncher } from "./AcceptanceLauncher";
import { VisualGallery } from "./VisualGallery";
import { FeedbackPanel } from "./FeedbackPanel";

const names: Record<string, string> = {
  requirements: "需求整理",
  resources: "资源与美术",
  plan: "方案与能力预检",
  implementation: "Lua 实现与完整初版",
  review: "8＋4 整体迭代",
  delivery: "交付与策划验收",
  closeout: "收尾与知识分析",
};
const hints: Record<string, string> = {
  requirements: "敲定玩法、人数和规则，整理需求文档。",
  resources: "检查现有素材与模板，列出需要补齐的资源。",
  plan: "验证制作环境，整理具体做法与检查安排。",
  implementation: "制作可以实际游玩的完整初版。",
  review: "逐轮检查体验，记录修改和复测结果。",
  delivery: "提交作品与报告，等待策划试玩和反馈。",
  closeout: "整理制作过程、用时和可复用经验。",
};
const shortNames: Record<string, string> = {
  requirements: "需求整理",
  resources: "资源美术",
  plan: "方案预检",
  implementation: "Lua 初版",
  review: "8＋4 迭代",
  delivery: "交付验收",
  closeout: "收尾复盘",
};
const affected = (i: ProductionIssue, stage: string) =>
  i.affectedStages
    ? i.affectedStages.includes(stage)
    : i.stage === stage || (i.kind === "blocker" && i.status !== "resolved");
function remembered(taskId: string) {
  try {
    return sessionStorage.getItem(`abya:production-stage:${taskId}`) ?? "";
  } catch {
    return "";
  }
}
function time(value: string) {
  const d = new Date(value);
  return Number.isNaN(d.getTime()) ? value : d.toLocaleString();
}

export function ProductionPanel({
  taskId,
  active,
  locale,
  onContinue,
}: {
  taskId: string;
  active: boolean;
  locale?: string;
  onContinue?: (
    message: string,
    group?: IntakeQuestionGroup,
    afterRevision?: number,
  ) => Promise<void>;
}) {
  const s = productionText(locale),
    en = locale === "en-US";
  const [view, setView] = useState<ProductionView>();
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [question, setQuestion] = useState("ask");
  const [playerMode, setPlayerMode] = useState("unspecified");
  const [editingPlayers, setEditingPlayers] = useState(false);
  const [feedback, setFeedback] = useState<Record<string, string>>({});
  const [selected, setSelected] = useState(() => remembered(taskId));
  const [visualFeedback, setVisualFeedback] = useState<string>();
  const sequence = useRef(0),
    alive = useRef(true),
    mutating = useRef(false);
  async function refresh() {
    if (mutating.current) return;
    const request = ++sequence.current;
    try {
      const result = await productionApi.get(taskId);
      if (alive.current && request === sequence.current) setView(result);
    } catch (e) {
      if (alive.current && request === sequence.current)
        setError(errorMessage(e));
    }
  }
  useEffect(() => {
    alive.current = true;
    setSelected(remembered(taskId));
    setEditingPlayers(false);
    setFeedback({});
    setError("");
    setNotice("");
    void refresh();
    const timer = window.setInterval(() => void refresh(), 4000);
    return () => {
      alive.current = false;
      sequence.current++;
      window.clearInterval(timer);
    };
  }, [taskId]);
  async function change(run: () => Promise<ProductionView>) {
    setBusy(true);
    mutating.current = true;
    setError("");
    sequence.current++;
    try {
      const result = await run();
      sequence.current++;
      if (alive.current) setView(result);
    } catch (e) {
      if (alive.current) setError(errorMessage(e));
    } finally {
      mutating.current = false;
      if (alive.current) {
        setBusy(false);
        void refresh();
      }
    }
  }
  const r = view?.record?.taskId === taskId ? view.record : undefined;
  const stage = view?.policy.stages.some((x) => x.id === selected)
    ? selected
    : (r?.currentStage ?? "requirements");
  const modern = view?.policy.iterationMode === "human-feedback";
  const label = (id: string) => modern && id === "review" ? (en ? "Playtest and iteration" : "人工验收与修改迭代") : stageName(id, view?.policy.stages.find(s => s.id === id)?.name ?? names[id] ?? id, locale);
  const issues =
    r?.issues.filter(
      (i) =>
        affected(i, stage) &&
        (r.stages[stage] !== "passed" || i.stage === stage),
    ) ?? [];
  const openIssues = issues.filter((i) => i.status !== "resolved");
  const groups =
    r?.questionGroups?.filter((g) => (g.stage ?? "requirements") === stage) ??
    [];
  const states = view?.stageStatuses ?? r?.stages ?? {};
  function select(id: string) {
    setSelected(id);
    try {
      sessionStorage.setItem(`abya:production-stage:${taskId}`, id);
    } catch {
      /* Selection works without storage. */
    }
  }
  async function decide(d: ProductionDocument, accepted: boolean) {
    if (!r) return;
    await change(async () => {
      const result = await productionApi.decide(
        taskId,
        r.revision,
        d.kind,
        d.sha256,
        accepted,
        feedback[d.kind] ?? "",
      );
      setFeedback((v) => ({ ...v, [d.kind]: "" }));
      try {
        await onContinue?.(
          `${label(d.kind)}文档第${d.revision}版已${accepted ? "确认" : "退回修改"}。请读取最新记录和反馈，继续当前阶段。`,
          undefined,
          result.record?.revision,
        );
        setNotice(
          en
            ? "Decision saved. See conversation steps for progress."
            : "决定已保存，后续进展会显示在顶部对话步骤中。",
        );
      } catch (e) {
        setNotice(
          `${en ? "Decision saved" : "决定已保存"}；${errorMessage(e)}`,
        );
      }
      return result;
    });
  }
  async function reveal(path: string) {
    setError("");
    try {
      await productionApi.revealDocument(taskId, path);
    } catch (e) {
      setError(errorMessage(e));
    }
  }
  const issueCard = (i: ProductionIssue) => (
    <article className={`production-issue-card issue-${i.status}`} key={i.id}>
      <div className="production-card-heading">
        <strong>{i.title || i.description.split(/[。\n]/)[0]}</strong>
        <span className={`production-status status-${i.status}`}>
          {i.status === "open"
            ? en
              ? "To do"
              : "待处理"
            : statusName(i.status, locale)}
        </span>
      </div>
      {i.stage !== stage && (
        <small>
          {en ? "From" : "来自"} {label(i.stage)}
        </small>
      )}
      <dl>
        <dt>{en ? "What happened" : "问题描述"}</dt>
        <dd>{i.description}</dd>
        <dt>{en ? "Next step" : "处理方案"}</dt>
        <dd>
          {i.fix ||
            i.resumeWhen ||
            (en ? "A solution is being investigated." : "正在确认处理方式。")}
        </dd>
        {i.recheck && (
          <>
            <dt>{en ? "Recheck" : "复验结果"}</dt>
            <dd>{i.recheck}</dd>
          </>
        )}
      </dl>
      <details>
        <summary>{en ? "Technical details" : "查看详细记录"}</summary>
        <p>{i.id}</p>
        {i.evidenceIds?.map((id) => {
          const e = r?.evidence.find((e) => e.id === id);
          return e ? (
            <button
              key={id}
              className="text-button"
              onClick={() => void reveal(e.path)}
            >
              {e.description || id}
            </button>
          ) : (
            <p key={id}>{id}</p>
          );
        })}
      </details>
    </article>
  );
  return (
    <div className="production-panel">
      {r && view && (
        <nav className="production-stage-nav" aria-label={s.method}>
          {view.policy.stages.map((item, index) => {
            const count =
              r.issues.filter(
                (i) =>
                  i.status !== "resolved" &&
                  affected(i, item.id) &&
                  (states[item.id] !== "passed" || i.stage === item.id),
              ).length +
              (r.questionGroups ?? []).filter(
                (g) =>
                  (g.stage ?? "requirements") === item.id &&
                  g.status === "pending",
              ).length;
            return (
              <button
                key={item.id}
                aria-pressed={stage === item.id}
                aria-label={`${index + 1} ${label(item.id)} ${statusName(states[item.id], locale)}`}
                title={label(item.id)}
                className={`production-stage-button ${stage === item.id ? "selected" : ""}`}
                onClick={() => select(item.id)}
              >
                <span className="production-stage-number">
                  {states[item.id] === "passed" ? (
                    <Check size={14} />
                  ) : (
                    index + 1
                  )}
                </span>
                <span>
                  <strong>
                    {en
                      ? label(item.id)
                      : (modern && item.id === "review" ? "试玩与修改" : modern && item.id === "implementation" ? "Lua 实现" : shortNames[item.id] ?? label(item.id))}
                  </strong>
                  <small>
                    {statusName(states[item.id], locale)}
                    {count > 0 && ` · ${count}${en ? " pending" : "项待处理"}`}
                  </small>
                  {r.currentStage === item.id && (
                    <em>{en ? "Current stage" : "当前执行阶段"}</em>
                  )}
                </span>
              </button>
            );
          })}
        </nav>
      )}
      <header className="production-heading">
        <h3>{r ? label(stage) : s.title}</h3>
        <button
          className="secondary-button"
          onClick={() => void refresh()}
          disabled={busy}
        >
          {s.refresh}
        </button>
      </header>
      {error && (
        <div role="alert" className="inline-error">
          {error}
        </div>
      )}
      {notice && <p role="status">{notice}</p>}
      {!view ? (
        <p>{s.loading}</p>
      ) : !r ? (
        <section className="production-card">
          <h4>{s.noRecord}</h4>
          <p>{s.enableHint}</p>
          <label className="field">
            {s.questionMode}
            <select
              value={question}
              onChange={(e) => setQuestion(e.target.value)}
            >
              <option value="ask">{s.ask}</option>
              <option value="no-followup">{s.noAsk}</option>
            </select>
          </label>
          <label className="field">
            {en ? "Players" : "玩法人数"}
            <select
              value={playerMode}
              onChange={(e) => setPlayerMode(e.target.value)}
            >
              <option value="unspecified">
                {en ? "Undecided" : "需求阶段敲定"}
              </option>
              <option value="single">{en ? "Single player" : "单人"}</option>
              <option value="multiplayer">{en ? "Multiplayer" : "多人"}</option>
            </select>
          </label>
          <button
            className="primary-button"
            disabled={busy || !active}
            onClick={() =>
              void change(() =>
                productionApi.update(taskId, 0, "initialize", {
                  questionMode: question,
                  playerMode,
                }),
              )
            }
          >
            {s.enable}
          </button>
        </section>
      ) : (
        <>
          <section
            className={`production-overview ${openIssues.length ? "has-issues" : ""}`}
          >
            <span className={`production-status status-${states[stage]}`}>
              {statusName(states[stage], locale)}
            </span>
            <p>
              {r.stageUpdates?.[stage]?.summary ||
                (en
                  ? `Progress and results for ${label(stage)}.`
                  : modern && stage === "review" ? "直接试玩并提出问题，AI修改后复验，直到达到预期。" : hints[stage])}
            </p>
            <small>
              {r.stageUpdates?.[stage]?.nextAction ||
                (openIssues.length
                  ? en
                    ? `${openIssues.length} items need attention below.`
                    : `有${openIssues.length}项问题需要处理，见下方处理方案。`
                  : groups.some((g) => g.status === "pending")
                    ? en
                      ? "Please answer the questions below."
                      : "请完成下方阶段问答，提交后继续。"
                    : states[stage] === "awaiting-confirmation"
                      ? en
                        ? "Read the document and confirm or request changes."
                        : "请阅读本阶段文档，确认或填写修改意见。"
                      : en
                        ? "Latest saved progress is shown here."
                        : "成果和进展会随制作过程更新。")}
            </small>
          </section>
          {view.warnings
            .filter((w) => w.startsWith(stage))
            .map((w) => (
              <div className="production-warning" key={w}>
                {w}
              </div>
            ))}
          <section className="production-card">
            <h4>
              <FileText size={17} />
              {en ? "Results and reports" : "阶段成果与报告"}
            </h4>
            {Object.values(r.documents)
              .filter((d) => d.kind === stage)
              .map((d) => {
                const decision = [...r.approvals]
                  .reverse()
                  .find(
                    (a) =>
                      a.kind === d.kind && a.documentRevision === d.revision,
                  );
                const pending = r.stages[d.kind] === "awaiting-confirmation";
                return (
                  <article className="production-result" key={d.kind}>
                    <div className="production-card-heading">
                      <strong>
                        {label(d.kind)} · {en ? "Version " : "第"}
                        {d.revision}
                        {en ? "" : "版"}
                      </strong>
                      <span className="production-status">
                        {decision
                          ? statusName(decision.decision, locale)
                          : s.document}
                      </span>
                    </div>
                    <small>
                      {d.path} · {time(d.submittedAt)}
                    </small>
                    <button
                      className="secondary-button"
                      title={s.revealDocumentHint}
                      onClick={() => void reveal(d.path)}
                    >
                      <FolderOpen size={15} />
                      {s.revealDocument}
                    </button>
                    {pending && (
                      <div className="production-approval">
                        <label className="field">
                          {s.feedback}
                          <textarea
                            value={feedback[d.kind] ?? ""}
                            onChange={(e) =>
                              setFeedback((v) => ({
                                ...v,
                                [d.kind]: e.target.value,
                              }))
                            }
                            rows={3}
                          />
                        </label>
                        <div className="production-actions">
                          <button
                            className="primary-button"
                            disabled={
                              busy ||
                              !active ||
                              view.warnings.some((w) => w.startsWith(d.kind))
                            }
                            onClick={() => void decide(d, true)}
                          >
                            {s.confirm}
                          </button>
                          <button
                            className="secondary-button"
                            disabled={
                              busy || !active || !feedback[d.kind]?.trim()
                            }
                            onClick={() => void decide(d, false)}
                          >
                            {s.reject}
                          </button>
                        </div>
                      </div>
                    )}
                  </article>
                );
              })}
            {(view.artifacts ?? r.artifacts ?? [])
              .filter(
                (a) =>
                  a.stage === stage && !a.mediaType &&
                  !Object.values(r.documents).some((d) => d.path === a.path),
              )
              .map((a) => (
                <article className="production-result" key={a.id}>
                  <div className="production-card-heading">
                    <strong>{a.title}</strong>
                    <span className="production-status">
                      {en ? "Draft" : "草稿"}
                    </span>
                  </div>
                  {a.summary && <p>{a.summary}</p>}
                  <small>
                    {a.path}
                    {a.updatedAt && ` · ${time(a.updatedAt)}`}
                  </small>
                  <button
                    className="secondary-button"
                    disabled={a.exists === false}
                    onClick={() => void reveal(a.path)}
                  >
                    <FolderOpen size={15} />
                    {a.exists === false
                      ? en
                        ? "File unavailable"
                        : "文件暂不可用"
                      : s.revealDocument}
                  </button>
                </article>
              ))}
            <button
              className="text-button"
              onClick={() =>
                void productionApi
                  .revealReport(
                    taskId,
                    ["requirements", "resources", "plan"].includes(stage)
                      ? "plan"
                      : ["implementation", "review"].includes(stage)
                        ? "review"
                        : "closeout",
                  )
                  .catch((e) => setError(errorMessage(e)))
              }
            >
              {en ? "Open combined stage report" : "打开相关阶段汇总报告"}
            </button>
          </section>
          <section className="production-card">
            <h4>
              {s.issues}
              <span className="production-count">{openIssues.length}</span>
            </h4>
            {!issues.length ? (
              <p className="production-empty">
                {en
                  ? "No issues recorded for this stage."
                  : "本阶段暂无已登记问题。"}
              </p>
            ) : (
              <>
                {openIssues.map(issueCard)}
                {issues.some((i) => i.status === "resolved") && (
                  <details>
                    <summary>
                      {en ? "Resolved issues" : "已解决的问题"} ·{" "}
                      {issues.filter((i) => i.status === "resolved").length}
                    </summary>
                    {issues
                      .filter((i) => i.status === "resolved")
                      .map(issueCard)}
                  </details>
                )}
              </>
            )}
          </section>
          <section className="production-card">
            <h4>{en ? "Stage questions" : "阶段问答"}</h4>
            {!groups.length && (
              <p className="production-empty">
                {en
                  ? "No questions for this stage."
                  : "本阶段暂无需要回答的问题。"}
              </p>
            )}
            {groups.map((group) => (
              <article className="production-question" key={group.id}>
                <div>
                  <strong>{group.title}</strong>
                  <small>
                    {group.status === "pending"
                      ? en
                        ? "Awaiting answers"
                        : "待回答"
                      : group.status === "submitted"
                        ? en
                          ? "Submitted"
                          : "已提交"
                        : en
                          ? "Cancelled"
                          : "已取消"}{" "}
                    · {group.questions.length}
                    {en ? " questions" : "题"}
                  </small>
                </div>
                <button
                  className={
                    group.status === "pending"
                      ? "primary-button"
                      : "secondary-button"
                  }
                  onClick={() =>
                    window.dispatchEvent(
                      new CustomEvent("abya:open-intake", {
                        detail: { taskId, groupId: group.id },
                      }),
                    )
                  }
                >
                  {group.status === "pending"
                    ? en
                      ? "Answer questions"
                      : "继续填写"
                    : en
                      ? "View / edit answers"
                      : "查看 / 修改答案"}
                </button>
              </article>
            ))}
          </section>
          {stage === "requirements" && (
            <section className="production-card">
              <h4>{en ? "Gameplay settings" : "玩法设置"}</h4>
              <div className="production-card-heading">
                <p>
                  {en ? "Players" : "玩法人数"}：
                  {r.playerMode === "multiplayer"
                    ? en
                      ? "Multiplayer"
                      : "多人"
                    : r.playerMode === "single"
                      ? en
                        ? "Single player"
                        : "单人"
                      : en
                        ? "To be decided"
                        : "待敲定"}
                </p>
                <button
                  className="text-button"
                  disabled={!active}
                  onClick={() => setEditingPlayers((v) => !v)}
                >
                  {en ? "Change requirements" : "修改需求"}
                </button>
              </div>
              {editingPlayers && (
                <>
                  <label className="field">
                    {en ? "Player mode" : "人数模式"}
                    <select
                      value={r.playerMode ?? "unspecified"}
                      disabled={busy || !active}
                      onChange={(e) =>
                        void change(() =>
                          productionApi.answer(
                            taskId,
                            r.revision,
                            "set-player-mode",
                            { playerMode: e.target.value },
                          ),
                        )
                      }
                    >
                      <option value="unspecified">
                        {en ? "Undecided" : "待敲定"}
                      </option>
                      <option value="single">
                        {en ? "Single player" : "单人"}
                      </option>
                      <option value="multiplayer">
                        {en ? "Multiplayer" : "多人"}
                      </option>
                    </select>
                  </label>
                  <small>
                    {en
                      ? "Changing players reopens the affected requirements."
                      : "更改人数后，会重新整理受影响的需求和方案。"}
                  </small>
                </>
              )}
            </section>
          )}
          {stage === "resources" && (
            <section className="production-card">
              <h4>{en ? "Selected templates" : "已选模板"}</h4>
              <p>
                {en ? "Workflow" : "制作模板"}：
                {r.skillPins.find((p) => p.id === r.taskTemplate)?.name ??
                  r.taskTemplate ??
                  (en ? "Not selected" : "尚未选择")}
              </p>
              <p>
                {en ? "Art" : "美术模板"}：
                {r.skillPins.find((p) => p.id === r.artTemplate)?.name ??
                  r.artTemplate ??
                  (en ? "Not selected" : "尚未选择")}
              </p>
            </section>
          )}
          {["review", "delivery"].includes(stage) && <AcceptanceLauncher key={taskId} taskId={taskId} active={active} en={en} />}
          {["resources", "plan", "review", "delivery"].includes(stage) && <VisualGallery taskId={taskId}
            artifacts={(view.artifacts ?? r.artifacts ?? []).filter(a => a.stage === "resources" || a.sourceType === "gameplay" || a.sourceType === "feedback")}
            bindings={r.documents.plan?.artifactBindings} en={en} onFeedback={active ? setVisualFeedback : undefined} />}
          {["resources", "plan", "review", "delivery"].includes(stage) && <FeedbackPanel key={taskId} taskId={taskId} record={r} active={active} en={en}
            selectedArtifact={visualFeedback} onUpdated={setView} onContinue={async (message, revision) => onContinue?.(message, undefined, revision)} />}
          {stage === "review" && modern && <section className="production-card"><h4>{en ? "Required checks" : "当前版本必要自测"}</h4>
            {(r.selfTests ?? []).filter(c => c.version === r.currentVersion).map((c, i) => <p key={i}>{c.id} · {statusName(c.status, locale)} · {c.observations}</p>)}
            <p>{en ? "Play, give feedback, and accept the final candidate when satisfied. There is no fixed round count." : "现在可以试玩并反馈；修改后复验，满意后确认交付。没有固定迭代轮数。"}</p></section>}
          {stage === "review" && (!modern || r.rounds.length > 0) && (
            <section className="production-card">
              <h4>
                {modern ? (en ? "Legacy review history" : "旧流程轮次（历史）") : `${s.rounds} · R${r.currentRound}`}
              </h4>
              {!r.rounds.length && <p>{s.noRounds}</p>}
              {r.rounds.map((round) => (
                <details key={`${round.cycle}-${round.number}`}>
                  <summary>
                    {s.cycle}
                    {round.cycle} · R{round.number} ·{" "}
                    {statusName(round.status, locale)}
                  </summary>
                  <p>
                    {round.inputVersion ?? s.none} →{" "}
                    {round.outputVersion ?? s.none}
                  </p>
                  {round.checks?.map((c) => (
                    <p key={c.dimensionId}>
                      {view.policy.dimensions.find(
                        (d) => d.id === c.dimensionId,
                      )?.name ?? c.dimensionId}{" "}
                      · {statusName(c.status, locale)} · {c.observations}
                    </p>
                  ))}
                  {round.questions?.map((q) => (
                    <article key={q.questionId}>
                      <strong>
                        {view.policy.questions.find(
                          (p) => p.id === q.questionId,
                        )?.text ?? q.questionId}
                      </strong>
                      <p>{q.conclusion}</p>
                      <p>{q.problem}</p>
                      <p>{q.change}</p>
                      <p>{q.recheck}</p>
                    </article>
                  ))}
                </details>
              ))}
            </section>
          )}
          {stage === "implementation" && (
            <section className="production-card">
              <h4>{en ? "Current build" : "当前作品版本"}</h4>
              <p>
                {r.currentVersion ??
                  (en
                    ? "The initial playable build has not been registered."
                    : "尚未登记可玩的初版。")}
              </p>
              <small>
                {en
                  ? "Open the Game instances tab to play."
                  : "可在“游戏实例”页打开本任务的游戏。"}
              </small>
            </section>
          )}
          {stage === "closeout" && <ProductionTiming taskId={taskId} en={en} />}
          {stage === "closeout" && (
            <section className="production-card">
              <h4>{s.knowledge}</h4>
              {r.knowledge.length ? (
                r.knowledge.map((k, i) => <p key={i}>{k.summary}</p>)
              ) : (
                <p className="production-empty">
                  {en
                    ? "No retrospective yet."
                    : "制作结束后在这里整理经验与改进建议。"}
                </p>
              )}
              <h4>{en ? "Stage timeline" : "阶段时间记录"}</h4>
              {!r.events?.length && (
                <p>
                  {en
                    ? "No recorded timing events yet."
                    : "旧任务尚无完整计时事件，后续阶段变化将自动记录。"}
                </p>
              )}
              {r.events?.map((e, i) => (
                <p key={i}>
                  <small>{time(e.at)}</small> {label(e.stage)} ·{" "}
                  {statusName(e.status, locale)}
                </p>
              ))}
            </section>
          )}
          <details className="production-card">
            <summary>
              {en ? "Stage records and evidence" : "阶段记录与证据"}
            </summary>
            {r.evidence
              .filter(
                (e) =>
                  e.stage === stage ||
                  (!e.stage && stage === "closeout") ||
                  issues.some((i) => i.evidenceIds?.includes(e.id)) ||
                  (stage === "review" &&
                    r.rounds.some((x) => x.evidenceIds?.includes(e.id))),
              )
              .map((e) => (
                <div className="production-evidence" key={e.id}>
                  <span>
                    {e.description}
                    <small>{e.reviewed ? s.verified : s.unreviewed}</small>
                  </span>
                  <button
                    className="text-button"
                    onClick={() => void reveal(e.path)}
                  >
                    {s.revealDocument}
                  </button>
                </div>
              ))}
            {r.approvals
              .filter((a) => a.kind === stage)
              .map((a, i) => (
                <p key={i}>
                  {en ? "Version " : "第"}
                  {a.documentRevision}
                  {en ? "" : "版"} · {statusName(a.decision, locale)} ·{" "}
                  {time(a.decidedAt)}
                  {a.feedback && ` · ${a.feedback}`}
                </p>
              ))}
          </details>
          <details className="production-card production-meta">
            <summary>{en ? "Workflow details" : "流程详情"}</summary>
            <p>
              {s.version} {r.workflowVersion} · {s.revision} {r.revision} ·{" "}
              {s.cycle} {r.cycle}
            </p>
            {view.availableUpdate && (
              <p>
                {s.newVersion}
                <button
                  className="text-button"
                  disabled={busy || !active}
                  onClick={() => {
                    if (window.confirm(s.upgradeConfirm))
                      void change(() =>
                        productionApi.upgrade(taskId, r.revision),
                      );
                  }}
                >
                  {s.upgrade}
                </button>
              </p>
            )}
            {view.warnings
              .filter((w) => !w.startsWith(stage))
              .map((w) => (
                <p key={w}>{w}</p>
              ))}
          </details>
        </>
      )}
    </div>
  );
}
