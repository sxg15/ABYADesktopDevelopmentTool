import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { archiveHash, definitionsHash, fileDigest, playerHash } from '../.codex/skills/abya-game-development-task/scripts/bot-artifact-core.mjs';

// 合成测试材料只验证检查器行为，不作为任何真实玩法验收证据。
export function fixture(t, provider = 'codex') {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'abya-bot-contract-'));
  t.after(() => {
    const resolved = fs.realpathSync(root), temp = fs.realpathSync(os.tmpdir());
    assert.equal(path.dirname(resolved), temp);
    assert.ok(path.basename(resolved).startsWith('abya-bot-contract-'));
    fs.rmSync(resolved, { recursive: true, force: true });
  });
  const write = (name, value) => {
    const target = path.join(root, name);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, JSON.stringify(value, null, 2));
    return target;
  };
  const definition = id => ({ ID: id, Name: id, LuaSource: 'function Init() end\nfunction OnThink() end',
    LuaTextResourceID: '', DefaultDifficulty: 'Medium', Difficulties: ['Easy', 'Medium', 'Hard', 'Hell']
      .map((Difficulty, i) => ({ Difficulty, ThinkInterval: .7 - i * .1, ReactionDelay: .1, Accuracy: .6 + i * .1, Lookahead: i })) });
  const before = { archiveGuid: 'archive-1', definitions: [definition('original')] };
  const candidate = { archiveGuid: 'archive-1', definitions: [definition('original'), definition('new')] };
  const bot = definitionId => ({ definitionId, actionChannel: 'match.action', observations: ['own.question'],
    observationContract: { source: 'message', channel: 'match.state', fields: ['round', 'question', 'score'],
      handshake: 'hello then ready', stateIdentity: 'round and revision', acceptedEvidence: 'question advances' },
    decisionPolicy: 'compute answer from own question', spawnRule: 'fill one slot', removeRule: 'end or leave',
    difficultyBehaviors: { Easy: 'slow errors', Medium: 'normal', Hard: 'faster', Hell: 'fast accurate' } });
  const plan = { schemaVersion: 1, enabled: true, target: { archiveId: 'archive-1', levelId: 'level-1' },
    networkMode: 'multiplayer', playerModel: 'manual', changedDefinitionIds: ['new'], removedDefinitionIds: [],
    allowRemoveAll: false, bots: [bot('original'), bot('new')] };
  write('plan.json', plan); write('definitions.before.json', before); write('definitions.current.json', before);
  write('definitions.candidate.json', candidate); write('definitions.reloaded.json', candidate);
  const archive = path.join(root, 'archive'), player = path.join(root, 'player.exe');
  write('archive/Main.PBArc', { Guid: 'archive-1' }); fs.writeFileSync(player, 'synthetic-player');
  write('player_Data/Managed/game.dll', { version: 1 });
  const run = { runId: 'run-1', startedAt: new Date(Date.now() - 10000).toISOString(), provider,
    target: { ...plan.target, buildId: playerHash(player) }, archiveHash: archiveHash(archive),
    instances: [{ id: 'host-1', role: 'host' }, { id: 'client-1', role: 'clientOnly' }] };
  const report = { schemaVersion: 1, status: 'passed', runId: run.runId, target: run.target, provider,
    definitionsHash: definitionsHash(candidate), archiveHash: run.archiveHash,
    staticChecks: [], evidence: [], cases: [], uncovered: ['synthetic tests only; no real gameplay claim'] };
  const bound = { target: run.target, archiveHash: run.archiveHash, definitionsHash: report.definitionsHash };
  for (const kind of ['lua-api', 'architecture-validate', 'architecture-lint']) {
    const name = `evidence/${kind}.json`, file = write(name, { ...bound, data: { ready: true } });
    report.staticChecks.push({ kind, path: name, sha256: fileDigest(file), successPointer: '/data/ready' });
  }
  function evidence(id, definitionId, changes = {}) {
    const value = { ...bound, runId: run.runId, provider, instanceId: 'host-1', role: 'host',
      capturedAt: new Date(Date.now() - 1000).toISOString(), definitionId, botId: `user-${definitionId}`,
      difficulty: 'Hard', kind: 'gameplay', data: { score: 10, joined: true, count: 16 }, ...changes };
    const name = `evidence/${id}.json`, file = write(name, value);
    report.evidence.push({ id, path: name, sha256: fileDigest(file) });
    return id;
  }
  for (const id of ['original', 'new']) {
    const start = evidence(`${id}-before`, id, { data: { score: 0 } });
    const end = evidence(`${id}-after`, id);
    const client = evidence(`${id}-client`, id, { instanceId: 'client-1', role: 'clientOnly' });
    for (const kind of ['join', 'observation', 'settlement', 'restart', 'cleanup', 'invalid-action', 'capacity-limit', 'manual-ownership']) {
      report.cases.push({ id: `${id}-${kind}`, definitionId: id, kind, status: 'passed', detail: 'synthetic assertion',
        assertions: [{ evidenceId: end, pointer: '/data/joined', operator: 'equals', expected: true }] });
    }
    report.cases.push({ id: `${id}-accepted`, definitionId: id, kind: 'accepted-action', status: 'passed', detail: 'score progression',
      assertions: [{ evidenceId: end, pointer: '/data/score', operator: 'increased', beforeEvidenceId: start, beforePointer: '/data/score' }] });
    report.cases.push({ id: `${id}-sync`, definitionId: id, kind: 'client-sync', status: 'passed', detail: 'same score',
      assertions: [{ evidenceId: client, pointer: '/data/score', operator: 'matches', beforeEvidenceId: end, beforePointer: '/data/score' }] });
    const samples = ['Easy', 'Medium', 'Hard', 'Hell'].map(difficulty => evidence(`${id}-${difficulty}`, id, { difficulty, kind: 'strategy' }));
    report.cases.push({ id: `${id}-difficulty`, definitionId: id, kind: 'difficulty-comparison', status: 'passed', detail: 'same input',
      assertions: samples.map(evidenceId => ({ evidenceId, pointer: '/data/score', operator: 'equals', expected: 10 })) });
  }
  const save = () => { write('run.json', run); write('validation.json', report); };
  save();
  return { root, write, before, candidate, plan, report, run, save, archive, player };
}
