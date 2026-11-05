import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { validateConfig, loadConfig, buildTarget } from '../src/build-config.mjs';
import { parseBuildArgs, verifyReport, runBuild } from '../src/build-command.mjs';
import { runUnity } from '../src/unity-process.mjs';

const server = { schemaVersion: 1, outputKind: 'DedicatedServer', il2cppMode: 'Release' };

// 真实临时工程覆盖配置、日志及进程边界，不需要启动 Unity。
async function fixture(t) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'Abya 构建 空格-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(path.join(root, 'ProjectSettings'));
  await writeFile(path.join(root, 'ProjectSettings/ProjectVersion.txt'), 'm_EditorVersion: 2022.3.62f2c1');
  await writeFile(path.join(root, 'config.json'), '\uFEFF' + JSON.stringify(server));
  return root;
}
function capture() {
  const stdout = [], stderr = [];
  return { stdout: { write: s => stdout.push(s) }, stderr: { write: s => stderr.push(s) },
    result: () => JSON.parse(stdout.join('')), lines: stdout };
}
function report(id, command, code = 0) {
  return { schemaVersion: 1, runId: id, command, success: code === 0, unityExitCode: code,
    issues: [], outputPaths: command === 'build' ? ['build.exe'] : [], message: '完成', errorCode: null };
}

test('构建参数拒绝未知、重复、缺值及非法超时', () => {
  for (const args of [['--bad'], ['--json', '--json'], ['--project'],
    ['--project', 'x', '--config', 'c', '--timeout', '-1']])
    assert.throws(() => parseBuildArgs(args));
  assert.equal(parseBuildArgs(['--help']).help, true);
});
test('发布模式明确，拒绝数字枚举和不兼容组合', () => {
  assert.equal(validateConfig(server).useBugDataCollectionTool, false);
  for (const changed of [{ il2cppMode: undefined }, { architecture: 99 }, { unknown: true },
    { targetPlatform: 'Linux' }, { useBugDataCollectionTool: true }, { scenes: [] }])
    assert.throws(() => validateConfig({ ...server, ...changed }));
});
test('全部平台映射和移动默认架构', () => {
  const mobile = validateConfig({ schemaVersion: 1, outputKind: 'Uaal',
    targetPlatform: 'Android', il2cppMode: 'Release' });
  assert.equal(mobile.architecture, 'Arm64');
  for (const [targetPlatform, architecture, expected] of [
    ['Windows', 'Intel32', 'Win'], ['Windows', 'Intel64', 'Win64'], ['Mac', 'Universal', 'OSXUniversal'],
    ['Linux', 'Intel64', 'Linux64'], ['Android', 'Arm64', 'Android'], ['iOS', 'Arm64', 'iOS']])
    assert.equal(buildTarget({ targetPlatform, architecture }), expected);
});
test('UTF8 BOM 配置及项目相对输出和显式覆盖', async t => {
  const root = await fixture(t);
  const c = await loadConfig(root, 'config.json', { outputDirectory: '输出 目录' });
  assert.equal(c.outputDirectory, path.join(root, '输出 目录'));
  await assert.rejects(loadConfig(root, 'config.json', { outputDirectory: 'Assets/bad' }));
});
test('旧报告、无产物和退出码矛盾均不能报告成功', () => {
  assert.throws(() => verifyReport(report('old', 'build'), 'new', 'build', { exitCode: 0 }));
  assert.throws(() => verifyReport(report('id', 'build'), 'id', 'build', { exitCode: 1 }));
  assert.throws(() => verifyReport({ ...report('id', 'build'), outputPaths: [] }, 'id', 'build', { exitCode: 0 }));
});
test('构建命令等待进程并返回当前任务报告，保留中文路径', async t => {
  const root = await fixture(t), io = capture();
  const code = await runBuild('build', ['--project', root, '--config', 'config.json', '--json'], io, {}, {
    assertProjectAvailable: async () => {}, findUnity: async () => ({ executable: 'fake', version: '2022.3.62f2c1' }),
    runUnity: async (_, args) => {
      const value = k => args[args.indexOf(k) + 1];
      assert.equal(value('-projectPath'), root);
      assert.equal(value('-buildTarget'), 'Win64');
      assert.equal(args.includes('-nographics'), false);
      await writeFile(value('-resultFile'), JSON.stringify(report(value('-runId'), 'build')));
      return { exitCode: 0 };
    },
  });
  assert.equal(code, 0);
  assert.equal(io.lines.length, 1);
  assert.equal(io.result().success, true);
  assert.equal(JSON.parse(await readFile(io.result().resultPath, 'utf8')).runId, io.result().runId);
});
test('缺失结果、启动失败和取消均输出一个失败 JSON', async t => {
  const root = await fixture(t);
  for (const [mode, expected] of [['missing', 23], ['startup', 20], ['spawn', 20], ['timeout', 21], ['cancelled', 22]]) {
    const io = capture();
    const code = await runBuild('check', ['--project', root, '--config', 'config.json', '--json'], io, {}, {
      assertProjectAvailable: async () => { if (mode === 'startup') throw Error('占用'); },
      findUnity: async () => ({ executable: 'fake', version: 'test' }),
      runUnity: async () => {
        if (mode === 'spawn') throw Error('无法启动进程');
        return { exitCode: 1, reason: ['timeout', 'cancelled'].includes(mode) ? mode : null };
      },
    });
    assert.equal(code, expected);
    assert.equal(io.lines.length, 1);
    assert.equal(io.result().success, false);
  }
});
test('check 不执行自动修复，参数错误不启动 Unity', async () => {
  const io = capture();
  assert.equal(await runBuild('check', ['--project', 'x', '--config', 'x', '--auto-fix', '--json'], io), 10);
  assert.equal(io.result().stage, 'arguments');
});
test('实际子进程非零退出与启动失败', async () => {
  const result = await runUnity(process.execPath, ['-e', 'process.exit(4)'],
    { project: process.cwd(), timeoutMs: 10000 });
  assert.equal(result.exitCode, 4);
  await assert.rejects(runUnity('abya-missing-executable-123', [], { project: process.cwd(), timeoutMs: 1000 }));
});
test('超时和取消结束本次真实子进程', async () => {
  const options = { project: process.cwd(), timeoutMs: 200 };
  const timeout = await runUnity(process.execPath, ['-e', 'setInterval(()=>{},1000)'], options);
  assert.equal(timeout.reason, 'timeout');
  const controller = new AbortController();
  const pending = runUnity(process.execPath, ['-e', 'setInterval(()=>{},1000)'],
    { ...options, timeoutMs: 10000, signal: controller.signal });
  controller.abort();
  assert.equal((await pending).reason, 'cancelled');
});
