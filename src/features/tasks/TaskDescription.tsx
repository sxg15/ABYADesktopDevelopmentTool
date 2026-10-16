import { useId, useState } from "react";
import { ChevronDown, ChevronUp, FileText } from "lucide-react";

function remembered(taskId: string) {
  try { return sessionStorage.getItem(`abya:task-description:${taskId}`) === "expanded"; }
  catch { return false; }
}

// Key this component by task ID so polling keeps state while task switches stay isolated.
export function TaskDescription({ taskId, description = "", workspacePath, en = false }: {
  taskId: string; description?: string; workspacePath: string; en?: boolean;
}) {
  const [expanded, setExpanded] = useState(() => remembered(taskId));
  const contentId = useId();
  const label = expanded ? (en ? "Collapse task description" : "收起任务说明") : (en ? "Expand task description" : "展开任务说明");
  function toggle() {
    const next = !expanded;
    setExpanded(next);
    try { sessionStorage.setItem(`abya:task-description:${taskId}`, next ? "expanded" : "collapsed"); }
    catch { /* The disclosure still works when browser storage is unavailable. */ }
  }
  return <section className="task-description">
    <button type="button" className="task-description-toggle" aria-label={label}
      aria-expanded={expanded} aria-controls={contentId} onClick={toggle}>
      <FileText size={15} aria-hidden="true" />
      <strong>{en ? "Task description" : "任务说明"}</strong>
      {!expanded && <span className="task-description-summary" aria-hidden="true">{description.trim().replace(/\s+/g, " ") || (en ? "View workspace" : "查看任务工作区")}</span>}
      <span className="task-description-action">{expanded ? (en ? "Collapse" : "收起") : (en ? "Expand" : "展开")}
        {expanded ? <ChevronUp size={16} /> : <ChevronDown size={16} />}</span>
    </button>
    <div id={contentId} className="task-description-body" hidden={!expanded} tabIndex={expanded ? 0 : -1}
      role="region" aria-label={en ? "Full task description and workspace" : "完整任务说明与工作区"}>
      <div className="task-description-copy">{description || (en ? "No task description." : "暂未填写任务说明。")}</div>
      <code className="task-workspace-path">{en ? "Task workspace" : "任务工作区"}: {workspacePath}</code>
    </div>
  </section>;
}
