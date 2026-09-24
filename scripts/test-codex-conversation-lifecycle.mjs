import { spawn, spawnSync } from 'node:child_process';
import readline from 'node:readline';
import { randomUUID } from 'node:crypto';
if (!process.env.ABYA_TEST_CODEX || !process.env.ABYA_TEST_WORKSPACE) throw new Error('Set ABYA_TEST_CODEX and ABYA_TEST_WORKSPACE');
const processServer = spawn(process.env.ABYA_TEST_CODEX, ['app-server'], { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
processServer.stderr.resume();
let sequence = 0;
const pending = new Map();
readline.createInterface({ input: processServer.stdout }).on('line', line => {
  let message; try { message = JSON.parse(line); } catch { return; }
  if (message.id !== undefined && pending.has(message.id)) {
    pending.get(message.id)(message); pending.delete(message.id);
  }
});
async function call(method, params) {
  const id = ++sequence;
  const message = await Promise.race([
    new Promise(resolve => { pending.set(id, resolve); processServer.stdin.write(JSON.stringify({ id, method, params }) + '\n'); }),
    new Promise((_, reject) => setTimeout(() => reject(new Error(`${method} timeout`)), 15000).unref()),
  ]);
  if (message.error) throw new Error(`${method}: ${message.error.message}`);
  return message.result;
}
let projectId, threadId;
try {
  await call('initialize', { clientInfo: { name: 'ABYA lifecycle validation', version: '1' }, capabilities: { experimentalApi: true } });
  processServer.stdin.write(JSON.stringify({ method: 'initialized', params: {} }) + '\n');
  const params = { idempotencyKey: `abya-ui-test-${randomUUID()}`, name: 'ABYA temporary lifecycle validation', roots: [{ path: process.env.ABYA_TEST_WORKSPACE }], metadata: { origin: 'ABYA integration test' } };
  projectId = (await call('project/create', params)).project.id;
  if ((await call('project/create', params)).project.id !== projectId) throw new Error('Project registration is not idempotent');
  threadId = (await call('thread/start', { cwd: process.env.ABYA_TEST_WORKSPACE, projectId })).thread.id;
  await call('thread/inject_items', { threadId, items: [{ type: 'message', role: 'user', content: [{ type: 'input_text', text: 'ABYA lifecycle test; no model turn or game action.' }] }] });
  await call('thread/metadata/update', { threadId, projectId });
  await call('thread/name/set', { threadId, name: 'ABYA lifecycle validation' });
  const before = (await call('thread/read', { threadId })).thread;
  if (before.name !== 'ABYA lifecycle validation' || before.projectId !== projectId) throw new Error('Native naming/project assignment failed');
  await call('thread/unsubscribe', { threadId });
  await call('thread/archive', { threadId });
  const archived = await call('thread/list', { archived: true, projectId, limit: 100, modelProviders: [], useStateDbOnly: true });
  const active = await call('thread/list', { archived: false, projectId, limit: 100, modelProviders: [], useStateDbOnly: true });
  const archiveRead=(await call('thread/read',{threadId})).thread;
  if (!archiveRead.path.includes('archived_sessions') || active.data.some(thread => thread.id === threadId)) throw new Error('Archive state failed');
  const restored = (await call('thread/unarchive', { threadId })).thread;
  if (restored.id !== threadId || restored.name !== before.name || restored.projectId !== projectId) throw new Error('Restore changed thread identity or metadata');
  await call('thread/archive', { threadId });
  console.log(JSON.stringify({ projectRegistration: true, idempotent: true, titleSync: true, archiveState: true, restoreIdentity: true, emptyThreadOmittedFromList: !archived.data.some(thread => thread.id === threadId) }));
} finally {
  if (projectId) await call('project/delete', { projectId }).catch(error => console.error(error.message));
  spawnSync('taskkill', ['/PID', String(processServer.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' });
}
