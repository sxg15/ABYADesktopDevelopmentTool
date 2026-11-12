// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { StageCard } from "./StageCard";

afterEach(cleanup);
it.each([
  ["passed", "passed", "已通过"], ["blocked", "blocked", "遇到问题"],
  ["in-progress", "active", "进行中"], ["awaiting-confirmation", "active", "待确认"],
  ["not-started", "neutral", "未开始"],
])("keeps the %s status when the stage is viewed", (status, tone, label) => {
  const onView = vi.fn();
  const props = { number: 3, name: "方案预检", status, current: false, onView };
  const { rerender } = render(<StageCard {...props} viewed={false}/>);
  const card = screen.getByRole("button", {name: new RegExp(label)});
  fireEvent.click(card);
  expect(onView).toHaveBeenCalledOnce();
  expect(card.getAttribute("data-tone")).toBe(tone);
  rerender(<StageCard {...props} viewed/>);
  expect(card.getAttribute("aria-pressed")).toBe("true");
  expect(card.getAttribute("aria-current")).toBeNull();
  expect(card.getAttribute("data-tone")).toBe(tone);
});
