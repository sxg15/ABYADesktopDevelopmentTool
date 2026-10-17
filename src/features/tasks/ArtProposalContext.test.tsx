// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ArtProposalContext } from "./ArtProposalContext";
import { artViewFixture } from "../../shared/artDesign.test-fixtures";
afterEach(cleanup);
it("shows literal visual constraints and only current runtime differences without changing approval", () => {
  const view = artViewFixture(), openRequirements = vi.fn(), review = vi.fn();
  view.record!.currentVersion = "v2";
  view.record!.artDesign!.direction!.visualRules = "保留四种身份色；<script>not executed</script>";
  const difference = { id: "ART-01", target: "物品库存", comparison: "", differences: "最后一枚被遮挡", status: "mismatch", designHash: "design-1", version: "v2", cycle: 1, evidenceIds: [] };
  view.record!.artDesign!.realizations = [difference, { ...difference, id: "old", version: "v1", differences: "历史问题" }];
  const result = render(<ArtProposalContext view={view} en={false} pending={0} onRequirements={openRequirements} onFeedback={vi.fn()} onReview={review}/>);
  fireEvent.click(screen.getByText("当前记录的视觉约束"));
  expect(screen.getByText(/保留四种身份色/).textContent).toContain("<script>not executed</script>");
  expect(result.container.querySelector("script")).toBeNull();
  expect(screen.getByText("1 项当前实机差异或阻塞")).toBeTruthy();
  expect(screen.queryByText("历史问题")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "查看设计依据与制作限制" }));
  expect(openRequirements).toHaveBeenCalledOnce(); expect(review).not.toHaveBeenCalled();
});
it("does not fabricate constraints for a pinned legacy task", () => {
  const view = artViewFixture(); view.artDesignStatus = { ...view.artDesignStatus!, enabled: false };
  const result = render(<ArtProposalContext view={view} en pending={0} onRequirements={vi.fn()} onFeedback={vi.fn()} onReview={vi.fn()}/>);
  expect(result.container.textContent).toBe("");
});
