import { useEffect, useRef, useState } from "react";
import { ImagePlus, ClipboardPaste } from "lucide-react";
import { productionApi, type ProductionRecord, type ProductionView } from "../../shared/production";
import { errorMessage } from "../../shared/api";
import { ArtifactMedia, ArtifactPreview } from "./ArtifactPreview";
import "./artReferences.css";

const imageTypes = ["image/png", "image/jpeg", "image/webp", "image/gif"];
function readImage(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error("Cannot read image / 图片读取失败"));
    reader.onabort = () => reject(new Error("Image read cancelled / 图片读取已取消"));
    reader.onload = () => resolve(String(reader.result).split(",")[1]);
    reader.readAsDataURL(file);
  });
}

export function ArtReferences({ taskId, record, active, en = false, onUpdated, onContinue }: {
  taskId: string; record: ProductionRecord; active: boolean; en?: boolean;
  onUpdated: (view: ProductionView) => void;
  onContinue?: (message: string, revision: number) => Promise<void>;
}) {
  const [busy, setBusy] = useState(false), [error, setError] = useState("");
  const [notice, setNotice] = useState(""), [summary, setSummary] = useState("");
  const [preview, setPreview] = useState<string>();
  const picker = useRef<HTMLInputElement>(null), locked = useRef(false), generation = useRef(0);
  const revision = useRef(record.revision);
  revision.current = Math.max(revision.current, record.revision);
  useEffect(() => {
    revision.current = record.revision; generation.current++; locked.current = false;
    setBusy(false); setError(""); setNotice(""); setPreview(undefined); setSummary("");
    return () => { generation.current++; };
    // Task identity owns this editor; polling must not reset focus or the note.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [taskId]);
  const items = (record.artifacts ?? []).filter(a => a.plannerReference && a.status !== "withdrawn");
  function updated(view: ProductionView) {
    if (view.record) revision.current = Math.max(revision.current, view.record.revision);
    onUpdated(view);
  }
  async function upload(files: File[]) {
    if (!active || locked.current || !files.length) return;
    setError(""); setNotice("");
    if (files.length > 8 || files.length + items.length > 32) {
      setError(en ? "Upload up to 8 images at a time, 32 per task." : "每次最多上传8张，本任务最多保留32张参考图。"); return;
    }
    if (files.some(f => !imageTypes.includes(f.type) || !f.size || f.size > 16 * 1024 * 1024 || f.name.length > 160)) {
      setError(en ? "Use PNG/JPEG/WebP/GIF, up to 16 MiB each and names up to 160 characters." : "支持 PNG/JPEG/WebP/GIF，每张不超过16 MiB，名称不超过160字，文件不得为空。"); return;
    }
    locked.current = true; setBusy(true);
    const token = generation.current; let saved = 0;
    try {
      for (const file of files) {
        const base64 = await readImage(file);
        if (token !== generation.current) return;
        setNotice(en ? `Saving ${saved + 1}/${files.length}…` : `正在保存 ${saved + 1}/${files.length} 张…`);
        const view = await productionApi.feedback(taskId, revision.current, "attach-art-reference", { name: file.name, base64, summary });
        if (token !== generation.current) return;
        saved++; updated(view);
      }
      setSummary("");
      setNotice(en ? `${saved} saved. Continue when you want AI to use them.` : `已保存${saved}张参考图。准备好后点击“让 AI 参考并继续”。`);
    } catch (e) {
      if (token === generation.current) {
        setNotice(en ? `${saved} saved; remaining images were not saved.` : `本次已保存${saved}张，其余未保存，请刷新后重试未保存的图片。`);
        setError(errorMessage(e));
      }
    } finally { if (token === generation.current) { locked.current = false; setBusy(false); } }
  }
  async function action(run: (current: () => boolean) => Promise<void>) {
    if (!active || locked.current) return;
    locked.current = true; setBusy(true); setError(""); setNotice("");
    const token = generation.current;
    try { await run(() => token === generation.current); }
    catch (e) { if (token === generation.current) setError(errorMessage(e)); }
    finally { if (token === generation.current) { locked.current = false; setBusy(false); } }
  }
  return <section className="production-card art-references" aria-label={en ? "Planner reference images" : "策划参考图片"}>
    <h4><ImagePlus size={22} />{en ? "Planner reference images" : "策划参考图片"}<span className="production-count">{items.length}/32</span></h4>
    <p>{en ? "Upload style, layout or asset references here. They stay with this task; uploading does not approve the art proposal." : "在这里上传风格、布局或素材参考图，随任务保存。上传参考图不会确认美术方案。"}</p>
    <label className="field">{en ? "Notes for the next upload (optional)" : "本次图片参考说明（可选）"}
      <textarea value={summary} maxLength={2000} disabled={busy || !active} onChange={e => setSummary(e.target.value)}
        placeholder={en ? "For example: use the palette, keep the existing layout." : "例如：参考配色，布局仍按现有方案。"} />
    </label>
    <div className="art-reference-paste" tabIndex={active ? 0 : -1} role="group" aria-label={en ? "Image paste area" : "图片粘贴区"}
      aria-disabled={!active || busy} onClick={e => e.currentTarget.focus()}
      onPaste={e => {
        const files = Array.from(e.clipboardData.items).filter(i => i.kind === "file").map(i => i.getAsFile()).filter((f): f is File => !!f);
        if (!files.length) return;
        e.preventDefault(); e.stopPropagation(); void upload(files);
      }}>
      <ClipboardPaste size={24} /><strong>{en ? "Click here, then press Ctrl+V to paste images" : "点击此区域，再按 Ctrl+V 粘贴图片"}</strong>
      <small>{en ? "Copy an image or take a screenshot first. You can also select files below." : "先复制图片或截图，再在此处粘贴；也可以通过下方按钮选择文件。"}</small>
    </div>
    <input ref={picker} type="file" hidden multiple accept={imageTypes.join(",")} aria-label={en ? "Reference image files" : "参考图片文件"}
      disabled={busy || !active} onChange={e => { const files = Array.from(e.target.files ?? []); e.target.value = ""; void upload(files); }} />
    <div className="art-reference-actions">
      <button className="primary-button" disabled={busy || !active} onClick={() => picker.current?.click()}><ImagePlus size={17} />{en ? "Upload reference images" : "上传参考图片"}</button>
      <small>{en ? "PNG / JPG / WebP / GIF · 16 MiB each · Up to 8 per upload" : "PNG / JPG / WebP / GIF · 单张16 MiB · 每次最多8张"}</small>
    </div>
    {!active && <p>{en ? "Restore this task to upload images." : "任务已结束或归档，恢复任务后可上传图片。"}</p>}
    {notice && <p className="task-action-notice" role="status">{notice}</p>}
    {error && <p className="error-message" role="alert">{error}</p>}
    {!!items.length && <><div className="visual-card-grid">{items.map(a => <article className="visual-card" key={a.id}>
      <div className="visual-thumbnail" role="button" tabIndex={0} aria-label={`${en ? "Preview" : "查看大图"} ${a.title}`}
        onClick={() => setPreview(a.id)} onKeyDown={e => { if (["Enter", " "].includes(e.key)) { e.preventDefault(); setPreview(a.id); } }}>
        <ArtifactMedia taskId={taskId} artifact={a} en={en} /><span className="preview-hint">{en ? "Click to enlarge" : "点击查看大图"}</span>
      </div>
      <strong>{a.title}</strong><small>{en ? "Planner reference · Saved" : "策划参考 · 已保存"}</small><p>{a.summary}</p>
      <button className="text-button" disabled={busy || !active} onClick={() => void action(async current => {
        const view = await productionApi.feedback(taskId, revision.current, "remove-art-reference", { id: a.id });
        if (current()) { updated(view); setNotice(en ? "Removed from references; history retained." : "已从参考图中移除，历史记录保留。"); }
      })}>{en ? "Remove reference" : "移除参考图"}</button>
    </article>)}</div>
    <div className="art-reference-actions"><button className="secondary-button" disabled={!active || busy || !onContinue}
      onClick={() => void action(async current => {
        await onContinue?.("策划已上传美术参考图片。请读取最新制作记录 artifacts 中 plannerReference=true 且 status 不是 withdrawn 的图片路径和说明，查看图片后完善美术需求。参考图不是验收成果；必要时修订文档并重新请求策划确认。", revision.current);
        if (current()) setNotice(en ? "Continuation requested. Check task guidance for AI progress; images remain saved." : "已请求继续，请关注顶部任务引导中的 AI 处理状态；参考图已保存。");
      })}>{en ? "Let AI use references and continue" : "让 AI 参考并继续"}</button>
      <small>{en ? "Uses this task's existing conversation." : "通过本任务的原会话继续处理。"}</small></div></>}
    {preview && <ArtifactPreview taskId={taskId} items={items} initialId={preview} en={en} onClose={() => setPreview(undefined)} />}
  </section>;
}
