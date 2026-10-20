// Explicit personal installation only; the repository bundle remains the canonical source.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const [mode, destination] = process.argv.slice(2);
if (!['--check', '--write'].includes(mode) || !destination || process.argv.length !== 4)
  throw new Error('Usage: node scripts/sync-design-skills.mjs --check|--write <personal-skills-directory>');
const root = path.resolve(destination);
if (!fs.statSync(root).isDirectory()) throw new Error('The explicit personal Skills directory must exist.');
const source = path.join(repository, '.codex/skills/abya-game-development-task/references/game-art-design');
function noLinks(file) {
  for (let p = file; p !== path.dirname(p); p = path.dirname(p))
    if (fs.existsSync(p) && fs.lstatSync(p).isSymbolicLink()) throw new Error(`Refusing linked path: ${p}`);
}
noLinks(root);
const writes = [];
function collect(from, to) {
  noLinks(from); noLinks(to);
  if (!to.startsWith(root + path.sep)) throw new Error('Destination escaped the Skills directory.');
  for (const entry of fs.readdirSync(from, { withFileTypes: true })) {
    const input = path.join(from, entry.name), output = path.join(to, entry.name);
    noLinks(input); noLinks(output);
    if (entry.isDirectory()) collect(input, output);
    else if (entry.isFile()) {
      const bytes = fs.readFileSync(input);
      const normalize = body => body.toString('utf8').replaceAll('\r\n', '\n');
      const textFile = /\.(?:md|json|mjs|c?js|py|ya?ml|txt|html?|css|svg)$/i.test(input);
      const previous = fs.existsSync(output) ? fs.readFileSync(output) : undefined;
      const matches = previous && (textFile ? normalize(previous) === normalize(bytes) : previous.equals(bytes));
      if (!matches) writes.push({ output, bytes });
    } else throw new Error(`Unsupported source: ${input}`);
  }
}
collect(source, path.join(root, 'game-art-design'));
collect(path.join(source, 'references/product-interface-design'), path.join(root, 'product-interface-design'));
collect(path.join(source, 'references/game-ui-design'), path.join(root, 'game-ui-design'));
if (mode === '--check') {
  console.log(JSON.stringify({ match: !writes.length, changedFiles: writes.map(x => path.relative(root, x.output)) }, null, 2));
  process.exitCode = writes.length ? 1 : 0;
} else if (writes.length) {
  const backup = path.join(path.dirname(root), 'skill-backups', `visual-refinement-${Date.now()}-${crypto.randomUUID()}`);
  noLinks(backup);
  for (const { output } of writes) {
    if (!fs.existsSync(output)) continue;
    const saved = path.join(backup, path.relative(root, output));
    fs.mkdirSync(path.dirname(saved), { recursive: true }); fs.copyFileSync(output, saved);
  }
  for (const { output, bytes } of writes) {
    fs.mkdirSync(path.dirname(output), { recursive: true }); fs.writeFileSync(output, bytes);
  }
  console.log(JSON.stringify({ updated: writes.length, backup, note: 'Unrelated files were not removed.' }));
} else console.log('Personal design Skills match the canonical bundle.');
