import { describe, expect, it } from "vitest";
import { connectionLabel, controlLabel, controlText } from "./useTaskControl";
import type { TaskControlState } from "../../shared/production";

describe("terminal connection and task execution are separate", () => {
  const state = { taskId:"t", conversationId:"c", state:"awaitingContinue", connection:"connected", queued:false, updatedAt:"", sequence:1 } satisfies TaskControlState;
  it("shows a reattached terminal waiting for an explicit continuation", () => {
    expect(connectionLabel(state)).toBe("终端已连接");
    expect(controlLabel(state)).toBe("等待继续");
    expect(controlText(state)).toContain("确认后点击“继续任务”");
    expect(controlText(state)).not.toContain("断开");
  });
  it("reads connection independently of paused or pending task state", () => {
    expect(connectionLabel({...state, state:"paused"})).toBe("终端已连接");
    expect(connectionLabel({...state, state:"disconnected", connection:"background"})).toContain("后台仍连接");
    expect(controlText({...state, state:"disconnected", connection:"background"})).not.toContain("断开");
    expect(connectionLabel(undefined)).toBe("连接状态核对中");
  });
});
