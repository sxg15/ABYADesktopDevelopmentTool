import assert from 'node:assert/strict';
import test from 'node:test';
import { parseBuildJson } from '../src/build-json.mjs';
import { verifyReport } from '../src/build-command.mjs';
import { run } from '../src/cli.mjs';
import { findUnity } from '../src/unity-process.mjs';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';

test('拒绝重复 JSON 键，允许不同对象内相同键', () => {
  assert.throws(() => parseBuildJson('{"outputKind":"Player","outputKind":"Uaal"}'));
  assert.throws(() => parseBuildJson('{"x":1,"\\u0078":2}'));
  assert.deepEqual(parseBuildJson('{"a":{"x":1},"b":{"x":2},"text":"x: { }"}'),
    { a: { x: 1 }, b: { x: 2 }, text: 'x: { }' });
});
test('带错误码的合法失败报告保留原构建退出码', () => {
  for (const code of [2, 3, 4, 10]) {
    const report = { schemaVersion: 1, runId: 'id', command: 'build', success: false,
      unityExitCode: code, issues: [], outputPaths: [] };
    assert.equal(verifyReport(report, 'id', 'build', { exitCode: code }).unityExitCode, code);
  }
});
test('build/check 在运行时实例发现前分流，帮助不要求实例', async () => {
  for (const command of ['build', 'check']) {
    const lines = [];
    const io = { stdout: { write: x => lines.push(x) }, stderr: { write: () => {} } };
    assert.equal(await run([command, '--help', '--json'], io, {}), 0);
    assert.match(JSON.parse(lines.join('')).help, /build\|check/);
  }
});
test('Unity 定位读取完整版本，显式错误路径不回退', async t => {
  const project = await mkdtemp(path.join(os.tmpdir(), 'abya-version-'));
  t.after(() => rm(project, { recursive: true, force: true }));
  await mkdir(path.join(project, 'ProjectSettings'));
  await writeFile(path.join(project, 'ProjectSettings/ProjectVersion.txt'), 'm_EditorVersion: 2022.3.62f2c1');
  await assert.rejects(findUnity(project, path.join(project, 'missing-unity')), /无法验证 Unity/);
});
