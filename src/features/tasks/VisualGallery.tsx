import { useEffect, useState } from "react";
import type { ProductionArtifact } from "../../shared/production";
import { ArtifactMedia, ArtifactPreview } from "./ArtifactPreview";
const labels: Record<string, string> = { reference: "参考图", mockup: "设计示意", render: "引擎渲染", gameplay: "游戏实拍", feedback: "反馈附件" };
export function VisualGallery({ taskId, artifacts, bindings = {}, en = false, onFeedback, feedbackActions = {} }: {
  taskId: string; artifacts: ProductionArtifact[]; bindings?: Record<string, string>; en?: boolean;
  onFeedback?: (id: string) => void; feedbackActions?: Record<string,string>;
}) {
  const items = artifacts.filter(a => a.mediaType && a.sourceType !== "feedback");
  const [selected, setSelected] = useState<string[]>([]);
  const [preview, setPreview] = useState<string>();
  useEffect(() => { setSelected([]); setPreview(undefined); }, [taskId]);
  if (!items.length) return null;
  const toggle = (id: string) => setSelected(old => old.includes(id) ? old.filter(v => v !== id) : [...old.slice(-1), id]);
  return <section className="production-card">
    <h4>{en ? "Visual proposals and results" : "视觉方案与实际效果"}</h4>
    <p>{en ? "Click an image to enlarge. Select two items to compare." : "点击图片查看大图；可选择两张进行对照。设计示意与游戏实拍分别标注。"}</p>
    <div className="visual-card-grid">{items.map(a => <article className="visual-card" key={a.id}>
      <div className="visual-thumbnail" role="button" tabIndex={0} aria-label={`${en ? "Preview" : "查看大图"} ${a.title}`}
        onClick={() => setPreview(a.id)} onKeyDown={e => { if (e.target === e.currentTarget && ["Enter", " "].includes(e.key)) { e.preventDefault(); setPreview(a.id); } }}>
        {a.mediaType?.startsWith("image/") ? <ArtifactMedia taskId={taskId} artifact={a} en={en} /> : <span>▶ {en ? "Play video" : "播放视频"}</span>}
        <span className="preview-hint">⊕ {en ? "Click to enlarge" : "点击查看大图"}</span>
      </div>
      <strong>{a.title}</strong><small>{en ? a.sourceType : labels[a.sourceType ?? ""]} · {a.visualVersion}</small>
      <p>{a.summary}</p>
      {a.temporary && <span className="production-status">{en ? "Temporary" : "临时素材"}</span>}
      {bindings[a.path] === a.sha256 && <span className="production-status">{en ? "Submitted with document" : "已关联确认文档"}</span>}
      <button className="text-button" aria-pressed={selected.includes(a.id)} onClick={() => toggle(a.id)}>{selected.includes(a.id) ? (en ? "Remove comparison" : "取消对照") : (en ? "Compare" : "加入对照")}</button>
      {onFeedback && <><button className="secondary-button" disabled={feedbackActions[a.id] !== ""} onClick={() => onFeedback(a.id)}>{en ? "Review / feedback" : "查看并反馈"}</button>
        {feedbackActions[a.id] !== "" && <small>{feedbackActions[a.id] || (en ? "Not submitted for review" : "尚未提交审查")}</small>}</>}
    </article>)}</div>
    <div className="visual-comparison">{selected.map(id => {
      const a = items.find(item => item.id === id);
      return a && <figure key={id}><ArtifactMedia taskId={taskId} artifact={a} en={en} /><figcaption>{a.title} · {a.visualVersion}</figcaption></figure>;
    })}</div>
    {preview && <ArtifactPreview taskId={taskId} items={items} initialId={preview} en={en} onClose={() => setPreview(undefined)} />}
  </section>;
}
