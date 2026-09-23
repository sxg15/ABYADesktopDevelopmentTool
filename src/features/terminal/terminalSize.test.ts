import { describe, expect, it } from "vitest";
import {
  TERMINAL_MAX_COLUMNS,
  TERMINAL_MAX_ROWS,
  TERMINAL_MIN_COLUMNS,
  TERMINAL_MIN_ROWS,
  clampTerminalSize,
  hostHasTerminalLayout,
} from "./terminalSize";

describe("clampTerminalSize", () => {
  it("keeps a normal xterm size", () => {
    expect(clampTerminalSize(80, 24)).toEqual({ columns: 80, rows: 24 });
  });

  it("clamps to the PTY limits used by Codex and Grok", () => {
    expect(clampTerminalSize(2, 1)).toEqual({
      columns: TERMINAL_MIN_COLUMNS,
      rows: TERMINAL_MIN_ROWS,
    });
    expect(clampTerminalSize(800, 400)).toEqual({
      columns: TERMINAL_MAX_COLUMNS,
      rows: TERMINAL_MAX_ROWS,
    });
  });

  it("rejects values that are not a real terminal grid", () => {
    expect(clampTerminalSize(0, 24)).toBeUndefined();
    expect(clampTerminalSize(80, Number.NaN)).toBeUndefined();
  });
});

describe("hostHasTerminalLayout", () => {
  it("requires a visible host with a non-zero box", () => {
    expect(hostHasTerminalLayout(undefined)).toBe(false);
    expect(
      hostHasTerminalLayout({
        hidden: false,
        clientWidth: 0,
        clientHeight: 480,
      } as HTMLElement),
    ).toBe(false);
    expect(
      hostHasTerminalLayout({
        hidden: true,
        clientWidth: 800,
        clientHeight: 480,
      } as HTMLElement),
    ).toBe(false);
    expect(
      hostHasTerminalLayout({
        hidden: false,
        clientWidth: 800,
        clientHeight: 480,
      } as HTMLElement),
    ).toBe(true);
  });
});
