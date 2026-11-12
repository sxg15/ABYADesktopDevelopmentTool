import { expect, it } from "vitest";
import type { ProductionIssue, ProductionRecord } from "../../shared/production";
import { issueObservation, outstandingChecks } from "./playtestObservations";

it("prefers planner-facing summaries and reads older issue records", () => {
  const old = {description:"original symptom"} as ProductionIssue;
  expect(issueObservation(old)).toBe("original symptom");
  expect(issueObservation({...old, playerSummary:"  new symptom  "})).toBe("new symptom");
  expect(issueObservation({...old, description:" ", diagnosis:{reproduction:"repro"}})).toBe("repro");
});

it("keeps only current, latest observations not represented by a shown issue", () => {
  const check = {id:"input", version:"v2", cycle:2, status:"failed", recordedAt:"", observations:"missed input"};
  const record = {currentVersion:"v2",cycle:2,selfTests:[
    check, {...check,status:"passed"}, {...check,id:"old-version",version:"v1"},
    {...check,id:"old-cycle",cycle:1}, {...check,id:"linked",blockedByIssueIds:["open-1"]},
    {...check,id:"visible",status:"blocked",blockedByIssueIds:["closed-1"]}
  ]} as ProductionRecord;
  expect(outstandingChecks(record,new Set(["open-1"])).map(c => c.id)).toEqual(["visible"]);
  expect(outstandingChecks({...record,currentVersion:undefined},new Set())).toEqual([]);
});
