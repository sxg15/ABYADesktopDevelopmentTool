// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api } from "../shared/api";
import { useDesktopUpdate } from "./useDesktopUpdate";

const build = { version: "0.4.0", releaseNumber: 1, verification: "verified", builtAt: "", release: "", dataDirectory: "", executable: "", testEnvironment: false };
const release = { version: "0.4.0", releaseNumber: 2, notes: "new", packageUrl: "https://github.com/example" };
beforeEach(() => {
  localStorage.clear();
  vi.spyOn(api, "latestDesktopRelease").mockResolvedValue(release);
  vi.spyOn(api, "desktopHasActiveTurn").mockResolvedValue(false);
});
afterEach(() => vi.restoreAllMocks());

it("offers a same-version newer build; snooze survives restart and manual check reopens", async () => {
  const start = () => renderHook(() => useDesktopUpdate(build, false, vi.fn()));
  const first = start();
  await waitFor(() => expect(first.result.current.dialog).toBeTruthy());
  act(() => { const dialog = first.result.current.dialog; if (dialog) dialog.props.onClose(); });
  first.unmount();
  const second = start();
  await waitFor(() => expect(second.result.current.checking).toBe(false));
  expect(second.result.current.dialog).toBeFalsy();
  await act(() => second.result.current.check(true));
  expect(second.result.current.dialog).toBeTruthy();
  second.unmount();
});

it("keeps a non-modal reminder during a live turn until idle", async () => {
  vi.useFakeTimers();
  try {
    vi.mocked(api.desktopHasActiveTurn).mockResolvedValueOnce(true);
    const hook = renderHook(() => useDesktopUpdate(build, false, vi.fn()));
    await act(async () => {});
    expect(hook.result.current.reminder).toBeTruthy();
    expect(hook.result.current.dialog).toBeFalsy();
    await act(() => vi.advanceTimersByTimeAsync(5000));
    expect(hook.result.current.dialog).toBeTruthy();
    hook.unmount();
  } finally { vi.useRealTimers(); }
});

it("recovers from offline checks and ignores older packages", async () => {
  vi.mocked(api.latestDesktopRelease).mockRejectedValueOnce(new Error("offline"))
    .mockResolvedValueOnce({ ...release, releaseNumber: 1 });
  const notify = vi.fn();
  const hook = renderHook(() => useDesktopUpdate(build, true, notify));
  await waitFor(() => expect(hook.result.current.checking).toBe(false));
  expect(notify).not.toHaveBeenCalled();
  await act(async () => { window.dispatchEvent(new Event("online")); });
  expect(hook.result.current.dialog).toBeFalsy();
  await act(() => hook.result.current.check(true));
  expect(hook.result.current.dialog).toBeTruthy();
  hook.unmount();
});
