import { useEffect, useId, useState } from "react";
import { createPortal } from "react-dom";

export function IconButtonTooltips({ closeLabel }: { closeLabel: string }) {
  const id = useId();
  const [tip, setTip] = useState<{ text: string; left: number; top: number; above: boolean }>();
  useEffect(() => {
    let owner: HTMLButtonElement | undefined;
    let originalTitle = "";
    let originalDescription: string | null = null;
    let addedLabel: string | undefined;
    function hide() {
      if (owner) {
        if (!owner.hasAttribute("title") && originalTitle) owner.setAttribute("title", originalTitle);
        const described = owner.getAttribute("aria-describedby")?.split(" ").filter(v => v !== id).join(" ");
        if (described) owner.setAttribute("aria-describedby", described); else owner.removeAttribute("aria-describedby");
        if (addedLabel && owner.getAttribute("aria-label") === addedLabel) owner.removeAttribute("aria-label");
      }
      owner = undefined; addedLabel = undefined; setTip(undefined);
    }
    function show(target: EventTarget | null) {
      const button = target instanceof Element ? target.closest("button") : null;
      if (!(button instanceof HTMLButtonElement) || button.textContent?.trim()
        || !button.querySelector("svg")) return;
      if (button === owner) return;
      hide();
      originalTitle = button.getAttribute("title") ?? "";
      const label = button.getAttribute("aria-label") || originalTitle;
      if (!label) return;
      const text = label === "Close" ? closeLabel : label;
      owner = button;
      // Replace delayed native hover UI without removing the accessible button name.
      if (!button.hasAttribute("aria-label")) { addedLabel = text; button.setAttribute("aria-label", text); }
      button.removeAttribute("title");
      originalDescription = button.getAttribute("aria-describedby");
      button.setAttribute("aria-describedby", [originalDescription, id].filter(Boolean).join(" "));
      const box = button.getBoundingClientRect();
      const above = box.bottom + 72 > window.innerHeight;
      setTip({ text, left: Math.min(Math.max(box.left + box.width / 2, 148), window.innerWidth - 148),
        top: above ? box.top - 8 : box.bottom + 8, above });
    }
    const enter = (e: Event) => show(e.target);
    const leave = (e: Event) => {
      const related = (e as MouseEvent).relatedTarget;
      if (!owner || !(related instanceof Node) || !owner.contains(related)) hide();
    };
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") hide(); };
    document.addEventListener("pointerover", enter);
    document.addEventListener("pointerout", leave);
    document.addEventListener("focusin", enter);
    document.addEventListener("focusout", leave);
    document.addEventListener("pointerdown", hide, true);
    document.addEventListener("scroll", hide, true);
    document.addEventListener("keydown", key);
    window.addEventListener("resize", hide);
    window.addEventListener("blur", hide);
    return () => {
      document.removeEventListener("pointerover", enter);
      document.removeEventListener("pointerout", leave);
      document.removeEventListener("focusin", enter);
      document.removeEventListener("focusout", leave);
      document.removeEventListener("pointerdown", hide, true);
      document.removeEventListener("scroll", hide, true);
      document.removeEventListener("keydown", key);
      window.removeEventListener("resize", hide);
      window.removeEventListener("blur", hide);
      hide();
    };
  }, [closeLabel, id]);
  return tip ? createPortal(<div id={id} role="tooltip" className="icon-action-tooltip"
    style={{ left: tip.left, top: tip.top, transform: tip.above ? "translate(-50%, -100%)" : "translateX(-50%)" }}>
    {tip.text}
  </div>, document.body) : null;
}
