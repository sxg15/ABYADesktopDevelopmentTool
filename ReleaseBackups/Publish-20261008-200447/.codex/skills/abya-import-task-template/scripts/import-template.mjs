import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const providers = [".codex", ".grok"];

function readTree(directory, relative = "") {
  const files = new Map();
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    const key = path.join(relative, entry.name);
    if (entry.isSymbolicLink()) throw new Error(`Skill must not contain links: ${absolute}`);
    if (entry.isDirectory()) {
      if ([".git", ".codex", ".grok", "node_modules"].includes(entry.name)) {
        throw new Error(`Provide a standalone Skill folder without ${entry.name}`);
      }
      for (const [name, bytes] of readTree(absolute, key)) files.set(name, bytes);
    } else if (entry.isFile()) files.set(key, fs.readFileSync(absolute));
    else throw new Error(`Unsupported Skill entry: ${absolute}`);
  }
  return files;
}

export function importTemplate({ source, destination, slug, displayName, description, replace = false, kind = "task" }) {
  if (!["task", "art"].includes(kind)) throw new Error("--kind must be task or art");
  const templateKind = `abya-${kind}-template`;
  const marker = `${templateKind}.json`;
  const prefix = `${templateKind}-`;
  if (![source, destination, slug, displayName, description].every(value => typeof value === "string" && value.trim())) {
    throw new Error("Required: --source --destination --slug --display-name --description");
  }
  const id = `${prefix}${slug}`;
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(slug) || id.length > 64) throw new Error("Invalid template slug");
  const sourceRoot = fs.realpathSync(source);
  const root = fs.realpathSync(destination);
  const files = readTree(sourceRoot);
  const entry = files.get("SKILL.md")?.toString("utf8").replace(/^\uFEFF/, "");
  const frontmatter = entry?.match(/^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/);
  const nameValue = frontmatter?.[1].match(/^name:\s*([a-z0-9-]+)\s*$/m)?.[1];
  if (!nameValue || !/^description:/m.test(frontmatter[1])) {
    throw new Error("SKILL.md requires frontmatter with a plain lowercase name and description");
  }
  if (["abya-task-template.json", "abya-art-template.json"].some(file => files.has(file))) throw new Error("Use the original source Skill, not an already imported template");
  const targets = providers.map(provider => {
    const skills = path.join(root, provider, "skills");
    if (!fs.existsSync(path.join(skills, "abya-game-development-task", "SKILL.md"))) {
      throw new Error(`Destination is missing the bundled workflow: ${skills}`);
    }
    const actual = fs.realpathSync(skills);
    if (!actual.startsWith(root + path.sep)) throw new Error("Destination skills folder escapes the selected root");
    const target = path.join(skills, id);
    if (sourceRoot === target || sourceRoot.startsWith(target + path.sep) || target.startsWith(sourceRoot + path.sep)) {
      throw new Error("Source and destination must not overlap");
    }
    if (fs.existsSync(target)) {
      if (fs.lstatSync(target).isSymbolicLink()) throw new Error(`Refusing linked destination: ${target}`);
      let existing;
      try { existing = JSON.parse(fs.readFileSync(path.join(target, marker), "utf8")); } catch { /* collision below */ }
      if (existing?.schemaVersion !== 1 || existing?.kind !== templateKind || existing?.id !== id) {
        throw new Error(`Refusing to overwrite an unmanaged Skill: ${target}`);
      }
      if (existing.sourceSkillName !== nameValue && !replace) throw new Error(`Different source requires --replace: ${target}`);
    }
    return target;
  });
  // Preserve optional frontmatter and all original resources. Put the selector
  // description last so YAML block descriptions in the source remain supported.
  const header = frontmatter[1]
    .replace(/^name:.*$/m, `name: ${id}`)
    .replace(/^description:[^\r\n]*(?:\r?\n[ \t]+[^\r\n]*)*/m, `description: ${JSON.stringify(`${kind === "art" ? "美术风格" : "任务"}模板：${description}。通过创建任务流程选择，或用户明确指定此模板时使用。`)}`);
  const routing = kind === "art"
    ? `\n## 美术模板入口\n\n本 Skill 只定义视觉风格，不替代玩法任务流程。遵循同级 ../abya-game-development-task/SKILL.md 的评估后美术选择流程；用户明确选择后再加载本模板及参考素材。保持已确认的玩法、设备和输入契约。项目或任务指定的字体优先于下文的字体建议；ABYA 多人任务的文本自由物体和 CustomUI 必须使用 DingTalk，不因粗体黑体或等宽数字建议换用其他字体。随包素材是可复用来源，须按实际接口导入、赋值、读回并完成实机验收，不能仅复制文件。\n\n`
    : `\n## 任务模板入口\n\n先读取同级 ../abya-game-development-task/SKILL.md 的模板选择与公共约束。\n用户明确指定本模板即视为已选择，不重复询问、不递归调用入口。使用下方流程组织任务；\n实例绑定、只读可行性、已有授权、架构校验和验收要求仍由公共流程提供。\n\n`;
  files.set("SKILL.md", Buffer.from(`---\n${header}\n---\n${routing}${entry.slice(frontmatter[0].length)}`));
  for (const [name, bytes] of files) {
    if (/\.(md|ya?ml)$/i.test(name)) files.set(name, Buffer.from(bytes.toString("utf8").replaceAll(`$${nameValue}`, `$${id}`)));
  }
  files.set(marker, Buffer.from(JSON.stringify({ schemaVersion: 1, kind: templateKind, id, displayName, description, sourceSkillName: nameValue }, null, 2) + "\n"));
  // Stage both providers before replacing either one; roll back on write failure.
  const staged = [];
  try {
    for (const target of targets) {
      const temporary = fs.mkdtempSync(path.join(path.dirname(target), ".template-import-"));
      const item = { target, temporary, backup: temporary + "-backup", installed: false };
      staged.push(item);
      for (const [name, bytes] of files) {
        const file = path.join(temporary, name);
        fs.mkdirSync(path.dirname(file), { recursive: true });
        fs.writeFileSync(file, bytes);
      }
    }
    for (const item of staged) {
      if (fs.existsSync(item.target)) fs.renameSync(item.target, item.backup);
      fs.renameSync(item.temporary, item.target);
      item.installed = true;
    }
  } catch (error) {
    for (const item of staged.reverse()) {
      if (item.installed) fs.rmSync(item.target, { recursive: true, force: true });
      if (fs.existsSync(item.backup)) fs.renameSync(item.backup, item.target);
    }
    throw error;
  } finally {
    for (const item of staged) fs.rmSync(item.temporary, { recursive: true, force: true });
  }
  for (const item of staged) fs.rmSync(item.backup, { recursive: true, force: true });
  return { id, targets, fileCount: files.size };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    const options = {};
    const names = { "--source": "source", "--destination": "destination", "--slug": "slug", "--display-name": "displayName", "--description": "description", "--kind": "kind" };
    for (let i = 2; i < process.argv.length; i++) {
      const flag = process.argv[i];
      if (flag === "--replace") options.replace = true;
      else if (names[flag]) options[names[flag]] = process.argv[++i];
      else throw new Error(`Unknown option: ${flag}`);
    }
    console.log(JSON.stringify(importTemplate(options), null, 2));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
