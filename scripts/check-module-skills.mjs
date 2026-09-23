import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";

const root = process.cwd();
const registry = JSON.parse(
  fs.readFileSync(path.join(root, "module-registry.json"), "utf8"),
);
const required = ["Purpose", "Ownership", "Public Contracts", "Dependencies", "Validation", "LLM Maintenance Rule"];
const bundledTaskSkills = [
  ".codex/skills/abya-game-development-task/SKILL.md",
  ".grok/skills/abya-game-development-task/SKILL.md",
];
const errors = [];
const templateSets = [];
for (const provider of [".codex", ".grok"]) {
  const skills = path.join(root, provider, "skills");
  for (const file of ["SKILL.md", "scripts/import-template.mjs"]) {
    if (!fs.existsSync(path.join(skills, "abya-import-task-template", file))) {
      errors.push(`${provider}: missing template importer ${file}`);
    }
  }
  const templates = new Map();
  for (const entry of fs.readdirSync(skills, { withFileTypes: true })) {
    const kind = ["abya-task-template", "abya-art-template"].find(kind => entry.name.startsWith(`${kind}-`));
    if (!kind) continue;
    try {
      const directory = path.join(skills, entry.name);
      const marker = JSON.parse(fs.readFileSync(path.join(directory, `${kind}.json`), "utf8"));
      const content = fs.readFileSync(path.join(directory, "SKILL.md"), "utf8");
      if (!entry.isDirectory() || marker.schemaVersion !== 1 || marker.kind !== kind
        || marker.id !== entry.name || entry.name.length > 64 || !/^abya-(task|art)-template-[a-z0-9]+(?:-[a-z0-9]+)*$/.test(entry.name)
        || ![marker.displayName, marker.description, marker.sourceSkillName].every(value => typeof value === "string" && value.trim())
        || !content.split(/\r?\n/).includes(`name: ${entry.name}`)) throw new Error("invalid template identity or metadata");
      templates.set(entry.name, JSON.stringify(marker));
    } catch (error) { errors.push(`${provider}/${entry.name}: ${error.message}`); }
  }
  templateSets.push(templates);
}
for (const id of new Set(templateSets.flatMap(templates => [...templates.keys()]))) {
  if (templateSets[0].get(id) !== templateSets[1].get(id)) errors.push(`Task template must have matching metadata for both providers: ${id}`);
}

for (const module of registry.modules) {
  const skillPath = path.join(root, module.skill);
  if (!fs.existsSync(skillPath)) {
    errors.push(`${module.id}: missing ${module.skill}`);
    continue;
  }
  const content = fs.readFileSync(skillPath, "utf8");
  for (const heading of required) {
    if (!content.includes(`## ${heading}`)) {
      errors.push(`${module.id}: ${module.skill} is missing "## ${heading}"`);
    }
  }
}

for (const skillPath of bundledTaskSkills) {
  const absolutePath = path.join(root, skillPath);
  if (!fs.existsSync(absolutePath)) {
    errors.push(`bundled task skill: missing ${skillPath}`);
    continue;
  }
  const content = fs.readFileSync(absolutePath, "utf8");
  if (!content.includes("name: abya-game-development-task")) {
    errors.push(`bundled task skill: ${skillPath} has the wrong skill name`);
  }
}

try {
  const changed = execFileSync("git", ["diff", "--name-only", "HEAD"], {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  })
    .split(/\r?\n/)
    .filter(Boolean)
    .map((value) => value.replaceAll("\\", "/"));
  for (const module of registry.modules) {
    const productionChanged = changed.some((file) =>
      module.paths.some((prefix) => file.startsWith(prefix)),
    );
    if (productionChanged && !changed.includes(module.skill)) {
      errors.push(`${module.id}: production files changed without ${module.skill}`);
    }
  }
} catch {
  // Existence and structure validation still runs before the first commit.
}

if (errors.length > 0) {
  console.error(errors.join("\n"));
  process.exit(1);
}

console.log(`Validated ${registry.modules.length} module Skills.`);
