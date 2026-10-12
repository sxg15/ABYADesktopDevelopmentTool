import type { IntakeQuestionGroup } from "../../shared/production";
export type AnswerField = { choice: string; other: string; note: string };
export const otherOption = (value: string) => /^(其他|其它|other)([（(：:\s].*)?$/i.test(value.trim());
export function decodeAnswers(group: IntakeQuestionGroup, answers: Record<string, string>) {
  return Object.fromEntries(group.questions.map(q => {
    const value = answers[q.id] ?? "";
    const choice = q.options.find(o => !otherOption(o) && (value === o || value.startsWith(`${o}\n补充说明：`)));
    return [q.id, { choice: choice ?? (value ? "__other" : ""), other: choice ? "" : value,
      note: choice ? value.slice(choice.length).replace(/^\n补充说明：/, "") : "" }];
  }));
}
export function encodeAnswers(group: IntakeQuestionGroup, fields: Record<string, AnswerField>) {
  return Object.fromEntries(group.questions.map(q => {
    const f = fields[q.id];
    return [q.id, !f ? "" : !q.options.length || f.choice === "__other" ? f.other :
      f.choice ? f.choice + (f.note.trim() ? `\n补充说明：${f.note}` : "") : ""];
  }));
}
