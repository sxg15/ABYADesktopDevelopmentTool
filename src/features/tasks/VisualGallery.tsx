import { useEffect, useState } from "react";
import { productionApi, type ProductionArtifact } from "../../shared/production";
import { errorMessage } from "../../shared/api";

const labels: Record<string, string> = { reference: "参考图", mockup: "设计示意", render: "引擎渲染", gameplay: "游戏实拍", feedback: "反馈附件" };
function Preview({ taskId, artifact }: { taskId: string; artifact: ProductionArtifact }) {
  const [source, setSource] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    let current = true; setSource(""); setError("");
    void productionApi.media(taskId, artifact.id).then(value => { if (current) setSource(value); })
      .catch(reason => { if (current) setError(errorMessage(reason)); });
    return () => { current = false; };
  }, [taskId, artifact.id]);
  if (error) return <p role="alert">{error}</p>;
  if (!source) return <p role="status">…</p>;
  return artifact.mediaType?.startsWith("video/")
    ? <video controls preload="metadata" src={source} aria-label={artifact.title} />
    : <a href={source} target="_blank" rel="noreferrer" title={artifact.title}><img src={source} alt={artifact.title} /></a>;
}
export function VisualGallery({ taskId, artifacts, bindings = {}, en = false, onFeedback }: {
  taskId: string; artifacts: ProductionArtifact[]; bindings?: Record<string, string>; en?: boolean;
  onFeedback?: (id: string) => void;
}) {
  const items = artifacts.filter(a => a.mediaType);
  const [selected, setSelected] = useState<string[]>([]);
  useEffect(() => { setSelected([]); }, [taskId]);
  if (!items.length) return null;
  const toggle = (id: string) => setSelected(old => old.includes(id) ? old.filter(v => v !== id) : [...old.slice(-1), id]);
  return <section className="production-card">
    <h4>{en ? "Visual proposals and results" : "视觉方案与实际效果"}</h4>
    <p>{en ? "Select up to two items to enlarge or compare. Concept images are not gameplay evidence." : "选择最多两项放大或对照；设计示意与游戏实拍分别标注。"}</p>
    <div className="visual-card-grid">{items.map((a, index) => <article className="visual-card" key={a.id}>
      {index < 6 && a.mediaType?.startsWith("image/") && <div className="visual-thumbnail"><Preview taskId={taskId} artifact={a} /></div>}
      <strong>{a.title}</strong><small>{en ? a.sourceType : labels[a.sourceType ?? ""]} · {a.visualVersion}</small>
      <p>{a.summary}</p>
      {a.temporary && <span className="production-status">{en ? "Temporary" : "临时素材"}</span>}
      {bindings[a.path] === a.sha256 && <span className="production-status">{en ? "Bound to plan" : "已关联执行计划"}</span>}
      {a.source && <small>{a.source}</small>}
      <button className="secondary-button" aria-pressed={selected.includes(a.id)} onClick={() => toggle(a.id)}>
        {en ? "Preview / compare" : "预览 / 对照"}
      </button>
      {onFeedback && <button className="secondary-button" onClick={() => onFeedback(a.id)}>{en ? "Give feedback" : "对此图提意见"}</button>}
    </article>)}</div>
    <div className="visual-comparison">{selected.map(id => {
      const a = items.find(item => item.id === id);
      return a && <figure key={id}><Preview taskId={taskId} artifact={a} /><figcaption>{a.title} · {a.visualVersion}</figcaption></figure>;
    })}</div>
  </section>;
}
