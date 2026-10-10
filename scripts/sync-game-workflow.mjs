import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Repository bundles only. Task workspaces and installed Publish content are not touched.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const skill = 'abya-game-development-task';
const commonFiles = [
  'references/production-stages.md',
  'references/production-records.md',
  'references/runtime-authoring.md',
  'references/whole-experience-review.md',
  'references/recording.md',
  'references/cli-commands.md',
  'references/gameplay-architecture.md',
  'references/bot-development.md',
  'assets/production/workflow-policy.json',
  'assets/production/workflow.template.json',
  'assets/production/round.template.json',
  'assets/production/requirements.template.md',
  'assets/production/plan.template.md',
  'assets/production/delivery.template.md',
  'assets/production/closeout.template.md',
];
const read = relative => fs.readFileSync(path.join(root, relative), 'utf8')
  .replace(/^\uFEFF/, '').replaceAll('\r\n', '\n');
const args = process.argv.slice(2);
if (args.some(arg => arg !== '--check') || args.length > 1) {
  console.error('Usage: node scripts/sync-game-workflow.mjs [--check]');
  process.exit(2);
}
const check = args.includes('--check');
const prefix = provider => `${provider}/skills/${skill}`;
const writes = commonFiles.map(file => ({
  target: `${prefix('.grok')}/${file}`,
  body: read(`${prefix('.codex')}/${file}`),
}));
for (const file of ['SKILL.md', 'scripts/import-template.mjs']) {
  writes.push({ target: `.grok/skills/abya-import-task-template/${file}`,
    body: read(`.codex/skills/abya-import-task-template/${file}`) });
}
for (const name of ['abya-task-template-multiplayer-workflow', 'abya-art-template-comic-arcade-ui']) {
  writes.push({ target: `.grok/skills/${name}/SKILL.md`, body: read(`.codex/skills/${name}/SKILL.md`) });
}
const entry = read(`${prefix('.codex')}/SKILL.md`);
const end = entry.indexOf('\n---\n', 4);
if (!entry.startsWith('---\n') || end < 0) throw new Error('Invalid canonical Skill frontmatter');
// Keep Grok-specific discovery metadata without maintaining a second business workflow.
const grokEntry = `${entry.slice(0, end)}\nwhen-to-use: Use for complete ABYA gameplay, major gameplay changes, or an explicitly requested production stage.\nuser-invocable: true${entry.slice(end)}`;
writes.push({ target: `${prefix('.grok')}/SKILL.md`, body: grokEntry });
const different = writes.filter(({ target, body }) => !fs.existsSync(path.join(root, target)) || read(target) !== body);
if (check && different.length) {
  console.error(`Stale managed workflow resources:\n${different.map(x => x.target).join('\n')}`);
  process.exit(1);
}
for (const { target, body } of check ? [] : different) {
  fs.mkdirSync(path.dirname(path.join(root, target)), { recursive: true });
  fs.writeFileSync(path.join(root, target), body, 'utf8');
}
console.log(check ? 'Managed workflow resources match.' : `Updated ${different.length} managed workflow resources.`);
