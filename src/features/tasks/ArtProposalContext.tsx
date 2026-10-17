import { ArrowRight, ClipboardCheck, FileText } from "lucide-react";
import type { ProductionDocument, ProductionView } from "../../shared/production";
import { ReadableText } from "./ReadableText";
import "./visualRefinement.css";

export function ArtProposalContext({ view, en, pending, document, onRequirements, onFeedback, onReview }: {
  view: ProductionView; en: boolean; pending: number; document?: ProductionDocument;
  onRequirements: () => void; onFeedback: () => void; onReview: () => void;
}) {
  const design = view.record?.artDesign;
  const requirements = design?.requirements.filter(item => !item.withdrawn) ?? [];
  const visualRules = design?.direction?.visualRules?.trim();
  const differences = design?.realizations.filter(item => item.designHash === view.artDesignStatus?.designHash
    && item.version === view.record?.currentVersion && item.cycle === view.record?.cycle && item.status !== "matched") ?? [];
  if (!view.artDesignStatus?.enabled) return null;
  return <aside className="art-proposal-context" aria-label={en ? "Requirements beside this proposal" : "本版方案对应的要求"}>
    <h5><ClipboardCheck size={17}/>{en ? "Requirements and review" : "要求与审阅"}</h5>
    {requirements.slice(0, 3).map(item => {
      const source = design?.sources.find(source => source.id === item.sourceId);
      const coverage = design?.coverage.find(coverage => coverage.id === item.id && coverage.inputHash === view.artDesignStatus?.inputHash);
      return <section key={item.id}><small>{item.id} · {source?.kind === "agent-transcribed-conversation" ? (en ? "AI-transcribed source" : "AI 转录来源") : (en ? "Original requirement" : "要求原文")}</small>
        <blockquote>{item.quote}</blockquote>
        {coverage?.designLocation && <p><strong>{en ? "AI-recorded design location" : "AI 记录的设计落点"}</strong>{coverage.designLocation}</p>}
      </section>;
    })}
    {!requirements.length && <p>{en ? "No structured requirements are registered yet. Check the original inputs." : "尚未登记结构化要求，请查看当前原始输入。"}</p>}
    <button className="text-button" onClick={onRequirements}>{en ? `View all ${requirements.length} requirements` : `查看全部 ${requirements.length} 项要求与参考`}<ArrowRight size={14}/></button>
    {visualRules && <details className="art-visual-brief"><summary>{en ? "Recorded visual constraints" : "当前记录的视觉约束"}</summary>
      <p>{en ? "AI-recorded design intent. Compare it with the current proposal and original requirements." : "AI 记录的设计依据，请与本版画面和策划原话一起核对。"}</p>
      <ReadableText text={visualRules}/>
      <button className="text-button" onClick={onRequirements}>{en ? "View rationale and production limits" : "查看设计依据与制作限制"}<ArrowRight size={14}/></button>
    </details>}
    {differences.length > 0 && <details className="art-visual-brief"><summary>{en ? `${differences.length} current runtime differences or blockers` : `${differences.length} 项当前实机差异或阻塞`}</summary>
      {differences.map(item => <section key={item.id}><strong>{item.id} · {item.target}</strong><p>{item.differences || item.comparison}</p></section>)}
    </details>}
    {pending > 0 && <div className="art-proposal-pending"><strong>{en ? `${pending} feedback items remain open` : `${pending} 项反馈尚未关闭`}</strong><p>{en ? "AI updates and your recheck are separate. Inspect the changes before approving this proposal." : "AI 提交修改后，需要你复验具体效果；整份方案仍需单独确认。"}</p><button className="secondary-button" onClick={onFeedback}>{en ? "View changes and rechecks" : "查看修改与复验"}<ArrowRight size={14}/></button></div>}
    {document && <button className="secondary-button" onClick={onReview}><FileText size={15}/>{en ? `Read version ${document.revision}` : `阅读第 ${document.revision} 版确认单`}</button>}
  </aside>;
}
