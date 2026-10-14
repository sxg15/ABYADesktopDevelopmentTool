// @vitest-environment jsdom
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { attentionApi, type NotificationActivation } from "../shared/notifications";
import { useNotificationActivations } from "./useNotificationActivations";
import { attentionFixture } from "../shared/notifications.test-fixtures";
import { StrictMode } from "react";
vi.mock("@tauri-apps/api/event", () => ({listen:vi.fn(async () => vi.fn())}));
vi.mock("../shared/notifications", () => ({attentionApi:{takeActivations:vi.fn(),acknowledgeActivations:vi.fn(async () => {})}}));
afterEach(() => {cleanup();vi.clearAllMocks();});
function Receiver({receive}:{receive:(activation:NotificationActivation)=>void}) {useNotificationActivations(receive);return null;}
it("receives a startup click once in StrictMode and acknowledges only after routing",async()=>{
  const activation={id:"click",status:"current",target:attentionFixture().items[0].target} as NotificationActivation;
  vi.mocked(attentionApi.takeActivations).mockResolvedValue([activation]);
  const receive=vi.fn();render(<StrictMode><Receiver receive={receive}/></StrictMode>);
  await waitFor(()=>expect(receive).toHaveBeenCalledOnce());
  expect(attentionApi.acknowledgeActivations).toHaveBeenCalledWith(["click"]);
  expect(receive.mock.invocationCallOrder[0]).toBeLessThan(vi.mocked(attentionApi.acknowledgeActivations).mock.invocationCallOrder[0]);
});
it("leaves a late read unacknowledged when the app unmounts",async()=>{
  let finish!:(value:NotificationActivation[])=>void;
  vi.mocked(attentionApi.takeActivations).mockReturnValue(new Promise(resolve=>{finish=resolve;}));
  const receive=vi.fn(), page=render(<Receiver receive={receive}/>);
  await waitFor(()=>expect(attentionApi.takeActivations).toHaveBeenCalled());page.unmount();
  await act(async()=>finish([{id:"late",status:"test",target:null}]));
  expect(receive).not.toHaveBeenCalled();expect(attentionApi.acknowledgeActivations).not.toHaveBeenCalled();
});
