import { useEffect, useRef, useState } from "react";
import { Modal } from "../../app/Modal";
import { productionApi, type ProductionArtifact } from "../../shared/production";
import { errorMessage } from "../../shared/api";

export function ArtifactMedia({ taskId, artifact, en = false }: { taskId: string; artifact: ProductionArtifact; en?: boolean }) {
  const [source, setSource] = useState("");
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let current = true; setSource(""); setError("");
    void productionApi.media(taskId, artifact.id).then(value => { if (current) setSource(value); })
      .catch(reason => { if (current) setError(errorMessage(reason)); });
    return () => { current = false; };
  }, [taskId, artifact.id, artifact.sha256, attempt]);
  if (error) return <span role="alert">{error} <button className="text-button" onClick={e => { e.stopPropagation(); setAttempt(v => v + 1); }}>{en ? "Retry" : "重试加载"}</button></span>;
  if (!source) return <span role="status">{en ? "Loading preview…" : "正在加载预览…"}</span>;
  return artifact.mediaType?.startsWith("video/")
    ? <video controls preload="metadata" src={source} aria-label={artifact.title} onError={() => setError(en ? "Video unavailable" : "视频加载失败")} />
    : <img draggable={false} src={source} alt={artifact.title} onError={() => setError(en ? "Image unavailable" : "图片加载失败")} />;
}

export function ArtifactPreview({ taskId, items, initialId, en = false, onClose }: {
  taskId: string; items: ProductionArtifact[]; initialId: string; en?: boolean; onClose: () => void;
}) {
  const [index, setIndex] = useState(Math.max(0, items.findIndex(a => a.id === initialId)));
  const [zoom, setZoom] = useState(1);
  const viewport = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; y: number; left: number; top: number } | null>(null);
  const artifact = items[index];
  if (!artifact) return null;
  function navigate(next: number) { setIndex(next); setZoom(1); }
  return <Modal wide title={`${artifact.title} · ${artifact.visualVersion ?? ""}`} onClose={onClose}>
    <div className="preview-toolbar">
      <button className="secondary-button" disabled={index === 0} onClick={() => navigate(index - 1)}>{en ? "Previous" : "上一张"}</button>
      <span>{index + 1} / {items.length}</span>
      <button className="secondary-button" disabled={index === items.length - 1} onClick={() => navigate(index + 1)}>{en ? "Next" : "下一张"}</button>
      <button className="secondary-button" onClick={() => setZoom(z => Math.max(0.5, z - 0.25))} aria-label={en ? "Zoom out" : "缩小"}>−</button>
      <button className="secondary-button" onClick={() => setZoom(1)}>{en ? "Fit window" : "适应窗口"}</button>
      <button className="secondary-button" onClick={() => setZoom(z => Math.min(4, z + 0.25))} aria-label={en ? "Zoom in" : "放大"}>＋</button>
      <span>{Math.round(zoom * 100)}%</span>
    </div>
    <div className="artifact-preview-viewport" ref={viewport} onPointerDown={e => {
      if (e.button !== 0 || artifact.mediaType?.startsWith("video/") || (e.target as HTMLElement).closest("button")) return;
      drag.current = { x: e.clientX, y: e.clientY, left: e.currentTarget.scrollLeft, top: e.currentTarget.scrollTop };
      e.currentTarget.setPointerCapture(e.pointerId);
    }} onPointerMove={e => {
      if (!drag.current) return;
      e.currentTarget.scrollLeft = drag.current.left - e.clientX + drag.current.x;
      e.currentTarget.scrollTop = drag.current.top - e.clientY + drag.current.y;
    }} onPointerUp={() => { drag.current = null; }} onPointerCancel={() => { drag.current = null; }}>
      <div className={`artifact-preview-media ${zoom === 1 ? "is-fit" : ""}`} style={{ width: `${zoom * 100}%` }}>
        <ArtifactMedia taskId={taskId} artifact={artifact} en={en} />
      </div>
    </div>
    <p>{artifact.summary}</p>
    <div className="modal-actions"><button className="secondary-button" onClick={onClose}>{en ? "Close preview" : "关闭预览"}</button></div>
  </Modal>;
}
