// Opt-in model-backed smoke test. Credentials remain in process memory.
import { spawn, spawnSync } from 'node:child_process';
import readline from 'node:readline';
const required = ['ABYA_TEST_CODEX', 'ABYA_TEST_WORKSPACE', 'ABYA_DESKTOP_CLI',
  'ABYA_DESKTOP_PIPE', 'ABYA_DESKTOP_PID', 'ABYA_DESKTOP_SESSION_TOKEN', 'ABYA_DEVELOPMENT_TASK_ID',
  'ABYA_DEVELOPMENT_CONVERSATION_ID'];
for (const key of required) if (!process.env[key]) throw new Error(`Missing ${key}`);
const child = spawn(process.env.ABYA_TEST_CODEX, ['app-server'], {
  stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true,
});
child.stderr.resume();
let sequence = 0;
const pending = new Map();
const commands = [];
let complete;
const finished = new Promise(resolve => { complete = resolve; });
readline.createInterface({ input: child.stdout }).on('line', line => {
  let message;
  try { message = JSON.parse(line); } catch { return; }
  if (message.id !== undefined && pending.has(message.id)) {
    const resolve = pending.get(message.id); pending.delete(message.id); resolve(message);
  }
  if (message.method === 'item/completed' && message.params?.item?.type === 'commandExecution') {
    const item = message.params.item;
    commands.push({ command: item.command, output: item.aggregatedOutput, exitCode: item.exitCode });
  }
  // Never auto-approve a request. The test uses the normal permission policy.
  if (message.id !== undefined && message.method) {
    child.stdin.write(JSON.stringify({ id: message.id, error: { code: -32601, message: 'No automatic approvals in this test' } }) + '\n');
  }
  if (message.method === 'turn/completed') complete(message.params.turn.status);
});
function call(method, params) {
  const id = ++sequence;
  return Promise.race([
    new Promise(resolve => { pending.set(id, resolve); child.stdin.write(JSON.stringify({ id, method, params }) + '\n'); }),
    new Promise((_, reject) => setTimeout(() => reject(new Error(`${method} timed out`)), 20000).unref()),
  ]).then(message => { if (message.error) throw new Error(`${method} failed`); return message.result; });
}
let threadId;
try {
  await call('initialize', { clientInfo: { name: 'ABYA CLI sandbox validation', version: '1' }, capabilities: { experimentalApi: true } });
  child.stdin.write(JSON.stringify({ method: 'initialized', params: {} }) + '\n');
  const config = {};
  for (const key of required.filter(key => !key.startsWith('ABYA_TEST_'))) config[`shell_environment_policy.set.${key}`] = process.env[key];
  config['shell_environment_policy.set.ABYA_DEVELOPMENT_PROVIDER'] = 'codex';
  config['shell_environment_policy.set.ABYA_DEVELOPMENT_WORKSPACE'] = process.env.ABYA_TEST_WORKSPACE;
  const result = await call('thread/start', {
    cwd: process.env.ABYA_TEST_WORKSPACE, config,
    developerInstructions: 'This is a read-only CLI integration test. Do not read configuration, credentials, files or directory listings. Do not print any token or pipe environment variable. Use exec_command once with its default working directory and no permission overrides to run: whoami; Get-Location; & $env:ABYA_DESKTOP_CLI doctor --json; & $env:ABYA_DESKTOP_CLI capabilities --json. Report results. Do not change anything or start games.',
  });
  threadId = result.thread.id;
  await call('turn/start', { threadId, input: [{ type: 'text', text: 'Run the read-only CLI integration test now.' }] });
  const status = await Promise.race([finished, new Promise((_, reject) => setTimeout(() => reject(new Error('Model test timed out')), 240000).unref())]);
  const output = commands.map(item => item.output ?? '').join('\n');
  const summary = {
    status, commandCount: commands.length,
    sandboxIdentity: /codexsandboxoffline/i.test(output),
    doctor: /"runtimeCliInstalled":true/.test(output),
    capabilities: /development_task_list/.test(output),
    errors: commands.some(item => item.exitCode !== 0),
  };
  console.log(JSON.stringify(summary));
  if (status !== 'completed' || !summary.sandboxIdentity || !summary.doctor || !summary.capabilities || summary.errors) process.exitCode = 1;
} finally {
  if (threadId) await call('thread/archive', { threadId }).catch(() => {});
  spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' });
}
