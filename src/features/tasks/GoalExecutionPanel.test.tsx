// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { GoalExecutionPanel } from "./GoalExecutionPanel";
import type { ProductionRecord, WorkbenchGoal } from "../../shared/production";
afterEach(()=>{cleanup();localStorage.clear();});
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
