import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Modal } from "../../app/Modal";
import { errorMessage } from "../../shared/api";
import { productionApi, type IntakeQuestionGroup, type ProductionView, type IntakeContinuation, continuationMessages } from "../../shared/production";
import { IntakeQuestions } from "./IntakeQuestions";
import { useTaskControl, controlText } from "../terminal/useTaskControl";
import "./production.css";

export function TaskIntakeDialog({ taskId, active, onContinue, onEnsureConnection }: {
  taskId: string; active: boolean;
  onContinue?: (message: string, group?: IntakeQuestionGroup) => Promise<void>;
  onEnsureConnection?: (group: IntakeQuestionGroup) => Promise<void>;
}) {
  const [view, setView] = useState<ProductionView>();
  const [selected, setSelected] = useState("");
  const [error, setError] = useState("");
  const [feedback, setFeedback] = useState("");
  const sequence = useRef(0);
  const alive = useRef(true);
  const seen = useRef(new Set<string>());
  useEffect(() => { setFeedback(""); setError(""); }, [selected]);
  async function refresh() {
    const request = ++sequence.current;
    try {
      const next = await productionApi.get(taskId);
      if (alive.current && request === sequence.current) { setView(next); setError(""); }
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
    window.dispatchEvent(new Event("abya:control-changed"));
  }
  const groups = view?.record?.questionGroups ?? [];
  const pending = groups.find(g => g.status === "pending");
  const pendingKey = pending ? `${taskId}:${pending.id}:${pending.answers.length}` : "";
  useEffect(() => {
    if (!pending || !active || seen.current.has(pendingKey)) return;
    seen.current.add(pendingKey);
    try { if (sessionStorage.getItem(`abya-intake-later:${pendingKey}`)) return; } catch { /* Optional dismissal memory. */ }
    setSelected(pending.id);
  }, [pendingKey, active]);
  const group = groups.find(g => g.id === selected);
  const control = useTaskControl(taskId, group?.provider === "codex" ? group.conversationId : undefined, "codex");
  function acceptView(next:ProductionView) {
    sequence.current++; setView(current=>(current?.record?.revision??0)>(next.record?.revision??0)?current:next);
  }
  async function submit(answers:Record<string,string>,revision:number) {
    if(!group)throw new Error("问题组已变化，请重新打开。");
    const result=await productionApi.submitAnswers(taskId,revision,group.id,answers);
    if(alive.current) {
      acceptView(result.production);
      if(result.warning)setError(result.warning);
      if(result.continuation)await continueResult(result.continuation,result.production.record?.questionGroups?.find(g=>g.id===group.id)??group);
    }
    return result.production;
  }
  function close() {
    if (group?.status === "pending") {
      try { sessionStorage.setItem(`abya-intake-later:${taskId}:${group.id}:${group.answers.length}`, "1"); } catch { /* Drafts are separately saved. */ }
    }
    setSelected("");
  }
  useLayoutEffect(() => {
    if (!group) return;
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") { event.stopPropagation(); close(); } };
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  }, [selected, group?.status]);
  if (!group || !view?.record) return pending ? <button className="primary-button intake-pending-button"
    onClick={() => setSelected(pending.id)}>有待回答问题 · 继续填写</button> : null;
  return <Modal wide title="阶段问答" onClose={close}>
    <p className="intake-dialog-hint">可返回修改。关闭或稍后回答会保留草稿，不提交答案，也不取消问题。</p>
    {error && <p role="alert">{error}</p>}
    {feedback && <p role="status">{feedback === continuationMessages.queued && control ? controlText(control) : feedback}</p>}
    <IntakeQuestions key={`${taskId}:${group.id}`} taskId={taskId} group={group}
      revision={view.record.revision} active={active} onContinue={onContinue}
      onEnsureConnection={onEnsureConnection} onContinuation={continueResult}
      onSaved={acceptView} onSubmit={submit} />
    <div className="modal-actions"><button className="secondary-button" onClick={close}>稍后回答 / 关闭</button></div>
  </Modal>;
}
