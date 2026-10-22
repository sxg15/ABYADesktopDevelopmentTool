import { useId, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent, type ReactNode } from "react";

const preferenceKey = "abya:workbench:split-ratio:v1";
const defaultRatio = 0.52;
function savedRatio() {
  try { const value = Number(localStorage.getItem(preferenceKey) ?? defaultRatio); return Number.isFinite(value) && value > 0 && value < 1 ? value : defaultRatio; }
  catch { return defaultRatio; }
}
// A layout preference only: resizing never remounts the form or writes task records.
export function WorkbenchSplit({ children, en }: { children: [ReactNode, ReactNode]; en: boolean }) {
  const root = useRef<HTMLDivElement>(null), divider = useRef<HTMLDivElement>(null);
  const [ratio, setRatio] = useState(savedRatio), [usable, setUsable] = useState(0), [dragging, setDragging] = useState(false);
  const drag = useRef<{ id: number; offset: number; original: number } | undefined>(undefined);
  const current = useRef(ratio), description = useId();
  const minimum = usable > 0 ? Math.min(120, usable * 0.25) / usable : 0.2;
  const maximum = usable > 0 ? 1 - Math.min(160, usable * 0.4) / usable : 0.8;
  const clamp = (value: number) => Math.max(minimum, Math.min(maximum, value));
  const rendered = clamp(ratio);
  useLayoutEffect(() => {
    const measure = () => {
      const height = (root.current?.getBoundingClientRect().height ?? 0) - (divider.current?.getBoundingClientRect().height ?? 24);
      if (height > 0) setUsable(height);
    };
    measure(); const observer = new ResizeObserver(measure);
    if (root.current) observer.observe(root.current);
    if (divider.current) observer.observe(divider.current);
    return () => observer.disconnect();
  }, []);
  function update(value: number, save = false) {
    current.current = clamp(value); setRatio(current.current);
    if (save) { try { localStorage.setItem(preferenceKey, String(current.current)); } catch { /* Resizing still works without storage. */ } }
  }
  function reset() { update(defaultRatio, true); }
  function start(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0 || !usable || drag.current) return;
    event.preventDefault(); event.currentTarget.focus({ preventScroll: true });
    event.currentTarget.setPointerCapture(event.pointerId);
    current.current = rendered;
    drag.current = { id: event.pointerId, offset: event.clientY - root.current!.getBoundingClientRect().top - rendered * usable, original: ratio };
    setDragging(true);
  }
  function move(event: PointerEvent<HTMLDivElement>) {
    if (drag.current?.id !== event.pointerId || !usable) return;
    update((event.clientY - root.current!.getBoundingClientRect().top - drag.current.offset) / usable);
  }
  function finish(event: PointerEvent<HTMLDivElement>, cancelled = false) {
    if (drag.current?.id !== event.pointerId) return;
    const original = drag.current.original; drag.current = undefined; setDragging(false);
    update(cancelled ? original : current.current, !cancelled);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  }
  function key(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape" && drag.current) {
      const { original, id } = drag.current; drag.current = undefined; setDragging(false); update(original);
      if (event.currentTarget.hasPointerCapture(id)) event.currentTarget.releasePointerCapture(id);
    } else if (!drag.current) {
      const step = event.shiftKey ? 0.1 : 0.03;
      if (event.key === "ArrowUp") update(rendered - step, true);
      else if (event.key === "ArrowDown") update(rendered + step, true);
      else if (event.key === "Home") update(minimum, true);
      else if (event.key === "End") update(maximum, true);
      else if (event.key === "Enter") reset();
      else return;
    } else return;
    event.preventDefault();
  }
  const label = en ? "Resize results and answers" : "调整成果区和回答区高度";
  return <div ref={root} className={`wb-main wb-split${dragging ? " is-resizing" : ""}`}
    style={{ gridTemplateRows: `minmax(0, ${rendered}fr) 24px minmax(0, ${1 - rendered}fr)` }}>
    {children[0]}
    <div ref={divider} className="wb-divider" role="separator" tabIndex={0} aria-orientation="horizontal"
      aria-label={label} aria-describedby={description} aria-valuemin={Math.round(minimum * 100)} aria-valuemax={Math.round(maximum * 100)}
      aria-valuenow={Math.round(rendered * 100)} aria-valuetext={en ? `Results ${Math.round(rendered * 100)}%, answers ${Math.round((1 - rendered) * 100)}%` : `成果区 ${Math.round(rendered * 100)}%，回答区 ${Math.round((1 - rendered) * 100)}%`}
      title={en ? "Drag to resize; double-click to reset" : "上下拖动调整高度，双击恢复默认"}
      onPointerDown={start} onPointerMove={move} onPointerUp={event => finish(event)} onPointerCancel={event => finish(event, true)}
      onLostPointerCapture={event => finish(event)} onKeyDown={key} onDoubleClick={reset}>
      <span className="wb-divider-grip" aria-hidden="true" />
      <span className="wb-divider-help" id={description}>{en ? "Drag or use Up/Down arrows to resize. Shift adjusts faster. Enter or double-click resets. Escape cancels a drag." : "向上拖动可扩大回答区；也可用上下方向键调整，Shift 加速，回车或双击恢复默认，Esc 取消拖动。"}</span>
    </div>
    {children[1]}
  </div>;
}
