import { useEffect, useRef, useState } from "react";
import { productionApi, type IntakeQuestionGroup, type ProductionView, type IntakeContinuation, continuationMessages } from "../../shared/production";
import { errorMessage } from "../../shared/api";

export function IntakeQuestions({ taskId, group, revision, active, onSaved, onContinue, onEnsureConnection, onContinuation, onSubmit }: {
  taskId: string; group: IntakeQuestionGroup; revision: number; active: boolean;
  onSaved: (view: ProductionView) => void;
  onContinue?: (message: string, group: IntakeQuestionGroup) => Promise<void>;
  onEnsureConnection?: (group: IntakeQuestionGroup) => Promise<void>;
  onContinuation?: (result: IntakeContinuation, group: IntakeQuestionGroup) => Promise<void>;
  onSubmit?: (answers:Record<string,string>,revision:number)=>Promise<ProductionView>;
}) {
  const key = `abya-intake:${taskId}:${group.id}`;
  const [answers, setAnswers] = useState<Record<string, string>>(() => {
    try {
      const cached = JSON.parse(localStorage.getItem(key) ?? "null");
      if (group.status === "pending" && cached?.version === group.answers.length) return cached.answers;
    } catch { /* Database remains authoritative when no local draft is available. */ }
    return group.draft;
  });
  const [index, setIndex] = useState(0);
  const [all, setAll] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const dirty = useRef(JSON.stringify(answers) !== JSON.stringify(group.draft));
  const revisionRef = useRef(revision); revisionRef.current = revision;
  const inFlight = useRef(false);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const pending = group.status === "pending";
  function edit(id: string, value: string) {
    const next = { ...answers, [id]: value };
    dirty.current = true; setAnswers(next); setError(""); setNotice("草稿待保存");
    try { localStorage.setItem(key, JSON.stringify({ version: group.answers.length, answers: next })); }
    catch { setError("本地草稿保存失败，请使用保存草稿按钮后再关闭。"); }
  }
  async function save(operation: string) {
    if (inFlight.current) return;
    if (operation === "submit-answers") {
      const missing = group.questions.findIndex(q => !q.optional && !answers[q.id]?.trim());
      if (missing !== -1) { setIndex(missing); setError(`请先回答第 ${missing + 1} 题，再提交本组答案。`); return; }
    }
    inFlight.current = true; setBusy(true); setError("");
    const snapshot = JSON.stringify(answers);
    try {
      const submission = operation === "submit-answers" && group.provider === "codex" && !onSubmit
        ? await productionApi.submitAnswers(taskId, revisionRef.current, group.id, answers) : undefined;
      const view = operation==="submit-answers" && group.provider==="codex" && onSubmit
        ? await onSubmit(answers,revisionRef.current)
        : submission?.production ?? await productionApi.answer(taskId, revisionRef.current, operation, { id: group.id, answers });
      if (!alive.current) return;
      revisionRef.current = view.record!.revision; onSaved(view);
      if (snapshot === JSON.stringify(answers)) dirty.current = false;
      if (operation === "submit-answers") localStorage.removeItem(key);
      setNotice(operation === "submit-answers" ? "答案已保存；文档仍需单独确认。" : "草稿已保存");
      if (submission?.warning) setError(submission.warning);
      if (submission?.continuation) {
        const savedGroup = view.record?.questionGroups?.find(g => g.id === group.id) ?? group;
        if (onContinuation) await onContinuation(submission.continuation, savedGroup);
        else setNotice(continuationMessages[submission.continuation.status]);
      }
      window.dispatchEvent(new Event("abya:control-changed"));
    } catch (e) {
      if (alive.current) {
        const code = (e as { code?: string })?.code;
        setError(code === "conflict" ? `${errorMessage(e)} 本机草稿已保留，请刷新记录后重试保存。`
          : code === "validation" ? errorMessage(e) : `${errorMessage(e)} 草稿已保留，保存状态请以最新记录为准。`);
      }
    }
    finally { inFlight.current = false; if (alive.current) setBusy(false); }
  }
  useEffect(() => {
    if (!pending || !active || !dirty.current || busy || error) return;
    const timer = window.setTimeout(() => void save("save-draft"), 600);
    return () => window.clearTimeout(timer);
  }, [answers, pending, active, busy, error]);
  const answered = group.questions.filter(q => answers[q.id]?.trim()).length;
  return <section className="production-card intake-questions">
    <h4>{group.title} · 已回答 {answered}/{group.questions.length}</h4>
    <p>可返回修改，整组提交。{pending ? "正在整理本阶段内容" : group.status === "submitted" ? "已提交" : "已取消"}</p>
    {error && <p role="alert" className="inline-error">{error}</p>}
    {notice && <p role="status">{notice}</p>}
    <nav className="production-actions" aria-label="选择阶段问题">{group.questions.map((q, i) =>
      <button key={q.id} className="secondary-button" aria-current={i === index ? "step" : undefined}
        onClick={() => setIndex(i)}>第 {i + 1} 题{answers[q.id]?.trim() ? " ✓" : ""}</button>)}</nav>
    {group.questions.filter((_, i) => all || i === index).map(q => <fieldset key={q.id} disabled={busy || !active || !pending}>
      <legend>{q.text}{q.optional ? "（可选）" : "（必答）"}</legend>
      {q.options.map(option => <label className="intake-option" key={option}><input type="radio" name={`${group.id}:${q.id}`}
        checked={answers[q.id] === option} onChange={() => edit(q.id, option)} />{option}</label>)}
      <label className="field">回答或补充<textarea aria-label={q.text} value={answers[q.id] ?? ""} maxLength={8000}
        onChange={e => edit(q.id, e.target.value)} rows={3} /></label>
    </fieldset>)}
    <div className="production-actions">
      <button className="secondary-button" disabled={index === 0} onClick={() => setIndex(i => i - 1)}>上一题</button>
      <button className="secondary-button" disabled={index === group.questions.length - 1} onClick={() => setIndex(i => i + 1)}>下一题</button>
      <button className="secondary-button" onClick={() => setAll(v => !v)}>{all ? "逐题查看" : "查看全部答案"}</button>
      {pending && <><button className="secondary-button" disabled={busy || !active} onClick={() => void save("save-draft")}>保存草稿</button>
        <button className="secondary-button" disabled={busy || !active} onClick={() => void save("cancel")}>取消本组问题</button>
        <button className="primary-button" disabled={busy || !active} onClick={() => void save("submit-answers")}>{group.provider === "codex" ? "提交并继续" : "提交本组答案"}</button></>}
      {group.status === "submitted" && <>
        <button className="secondary-button" disabled={busy || !active} onClick={() => void save("amend")}>修改已提交答案</button>
        <button className="primary-button" disabled={busy || !active || (group.provider !== "codex" && !onContinue)} onClick={async () => {
          if (inFlight.current) return;
          inFlight.current = true;
          setBusy(true); setError("");
          try {
            if (group.provider === "codex") {
              const revision = group.answers[group.answers.length - 1].revision;
              let result = await productionApi.continueQuestions(taskId, group.id, revision);
              if (onContinuation) { await onContinuation(result, group); }
              else {
                if (result.status === "needsConnection" && onEnsureConnection && result.requestId) {
                  setNotice(continuationMessages.needsConnection);
                  await onEnsureConnection(group);
                  if (!alive.current) return;
                  result = await productionApi.flushContinuation(taskId, group.conversationId, result.requestId);
                }
                if (alive.current) setNotice(continuationMessages[result.status]);
              }
              window.dispatchEvent(new Event("abya:control-changed"));
            } else {
              await onContinue?.(`需求问题组 ${group.id} 的答案已在 APP 提交。请 production get 读取最新答案和人数模式，继续梳理需求；尚未批准需求文档。`, group);
              if (alive.current) setNotice("继续请求已发送，请查看原会话。");
            }
          } catch (e) { setError(errorMessage(e)); }
          finally { inFlight.current = false; if (alive.current) setBusy(false); }
        }}>继续任务</button>
      </>}
    </div>
    {group.answers.length > 0 && <details><summary>已提交答案历史（{group.answers.length} 版）</summary>
      {group.answers.map(a => <article key={a.revision}><strong>第 {a.revision} 版 · {a.submittedAt}</strong>
        {group.questions.map(q => <p key={q.id}>{q.text}：{a.answers[q.id] || "未填写"}</p>)}</article>)}</details>}
  </section>;
}
