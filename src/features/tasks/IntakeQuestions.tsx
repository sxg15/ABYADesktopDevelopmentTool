import { useEffect, useRef, useState } from "react";
import { productionApi, type IntakeQuestionGroup, type ProductionView, type IntakeContinuation, continuationMessages } from "../../shared/production";
import { errorMessage } from "../../shared/api";
import { decodeAnswers, encodeAnswers, otherOption, type AnswerField } from "./intakeDraft";

export function IntakeQuestions({ taskId, group, revision, active, onSaved, onEnsureConnection, onContinuation, onSubmit, onDefer }: {
  taskId: string; group: IntakeQuestionGroup; revision: number; active: boolean;
  onSaved: (view: ProductionView) => void;
  onContinue?: (message: string, group: IntakeQuestionGroup) => Promise<void>;
  onEnsureConnection?: (group: IntakeQuestionGroup) => Promise<void>;
  onContinuation?: (result: IntakeContinuation, group: IntakeQuestionGroup) => Promise<void>;
  onSubmit?: (answers: Record<string,string>, revision: number) => Promise<ProductionView>;
  onDefer?: () => void;
}) {
  const key = `abya-intake:${taskId}:${group.id}`;
  const [fields, setFields] = useState<Record<string, AnswerField>>(() => {
    try {
      const cached = JSON.parse(localStorage.getItem(key) ?? "null");
      if (group.status === "pending" && cached?.version === group.answers.length) {
        const valid = group.questions.every(q => ["choice", "other", "note"].every(k => typeof cached.fields?.[q.id]?.[k] === "string"));
        return valid ? cached.fields : decodeAnswers(group, cached.answers ?? {});
      }
    } catch { /* Fall back to the durable draft. */ }
    return decodeAnswers(group, group.draft);
  });
  const answers = encodeAnswers(group, fields);
  const latest = useRef(answers); latest.current = answers;
  const [index, setIndex] = useState(0);
  const questionRef = useRef<HTMLFieldSetElement>(null), previousIndex = useRef(0);
  useEffect(() => { if (previousIndex.current !== index) questionRef.current?.focus(); previousIndex.current = index; }, [index]);
  const [busy, setBusy] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [saveError, setSaveError] = useState("");
  const [notice, setNotice] = useState("");
  const revisionRef = useRef(revision); revisionRef.current = Math.max(revisionRef.current, revision);
  const saved = useRef(JSON.stringify(encodeAnswers(group, decodeAnswers(group, group.draft))));
  const savingRequest = useRef<Promise<void> | undefined>(undefined);
  const submitting = useRef(false), composing = useRef(false), alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const pending = group.status === "pending";
  function edit(id: string, patch: Partial<AnswerField>) {
    const next = { ...fields, [id]: { ...fields[id], ...patch } };
    setFields(next); setError(""); setSaveError("");
    latest.current = encodeAnswers(group, next);
    try { localStorage.setItem(key, JSON.stringify({ version: group.answers.length, fields: next, answers: latest.current })); }
    catch { setSaveError("本地恢复副本保存失败，正在尝试保存到任务记录。"); }
  }
  async function saveDraft() {
    if (savingRequest.current) return savingRequest.current;
    if (!pending || !active || composing.current) return;
    const snapshot = JSON.stringify(latest.current);
    if (snapshot === saved.current) return;
    setSaving(true);
    const request = (async () => {
      try {
        const view = await productionApi.answer(taskId, revisionRef.current, "save-draft", { id: group.id, answers: JSON.parse(snapshot) });
        if (!alive.current) return;
        revisionRef.current = Math.max(revisionRef.current, view.record!.revision);
        saved.current = snapshot; onSaved(view); setSaveError("");
      } catch (e) { if (alive.current) setSaveError(`自动保存失败：${errorMessage(e)}。输入已保留，请重试。`); }
      finally { savingRequest.current = undefined; if (alive.current) setSaving(false); }
    })();
    savingRequest.current = request;
    return request;
  }
  useEffect(() => {
    if (!pending || !active || busy || saving || saveError || JSON.stringify(answers) === saved.current) return;
    const timer = window.setTimeout(() => void saveDraft(), 700);
    return () => window.clearTimeout(timer);
  }, [fields, pending, active, busy, saving, saveError]);
  async function submit() {
    if (submitting.current) return;
    const missing = group.questions.findIndex(q => !q.optional && !latest.current[q.id]?.trim());
    if (missing >= 0) { setIndex(missing); setError(`请先回答第 ${missing + 1} 题，再提交本组答案。`); return; }
    if (Object.values(latest.current).some(a => new TextEncoder().encode(a).length > 8000)) {
      setError("回答过长，请缩短到 8000 字节以内后提交。"); return;
    }
    submitting.current = true; setBusy(true); setError("");
    try {
      await savingRequest.current;
      const value = { ...latest.current };
      if (onSubmit) await onSubmit(value, revisionRef.current);
      else {
        const result = group.provider === "codex"
          ? await productionApi.submitAnswers(taskId, revisionRef.current, group.id, value)
          : { production: await productionApi.answer(taskId, revisionRef.current, "submit-answers", { id: group.id, answers: value }) };
        onSaved(result.production);
        if ("continuation" in result && result.continuation) await onContinuation?.(result.continuation, group);
      }
      localStorage.removeItem(key);
      window.dispatchEvent(new Event("abya:control-changed"));
    } catch (e) { if (alive.current) setError(`${errorMessage(e)} 输入已保留，请核对提交状态后重试。`); }
    finally { submitting.current = false; if (alive.current) setBusy(false); }
  }
  async function resume() {
    if (submitting.current) return;
    submitting.current = true; setBusy(true); setError("");
    try {
      if (group.provider === "codex") {
        let result = await productionApi.continueQuestions(taskId, group.id, group.answers[group.answers.length - 1].revision);
        if (onContinuation) await onContinuation(result, group);
        else {
          if (result.status === "needsConnection" && result.requestId && onEnsureConnection) {
            await onEnsureConnection(group);
            result = await productionApi.flushContinuation(taskId, group.conversationId, result.requestId);
          }
          setNotice(continuationMessages[result.status]);
        }
      } else throw new Error("此历史会话的服务商已移除。已保存的答案仍保留，请在 Codex 对话中读取任务记录；不会自动续接旧会话。");
    } catch (e) { setError(errorMessage(e)); }
    finally { submitting.current = false; if (alive.current) setBusy(false); }
  }
  const answered = group.questions.filter(q => answers[q.id]?.trim()).length;
  const complete = group.questions.every(q => q.optional || answers[q.id]?.trim());
  const q = group.questions[index], field = fields[q.id];
  return <section className="intake-questions">
    <h3>{group.title}</h3>
    <p>{group.provider === "codex" ? "请完成以下问题，提交后 AI 将继续处理。" : "此历史会话的服务商已移除；可保存答案，但不会自动续接旧会话。"}已回答 {answered}/{group.questions.length}</p>
    {error && <p role="alert" className="inline-error">{error}</p>}
    {notice && <p role="status">{notice}</p>}
    <nav className="intake-progress" aria-label="选择阶段问题">{group.questions.map((item, i) =>
      <button key={item.id} className="secondary-button" aria-current={i === index ? "step" : undefined}
        onClick={() => setIndex(i)}>第 {i + 1} 题{answers[item.id]?.trim() ? " ✓" : ""}</button>)}</nav>
    <fieldset key={q.id} ref={questionRef} tabIndex={-1} disabled={busy || !active || !pending}>
      <legend>{q.text}{q.optional ? "（可选）" : "（必答）"}</legend>
      {q.options.filter(o => !otherOption(o)).map(option => <label className="intake-option" key={option}>
        <input type="radio" name={`${group.id}:${q.id}`} checked={field.choice === option}
          onChange={() => edit(q.id, { choice: option })} />{option}</label>)}
      {!!q.options.length && <label className="intake-option"><input type="radio" name={`${group.id}:${q.id}`}
        checked={field.choice === "__other"} onChange={() => edit(q.id, { choice: "__other" })} />其他</label>}
      {(!q.options.length || field.choice === "__other") ? <label className="field">
        {q.options.length ? "请描述你的想法" : "你的回答"}
        <textarea aria-label={q.text} value={field.other} rows={4} maxLength={8000}
          onCompositionStart={() => { composing.current = true; }} onCompositionEnd={() => { composing.current = false; void saveDraft(); }}
          onChange={e => edit(q.id, { other: e.target.value })} /></label>
        : field.choice && <label className="field intake-note">补充说明（可选）
          <textarea aria-label={`${q.text} 补充说明`} value={field.note} rows={2} maxLength={8000}
            onCompositionStart={() => { composing.current = true; }} onCompositionEnd={() => { composing.current = false; void saveDraft(); }}
            onChange={e => edit(q.id, { note: e.target.value })} /></label>}
    </fieldset>
    <div className="intake-save-status" role="status">{saveError || (saving ? "正在自动保存…" : JSON.stringify(answers) === saved.current ? "已自动保存" : "等待自动保存")}
      {saveError && <button className="text-button" onClick={() => { setSaveError(""); void saveDraft(); }}>重试保存</button>}</div>
    <div className="production-actions intake-navigation">
      {onDefer && <button className="secondary-button intake-defer" onClick={onDefer}>稍后回答 / 关闭</button>}
      <button className="secondary-button" disabled={index === 0 || busy} onClick={() => setIndex(i => i - 1)}>上一题</button>
      {index < group.questions.length - 1 && <button className="secondary-button" disabled={busy} onClick={() => setIndex(i => i + 1)}>下一题</button>}
      {pending && (complete || index === group.questions.length - 1) && <button className="primary-button" disabled={busy || !active} onClick={() => void submit()}>{busy ? "正在提交…" : group.provider === "codex" ? "提交并继续" : "保存答案"}</button>}
      {group.status === "submitted" && group.provider === "codex" && <button className="primary-button" disabled={busy || !active} onClick={() => void resume()}>继续任务</button>}
    </div>
    {group.answers.length > 0 && <details><summary>已提交答案历史（{group.answers.length} 版）</summary>
      {group.answers.map(a => <article key={a.revision}><strong>第 {a.revision} 版 · {a.submittedAt}</strong>
        {group.questions.map(item => <p key={item.id}>{item.text}：{a.answers[item.id] || "未填写"}</p>)}</article>)}</details>}
  </section>;
}
