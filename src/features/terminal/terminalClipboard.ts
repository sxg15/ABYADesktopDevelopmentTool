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
export function terminalInputQueue(write: (data: string) => Promise<void>, onError: (error: unknown) => void, timeoutMs=4000) {
  let queue=Promise.resolve(), halted=false, pendingBytes=0;
  const fail=(error:unknown)=>{if(!halted){halted=true;onError(error);}};
  const send=(data:string)=>{
    if(halted)return Promise.resolve();
    const bytes=new TextEncoder().encode(data).length;
    if(pendingBytes+bytes>1024*1024){fail(new Error("终端待发送输入过多，已停止后续输入。请重建终端连接，不会自动重发。"));return Promise.resolve();}
    pendingBytes+=bytes;
    queue=queue.then(async()=>{
      try {
        for(let index=0;index<data.length&&!halted;){
          let end=Math.min(index+8192,data.length);
          const last=data.charCodeAt(end-1);if(end<data.length&&last>=0xd800&&last<=0xdbff)end--;
          let timer:ReturnType<typeof setTimeout>|undefined;
          try {await Promise.race([write(data.slice(index,end)),new Promise<never>((_,reject)=>{timer=setTimeout(()=>reject(new Error("终端输入未确认，已停止后续输入且不会自动重发。请核对执行结果后重建终端连接。")),timeoutMs);})]);}
          finally {if(timer)clearTimeout(timer);}
          index=end;
        }
      } catch(error){fail(error);} finally{pendingBytes-=bytes;}
    });
    return queue;
  };
  return Object.assign(send,{dispose:()=>{halted=true;}});
}
