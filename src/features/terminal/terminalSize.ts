/** Must match MIN/MAX columns and rows in the Codex and Grok terminal modules. */
export const TERMINAL_MIN_COLUMNS = 20;
export const TERMINAL_MAX_COLUMNS = 500;
export const TERMINAL_MIN_ROWS = 5;
export const TERMINAL_MAX_ROWS = 200;

export function clampTerminalSize(
  columns: number,
  rows: number,
): { columns: number; rows: number } | undefined {
  if (!Number.isFinite(columns) || !Number.isFinite(rows)) {
    return undefined;
  }
  const nextColumns = Math.trunc(columns);
  const nextRows = Math.trunc(rows);
  if (nextColumns < 1 || nextRows < 1) {
    return undefined;
  }
  return {
    columns: Math.min(
      TERMINAL_MAX_COLUMNS,
      Math.max(TERMINAL_MIN_COLUMNS, nextColumns),
    ),
    rows: Math.min(TERMINAL_MAX_ROWS, Math.max(TERMINAL_MIN_ROWS, nextRows)),
  };
}

export function hostHasTerminalLayout(host: HTMLElement | null | undefined) {
  return Boolean(
    host && !host.hidden && host.clientWidth > 0 && host.clientHeight > 0,
  );
}
