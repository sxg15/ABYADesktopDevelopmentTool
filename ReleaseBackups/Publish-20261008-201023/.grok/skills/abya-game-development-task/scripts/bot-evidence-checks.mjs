import path from 'node:path';
import { archiveHash, definitionsHash, evidencePath, fileDigest, playerHash, pointer, readJson, requireThat, same } from './bot-artifact-core.mjs';
import { definitionMap, difficulties } from './bot-definition-checks.mjs';

// 同时校验当前保存文件、Player 文件及重载定义，不以报告自填 passed 为依据。
export function validateEvidence(root, plan, candidate, report, run, reloaded, archive, player) {
  requireThat(same(plan.target, { archiveId: report.target.archiveId, levelId: report.target.levelId }), '报告目标不匹配');
  requireThat(same(run.target, report.target) && run.runId === report.runId && run.provider === report.provider, '报告不是当前运行批次');
  requireThat(report.definitionsHash === definitionsHash(candidate), '报告候选定义已过期');
  definitionMap(reloaded, plan.target.archiveId);
  requireThat(definitionsHash(reloaded) === report.definitionsHash, '保存重载后的定义与候选不同');
  requireThat(archive && player, '验收需要 --archive 和 --player 的实际路径');
  requireThat(readJson(path.join(archive, 'Main.PBArc')).Guid === plan.target.archiveId, '实际保存存档 GUID 不匹配');
  requireThat(report.archiveHash === run.archiveHash && archiveHash(archive) === report.archiveHash, '实际存档内容已变化');
  requireThat(playerHash(player) === report.target.buildId, 'Player 构建不匹配，buildId 必须包含 Data 和运行库');
  const started = Date.parse(run.startedAt);
  requireThat(Number.isFinite(started) && started <= Date.now(), '运行起始时间无效');
  requireThat(Array.isArray(run.instances) && (report.status !== 'passed' || run.instances.length > 0), '缺少托管实例身份');
  const instances = new Map();
  for (const instance of run.instances) {
    requireThat(typeof instance.id === 'string' && instance.id.trim() && !instances.has(instance.id)
      && ['host', 'clientOnly', 'offline'].includes(instance.role), '托管实例重复或角色无效');
    instances.set(instance.id, instance.role);
  }
  function loadBound(file, hash, runtime) {
    const location = evidencePath(root, file);
    requireThat(fileDigest(location) === hash, `证据哈希不匹配: ${file}`);
    const record = readJson(location);
    requireThat(same(record.target, report.target) && record.definitionsHash === report.definitionsHash
      && record.archiveHash === report.archiveHash, `证据版本/归属不匹配: ${file}`);
    if (runtime) {
      const captured = Date.parse(record.capturedAt);
      requireThat(record.runId === run.runId && record.provider === run.provider
        && Number.isFinite(captured) && captured >= started && captured <= Date.now(), `运行证据过期: ${file}`);
      requireThat(instances.has(record.instanceId) && instances.get(record.instanceId) === record.role, '证据实例不属于本批次');
      requireThat(typeof record.botId === 'string' && record.botId.trim()
        && plan.bots.some(bot => bot.definitionId === record.definitionId)
        && difficulties.includes(record.difficulty), '证据缺少准确 Bot 身份/难度');
      requireThat(['gameplay', 'diagnostics', 'strategy'].includes(record.kind), '证据种类无效');
    }
    return record;
  }
  for (const kind of ['lua-api', 'architecture-validate', 'architecture-lint']) {
    const checks = report.staticChecks.filter(check => check.kind === kind);
    requireThat(checks.length > 0, `缺少静态校验: ${kind}`);
    for (const check of checks) {
      requireThat(check.successPointer.startsWith('/data/'), '静态断言应指向工具结果 data');
      const record = loadBound(check.path, check.sha256, false);
      requireThat(pointer(record, check.successPointer) === true, `静态校验失败: ${kind}`);
    }
  }
  if (report.status === 'static-passed') return { accepted: false, status: 'static-passed' };
  const evidence = new Map();
  for (const entry of report.evidence) {
    requireThat(!evidence.has(entry.id), '证据 ID 重复');
    evidence.set(entry.id, loadBound(entry.path, entry.sha256, true));
  }
  const evaluated = new Map(), caseIds = new Set();
  for (const test of report.cases) {
    requireThat(!caseIds.has(test.id), '验收用例 ID 重复');
    caseIds.add(test.id);
    requireThat(plan.bots.some(bot => bot.definitionId === test.definitionId), '验收用例引用未知定义');
    if (test.status !== 'passed') continue;
    requireThat(test.assertions.length > 0, `用例没有状态断言: ${test.id}`);
    const observed = new Set();
    for (const assertion of test.assertions) {
      const record = evidence.get(assertion.evidenceId);
      requireThat(record && record.definitionId === test.definitionId, '断言证据缺失或 Bot 定义不匹配');
      requireThat(assertion.pointer.startsWith('/data/'), '断言必须读取实际捕获的 data');
      const actual = pointer(record, assertion.pointer);
      observed.add(record);
      let expected = assertion.expected;
      if (['changed', 'increased', 'matches'].includes(assertion.operator)) {
        const previous = evidence.get(assertion.beforeEvidenceId);
        requireThat(previous && (assertion.operator === 'matches' || previous.instanceId === record.instanceId) && previous.botId === record.botId
          && previous.definitionId === record.definitionId && previous.difficulty === record.difficulty
          && (assertion.operator === 'matches' || Date.parse(previous.capturedAt) <= Date.parse(record.capturedAt)), '前后证据身份或时间不一致');
        requireThat(assertion.beforePointer?.startsWith('/data/'), '前置断言路径无效');
        expected = pointer(previous, assertion.beforePointer);
        observed.add(previous);
      } else requireThat(Object.hasOwn(assertion, 'expected'), '断言缺少 expected');
      const numeric = Number.isFinite(actual) && Number.isFinite(expected);
      const passed = { equals: () => same(actual, expected), matches: () => same(actual, expected),
        contains: () => Array.isArray(actual) && actual.some(item => same(item, expected)),
        greaterThan: () => numeric && actual > expected, lessOrEqual: () => numeric && actual <= expected,
        changed: () => !same(actual, expected), increased: () => numeric && actual > expected }[assertion.operator]?.();
      requireThat(passed, `状态断言失败: ${test.id} ${assertion.pointer}`);
    }
    if (test.kind === 'accepted-action') {
      requireThat(test.assertions.some(a => ['changed', 'increased'].includes(a.operator)
        && evidence.get(a.evidenceId).kind === 'gameplay'
        && evidence.get(a.beforeEvidenceId).kind === 'gameplay'), '投递计数不能代替玩法接受证据');
    }
    if (test.kind !== 'difficulty-comparison') requireThat([...observed].some(r => r.kind === 'gameplay'),
      `用例缺少玩法状态: ${test.kind}`);
    if (test.kind === 'client-sync') {
      requireThat(test.assertions.some(a => a.operator === 'matches'
        && evidence.get(a.evidenceId).kind === 'gameplay' && evidence.get(a.beforeEvidenceId).kind === 'gameplay'
        && new Set([evidence.get(a.evidenceId).role, evidence.get(a.beforeEvidenceId).role]).size === 2
        && [evidence.get(a.evidenceId).role, evidence.get(a.beforeEvidenceId).role].every(role => ['host', 'clientOnly'].includes(role))),
      '双端同步需要独立 Host/ClientOnly 的相同 Bot 身份');
    }
    if (test.kind === 'difficulty-comparison') requireThat(
      difficulties.every(d => [...observed].some(r => r.difficulty === d)), '难度比较未覆盖四档');
    const key = test.definitionId;
    if (!evaluated.has(key)) evaluated.set(key, new Map());
    requireThat(!evaluated.get(key).has(test.kind), '同一 Bot 的验收种类重复');
    evaluated.get(key).set(test.kind, [...observed]);
  }
  if (report.status !== 'passed') return { accepted: false, status: report.status };
  requireThat(report.cases.every(test => test.status === 'passed'), '存在失败或未执行用例');
  for (const bot of plan.bots) {
    const cases = evaluated.get(bot.definitionId) ?? new Map();
    const required = ['join', 'observation', 'accepted-action', 'settlement', 'restart', 'cleanup', 'invalid-action', 'capacity-limit', 'difficulty-comparison'];
    if (plan.playerModel === 'manual') required.push('manual-ownership');
    if (plan.networkMode === 'multiplayer') required.push('client-sync');
    requireThat(required.every(kind => cases.has(kind)), `${bot.definitionId}: 缺少必测用例`);
    const role = plan.networkMode === 'multiplayer' ? 'host' : 'offline';
    requireThat(difficulties.some(d => ['join', 'observation', 'accepted-action', 'settlement', 'restart', 'cleanup']
      .every(kind => cases.get(kind).some(r => r.role === role && r.difficulty === d && r.kind === 'gameplay'))),
    `${bot.definitionId}: 至少一档应在真实实例覆盖完整对局`);
  }
  return { accepted: true, status: 'passed' };
}
