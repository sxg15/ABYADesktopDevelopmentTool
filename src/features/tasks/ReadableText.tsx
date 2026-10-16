import { Fragment } from "react";
import "./readableText.css";

// A deliberately small, text-only presentation layer. No HTML, links or images execute.
function inline(text: string) {
  return text.split(/(\*\*[^*\n]{1,160}\*\*)/g).map((part, i) =>
    part.startsWith("**") && part.endsWith("**") && part.length > 4
      ? <strong key={i}>{part.slice(2, -2)}</strong> : <Fragment key={i}>{part}</Fragment>);
}
function fields(text: string) {
  const label = text.match(/^([^：:\n]{1,24}[：:])(.*)$/);
  return label && !text.startsWith("**")
    ? <><strong className="readable-field">{label[1]}</strong>{inline(label[2])}</> : inline(text);
}

export function ReadableText({ text }: { text: string }) {
  const lines = text.split(/\r?\n/);
  let fence: string | undefined;
  return <div className="readable-text">{lines.map((line, i) => {
    const marker = line.match(/^\s*(`{3,}|~{3,})/);
    const code = !!fence || !!marker;
    if (marker && !fence) fence = marker[1];
    else if (marker && fence && marker[1][0] === fence[0] && marker[1].length >= fence.length) fence = undefined;
    const newline = i < lines.length - 1 ? "\n" : "";
    if (code) return <div className="readable-code-line" key={i}>{line}{newline}</div>;
    if (!line.trim()) return <div className="readable-blank" key={i}>{newline}</div>;
    const heading = line.match(/^\s*(#{1,6})\s+(.+)$/);
    if (heading) return <div role="heading" aria-level={Math.min(heading[1].length + 1, 6)} className="readable-section" key={i}>{inline(heading[2])}{newline}</div>;
    const numbered = line.match(/^(\s*(?:\d+(?:\.\d+)*[.)、]?|[一二三四五六七八九十]+[、.])\s+)(.*)$/);
    const bullet = line.match(/^(\s*[-*+]\s+)(.*)$/);
    const prefix = numbered ?? bullet;
    return <div className={`readable-line${prefix ? " readable-item" : ""}${i === 0 ? " readable-lead" : ""}`} key={i}>
      {prefix ? <><span className={numbered ? "readable-number" : "readable-bullet"}>{prefix[1]}</span><span>{fields(prefix[2])}{newline}</span></> : <>{fields(line)}{newline}</>}
    </div>;
  })}</div>;
}
