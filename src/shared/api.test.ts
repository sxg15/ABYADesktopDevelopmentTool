import { beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { terminalApi } from "./api";
import type { TerminalProvider } from "./types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue([]) }));
beforeEach(() => vi.clearAllMocks());

it("rejects a stale provider before invoking any native command", async () => {
  const retired = "grok" as TerminalProvider;
  await expect(terminalApi.listConversations(retired, "task")).rejects.toThrow("Unsupported");
  await expect(terminalApi.write(retired, "old-conversation", "hello")).rejects.toThrow("Unsupported");
  await expect(terminalApi.copyHistory(retired, "task", "old-conversation")).rejects.toThrow("Unsupported");
  expect(invoke).not.toHaveBeenCalled();
});

it("retains Codex native command names and conversation ownership arguments", async () => {
  await terminalApi.listConversations("codex", "task");
  await terminalApi.copyHistory("codex", "task", "conversation");
  expect(invoke).toHaveBeenCalledWith("list_codex_conversations", { taskId: "task" });
  expect(invoke).toHaveBeenCalledWith("copy_terminal_history", { provider: "codex", taskId: "task", conversationId: "conversation" });
});
