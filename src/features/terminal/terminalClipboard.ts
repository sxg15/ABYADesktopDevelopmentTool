import type { Terminal } from "@xterm/xterm";

export function installTerminalClipboard(
  host: HTMLElement, terminal: Pick<Terminal, "attachCustomKeyEventHandler" | "paste" | "modes">,
  pasteImage: () => void, reportError: () => void,
  readClipboard: () => Promise<{ kind: "text"; text: string } | { kind: "image" | "empty" }>,
) {
  let disposed = false;
  let reading = false;
  const text = (value: string) => {
    if (disposed) return;
    if (new TextEncoder().encode(value).length > 1024 * 1024 || !terminal.modes.bracketedPasteMode) {
      reportError(); return;
    }
    // Keep text literal: never allow clipboard control sequences to escape paste mode.
    terminal.paste(value.replace(/[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]/g, ""));
  };
  const read = async (plain: boolean) => {
    if (reading || disposed) return;
    reading = true;
    try {
      const content = await readClipboard();
      if (content.kind === "text") text(content.text);
      else if (!plain && !disposed && content.kind === "image") pasteImage();
    } catch { if (!disposed) reportError(); }
    finally { reading = false; }
  };
  terminal.attachCustomKeyEventHandler(event => {
    if (!event.isComposing && (event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === "v") {
      event.preventDefault(); event.stopPropagation();
      if (event.type === "keydown" && !event.repeat) void read(event.shiftKey);
      return false;
    }
    return true;
  });
  const onPaste = (event: ClipboardEvent) => {
    event.preventDefault(); event.stopImmediatePropagation();
    if (reading || disposed || !event.clipboardData) return;
    const data = event.clipboardData;
    if (Array.from(data.types).includes("text/plain")) text(data.getData("text/plain"));
    else if (Array.from(data.items).some(item => item.type.startsWith("image/"))) pasteImage();
  };
  host.addEventListener("paste", onPaste, true);
  return () => { disposed = true; host.removeEventListener("paste", onPaste, true); };
}

// Each IPC write stays below the backend's 64 KiB UTF-8 bound; preserve order and surrogate pairs.
export function terminalInputQueue(write: (data: string) => Promise<void>, onError: (error: unknown) => void) {
  let queue = Promise.resolve();
  return (data: string) => {
    const chunks: string[] = [];
    for (let index = 0; index < data.length;) {
      let end = Math.min(index + 8192, data.length);
      const last = data.charCodeAt(end - 1);
      if (end < data.length && last >= 0xd800 && last <= 0xdbff) end--;
      chunks.push(data.slice(index, end)); index = end;
    }
    queue = queue.then(async () => { for (const chunk of chunks) await write(chunk); }).catch(onError);
    return queue;
  };
}
