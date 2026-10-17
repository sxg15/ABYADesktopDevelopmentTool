import { useMemo, useState } from "react";
import { ArrowDown, ArrowUp, RefreshCw, Search } from "lucide-react";
import { Modal } from "../../app/Modal";
import type { MessageKey } from "../../i18n";
import "./terminalEnhancements.css";

export function historyMatches(text: string, query: string, matchCase = false) {
  const matches: { start: number; end: number }[] = [];
  if (!query) return { matches, limited: false };
  const escaped = query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const pattern = new RegExp(escaped, matchCase ? "gu" : "giu");
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(text))) {
    if (matches.length === 500) return { matches, limited: true };
    matches.push({ start: match.index, end: match.index + match[0].length });
  }
  return { matches, limited: false };
}

export function TerminalHistorySearch({ readSnapshot, onClose, t }: {
  readSnapshot: () => string; onClose: () => void; t: (key: MessageKey) => string;
}) {
  const [snapshot, setSnapshot] = useState(readSnapshot), [query, setQuery] = useState("");
  const [index, setIndex] = useState(0), [matchCase, setMatchCase] = useState(false);
  const { matches, limited } = useMemo(() => historyMatches(snapshot, query, matchCase), [snapshot, query, matchCase]);
  const current = Math.min(index, Math.max(matches.length - 1, 0)), match = matches[current];
  return <Modal wide title={t("terminalSearchHistory")} onClose={onClose}>
    <div className="terminal-history-search">
      <p>{t("terminalSearchHelp")}</p>
      <div className="history-search-controls"><label><Search size={17}/><input autoFocus maxLength={200} aria-label={t("terminalSearchQuery")} value={query} onChange={e => { setQuery(e.target.value); setIndex(0); }}/></label>
        <button className="secondary-button" onClick={() => { setSnapshot(readSnapshot()); setIndex(0); }}><RefreshCw size={15}/>{t("terminalSearchRefresh")}</button></div>
      <label className="history-search-case"><input type="checkbox" checked={matchCase} onChange={e => { setMatchCase(e.target.checked); setIndex(0); }}/>{t("terminalSearchCase")}</label>
      <div className="history-search-navigation"><span role="status">{match ? `${t("terminalSearchPosition")} ${current + 1} / ${matches.length}${limited ? "+" : ""}` : t(query ? "terminalSearchNoMatch" : "terminalSearchEmpty")}</span>
        <button className="icon-button" title={t("terminalSearchPrevious")} disabled={!match} onClick={() => setIndex((current + matches.length - 1) % matches.length)}><ArrowUp size={16}/></button>
        <button className="icon-button" title={t("terminalSearchNext")} disabled={!match} onClick={() => setIndex((current + 1) % matches.length)}><ArrowDown size={16}/></button></div>
      {limited && <p>{t("terminalSearchLimit")}</p>}
      {match && <pre className="history-search-result">{match.start > 320 && "…\n"}{snapshot.slice(Math.max(0, match.start - 320), match.start)}<mark>{snapshot.slice(match.start, match.end)}</mark>{snapshot.slice(match.end, match.end + 500)}{match.end + 500 < snapshot.length && "\n…"}</pre>}
    </div>
  </Modal>;
}
