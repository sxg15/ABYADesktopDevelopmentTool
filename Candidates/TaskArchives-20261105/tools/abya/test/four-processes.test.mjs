import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { findInstance, request } from '../src/cli.mjs';

const hostSource = `
import { createServer } from 'node:http';
const identity = { processId: process.pid, projectPath: process.cwd(), target: 'Player',
  desktopInstanceId: 'desktop-' + process.pid, runtimeInstanceId: 'shared-legacy-runtime-id' };
const server = createServer(async (req, res) => {
  let raw = ''; for await (const chunk of req) raw += chunk;
  const input = JSON.parse(raw);
  res.setHeader('Content-Type', 'application/json');
  res.end(JSON.stringify({ version: 1, requestId: input.requestId, success: true,
    content: [{ type: 'text', text: JSON.stringify(identity) }] }));
});
server.listen(0, '127.0.0.1', () => process.send({ ...identity, token: 'synthetic-token',
  endpoint: 'http://127.0.0.1:' + server.address().port }));
process.on('disconnect', () => server.close(() => process.exit(0)));
`;

test('four simultaneous processes remain independently addressable with a shared legacy Runtime ID',
  { timeout: 15000 }, async t => {
    const folder = await mkdtemp(path.join(os.tmpdir(), 'abya-four-process-test-'));
    const children = [];
    t.after(async () => {
      await Promise.all(children.map(async child => {
        if (child.exitCode !== null) return;
        const exited = once(child, 'exit');
        child.kill();
        await exited;
      }));
      assert.equal(path.dirname(folder), path.resolve(os.tmpdir()));
      assert.ok(path.basename(folder).startsWith('abya-four-process-test-'));
      await rm(folder, { recursive: true, force: true });
    });
    const hosts = await Promise.all(Array.from({ length: 4 }, async () => {
      const child = spawn(process.execPath, ['--input-type=module', '-e', hostSource],
        { stdio: ['ignore', 'ignore', 'ignore', 'ipc'], windowsHide: true });
      children.push(child);
      const [descriptor] = await once(child, 'message');
      await writeFile(path.join(folder, `${descriptor.processId}.json`), JSON.stringify(descriptor));
      return descriptor;
    }));
    assert.equal(new Set(hosts.map(host => host.processId)).size, 4);
    const env = { ABYA_CLI_INSTANCE_DIR: folder };
    await assert.rejects(findInstance({}, env), { code: 'ambiguous_instance' });
    for (const host of hosts) {
      const selected = await findInstance({ instance: String(host.processId) }, env);
      const response = await request(selected, 'status');
      const identity = JSON.parse(response.content[0].text);
      assert.equal(identity.processId, host.processId);
      assert.equal(identity.desktopInstanceId, host.desktopInstanceId);
    }
  });
