import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fixture } from './bot-workflow.fixture.mjs';
import { validateArtifacts } from '../.codex/skills/abya-game-development-task/scripts/validate-bot-artifacts.mjs';
import { definitionsHash, fileDigest, readJson } from '../.codex/skills/abya-game-development-task/scripts/bot-artifact-core.mjs';

const base = '.codex/skills/abya-game-development-task';
const accept = f => validateArtifacts(f.root, 'acceptance', f);
// 修改捕获并重新计算文件摘要，验证内容语义检查不只比较 SHA-256。
function editEvidence(f, id, edit) {
  const ref = f.report.evidence.find(e => e.id === id), file = path.join(f.root, ref.path);
  const value = readJson(file); edit(value); f.write(ref.path, value); ref.sha256 = fileDigest(file); f.save();
}

for (const provider of ['codex', 'grok']) test(`${provider}: independent deployed validator accepts coherent synthetic materials`, async t => {
  const f = fixture(t, provider);
  const module = await import(`../.${provider}/skills/abya-game-development-task/scripts/validate-bot-artifacts.mjs`);
  assert.equal(module.validateArtifacts(f.root).status, 'candidate-checked');
  assert.equal(module.validateArtifacts(f.root, 'acceptance', f).accepted, true);
});

test('prewrite rejects stale data, unrelated edits and undeclared removal without mutating inputs', t => {
  const f = fixture(t), initial = fileDigest(path.join(f.root, 'definitions.before.json'));
  const current = structuredClone(f.before); current.definitions[0].Name = 'other author';
  f.write('definitions.current.json', current);
  assert.throws(() => validateArtifacts(f.root), /其他写入/);
  f.write('definitions.current.json', f.before);
  f.candidate.definitions[0].Name = 'accidental edit'; f.write('definitions.candidate.json', f.candidate);
  assert.throws(() => validateArtifacts(f.root), /无关定义/);
  f.candidate.definitions.shift(); f.write('definitions.candidate.json', f.candidate);
  assert.throws(() => validateArtifacts(f.root), /无关定义/);
  assert.equal(fileDigest(path.join(f.root, 'definitions.before.json')), initial);
});

test('disabled Bot workflow preserves existing definitions and never clears them', t => {
  const f = fixture(t); f.plan.enabled = false; f.plan.changedDefinitionIds = []; f.plan.bots = [];
  f.write('plan.json', f.plan); f.write('definitions.candidate.json', f.before);
  assert.equal(accept(f).status, 'not-applicable');
  f.write('definitions.candidate.json', { archiveGuid: f.before.archiveGuid, definitions: [] });
  assert.throws(() => accept(f), /无关定义/);
});

test('explicit removal needs correct IDs and separate clear-all intent', t => {
  const f = fixture(t); f.plan.changedDefinitionIds = []; f.plan.removedDefinitionIds = ['original']; f.plan.bots = [];
  f.write('plan.json', f.plan); f.write('definitions.candidate.json', { archiveGuid: f.before.archiveGuid, definitions: [] });
  assert.throws(() => validateArtifacts(f.root), /allowRemoveAll/);
  f.plan.allowRemoveAll = true; f.write('plan.json', f.plan);
  assert.equal(validateArtifacts(f.root).status, 'candidate-checked');
  assert.throws(() => accept(f), /普通玩法回归/);
});

test('numeric readback enums and string input enums have identical hashes', t => {
  const f = fixture(t), numeric = structuredClone(f.candidate);
  for (const d of numeric.definitions) { d.DefaultDifficulty = 1; d.Difficulties.forEach((p, i) => p.Difficulty = i); }
  assert.equal(definitionsHash(numeric), definitionsHash(f.candidate));
  f.write('definitions.reloaded.json', numeric); assert.equal(accept(f).accepted, true);
});

for (const [name, mutate] of [
  ['duplicate IDs', f => f.candidate.definitions.push(f.candidate.definitions[0])],
  ['invalid difficulty', f => f.candidate.definitions[1].Difficulties[0].ThinkInterval = .01],
  ['missing script', f => f.candidate.definitions[1].LuaSource = ''],
  ['duplicate difficulty', f => f.candidate.definitions[1].Difficulties[0].Difficulty = 'Hard'],
  ['wrong archive', f => f.candidate.archiveGuid = 'another-archive']
]) test(`prewrite rejects ${name}`, t => {
  const f = fixture(t); mutate(f); f.write('definitions.candidate.json', f.candidate);
  assert.throws(() => validateArtifacts(f.root));
});

test('message observations require a channel and all four behavioral descriptions', t => {
  const f = fixture(t); delete f.plan.bots[0].observationContract.channel; f.write('plan.json', f.plan);
  assert.throws(() => validateArtifacts(f.root), /消息观察/);
  f.plan.bots[0].observationContract.channel = 'state'; delete f.plan.bots[0].difficultyBehaviors.Hell;
  f.write('plan.json', f.plan); assert.throws(() => validateArtifacts(f.root), /Hell/);
});

for (const [name, mutate] of [
  ['old batch', f => f.run.runId = 'new-run'],
  ['changed provider', f => f.run.provider = 'grok'],
  ['missing static lint', f => f.report.staticChecks.pop()],
  ['incomplete case', f => f.report.cases[0].status = 'not-run'],
  ['missing required kind', f => f.report.cases = f.report.cases.filter(c => c.kind !== 'cleanup')],
  ['missing independent client', f => f.report.cases.find(c => c.kind === 'client-sync').assertions.pop()],
  ['missing difficulty sample', f => f.report.cases.find(c => c.kind === 'difficulty-comparison').assertions.pop()],
  ['false passed assertion', f => f.report.cases[0].assertions[0].expected = false],
  ['escape path', f => f.report.evidence[0].path = '../outside.json'],
  ['old file hash', f => f.report.evidence[0].sha256 = '0'.repeat(64)],
  ['unknown evidence', f => f.report.cases[0].assertions[0].evidenceId = 'missing'],
  ['report extra field', f => f.report.ignoreErrors = true]
]) test(`acceptance rejects ${name}`, t => {
  const f = fixture(t); mutate(f); f.save(); assert.throws(() => accept(f));
});

test('runtime evidence must belong to current archive, actual Bot and declared instance', t => {
  const f = fixture(t);
  editEvidence(f, 'original-after', r => r.target.archiveId = 'other');
  assert.throws(() => accept(f), /版本\/归属/);
  editEvidence(f, 'original-after', r => { r.target.archiveId = 'archive-1'; r.instanceId = 'foreign'; });
  assert.throws(() => accept(f), /实例/);
  editEvidence(f, 'original-after', r => { r.instanceId = 'host-1'; r.botId = 'foreign'; });
  assert.throws(() => accept(f), /前后证据/);
});

test('captured evidence cannot predate run or come from the future', t => {
  const f = fixture(t);
  editEvidence(f, 'original-after', r => r.capturedAt = '2000-01-01T00:00:00Z');
  assert.throws(() => accept(f), /过期/);
  editEvidence(f, 'original-after', r => r.capturedAt = '2999-01-01T00:00:00Z');
  assert.throws(() => accept(f), /过期/);
});

test('submitted or simulated-only progress is not gameplay acceptance', t => {
  const f = fixture(t);
  editEvidence(f, 'original-after', r => r.kind = 'diagnostics');
  assert.throws(() => accept(f), /玩法状态|玩法接受/);
});

test('actual saved files, Player and reloaded definitions invalidate old acceptance', t => {
  const f = fixture(t);
  f.write('archive/AI/new.json', { changed: true }); assert.throws(() => accept(f), /存档内容/);
  fs.unlinkSync(path.join(f.archive, 'AI/new.json'));
  fs.writeFileSync(f.player, 'updated-player'); assert.throws(() => accept(f), /Player/);
  fs.writeFileSync(f.player, 'synthetic-player');
  f.write('definitions.reloaded.json', f.before); assert.throws(() => accept(f), /重载/);
});

test('unchanged Unity exe with updated Managed assembly invalidates the report', t => {
  const f = fixture(t), oldExe = fileDigest(f.player);
  f.write('player_Data/Managed/game.dll', { version: 2 });
  assert.equal(fileDigest(f.player), oldExe);
  assert.throws(() => accept(f), /Player/);
});

test('client synchronization compares actual peer values, not two unrelated expected values', t => {
  const f = fixture(t); editEvidence(f, 'original-client', r => r.data.score = 99);
  assert.throws(() => accept(f), /状态断言失败/);
});

test('report cannot promote a failed static result or missing source capture', t => {
  const f = fixture(t), check = f.report.staticChecks[0], file = path.join(f.root, check.path);
  const value = readJson(file); value.data.ready = false; f.write(check.path, value);
  check.sha256 = fileDigest(file); f.save(); assert.throws(() => accept(f), /静态校验失败/);
  fs.unlinkSync(file); assert.throws(() => accept(f), /ENOENT/);
});

test('CLI returns one JSON and distinct exit codes for passed, incomplete and invalid', t => {
  const f = fixture(t), script = path.resolve(base, 'scripts/validate-bot-artifacts.mjs');
  const run = () => spawnSync(process.execPath, [script, '--root', f.root, '--mode', 'acceptance',
    '--archive', f.archive, '--player', f.player], { encoding: 'utf8' });
  assert.equal(run().status, 0);
  f.report.status = 'runtime-unverified'; f.save();
  const incomplete = run(); assert.equal(incomplete.status, 3); assert.equal(JSON.parse(incomplete.stdout).accepted, false);
  f.report.status = 'invented'; f.save(); assert.equal(run().status, 2);
});

test('junction evidence pointing outside the artifact root is rejected', t => {
  const f = fixture(t), outside = fixture(t);
  fs.symlinkSync(outside.root, path.join(f.root, 'escape'), 'junction');
  outside.write('foreign.json', {}); f.report.evidence[0].path = 'escape/foreign.json'; f.save();
  assert.throws(() => accept(f), /越界/);
  fs.unlinkSync(path.join(f.root, 'escape'));
});

test('both providers distribute the same complete Bot resources and readable relative links', () => {
  const files = ['references/bot-development.md', 'references/bot-artifacts.md',
    'references/gameplay-architecture.md', 'assets/gameplay-architecture.v1.template.json',
    ...fs.readdirSync(`${base}/scripts`).map(name => `scripts/${name}`),
    ...fs.readdirSync(`${base}/assets/bot-integration`).map(name => `assets/bot-integration/${name}`)];
  for (const relative of files) {
    const left = fs.readFileSync(path.join(base, relative), 'utf8');
    const right = fs.readFileSync(path.join(base.replace('.codex', '.grok'), relative), 'utf8');
    assert.equal(left.replaceAll('\r\n', '\n'), right.replaceAll('\r\n', '\n'), relative);
  }
  for (const provider of ['.codex', '.grok']) {
    for (const file of ['abya-game-development-task/SKILL.md', 'abya-game-development-task/references/bot-development.md',
      'abya-task-template-multiplayer-workflow/SKILL.md', 'abya-task-template-multiplayer-workflow/references/network-and-acceptance.md']) {
      const filename = path.join(provider, 'skills', file);
      for (const match of fs.readFileSync(filename, 'utf8').matchAll(/\]\(([^)]+\.md)\)/g)) {
        if (!match[1].includes('://')) assert.ok(fs.existsSync(path.resolve(path.dirname(filename), match[1])), match[1]);
      }
    }
  }
});
