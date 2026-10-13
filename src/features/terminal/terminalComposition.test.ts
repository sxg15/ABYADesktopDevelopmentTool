// @vitest-environment jsdom
import {afterEach, expect, it, vi} from "vitest";
import {installTerminalComposition} from "./terminalComposition";

afterEach(()=>{document.body.replaceChildren();vi.useRealTimers();});
function fixture() {
  const host=document.createElement("div");
  const textarea=document.createElement("textarea");
  textarea.className="xterm-helper-textarea";
  textarea.style.cssText="position:absolute;left:24px;top:300px;height:16px;line-height:16px";
  host.append(textarea);document.body.append(host);
  return {host,textarea,guard:installTerminalComposition(host)};
}
it("keeps the composition anchor while output changes the terminal cursor",()=>{
  const {host,textarea,guard}=fixture();
  textarea.value="ni";
  textarea.dispatchEvent(new CompositionEvent("compositionstart",{bubbles:true}));
  textarea.style.top="40px";textarea.style.left="210px";
  textarea.dispatchEvent(new CompositionEvent("compositionupdate",{bubbles:true,data:"nihao"}));
  expect(host.style.getPropertyValue("--terminal-ime-top")).toBe("300px");
  expect(host.style.getPropertyValue("--terminal-ime-left")).toBe("24px");
  expect(guard.isComposing()).toBe(true);
  expect(textarea.value).toBe("ni"); // Value and commit remain entirely xterm-owned.
  guard.dispose();
});
it("releases after the native commit handler and refreshes the next anchor",()=>{
  vi.useFakeTimers();
  const {host,textarea,guard}=fixture();
  textarea.dispatchEvent(new CompositionEvent("compositionstart",{bubbles:true}));
  let anchoredAtCommit=false;
  textarea.addEventListener("compositionend",()=>setTimeout(()=>{anchoredAtCommit=guard.isComposing();},0));
  textarea.dispatchEvent(new CompositionEvent("compositionend",{bubbles:true}));
  vi.runAllTimers();
  expect(anchoredAtCommit).toBe(true);
  expect(host.classList.contains("terminal-ime-composing")).toBe(false);
  textarea.style.top="320px";
  textarea.dispatchEvent(new CompositionEvent("compositionstart",{bubbles:true}));
  expect(host.style.getPropertyValue("--terminal-ime-top")).toBe("320px");
  guard.dispose();
});
it("a late release cannot cancel the next composition; blur and disposal clear the anchor",()=>{
  vi.useFakeTimers();
  const {host,textarea,guard}=fixture();
  textarea.dispatchEvent(new CompositionEvent("compositionstart",{bubbles:true}));
  textarea.dispatchEvent(new CompositionEvent("compositionend",{bubbles:true}));
  textarea.dispatchEvent(new CompositionEvent("compositionstart",{bubbles:true}));
  vi.runAllTimers();expect(guard.isComposing()).toBe(true);
  textarea.dispatchEvent(new FocusEvent("focusout",{bubbles:true}));
  vi.runAllTimers();expect(guard.isComposing()).toBe(false);
  guard.dispose();
  textarea.dispatchEvent(new CompositionEvent("compositionstart",{bubbles:true}));
  expect(host.style.getPropertyValue("--terminal-ime-top")).toBe("");
});
