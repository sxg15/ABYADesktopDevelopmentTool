// xterm follows the PTY cursor on every render, including the intermediate cursor
// positions of a streaming TUI redraw. An IME must stay anchored while choosing text.
// Keep xterm in charge of composition values, key handling and the final onData event.
export function installTerminalComposition(host: HTMLElement) {
  let active = false;
  let releaseTimer: ReturnType<typeof setTimeout> | undefined;
  const clear = () => {
    active = false;
    host.classList.remove("terminal-ime-composing");
    for (const name of ["left", "top", "height", "line-height"]) {
      host.style.removeProperty(`--terminal-ime-${name}`);
    }
  };
  const start = (event: Event) => {
    if (!(event.target instanceof HTMLTextAreaElement)
      || !event.target.classList.contains("xterm-helper-textarea")) return;
    if (releaseTimer !== undefined) clearTimeout(releaseTimer);
    // Drop the preceding composition's override before measuring a new anchor.
    clear();
    const style = getComputedStyle(event.target);
    for (const name of ["left", "top", "height", "line-height"]) {
      host.style.setProperty(`--terminal-ime-${name}`, style.getPropertyValue(name));
    }
    active = true;
    host.classList.add("terminal-ime-composing");
  };
  const end = (event: Event) => {
    if (!(event.target instanceof HTMLTextAreaElement)
      || !event.target.classList.contains("xterm-helper-textarea")) return;
    if (releaseTimer !== undefined) clearTimeout(releaseTimer);
    // Bubble after xterm's compositionend handler; its delayed commit runs first.
    releaseTimer = setTimeout(clear, 0);
  };
  host.addEventListener("compositionstart", start, true);
  host.addEventListener("compositionend", end);
  host.addEventListener("focusout", end);
  return {
    isComposing: () => active,
    dispose() {
      if (releaseTimer !== undefined) clearTimeout(releaseTimer);
      host.removeEventListener("compositionstart", start, true);
      host.removeEventListener("compositionend", end);
      host.removeEventListener("focusout", end);
      clear();
    },
  };
}
