// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { historyMatches, TerminalHistorySearch } from "./TerminalHistorySearch";
import { translator } from "../../i18n";
afterEach(cleanup);
it("searches literal Unicode and punctuation without interpreting patterns", () => {
  expect(historyMatches("先读 [a.*]，再读 [a.*]。", "[a.*]").matches).toHaveLength(2);
  expect(historyMatches("方案\n方案", "方案").matches).toEqual([{start:0,end:2},{start:3,end:5}]);
  expect(historyMatches("Status status", "STATUS").matches).toHaveLength(2);
  expect(historyMatches("Status status", "Status", true).matches).toHaveLength(1);
  expect(historyMatches("anything", "").matches).toHaveLength(0);
});
it("bounds large match sets", () => {
  const result = historyMatches("needle ".repeat(2000), "needle");
  expect(result.matches).toHaveLength(500); expect(result.limited).toBe(true);
});
it("keeps a stable snapshot until explicitly refreshed and renders hostile text inertly", () => {
  const read = vi.fn(() => "<img src=x onerror=alert(1)> needle second needle");
  const page = render(<TerminalHistorySearch readSnapshot={read} onClose={vi.fn()} t={translator("en-US")}/>);
  fireEvent.change(screen.getByLabelText("History keyword"), {target:{value:"needle"}});
  expect(screen.getByRole("status").textContent).toBe("Match 1 / 2");
  fireEvent.click(screen.getByTitle("Next match")); expect(screen.getByRole("status").textContent).toBe("Match 2 / 2");
  expect(document.querySelector(".history-search-result img")).toBeNull();
  read.mockReturnValue("needle updated"); page.rerender(<TerminalHistorySearch readSnapshot={read} onClose={vi.fn()} t={translator("en-US")}/>);
  expect(read).toHaveBeenCalledTimes(1); expect(screen.getByRole("status").textContent).toBe("Match 2 / 2");
  fireEvent.click(screen.getByRole("button", {name:"Refresh history snapshot"}));
  expect(screen.getByRole("status").textContent).toBe("Match 1 / 1");
});
