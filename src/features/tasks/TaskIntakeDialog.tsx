import { useEffect, useRef, useState } from "react";
import { Modal } from "../../app/Modal";
import { errorMessage } from "../../shared/api";
import { productionApi, type IntakeQuestionGroup, type ProductionView, type IntakeContinuation, continuationMessages } from "../../shared/production";
import { IntakeQuestions } from "./IntakeQuestions";
import { useTaskControl, controlText } from "../terminal/useTaskControl";
import "./production.css";

export function TaskIntakeDialog({ taskId, active, onContinue, onEnsureConnection, requestedGroup }: {
  taskId: string; active: boolean;
  onContinue?: (message: string, group?: IntakeQuestionGroup) => Promise<void>;
  onEnsureConnection?: (group: IntakeQuestionGroup) => Promise<void>;
  requestedGroup?: { requestId: string; groupId: string };
}) {
  const [view, setView] = useState<ProductionView>();
  const [selected, setSelected] = useState("");
  const [error, setError] = useState("");
  const [feedback, setFeedback] = useState("");
  const [retryGroup, setRetryGroup] = useState<IntakeQuestionGroup>();
  const [retrying, setRetrying] = useState(false);
  const sequence = useRef(0);
  const alive = useRef(true);
  const seen = useRef(new Set<string>());
  const autoOpened = useRef(false);
  const submitted = useRef(new Set<string>());
  useEffect(() => {
    if (!requestedGroup) return;
    autoOpened.current = true;
    setSelected(requestedGroup.groupId);
    void refresh();
  }, [requestedGroup?.requestId]);
  async function refresh() {
    const request = ++sequence.current;
    try {
      const next = await productionApi.get(taskId);
      if (alive.current && request === sequence.current) { setView(old => (old?.record?.revision ?? 0) > (next.record?.revision ?? 0) ? old : next); }
    } catch (e) { if (alive.current) setError(errorMessage(e)); }
  }
  useEffect(() => {
    alive.current = true; void refresh();
    const timer = window.setInterval(() => void refresh(), 3000);
    const open = (event: Event) => {
      const detail = (event as CustomEvent<{ taskId: string; groupId: string }>).detail;
      if (detail.taskId === taskId) { setSelected(detail.groupId); void refresh(); }
    };
    window.addEventListener("abya:open-intake", open);
    return () => { alive.current = false; sequence.current++; window.clearInterval(timer); window.removeEventListener("abya:open-intake", open); };
  }, [taskId]);
  async function continueResult(result: IntakeContinuation, group: IntakeQuestionGroup) {
    setFeedback(continuationMessages[result.status]); setError("");
    if (result.status === "needsConnection" && result.requestId && onEnsureConnection) {
      try {
        await onEnsureConnection(group);
        if (!alive.current) return;
        result = await productionApi.flushContinuation(taskId, group.conversationId, result.requestId);
        setFeedback(continuationMessages[result.status]);
      } catch(e) { if(alive.current) setError(errorMessage(e)); }
    }
    if (["needsConnection", "paused", "needsReview"].includes(result.status)) setRetryGroup(group);
    else setRetryGroup(undefined);
    window.dispatchEvent(new Event("abya:control-changed"));
  }
  const groups = view?.record?.questionGroups ?? [];
  const pending = groups.find(g => g.status === "pending" && !submitted.current.has(`${g.id}:${g.answers.length}`));
  const pendingKey = pending ? `${taskId}:${pending.id}:${pending.answers.length}` : "";
  useEffect(() => {
    if (!pending || !active || autoOpened.current || seen.current.has(pendingKey)) return;
    if (document.querySelector('[aria-modal="true"]')) return;
    seen.current.add(pendingKey);
    try { if (sessionStorage.getItem(`abya-intake-later:${pendingKey}`)) return; } catch { /* Optional dismissal memory. */ }
    autoOpened.current = true; setSelected(pending.id);
  }, [pendingKey, active]);
  const group = groups.find(g => g.id === selected);
  const trackedGroup = group ?? retryGroup;
  const control = useTaskControl(taskId, trackedGroup?.provider === "codex" ? trackedGroup.conversationId : undefined, "codex");
  function acceptView(next:ProductionView) {
    sequence.current++; setView(current=>(current?.record?.revision??0)>(next.record?.revision??0)?current:next);
  }
  async function submit(answers:Record<string,string>,revision:number) {
    if(!group)throw new Error("问题组已变化，请重新打开。");
    const result=group.provider === "codex" ? await productionApi.submitAnswers(taskId,revision,group.id,answers)
      : { production: await productionApi.answer(taskId,revision,"submit-answers",{id:group.id,answers}) };
    if(alive.current) {
      acceptView(result.production);
      submitted.current.add(`${group.id}:${group.answers.length}`); setSelected(current => current === group.id ? "" : current); autoOpened.current = true;
      const savedGroup = result.production.record?.questionGroups?.find(g => g.id === group.id) ?? group;
      setFeedback("答案已提交，无需重复提交。"); setError(result.warning ?? "");
      if (result.warning) setRetryGroup(savedGroup);
      if(result.continuation)void continueResult(result.continuation,result.production.record?.questionGroups?.find(g=>g.id===group.id)??group);
      else if (group.provider !== "codex") {
        setFeedback("答案已保存。此历史会话的服务商已移除，请在 Codex 对话中读取已保存的任务记录；未自动续接旧会话。");
        setRetryGroup(undefined);
      }
    }
    return result.production;
  }
  function close() {
    if (group?.status === "pending") {
      try { sessionStorage.setItem(`abya-intake-later:${taskId}:${group.id}:${group.answers.length}`, "1"); } catch { /* Drafts are separately saved. */ }
    }
    setSelected("");
  }
  async function retryContinuation() {
    if (!retryGroup || retrying) return;
    setRetrying(true); setError("");
    try {
      if (retryGroup.provider !== "codex") throw new Error("此历史会话的服务商已移除，答案仍保留，不能自动续接旧会话。");
      if (onEnsureConnection) await onEnsureConnection(retryGroup);
      if (!alive.current) return;
      if (retryGroup.provider === "codex") {
        const version = retryGroup.answers[retryGroup.answers.length - 1]?.revision;
        if (version === undefined) throw new Error("正在核对已提交答案，请稍后重试。");
        await continueResult(await productionApi.continueQuestions(taskId, retryGroup.id, version), retryGroup);

      }
    } catch (e) { if (alive.current) setError(errorMessage(e)); }
    finally { if (alive.current) setRetrying(false); }
  }
  if (!group || !view?.record) return <>
    {feedback && <p role="status" className="task-action-notice">{feedback}</p>}
    {error && <p role="alert" className="inline-error">{error} 请前往终端核对并恢复连接。</p>}
    {retryGroup && <button className="secondary-button" disabled={retrying} onClick={() => void retryContinuation()}>{retrying ? "正在恢复原会话…" : "重试继续（不重复提交答案）"}</button>}
    {pending && <button className="primary-button intake-pending-button"
      onClick={() => setSelected(pending.id)}>有待回答问题 · 继续填写</button>}
  </>;
  return <Modal wide title="阶段问答" onClose={close}>
    <p className="intake-dialog-hint">可返回修改，内容自动保存。稍后回答会保留当前填写内容。</p>
    {error && <p role="alert">{error}</p>}
    {feedback && <p role="status">{feedback === continuationMessages.queued && control ? controlText(control) : feedback}</p>}
    <IntakeQuestions key={`${taskId}:${group.id}`} taskId={taskId} group={group}
      revision={view.record.revision} active={active} onContinue={onContinue}
      onEnsureConnection={onEnsureConnection} onContinuation={continueResult}
      onSaved={acceptView} onSubmit={submit} onDefer={close} />
  </Modal>;
}
