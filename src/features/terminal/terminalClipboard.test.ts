// @vitest-environment jsdom
import { describe, expect, it, vi } from "vitest";
import { installTerminalClipboard, terminalInputQueue } from "./terminalClipboard";

function fixture(content: { kind: "text"; text: string } | { kind: "image" | "empty" }) {
  let key!: (event: KeyboardEvent) => boolean;
  const host = document.createElement("div");
  const paste = vi.fn(), image = vi.fn(), error = vi.fn();
  const read = vi.fn(async () => content);
  const terminal = { attachCustomKeyEventHandler: (handler: typeof key) => { key = handler; }, paste, modes: { bracketedPasteMode: true } };
  const dispose = installTerminalClipboard(host, terminal as unknown as Parameters<typeof installTerminalClipboard>[1], image, error, read);
  const press = (shift = false) => key(new KeyboardEvent("keydown", { key: "v", ctrlKey: true, shiftKey: shift, cancelable: true }));
  return { host, terminal, paste, image, error, read, dispose, press, key: (e: KeyboardEvent) => key(e) };
}
describe("terminal paste routing", () => {
  it("pastes Chinese multiline text once without forwarding Ctrl+V or a submit key", async () => {
    const f = fixture({ kind: "text", text: "中文\n第二行" });
    expect(f.press()).toBe(false);
    f.key(new KeyboardEvent("keyup", { key: "v", ctrlKey: true }));
    await vi.waitFor(() => expect(f.paste).toHaveBeenCalledExactlyOnceWith("中文\n第二行"));
    expect(f.image).not.toHaveBeenCalled(); expect(f.read).toHaveBeenCalledTimes(1);
    expect(f.key(new KeyboardEvent("keydown", { key: "c", ctrlKey: true }))).toBe(true);
    f.dispose();
  });
  it("only forwards image paste when an image is present; Shift+V stays text-only", async () => {
    const f = fixture({ kind: "image" }); f.press(true);
    await vi.waitFor(() => expect(f.read).toHaveBeenCalledOnce());
    expect(f.image).not.toHaveBeenCalled();
    f.press(); await vi.waitFor(() => expect(f.image).toHaveBeenCalledOnce());
    expect(f.paste).not.toHaveBeenCalled(); f.dispose();
  });
  it("handles right-click paste once and strips terminal escape sequences", () => {
    const f = fixture({ kind: "empty" });
    const event = new Event("paste", { cancelable: true });
    Object.defineProperty(event, "clipboardData", { value: { types: ["text/plain"], getData: () => "a\x1b[201~\x03b", items: [] } });
    f.host.dispatchEvent(event);
    expect(f.paste).toHaveBeenCalledExactlyOnceWith("a[201~b");
    expect(event.defaultPrevented).toBe(true); f.dispose();
    f.host.dispatchEvent(event); expect(f.paste).toHaveBeenCalledTimes(1);
  });
  it("refuses text until bracketed paste is enabled", async () => {
    const f = fixture({ kind: "text", text: "line1\nline2" });
    f.terminal.modes.bracketedPasteMode = false; f.press();
    await vi.waitFor(() => expect(f.error).toHaveBeenCalledOnce());
    expect(f.paste).not.toHaveBeenCalled(); f.dispose();
  });
  it("chunks large Unicode input below IPC limits and preserves typing order", async () => {
    const received: string[] = [];
    const send = terminalInputQueue(async data => { received.push(data); }, error => { throw error; });
    const text = "汉字🐟".repeat(30000);
    const pasted = send(text); const typed = send("after"); await Promise.all([pasted, typed]);
    expect(received.join("")).toBe(text + "after");
    expect(received.every(chunk => new TextEncoder().encode(chunk).length < 65536)).toBe(true);
    expect(received.every(chunk => !/^[\udc00-\udfff]|[\ud800-\udbff]$/.test(chunk))).toBe(true);
  });
});
