// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { GameClientsView } from "./GameClientsView";
import { GameClientSettings } from "./GameClientSettings";
import { GitHubAccountSettings } from "./GitHubAccountSettings";
import { openUrl } from "@tauri-apps/plugin-opener";
import { SettingsView } from "../settings/SettingsView";
import { clientApi, emptyClientSelection, type GameClientState, type ClientRelease, type ClientInstallation, type GameClientSelection, type LoginAuthorization } from "../../shared/gameClients";
import { api } from "../../shared/api";
import { translator } from "../../i18n";
import type { AppSettings } from "../../shared/types";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
vi.mock("../task-notifications/NotificationSettingsPanel", () => ({ NotificationSettingsPanel: () => null }));
const t = translator("zh-CN");
const operation = { active: false, kind: "", stage: "", completedBytes: 0, totalBytes: 0, errorCode: null };
const release: ClientRelease = { id: 1, tagName: "v1.0", name: "Windows v1.0", body: "Test notes", draft: false, prerelease: false, immutable: false,
  publishedAt: "2026-10-14T00:00:00Z", assets: [{ id: 10, name: "Windows.zip", size: 2048, digest: null, updatedAt: "2026-10-14", state: "uploaded" }] };
const record: ClientInstallation = { id: "installed-1", releaseId: 1, tagName: "v1.0", prerelease: false, publishedAt: "2026-10-14", assetId: 10,
  assetName: "Windows.zip", assetUpdatedAt: "2026-10-14", sha256: "abc", digestVerified: true, directory: "D:\\Clients\\one", executableCandidates: ["Game.exe"], installedAt: "2026-10-14T00:00:00Z" };
let state: GameClientState;
const authorization = (): LoginAuthorization => ({ id: "attempt", userCode: "ABCD-EFGH", verificationUri: "https://github.com/login/device", expiresAt: new Date(Date.now() + 900_000).toISOString(), pollAfterSeconds: 1 });
const view = (onOpenSettings = vi.fn()) => render(<GameClientsView t={t} notify={vi.fn()} onOpenSettings={onOpenSettings} />);

beforeEach(() => {
  state = { repository: "sxg15/PlatverseOutput", account: { login: "fixture", canManage: true, remembered: true }, installations: [], defaultStorageRoot: "D:\\Clients", operation };
  vi.spyOn(clientApi, "state").mockImplementation(async () => structuredClone(state));
  vi.spyOn(clientApi, "operation").mockResolvedValue(operation);
  vi.spyOn(clientApi, "releases").mockResolvedValue([release]);
  vi.spyOn(clientApi, "release").mockResolvedValue(release);
  vi.spyOn(clientApi, "startLogin").mockImplementation(async () => authorization());
  vi.spyOn(clientApi, "pollLogin").mockImplementation(async () => ({ status: "pending", authorization: authorization() }));
  vi.spyOn(clientApi, "cancelLogin").mockResolvedValue();
  vi.mocked(openUrl).mockResolvedValue();
  vi.spyOn(clientApi, "logout").mockImplementation(async () => { state.account = null; return state; });
  vi.spyOn(clientApi, "deleteRelease").mockResolvedValue();
  vi.spyOn(clientApi, "deleteAsset").mockResolvedValue();
  vi.spyOn(clientApi, "install").mockImplementation(async () => { state.installations = [record]; return record; });
  vi.spyOn(clientApi, "checkUpdate").mockResolvedValue(null);
  vi.spyOn(api, "updateSettings");
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it("shows the empty private repository with a create action after login", async () => {
  vi.mocked(clientApi.releases).mockResolvedValue([]); view();
  await screen.findByText(t("gcNoReleases"));
  expect((screen.getByRole("button", { name: t("gcNewRelease") }) as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: t("gcNewRelease") }));
  expect(screen.getByRole("dialog", { name: t("gcNewRelease") })).toBeTruthy();
  expect((screen.getByLabelText(t("gcDraft")) as HTMLInputElement).checked).toBe(true);
});

it("routes unsigned users to settings and exposes no login form in client management", async () => {
  state.account = null; const navigate = vi.fn(); view(navigate);
  await screen.findByText(t("gcNotLoggedIn"));
  expect(clientApi.releases).not.toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: t("gcLogin") })).toBeNull();
  expect(document.querySelector("input[type=password]")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: t("gcGoToLogin") }));
  expect(navigate).toHaveBeenCalledWith(undefined, true);
  expect(clientApi.startLogin).not.toHaveBeenCalled();
});

it("settings starts browser authorization with the selected persistence option and can cancel it", async () => {
  state.account = null; render(<GitHubAccountSettings t={t} onChanged={vi.fn()} />);
  await waitFor(() => expect((screen.getByRole("button", { name: t("gcLogin") }) as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(screen.getByLabelText(t("gcRemember")));
  fireEvent.click(screen.getByRole("button", { name: t("gcLogin") }));
  const code = await screen.findByLabelText(t("gcDeviceCode"));
  expect((code as HTMLInputElement).value).toBe("ABCD-EFGH");
  expect(clientApi.startLogin).toHaveBeenCalledWith(false);
  expect(openUrl).toHaveBeenCalledWith("https://github.com/login/device");
  fireEvent.click(screen.getByRole("button", { name: t("gcCancelLogin") }));
  await screen.findByText(t("gcOAuthCancelled"));
  expect(clientApi.cancelLogin).toHaveBeenCalledWith("attempt");
  expect(screen.queryByLabelText(t("gcDeviceCode"))).toBeNull();
});

it("settings completes authorization automatically and refreshes dependent client controls", async () => {
  state.account = null; state.authorization = authorization(); const changed = vi.fn();
  vi.mocked(clientApi.pollLogin).mockImplementation(async () => {
    state.account = { login: "browser-user", canManage: true, remembered: true }; state.authorization = null;
    return { status: "authenticated", authorization: null };
  });
  render(<GitHubAccountSettings t={t} onChanged={changed} />);
  await screen.findByLabelText(t("gcDeviceCode"));
  await screen.findByText(`${t("gcSignedInAs")}: browser-user`, {}, { timeout: 3000 });
  expect(changed).toHaveBeenCalledOnce();
  expect(screen.queryByLabelText(t("gcDeviceCode"))).toBeNull();
  expect(api.updateSettings).not.toHaveBeenCalled();
});

it("settings displays denied authorization and allows a fresh sign-in", async () => {
  state.account = null; state.authorization = authorization();
  vi.mocked(clientApi.pollLogin).mockRejectedValue({ code: "clientOAuthDenied" });
  render(<GitHubAccountSettings t={t} onChanged={vi.fn()} />);
  await screen.findByText(t("gcOAuthDenied"), {}, { timeout: 3000 });
  expect(screen.queryByLabelText(t("gcDeviceCode"))).toBeNull();
  expect((screen.getByRole("button", { name: t("gcLogin") }) as HTMLButtonElement).disabled).toBe(false);
});

it("settings expires an old code locally without exchanging it", async () => {
  state.account = null; state.authorization = { ...authorization(), expiresAt: new Date(Date.now() - 1000).toISOString() };
  render(<GitHubAccountSettings t={t} onChanged={vi.fn()} />);
  await screen.findByText(t("gcOAuthExpired"), {}, { timeout: 3000 });
  expect(clientApi.pollLogin).not.toHaveBeenCalled();
  expect(clientApi.cancelLogin).toHaveBeenCalledWith("attempt");
});

it("settings ignores a late poll response after cancellation", async () => {
  state.account = null; state.authorization = authorization(); const changed = vi.fn();
  let finish!: (value: { status: "authenticated"; authorization: null }) => void;
  vi.mocked(clientApi.pollLogin).mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  render(<GitHubAccountSettings t={t} onChanged={changed} />);
  await waitFor(() => expect(clientApi.pollLogin).toHaveBeenCalledOnce(), { timeout: 3000 });
  fireEvent.click(screen.getByRole("button", { name: t("gcCancelLogin") }));
  await screen.findByText(t("gcOAuthCancelled"));
  finish({ status: "authenticated", authorization: null });
  await waitFor(() => expect(screen.queryByLabelText(t("gcDeviceCode"))).toBeNull());
  expect(changed).not.toHaveBeenCalled();
});

it("settings preserves the authorization code and recovery link when the browser cannot open", async () => {
  state.account = null; vi.mocked(openUrl).mockRejectedValue(new Error("unavailable"));
  render(<GitHubAccountSettings t={t} onChanged={vi.fn()} />);
  await waitFor(() => expect((screen.getByRole("button", { name: t("gcLogin") }) as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: t("gcLogin") }));
  await screen.findByText(t("gcBrowserFailed"));
  expect((screen.getByLabelText(t("gcDeviceCode")) as HTMLInputElement).value).toBe("ABCD-EFGH");
  expect(screen.getByRole("button", { name: t("gcOpenBrowser") })).toBeTruthy();
});

it("settings drops the pending authorization and refreshes consumers on logout", async () => {
  state.authorization = authorization(); const changed = vi.fn();
  render(<GitHubAccountSettings t={t} onChanged={changed} />);
  await screen.findByLabelText(t("gcDeviceCode"));
  fireEvent.click(screen.getByRole("button", { name: t("gcLogout") }));
  await screen.findByText(t("gcNotLoggedIn"));
  expect(clientApi.logout).toHaveBeenCalledOnce(); expect(changed).toHaveBeenCalledOnce();
  expect(screen.queryByLabelText(t("gcDeviceCode"))).toBeNull();
});

it("requires exact-target confirmation and cancel performs no delete", async () => {
  view(); await screen.findByText("Test notes");
  fireEvent.click(screen.getByRole("button", { name: t("gcDeleteRelease") }));
  const dialog = screen.getByRole("dialog");
  expect(within(dialog).getByText("v1.0")).toBeTruthy();
  expect(clientApi.deleteRelease).not.toHaveBeenCalled();
  fireEvent.click(within(dialog).getByRole("button", { name: t("cancel") }));
  expect(clientApi.deleteRelease).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: `${t("gcDeleteAsset")}: Windows.zip` }));
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: t("gcDeleteConfirmButton") }));
  await waitFor(() => expect(clientApi.deleteAsset).toHaveBeenCalledWith(1, 10));
});

it("read-only accounts can install but cannot create, upload, edit or delete", async () => {
  state.account!.canManage = false; view(); await screen.findByText("Test notes");
  for (const name of [t("gcNewRelease"), t("gcUpload"), t("gcEditRelease"), t("gcDeleteRelease")]) {
    expect((screen.getByRole("button", { name }) as HTMLButtonElement).disabled).toBe(true);
  }
  expect((screen.getByRole("button", { name: t("gcInstall") }) as HTMLButtonElement).disabled).toBe(false);
});

it("installing a release does not save settings or force navigation", async () => {
  const navigate = vi.fn(); view(navigate); await screen.findByText("Test notes");
  fireEvent.click(screen.getByRole("button", { name: t("gcInstall") }));
  await screen.findByText("v1.0 · Windows.zip");
  expect(clientApi.install).toHaveBeenCalledWith(1, 10, "D:\\Clients");
  expect(api.updateSettings).not.toHaveBeenCalled(); expect(navigate).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: t("gcUseInstalled") }));
  expect(navigate).toHaveBeenCalledWith("installed-1");
});

function SelectionHarness({ initial, preferred }: { initial: GameClientSelection; preferred?: string }) {
  const [selection, setSelection] = useState(initial);
  return <><GameClientSettings value={selection} onChange={setSelection} t={t} onManage={vi.fn()} onBusyChange={vi.fn()} preferredInstallationId={preferred} />
    <output data-testid="selection">{JSON.stringify(selection)}</output></>;
}

it("keeps the local path while selecting an offline installed remote version and requires one of multiple EXEs", async () => {
  state.account = null; state.installations = [{ ...record, executableCandidates: ["Game.exe", "Other.exe"] }];
  render(<SelectionHarness initial={{ ...emptyClientSelection(), localExecutablePath: "C:\\Old\\Game.exe" }} preferred="installed-1" />);
  const exe = await screen.findByLabelText(t("gcExe"));
  expect((exe as HTMLSelectElement).value).toBe("");
  fireEvent.change(exe, { target: { value: "Other.exe" } });
  expect(JSON.parse(screen.getByTestId("selection").textContent!).executableRelativePath).toBe("Other.exe");
  fireEvent.click(screen.getByRole("button", { name: t("gcLocal") }));
  expect((screen.getByLabelText(t("gameExecutable")) as HTMLInputElement).value).toBe("C:\\Old\\Game.exe");
  expect(api.updateSettings).not.toHaveBeenCalled();
});

it("shows a newer version as optional and retains the selected installation", async () => {
  state.installations = [record];
  vi.mocked(clientApi.checkUpdate).mockResolvedValue({ releaseId: 2, tagName: "v2", currentTag: "v1.0" });
  render(<SelectionHarness initial={{ ...emptyClientSelection(), source: "remote", installationId: record.id, executableRelativePath: "Game.exe" }} />);
  await screen.findByRole("option", { name: /v1.0 · Windows.zip/ });
  fireEvent.click(screen.getByRole("button", { name: t("gcCheckUpdate") }));
  await screen.findByText(`${t("gcUpdate")}: v2`);
  expect(JSON.parse(screen.getByTestId("selection").textContent!).installationId).toBe(record.id);
  expect(clientApi.install).not.toHaveBeenCalled();
});

it("saves the source, local fallback and remote selection together only on Save", async () => {
  state.installations = [record];
  const settings: AppSettings = { gameExecutablePath: "C:\\Old.exe", workspaceRootPath: "D:\\Work", locale: "zh-CN", desktopCliPort: 47600,
    gameGatewayPort: 47610, preferredAdapterId: "", lanBroadcastEnabled: true, toolId: "fixture" };
  vi.spyOn(api, "desktopCliState").mockResolvedValue({ running: false } as never);
  vi.spyOn(api, "gameConnectionState").mockResolvedValue({ running: false, advertisedAddresses: [] } as never);
  vi.spyOn(api, "listLanInterfaces").mockResolvedValue([]);
  vi.spyOn(api, "paths").mockResolvedValue({ dataDir: "D:\\Fixture" } as never);
  vi.mocked(api.updateSettings).mockResolvedValue({ ...settings, gameExecutablePath: "D:\\Clients\\one\\Game.exe" });
  render(<SettingsView settings={settings} locale="zh-CN" t={t} onSaved={vi.fn()} notify={vi.fn()} onManageClients={vi.fn()} preferredInstallationId={record.id} />);
  await screen.findByLabelText(t("gcExe"));
  expect(api.updateSettings).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: t("save") }));
  await waitFor(() => expect(api.updateSettings).toHaveBeenCalledWith(expect.objectContaining({
    gameClient: expect.objectContaining({ source: "remote", installationId: record.id, executableRelativePath: "Game.exe", localExecutablePath: "C:\\Old.exe" }),
  })));
});
