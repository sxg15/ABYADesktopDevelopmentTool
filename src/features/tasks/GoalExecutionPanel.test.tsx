// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { GoalExecutionPanel,goalExecutionStatus } from "./GoalExecutionPanel";
import type { ProductionRecord, WorkbenchGoal } from "../../shared/production";
afterEach(()=>{cleanup();localStorage.clear();});
it.each(["repairRequired","externalBlocked"])("keeps a same-goal recovery action for %s",async state=>{
 const goal={id:"goal",stage:"implementation",cycle:1,status:"in-progress",sourceType:"work"} as WorkbenchGoal;
 const record={taskId:"task",revision:8,currentStage:"implementation",cycle:1,workbench:{activeGoalId:"goal",execution:{state},attempts:[{id:"attempt",goalId:"goal",status:"finished"}]}} as unknown as ProductionRecord;
 const resume=vi.fn().mockResolvedValue({status:"started"});render(<GoalExecutionPanel record={record} goal={goal} active checking={false} onResume={resume}/>);
 fireEvent.click(screen.getByRole("button",{name:"诊断并恢复此目标"}));await waitFor(()=>expect(resume).toHaveBeenCalledOnce());
 expect(resume.mock.calls[0][0]).toBe("goal");expect(screen.queryByText("本轮已结束，查看结果与缺口")).toBeNull();
});
it("exhausted preflight cannot mask a newly running or uncertain attempt",()=>{
 const goal={id:"goal",stage:"resources",cycle:1,status:"in-progress",sourceType:"work"} as WorkbenchGoal;
 const record={taskId:"task",currentStage:"resources",workbench:{execution:{preflight:{state:"exhausted",goalId:"goal"}},attempts:[{goalId:"goal",status:"finished"}]}} as unknown as ProductionRecord;
 expect(goalExecutionStatus(record,goal)).toBe("recoveryExhausted");
 record.workbench!.attempts![0].status="running";expect(goalExecutionStatus(record,goal)).toBe("running");
 record.workbench!.attempts![0].status="needsReview";expect(goalExecutionStatus(record,goal)).toBe("needsReview");
});
it("resumes the original interrupted goal with one stable intent and retained history",async()=>{
  const goal={id:"original-goal",title:"同一个实现目标",stage:"implementation",cycle:1,status:"in-progress",sourceType:"work"} as WorkbenchGoal;
  const record={taskId:"task",revision:7,currentStage:"implementation",cycle:1,workbench:{attempts:[{id:"old-attempt",goalId:goal.id,status:"interrupted",reason:"旧执行已退出",updatedAt:"2026-10-22T01:00:00Z"}]}} as ProductionRecord;
  const resume=vi.fn().mockRejectedValueOnce(new Error("transport lost")).mockResolvedValueOnce({status:"started"});
  render(<GoalExecutionPanel record={record} goal={goal} active checking={false} onResume={resume}/>);
  expect(screen.getByText("被中断")).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"继续此子流程"}));
  await screen.findByRole("alert");
  fireEvent.click(screen.getByRole("button",{name:"继续此子流程"}));
  await waitFor(()=>expect(resume).toHaveBeenCalledTimes(2));
  expect(resume.mock.calls[0]).toEqual(["original-goal",7,expect.any(String),undefined]);
  expect(resume.mock.calls[1]).toEqual(resume.mock.calls[0]);
  expect(screen.getByText("old-attempt")).toBeTruthy();
});
it.each(["finished","interrupted","paused","failed"])("does not ask the user to continue a converged autonomous goal after %s",(status)=>{
 const goal={id:"goal",stage:"implementation",cycle:1,status:"in-progress",sourceType:"work"} as WorkbenchGoal;
 const record={taskId:"task",revision:1,currentStage:"implementation",cycle:1,workbench:{activeGoalId:"goal",execution:{state:"settledWithGaps"},attempts:[{id:"done",goalId:"goal",status,updatedAt:"2026-10-23T00:00:00Z"}]}} as unknown as ProductionRecord;
 render(<GoalExecutionPanel record={record} goal={goal} active checking={false} onResume={vi.fn()}/>);
 expect(screen.getAllByText("本轮已结束，查看结果与缺口").length).toBeGreaterThan(0);
 expect(screen.queryByRole("button",{name:"继续此子流程"})).toBeNull();expect(screen.getByText("done")).toBeTruthy();
});
