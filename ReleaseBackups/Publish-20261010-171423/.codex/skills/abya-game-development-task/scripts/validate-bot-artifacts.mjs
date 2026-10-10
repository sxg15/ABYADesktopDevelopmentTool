import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { archiveHash, playerHash, readJson, requireThat, validateSchema } from './bot-artifact-core.mjs';
import { validateCandidate } from './bot-definition-checks.mjs';
import { validateEvidence } from './bot-evidence-checks.mjs';

// 只读本地检查，不调用游戏、不修改存档、不推断任务已获用户接受。
export function validateArtifacts(directory, mode = 'prewrite', options = {}) {
  requireThat(['prewrite', 'acceptance'].includes(mode), 'mode 应为 prewrite 或 acceptance');
  const root = path.resolve(directory), read = name => readJson(path.join(root, name));
  const assets = new URL('../assets/bot-integration/', import.meta.url);
  const plan = read('plan.json');
  validateSchema(plan, readJson(new URL('plan.schema.json', assets)));
  const candidate = read('definitions.candidate.json');
  const hash = validateCandidate(plan, read('definitions.before.json'), read('definitions.current.json'), candidate);
  if (mode === 'prewrite') return { accepted: false, status: 'candidate-checked', definitionsHash: hash };
  if (!plan.enabled) return { accepted: false, status: 'not-applicable', definitionsHash: hash };
  requireThat(plan.bots.length > 0, '删除所有 Bot 后应执行普通玩法回归，不得申报 Bot 运行通过');
  const report = read('validation.json');
  validateSchema(report, readJson(new URL('validation.schema.json', assets)));
  requireThat(report.status !== 'not-applicable', '已启用的 Bot 验收不能标记为未适用');
  if (['blocked', 'failed', 'runtime-unverified'].includes(report.status))
    return { accepted: false, status: report.status, definitionsHash: hash };
  return validateEvidence(root, plan, candidate, report, read('run.json'), read('definitions.reloaded.json'),
    options.archive, options.player);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const args = process.argv.slice(2), options = {};
    for (let i = 0; i < args.length; i += 2) {
      requireThat(['--root', '--mode', '--archive', '--player'].includes(args[i]) && args[i + 1]
        && !args[i + 1].startsWith('--') && !Object.hasOwn(options, args[i].slice(2)), '参数无效或重复');
      options[args[i].slice(2)] = args[i + 1];
    }
    if (options.mode === 'player-hash') {
      requireThat(options.player, '缺少 --player');
      console.log(JSON.stringify({ buildId: playerHash(options.player) }));
    } else if (options.mode === 'archive-hash') {
      requireThat(options.archive, '缺少 --archive');
      console.log(JSON.stringify({ archiveHash: archiveHash(options.archive) }));
    } else {
      requireThat(options.root, '缺少 --root');
      const result = validateArtifacts(options.root, options.mode, options);
      console.log(JSON.stringify({ success: true, ...result }));
      if (options.mode === 'acceptance' && !result.accepted && result.status !== 'not-applicable') process.exitCode = 3;
    }
  } catch (error) {
    console.log(JSON.stringify({ success: false, accepted: false, error: error.message }));
    process.exitCode = 2;
  }
}
