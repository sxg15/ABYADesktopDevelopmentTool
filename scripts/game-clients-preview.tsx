// Isolated browser fixtures. No GitHub calls, credentials, native writes or game launches.
import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { GameClientsView } from "../src/features/game-clients/GameClientsView";
import { SettingsView } from "../src/features/settings/SettingsView";
import { clientApi, emptyClientSelection, type ClientRelease, type GameClientState } from "../src/shared/gameClients";
import { api } from "../src/shared/api";
import { attentionApi } from "../src/shared/notifications";
import { translator } from "../src/i18n";
import type { AppSettings } from "../src/shared/types";
import "../src/App.css";

let fixture = "populated";
let releases: ClientRelease[] = [
  { id: 2, tagName: "Development-0.8.45", name: "Windows 开发客户端 0.8.45", body: "新增 Bot 诊断支持，修复多人会话恢复。\n此处为隔离界面测试数据，不代表仓库中存在这个版本。", draft: false, prerelease: false, immutable: false, publishedAt: "2026-10-14T08:00:00Z",
    assets: [{ id: 20, name: "ABYA_PB_Development_0.8.45_Windows_x64.zip", size: 1_264_000_000, digest: "sha256:fixture", updatedAt: "2026-10-14", state: "uploaded" }, { id: 21, name: "build-manifest.json", size: 2048, digest: null, updatedAt: "2026-10-14", state: "uploaded" }] },
  { id: 1, tagName: "Development-0.8.44", name: "Windows 开发客户端 0.8.44", body: "上一个稳定版本。", draft: false, prerelease: false, immutable: false, publishedAt: "2026-10-12T08:00:00Z", assets: [] },
];
const installation = { id: "fixture", releaseId: 1, tagName: "Development-0.8.44", prerelease: false, publishedAt: "2026-10-12", assetId: 10, assetName: "ABYA_PB_Windows_x64.zip", assetUpdatedAt: "2026-10-12", sha256: "fixture", digestVerified: true,
  directory: "D:\\游戏客户端\\PlatverseOutput\\1-10-fixture", executableCandidates: ["Windows/ABYA_PB.exe"], installedAt: "2026-10-12T08:00:00Z" };
const state: GameClientState = { repository: "sxg15/PlatverseOutput", account: { login: "preview-account", canManage: true, remembered: true }, defaultStorageRoot: "D:\\游戏客户端", installations: [installation], operation: { active: false, kind: "", stage: "", completedBytes: 0, totalBytes: 0, errorCode: null } };
clientApi.state = async () => ({ ...state, account: fixture === "login" ? null : { ...state.account!, canManage: fixture !== "readonly" } });
clientApi.operation = async () => fixture === "progress" ? { active: true, kind: "install", stage: "downloading", completedBytes: 632_000_000, totalBytes: 1_264_000_000, errorCode: null } : state.operation;
clientApi.releases = async () => fixture === "empty" ? [] : releases;
clientApi.release = async id => releases.find(item => item.id === id)!;
clientApi.login = async () => { fixture = "populated"; return state; };
clientApi.logout = async () => { fixture = "login"; return { ...state, account: null }; };
clientApi.checkUpdate = async () => ({ releaseId: 2, tagName: releases[0].tagName, currentTag: installation.tagName });
clientApi.saveRelease = async input => { const release = { ...releases[0], ...input, id: input.id ?? 3 }; releases = [release, ...releases.filter(item => item.id !== release.id)]; return release; };
clientApi.deleteRelease = async id => { releases = releases.filter(item => item.id !== id); };
clientApi.deleteAsset = async (id, assetId) => { releases = releases.map(item => item.id === id ? { ...item, assets: item.assets.filter(asset => asset.id !== assetId) } : item); };
clientApi.install = async () => { throw { code: "clientIntegrity" }; };
clientApi.cancel = async () => { fixture = "populated"; };
api.paths = async () => ({ dataDir: "D:\\Fixtures\\Data" }) as never;
api.listLanInterfaces = async () => [];
api.desktopCliState = async () => ({ running: false, endpoint: "隔离预览" }) as never;
api.gameConnectionState = async () => ({ running: false, advertisedAddresses: [] }) as never;
attentionApi.settings = async () => ({ enabled: false, available: false, reason: "test", testEnvironment: true }) as never;
const initialSettings: AppSettings = { gameExecutablePath: `${installation.directory}\\Windows\\ABYA_PB.exe`, gameClient: { ...emptyClientSelection(), source: "remote", installationId: "fixture", executableRelativePath: "Windows/ABYA_PB.exe", localExecutablePath: "D:\\LocalPlayer\\ABYA_PB.exe", storageRoot: "D:\\游戏客户端" }, workspaceRootPath: "D:\\任务工作区", locale: "zh-CN", desktopCliPort: 47600, gameGatewayPort: 47610, preferredAdapterId: "", lanBroadcastEnabled: true, toolId: "preview" };
api.updateSettings = async input => ({ ...initialSettings, ...input });
function Preview() {
  const [mode, setMode] = useState("populated"); const [page, setPage] = useState("clients"); const [settings, setSettings] = useState(initialSettings);
  const [language, setLanguage] = useState<"zh-CN" | "en-US">("zh-CN"); const [message, setMessage] = useState(""); const t = translator(language);
  return <><div style={{ height: 40, padding: "6px 12px", display: "flex", alignItems: "center", gap: 12, background: "#fff6d9", fontSize: 12 }}>
    <strong>隔离界面预览 · 无真实 GitHub 操作</strong>
    <select aria-label="Fixture" value={mode} onChange={event => { fixture = event.target.value; setMode(fixture); }}>
      {["populated", "empty", "login", "readonly", "progress"].map(item => <option key={item}>{item}</option>)}
    </select><button onClick={() => setLanguage(language === "zh-CN" ? "en-US" : "zh-CN")}>中文 / English</button><span>{message}</span>
  </div><div className="app-shell" style={{ height: "calc(100% - 40px)" }}>
    <aside className="app-sidebar"><div className="brand"><div className="brand-mark">A</div><div><strong>ABYA</strong><span>Development Tool</span></div></div>
      <nav><button>{t("tasks")}</button><button>{t("archiveTransfer")}</button><button className={page === "clients" ? "active" : ""} onClick={() => setPage("clients")}>{t("gameClients")}</button><button>{t("logs")}</button><button className={page === "settings" ? "active" : ""} onClick={() => setPage("settings")}>{t("settings")}</button></nav>
      <button className="client-sidebar-update" onClick={() => setPage("clients")}><span><strong>{t("gcUpdate")}</strong><span>Development-0.8.45</span></span></button>
    </aside><main className="app-main" key={`${mode}-${language}`}>
      {page === "clients" ? <GameClientsView settings={settings} t={t} notify={setMessage} onOpenSettings={() => setPage("settings")} />
        : <SettingsView settings={settings} locale={language} t={t} notify={setMessage} onSaved={setSettings} onManageClients={() => setPage("clients")} />}
    </main></div></>;
}
createRoot(document.getElementById("root")!).render(<Preview />);
