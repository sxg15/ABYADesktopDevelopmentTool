// Isolated native acceptance fixtures. Never reads or modifies normal ABYA data.
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { DatabaseSync } from "node:sqlite";

const [operation, directory, argument = "1", taskIndex = "1"] = process.argv.slice(2);
if (!directory || !["init", "question", "review", "resolve", "archive", "delete", "disable", "enable", "corrupt", "report"].includes(operation)) {
  throw new Error("Usage: node scripts/test-task-notifications.mjs <operation> <isolated directory> [revision] [task index]");
}
const root = path.resolve(directory), marker = path.join(root, "notification-fixture.json");
if (operation === "init" && fs.existsSync(root)) throw new Error("Fixture initialization requires a fresh directory.");
if (operation !== "init" && !fs.existsSync(marker)) throw new Error("Not an owned notification fixture.");
if (operation === "init") {
  fs.mkdirSync(root, { recursive: true });
  fs.writeFileSync(marker, JSON.stringify({ schema: "abya-notification-fixture/v1", ids: [crypto.randomUUID(), crypto.randomUUID()] }));
}
const fixture = JSON.parse(fs.readFileSync(marker, "utf8"));
if (fixture.schema !== "abya-notification-fixture/v1") throw new Error("Wrong fixture schema.");
const data = path.join(root, "Local", "ABYA Desktop Development Tool", "Data");
fs.mkdirSync(data, { recursive: true });
const db = new DatabaseSync(path.join(data, "app.db"));
db.exec("PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;");
const now = () => new Date().toISOString();
const id = fixture.ids[Number(taskIndex)];
if (!id) throw new Error("Invalid fixture task index.");
if (operation === "init") {
  db.exec(`CREATE TABLE settings(key TEXT PRIMARY KEY,value_json TEXT NOT NULL,updated_at TEXT NOT NULL);
    CREATE TABLE development_tasks(id TEXT PRIMARY KEY,title TEXT NOT NULL,description TEXT NOT NULL DEFAULT '',status TEXT NOT NULL,
      created_at TEXT NOT NULL,updated_at TEXT NOT NULL,completed_at TEXT,archived_at TEXT,workspace_path TEXT NOT NULL DEFAULT '');
    CREATE TABLE task_production(task_id TEXT PRIMARY KEY REFERENCES development_tasks(id) ON DELETE CASCADE,revision INTEGER NOT NULL,body TEXT NOT NULL);`);
  db.prepare("INSERT INTO settings VALUES('application',?,?)").run(JSON.stringify({ locale: "zh-CN", desktopCliPort: 43781,
    gameGatewayPort: 43782, lanBroadcastEnabled: false, workspaceRootPath: path.join(root, "Workspaces") }), now());
  const policy = JSON.parse(fs.readFileSync(new URL("../.codex/skills/abya-game-development-task/assets/production/workflow-policy.json", import.meta.url), "utf8"));
  for (const [index, taskId] of fixture.ids.entries()) {
    const workspace = path.join(root, "Workspaces", taskId);
    fs.mkdirSync(workspace, {recursive: true});
    db.prepare("INSERT INTO development_tasks(id,title,description,status,created_at,updated_at,workspace_path) VALUES(?,?,?,'active',?,?,?)")
      .run(taskId, index ? "后台任务 B · 翻卡片" : "前台任务 A · 通知验收", "仅用于通知验收，不启动 AI", now(), now(), workspace);
    const record = {schema:"abya.game-development-workflow/v1",taskId,revision:1,workflowVersion:policy.workflowVersion,policy,
      questionMode:"ask",playerMode:"single",currentStage:"requirements",currentRound:0,cycle:1,
      stages:Object.fromEntries(policy.stages.map(s=>[s.id,s.id === "requirements" ? "in-progress" : "not-started"])),
      documents:{},approvals:[],questionGroups:[],versionDetails:{},issues:[],evidence:[],rounds:[],milestones:{},checks:[],knowledge:[],skillPins:[],updatedAt:now()};
    db.prepare("INSERT INTO task_production VALUES(?,1,?)").run(taskId,JSON.stringify(record));
  }
} else if (operation === "report") {
  console.log(JSON.stringify({ids:fixture.ids, tasks:db.prepare("SELECT id,status FROM development_tasks").all(),
    receipts:db.prepare("SELECT task_id,disposition,active,event_key FROM task_notification_receipts ORDER BY observed_at").all(),
    production:db.prepare("SELECT task_id,body FROM task_production").all().map(r=>{
      try {const v=JSON.parse(r.body);return {taskId:r.task_id,revision:v.revision,answers:v.questionGroups?.flatMap(g=>g.answers).length,approvals:v.approvals.length};}
      catch {return {taskId:r.task_id,unreadable:true};}
    })},null,2));
} else if (["disable","enable"].includes(operation)) {
  db.prepare("INSERT INTO settings VALUES('taskNotifications',?,?) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json")
    .run(JSON.stringify({enabled:operation === "enable"}),now());
} else if (operation === "delete") {
  db.prepare("DELETE FROM development_tasks WHERE id=?").run(id);
} else if (operation === "archive") {
  db.prepare("UPDATE development_tasks SET status='archived' WHERE id=?").run(id);
} else if (operation === "corrupt") {
  db.prepare("UPDATE task_production SET body='invalid fixture' WHERE task_id=?").run(id);
} else {
  const record = JSON.parse(db.prepare("SELECT body FROM task_production WHERE task_id=?").get(id).body);
  record.revision++; record.updatedAt=now();
  if (operation === "question") {
    record.questionGroups = [{id:"question-"+argument,stage:"requirements",cycle:1,title:"通知验收问题 "+argument,provider:"codex",
      conversationId:"00000000-0000-0000-0000-000000000001",questions:[{id:"color",text:"请选择卡片颜色",options:["蓝色","绿色"],optional:false}],
      draft:{},answers:[],status:"pending",publishedAt:now(),updatedAt:now()}];
  } else if (operation === "review") {
    record.questionGroups=[];
    const relative="artifacts/game-development/requirements.md";
    const workspace = db.prepare("SELECT workspace_path FROM development_tasks WHERE id=?").get(id).workspace_path;
    const file=path.join(workspace,relative),content="# 通知验收需求\n\n第 "+argument+" 版：仅核对通知与导航，不执行玩法制作。\n";
    fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,content);
    record.documents.requirements={kind:"requirements",revision:Number(argument),path:relative,sha256:crypto.createHash("sha256").update(content).digest("hex"),content,submittedAt:now()};
    record.stages.requirements="awaiting-confirmation";
  } else {
    record.questionGroups.forEach(g=>{g.status="answered";});
    record.stages.requirements="in-progress";
  }
  db.prepare("UPDATE task_production SET revision=?,body=? WHERE task_id=?").run(record.revision,JSON.stringify(record),id);
}
db.close();
if (operation !== "report") console.log(JSON.stringify({operation,root,taskId:id}));
