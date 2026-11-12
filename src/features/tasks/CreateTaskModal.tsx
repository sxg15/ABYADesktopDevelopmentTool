import {useState} from "react";
import {Modal} from "../../app/Modal";
import {api,errorMessage} from "../../shared/api";
import {productionApi} from "../../shared/production";
import {productionText} from "./productionText";
import type {DevelopmentTask} from "../../shared/types";
import type {MessageKey} from "../../i18n";
export function CreateTaskModal({
  t,
  locale,
  onClose,
  onCreated,
}: {
  t: (key: MessageKey) => string;
  locale?: string;
  onClose: () => void;
  onCreated: (task: DevelopmentTask) => void;
}) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [error, setError] = useState("");

  const [mode, setMode] = useState("full");
  const [questions, setQuestions] = useState("ask");
  const [playerMode, setPlayerMode] = useState("unspecified");
  const [created, setCreated] = useState<DevelopmentTask>();
  const [creating, setCreating] = useState(false);
  const s = productionText(locale);

  async function create() {
    setCreating(true);
    try {
      const task = created ?? await api.createTask(title, description);
      setCreated(task);
      if (mode === "full") {
        const existing = await productionApi.get(task.id);
        if (!existing.record) await productionApi.update(task.id, 0, "initialize", { questionMode: questions, playerMode });
      }
      onCreated(task);
      onClose();
    } catch (value) {
      setError(errorMessage(value));
    } finally { setCreating(false); }
  }

  return (
    <Modal title={t("newTask")} onClose={onClose}>
      <div className="form-stack">
        <label className="field"><span>{s.taskMode}</span><select value={mode} disabled={creating || Boolean(created)} onChange={e => setMode(e.target.value)}><option value="full">{s.fullTask}</option><option value="scoped">{s.scopedTask}</option></select></label>
        {mode === "full" && <label className="field"><span>{s.questionMode}</span><select value={questions} disabled={creating} onChange={e => setQuestions(e.target.value)}><option value="ask">{s.ask}</option><option value="no-followup">{s.noAsk}</option></select></label>}
        {mode === "full" && <label className="field">玩法人数<select value={playerMode} disabled={creating} onChange={e => setPlayerMode(e.target.value)}>
          <option value="unspecified">未明确，在需求阶段敲定</option><option value="single">单人</option><option value="multiplayer">多人</option>
        </select></label>}
        <label className="field">
          <span>{t("taskTitle")}</span>
          <input
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            autoFocus
          />
        </label>
        <label className="field">
          <span>{t("description")}</span>
          <textarea
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            rows={4}
          />
        </label>
      </div>
      {error && <div className="inline-error">{error}</div>}
      <footer className="modal-actions">
        <button className="secondary-button" onClick={onClose}>
          {t("cancel")}
        </button>
        <button className="primary-button" disabled={!title.trim() || creating} onClick={create}>
          {t("create")}
        </button>
      </footer>
    </Modal>
  );
}
