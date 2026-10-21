import { useEffect, useRef, useState } from "react";
import { Download, FolderOpen } from "lucide-react";
import { productionApi } from "../../shared/production";
import { errorMessage } from "../../shared/api";
import "./programmerIssueExport.css";

export function ProgrammerIssueExport({ taskId, revision, issueIds, en = false, single = false }: {
  taskId: string; revision: number; issueIds: string[]; en?: boolean; single?: boolean;
}) {
  const [busy, setBusy] = useState(false), [path, setPath] = useState(""), [error, setError] = useState("");
  const pending = useRef(false), generation = useRef(0);
  const identity = JSON.stringify([taskId, issueIds]);
  useEffect(() => {
    generation.current++; pending.current = false;
    setBusy(false); setPath(""); setError("");
    return () => { generation.current++; };
  }, [identity]);
  async function reveal(saved: string, request: number) {
    if (generation.current === request) setError("");
    try { await productionApi.revealDocument(taskId, saved); }
    catch (e) { if (generation.current === request) setError(`${en ? "File saved; could not reveal it" : "文档已保存，但无法定位文件"}：${errorMessage(e)}`); }
  }
  async function exportFile(packageZip = false) {
    if (pending.current || !issueIds.length) return;
    const request = generation.current;
    pending.current = true; setBusy(true); setError(""); setPath("");
    try {
      const saved = await (packageZip ? productionApi.exportDeveloperPackage(taskId, revision, issueIds) : productionApi.exportProgrammerIssues(taskId, revision, issueIds));
      if (generation.current !== request) return;
      setPath(saved);
      await reveal(saved, request);
    } catch (e) { if (generation.current === request) setError(errorMessage(e)); }
    finally { if (generation.current === request) { pending.current = false; setBusy(false); } }
  }
  return <div className="programmer-issue-export">
    <button className="secondary-button" disabled={busy || !issueIds.length} onClick={() => void exportFile()}>
      <Download size={15} aria-hidden="true" />{busy ? (en ? "Exporting…" : "正在导出…") : single ? (en ? "Export this issue (MD)" : "导出此问题（MD）") : (en ? "Export programmer issues (MD)" : "导出程序问题汇总（MD）")}
    </button>
    <button className="secondary-button" disabled={busy || !issueIds.length} onClick={() => void exportFile(true)}>{en ? "Export feedback package (ZIP)" : "导出开发反馈包（ZIP）"}</button>
    {path && <div role="status"><span>{en ? "Saved in task folder" : "已保存到任务目录"}：{path}</span>
      <button className="text-button" onClick={() => void reveal(path, generation.current)}><FolderOpen size={14} aria-hidden="true"/>{en ? "Show in File Explorer" : "在文件夹中显示"}</button>
    </div>}
    {error && <p role="alert">{error}</p>}
  </div>;
}
