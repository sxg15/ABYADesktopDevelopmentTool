// Disposable native UI fixture. No real account, network release, or runnable game.
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { DatabaseSync } from "node:sqlite";

const [action, directory] = process.argv.slice(2);
if (!["init", "report"].includes(action) || !directory) throw new Error("Usage: node scripts/test-game-clients-native.mjs init|report <isolated directory>");
const root = path.resolve(directory);
const marker = path.join(root, "game-clients-fixture.json");
if (action === "init" && fs.existsSync(root)) throw new Error("Use a fresh fixture directory");
if (action === "report" && !fs.existsSync(marker)) throw new Error("Not an owned fixture");
if (action === "init") {
  fs.mkdirSync(root, { recursive: true });
  fs.writeFileSync(marker, JSON.stringify({ schema: "abya-game-clients-fixture/v1" }));
}
if (JSON.parse(fs.readFileSync(marker, "utf8")).schema !== "abya-game-clients-fixture/v1") throw new Error("Wrong marker");
const data = path.join(root, "Local", "ABYA Desktop Development Tool", "Data");
fs.mkdirSync(data, { recursive: true });
const db = new DatabaseSync(path.join(data, "app.db"));
db.exec("PRAGMA busy_timeout=5000");
if (action === "init") {
  const local = path.join(root, "LocalPlayer", "Legacy.exe");
  const install = path.join(root, "Clients", "PlatverseOutput", "fixture");
  fs.mkdirSync(path.join(install, "Game_Data"), { recursive: true });
  fs.mkdirSync(path.dirname(local), { recursive: true });
  fs.writeFileSync(local, "MZ non-runnable UI fixture");
  fs.writeFileSync(path.join(install, "Game.exe"), "MZ non-runnable UI fixture");
  fs.writeFileSync(path.join(install, "UnityPlayer.dll"), "non-runnable UI fixture");
  const id = crypto.randomUUID();
  const record = { id, releaseId: 1, tagName: "本地选择验收样本", prerelease: false, publishedAt: "2026-10-14T00:00:00Z", assetId: 1,
    assetName: "UI-Fixture-Windows.zip", assetUpdatedAt: "2026-10-14T00:00:00Z", sha256: "fixture-not-from-github", digestVerified: false,
    directory: install, storageRoot: path.join(root, "Clients"), executableCandidates: ["Game.exe"], installedAt: new Date().toISOString() };
  fs.writeFileSync(path.join(install, ".abya-client.json"), JSON.stringify(record));
  db.exec("CREATE TABLE settings(key TEXT PRIMARY KEY,value_json TEXT NOT NULL,updated_at TEXT NOT NULL)");
  const insert = db.prepare("INSERT INTO settings VALUES(?,?,?)");
  insert.run("application", JSON.stringify({ locale: "zh-CN", gameExecutablePath: local, workspaceRootPath: path.join(root, "Workspaces"), desktopCliPort: 43921, gameGatewayPort: 43922, lanBroadcastEnabled: false }), new Date().toISOString());
  insert.run("taskNotifications", JSON.stringify({ enabled: false }), new Date().toISOString());
  insert.run(`game-clients/installation/${id}`, JSON.stringify(record), new Date().toISOString());
  console.log(JSON.stringify({ root, localAppData: path.join(root, "Local"), webView: path.join(root, "WebView"), installationId: id }));
} else {
  const settings = JSON.parse(db.prepare("SELECT value_json FROM settings WHERE key='application'").get().value_json);
  console.log(JSON.stringify({ gameExecutablePath: settings.gameExecutablePath, gameClient: settings.gameClient,
    githubCredentialRows: db.prepare("SELECT count(*) AS count FROM settings WHERE key='game-clients/github-account'").get().count }, null, 2));
}
db.close();
