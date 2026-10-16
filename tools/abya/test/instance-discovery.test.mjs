import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { findInstance } from '../src/cli.mjs';

async function fixture(t) {
  const folder = await mkdtemp(path.join(os.tmpdir(), 'abya-discovery-test-'));
  t.after(async () => {
    assert.equal(path.dirname(folder), path.resolve(os.tmpdir()));
    assert.ok(path.basename(folder).startsWith('abya-discovery-test-'));
    await rm(folder, { recursive: true, force: true });
  });
  let index = 0;
  const projectPath = path.resolve('Discovery Project');
  return {
    env: { ABYA_CLI_INSTANCE_DIR: folder }, projectPath,
    put: async value => writeFile(path.join(folder, `${index++}.json`), JSON.stringify(value)),
    host: async (identity = {}, authorized = true) => {
      const server = createServer(async (req, res) => {
        assert.equal(req.url, '/api/v1/status'); // Discovery must never invoke gameplay writes.
        let raw = '';
        for await (const chunk of req) raw += chunk;
        const input = JSON.parse(raw);
        res.writeHead(authorized ? 200 : 401, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ version: 1, requestId: input.requestId, success: authorized,
          content: [{ type: 'text', text: JSON.stringify({ processId: process.pid,
            projectPath, target: 'Player', desktopInstanceId: 'wanted', ...identity }) }] }));
      });
      await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
      t.after(() => new Promise(resolve => server.close(resolve)));
      return { processId: process.pid, projectPath, token: 'synthetic-token',
        endpoint: `http://127.0.0.1:${server.address().port}` };
    },
  };
}

test('explicit PID selects the live host despite a stale registration', async t => {
  const f = await fixture(t);
  const live = await f.host();
  await f.put(live);
  await f.put({ ...live, endpoint: 'http://127.0.0.1:1' });
  assert.equal((await findInstance({ instance: String(process.pid) }, f.env)).endpoint, live.endpoint);
});

for (const [label, identity, authorized] of [
  ['PID mismatch', { processId: -1 }, true],
  ['project mismatch', { projectPath: path.resolve('Foreign Project') }, true],
  ['desktop launch mismatch', { desktopInstanceId: 'foreign' }, true],
  ['authentication failure', {}, false],
]) {
  test(`duplicate discovery rejects ${label}`, async t => {
    const f = await fixture(t);
    f.env.ABYA_CLI_DESKTOP_INSTANCE_ID = 'wanted';
    const live = await f.host();
    await f.put(live);
    await f.put(await f.host(identity, authorized));
    assert.equal((await findInstance({ instance: String(process.pid) }, f.env)).endpoint, live.endpoint);
  });
}

test('two verified live endpoints remain ambiguous even with an explicit PID', async t => {
  const f = await fixture(t);
  await f.put(await f.host());
  await f.put(await f.host());
  await assert.rejects(findInstance({ instance: String(process.pid) }, f.env), { code: 'ambiguous_instance' });
});

test('all stale registrations return not found instead of an ambiguous match', async t => {
  const f = await fixture(t);
  const denied = await f.host({}, false);
  await f.put(denied);
  await f.put({ ...denied, endpoint: 'http://127.0.0.1:1' });
  await assert.rejects(findInstance({ instance: String(process.pid) }, f.env), { code: 'instance_not_found' });
});
