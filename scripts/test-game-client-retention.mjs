// Owned native acceptance fixture. Never reads real APP data or launches game sentinels.
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";

const [action, directory] = process.argv.slice(2);
if (!["init", "report"].includes(action) || !directory) throw new Error("Usage: init|report <fresh isolated directory>");
const root = path.resolve(directory);
const marker = path.join(root, "client-retention-fixture.json");
const schema = "abya-client-retention-fixture/v1";
if (action === "init") {
  if (fs.existsSync(root)) throw new Error("Use a fresh fixture directory");
  fs.mkdirSync(root, { recursive: true });
  fs.writeFileSync(marker, JSON.stringify({ schema }));
}
const fixture = JSON.parse(fs.readFileSync(marker, "utf8"));
assert.equal(fixture.schema, schema);
const data = path.join(root, "Local", "ABYA Desktop Development Tool", "Data");
fs.mkdirSync(data, { recursive: true });
const db = new DatabaseSync(path.join(data, "app.db"));
db.exec("PRAGMA busy_timeout=5000");
if (action === "init") {
  const records = [1, 2, 3].map(releaseId => {
    const id = crypto.randomUUID();
    const storageRoot = path.join(root, releaseId === 1 ? "历史目录" : "所选目录");
    const directory = path.join(storageRoot, "PlatverseOutput", `${releaseId}-${releaseId}-${id}`);
    fs.mkdirSync(path.join(directory, "Game_Data"), { recursive: true });
    fs.writeFileSync(path.join(directory, "Game.exe"), "MZ non-runnable fixture");
    fs.writeFileSync(path.join(directory, "UnityPlayer.dll"), "non-runnable fixture");
    const record = { id, releaseId, tagName: `离线验收 v${releaseId}`, prerelease: false, publishedAt: "2026-10-16T00:00:00Z", assetId: releaseId, assetName: `Windows-v${releaseId}.zip`, assetUpdatedAt: "2026-10-16", sha256: `owned-fixture-${id}`, digestVerified: false, directory, storageRoot, executableCandidates: ["Game.exe"], installedAt: new Date().toISOString() };
    fs.writeFileSync(path.join(directory, ".abya-client.json"), JSON.stringify(record));
    return record;
  });
  const selected = records[2];
  const local = path.join(root, "本地游戏", "Local.exe");
  fs.mkdirSync(path.dirname(local), { recursive: true }); fs.writeFileSync(local, "MZ non-runnable local fixture");
  const outside = path.join(records[0].storageRoot, "PlatverseOutput", "user-folder");
  fs.mkdirSync(outside); fs.writeFileSync(path.join(outside, "keep.txt"), "user content must remain");
  db.exec("CREATE TABLE settings(key TEXT PRIMARY KEY,value_json TEXT NOT NULL,updated_at TEXT NOT NULL)");
  const insert = db.prepare("INSERT INTO settings VALUES(?,?,?)");
  const gameClient = { source: "remote", localExecutablePath: local, installationId: selected.id, executableRelativePath: "Game.exe", storageRoot: selected.storageRoot };
  insert.run("application", JSON.stringify({ locale: "zh-CN", gameExecutablePath: path.join(selected.directory, "Game.exe"), gameClient, workspaceRootPath: path.join(root, "Workspaces"), desktopCliPort: 43921, gameGatewayPort: 43922, lanBroadcastEnabled: false }), new Date().toISOString());
  insert.run("taskNotifications", JSON.stringify({ enabled: false }), new Date().toISOString());
  for (const record of records) insert.run(`game-clients/installation/${record.id}`, JSON.stringify(record), new Date().toISOString());
  fs.writeFileSync(marker, JSON.stringify({ schema, records, local, outside }));
  console.log(JSON.stringify({ root, localAppData: path.join(root, "Local"), webView: path.join(root, "WebView"), selected: selected.id, before: records.length }));
} else {
  const settings = JSON.parse(db.prepare("SELECT value_json FROM settings WHERE key='application'").get().value_json);
  const rows = db.prepare("SELECT value_json FROM settings WHERE key LIKE 'game-clients/installation/%'").all().map(row => JSON.parse(row.value_json));
  const selected = fixture.records[2];
  assert.equal(rows.length, 1); assert.equal(rows[0].id, selected.id);
  assert.equal(settings.gameClient.installationId, selected.id);
  assert.equal(settings.gameClient.source, "remote");
  assert.equal(settings.gameClient.localExecutablePath, fixture.local);
  assert.equal(settings.lanBroadcastEnabled, false);
  assert.equal(settings.gameExecutablePath, path.join(selected.directory, "Game.exe"));
  for (const old of fixture.records.slice(0, 2)) assert.equal(fs.existsSync(old.directory), false);
  assert.equal(fs.readFileSync(path.join(selected.directory, "Game.exe"), "utf8"), "MZ non-runnable fixture");
  assert.equal(fs.readFileSync(path.join(fixture.outside, "keep.txt"), "utf8"), "user content must remain");
  const authRows = db.prepare("SELECT count(*) AS count FROM settings WHERE key='game-clients/github-account'").get().count;
  assert.equal(authRows, 0);
  console.log(JSON.stringify({ result: "PASS", installedBefore: 3, installedAfter: 1, selectedTag: selected.tagName, obsoleteDirectoriesRemoved: 2, selectedAndLocalPreserved: true, userFolderPreserved: true, githubCredentialRows: authRows }));
}
db.close();
