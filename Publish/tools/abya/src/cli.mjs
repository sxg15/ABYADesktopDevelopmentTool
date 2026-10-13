import { runBuild } from './build-command.mjs';
import { randomUUID } from 'node:crypto';
import { readFile, readdir, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

const sessionId = process.env.ABYA_CLI_SESSION_ID || randomUUID();
const exitCodes = {
  invalid_request: 2,
  unauthorized: 4,
  invalid_origin: 4,
  instance_not_found: 3,
  ambiguous_instance: 3,
  capability_unavailable: 5,
  cancelled: 7,
  timeout: 7,
  outcome_unknown: 7,
};

export class CliError extends Error {
  constructor(code, message) {
    super(message);
    this.code = code;
  }
}

export function parseArgs(args) {
  const positional = [];
  const flags = {};
  const allowed = new Set([
    'json', 'project', 'instance', 'profile', 'input-file',
    'output-dir', 'endpoint',
  ]);
  for (let index = 0; index < args.length; index++) {
    const value = args[index];
    if (!value.startsWith('--')) {
      positional.push(value);
      continue;
    }
    const key = value.slice(2);
    if (!allowed.has(key) || Object.hasOwn(flags, key))
      throw new CliError('invalid_request', `Unknown or repeated option: ${value}`);
    if (key === 'json') {
      flags.json = true;
      continue;
    }
    if (!args[index + 1] || args[index + 1].startsWith('--'))
      throw new CliError('invalid_request', `Missing value for ${value}`);
    flags[key] = args[++index];
  }
  return { positional, flags };
}

export async function findInstance(flags, env = process.env) {
  if (flags.endpoint) {
    const url = new URL(flags.endpoint);
    if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' ||
        !env.ABYA_CLI_TOKEN)
      throw new CliError('invalid_request',
        'Explicit endpoint requires loopback HTTP and ABYA_CLI_TOKEN.');
    return { endpoint: url.origin, token: env.ABYA_CLI_TOKEN };
  }
  const directory = env.ABYA_CLI_INSTANCE_DIR || path.join(
    env.LOCALAPPDATA || '', 'AbyaPB', 'Cli', 'instances');
  if (!env.ABYA_CLI_INSTANCE_DIR && !env.LOCALAPPDATA)
    throw new CliError('instance_not_found', 'Windows user instance storage is unavailable.');
  let files;
  try {
    files = await readdir(directory);
  } catch (error) {
    if (error.code === 'ENOENT') files = [];
    else throw error;
  }
  const instances = [];
  for (const file of files.filter(name => name.endsWith('.json'))) {
    try {
      const data = JSON.parse(await readFile(path.join(directory, file), 'utf8'));
      if (!data.token || !data.processId || !data.projectPath ||
          new URL(data.endpoint).hostname !== '127.0.0.1' || new URL(data.endpoint).protocol !== 'http:') continue;
      if (flags.instance && String(data.processId) !== flags.instance) continue;
      if (flags.project && path.resolve(data.projectPath).toLowerCase() !==
          path.resolve(flags.project).toLowerCase()) continue;
      process.kill(data.processId, 0);
      instances.push(data);
    } catch { /* Ignore stale or incomplete instance descriptors. */ }
  }
  if (instances.length === 0)
    throw new CliError('instance_not_found', 'No matching ABYA instance is running.');
  if (instances.length > 1)
    throw new CliError('ambiguous_instance', 'Multiple instances match; specify --instance.');
  return instances[0];
}

export async function request(instance, method, data = {}, options = {}) {
  const requestId = options.requestId || randomUUID();
  options.onRequestId?.(requestId);
  const response = await (options.fetchImpl || fetch)(
    `${instance.endpoint}/api/v1/${method}`, {
      method: 'POST',
      redirect: 'error',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${instance.token}`,
      },
      body: JSON.stringify({ ...data, requestId, sessionId }),
      signal: options.signal || AbortSignal.timeout(15_000),
    });
  let body;
  try {
    body = await response.json();
  } catch {
    throw new CliError('invalid_response', 'The instance returned invalid JSON.');
  }
  if (!response.ok)
    throw new CliError(body.errorCode || 'request_failed',
      body.message || `Instance returned HTTP ${response.status}.`);
  if (body.requestId !== requestId || body.version !== 1)
    throw new CliError('invalid_response', 'CLI protocol version or request ID mismatch.');
  return { requestId, ...body };
}

async function outputContent(result, flags) {
  const content = [];
  for (const [index, block] of (result.content || []).entries()) {
    if (block.type !== 'image') {
      content.push(block);
      continue;
    }
    if (!flags['output-dir'])
      throw new CliError('invalid_request',
        'Image output requires --output-dir.');
    const extension = block.mimeType === 'image/jpeg' ? '.jpg' :
      block.mimeType === 'image/png' ? '.png' : null;
    if (!extension)
      throw new CliError('invalid_response', 'Unsupported image MIME type.');
    const folder = path.resolve(flags['output-dir']);
    await mkdir(folder, { recursive: true });
    const file = path.join(folder, `${result.requestId}-${index}${extension}`);
    await writeFile(file, Buffer.from(block.data, 'base64'), { flag: 'wx' });
    content.push({ type: 'image', mimeType: block.mimeType, path: file });
  }
  const firstText = content.find(block => block.type === 'text')?.text;
  let data;
  try { data = JSON.parse(firstText); } catch { data = firstText; }
  const denied = data?.approval === 'denied';
  return { schemaVersion: 1, requestId: result.requestId, success: result.success && !denied,
    executionState: denied ? 'not_executed' : result.success ? 'completed' : 'failed', data, content };
}

function ensureCommand(positional) {
  if (positional[0] === 'cancel' && positional.length === 2)
    return { method: 'cancel', input: { targetRequestId: positional[1] } };
  if (positional[0] === 'status' && positional.length === 1)
    return { method: 'status', input: {} };
  if (positional[0] !== 'capability')
    throw new CliError('invalid_request', 'Expected status or capability command.');
  const [, command, ...values] = positional;
  if (command === 'list' && values.length === 0)
    return { method: 'catalog', input: {} };
  if (command === 'search' && values.length > 0)
    return { method: 'invoke', input: {
      capabilityId: 'capability_search',
      arguments: { query: values.join(' ') },
    } };
  if (command === 'describe' && values.length === 1)
    return { method: 'describe', input: { capabilityId: values[0] } };
  if (command === 'run' && values.length === 1)
    return { method: 'invoke', input: { capabilityId: values[0] } };
  throw new CliError('invalid_request', 'Invalid capability command or arguments.');
}

async function readInput(file) {
  if (file !== '-') return readFile(file, 'utf8');
  let content = '';
  for await (const chunk of process.stdin) content += chunk.toString();
  return content;
}

export async function run(args, io = process, env = process.env) {
  if (['build', 'check'].includes(args[0])) return runBuild(args[0], args.slice(1), io, env);
  if (args[0] === '--version') { io.stdout.write('abya 0.2.0 protocol=1\n'); return 0; }
  if (args.length === 0 || args.includes('--help')) {
    io.stdout.write('abya status | capability list/search/describe/run; --instance PID --json --input-file FILE|- --output-dir DIR\n');
    return 0;
  }
  let flags = { json: args.includes('--json') };
  let cancellation;
  let onInterrupt;
  let timeout;
  let timedOut = false;
  try {
    const parsed = parseArgs(args);
    flags = parsed.flags;
    if (!/^[a-zA-Z0-9_-]{1,128}$/.test(sessionId))
      throw new CliError('invalid_request', 'Invalid CLI session identity.');
    const command = ensureCommand(parsed.positional);
    const instance = await findInstance(flags, env);
    if (command.method === 'cancel') {
      const result = await request(instance, 'cancel', command.input, { signal: AbortSignal.timeout(5000) });
      const output = await outputContent(result, flags);
      io.stdout.write(JSON.stringify(output) + '\n');
      return output.success ? 0 : 7;
    }
    const status = await request(instance, 'status', {}, { requestId: env.ABYA_CLI_REQUEST_ID });
    if (!status.success)
      throw new CliError('request_failed', 'Instance status failed.');
    const identity = JSON.parse(status.content[0].text);
    if (env.ABYA_CLI_DESKTOP_INSTANCE_ID && identity.desktopInstanceId !== env.ABYA_CLI_DESKTOP_INSTANCE_ID)
      throw new CliError('instance_not_found', 'Instance does not belong to this desktop launch.');
    if (flags.project && path.resolve(identity.projectPath).toLowerCase() !==
        path.resolve(flags.project).toLowerCase())
      throw new CliError('instance_not_found', 'Instance project does not match.');
    if (flags.instance && String(identity.processId) !== flags.instance)
      throw new CliError('instance_not_found', 'Instance process does not match.');
    if (!['EditorDev', 'Player'].includes(identity.target))
      throw new CliError('invalid_response', 'Unknown instance target.');

    if (command.method === 'status') {
      const output = await outputContent(status, flags);
      io.stdout.write(JSON.stringify(output.data, null, flags.json ? 0 : 2) + '\n');
      return 0;
    }
    const input = command.input;
    if (flags.profile) {
      if (command.method === 'catalog') input.profile = flags.profile;
      else if (input.capabilityId === 'capability_search')
        input.arguments.profile = flags.profile;
      else throw new CliError('invalid_request', '--profile requires list or search.');
    }
    if (parsed.positional[1] === 'run') {
      if (!flags['input-file'])
        throw new CliError('invalid_request', 'run requires --input-file <file|->.');
      let raw;
      try {
        raw = await readInput(flags['input-file']);
        if (Buffer.byteLength(raw, 'utf8') > 4 * 1024 * 1024) throw new Error('Input too large');
        input.arguments = JSON.parse(raw.replace(/^\uFEFF/, ''));
      } catch {
        throw new CliError('invalid_request', 'Input must be a readable JSON object.');
      }
      if (!input.arguments || Array.isArray(input.arguments) ||
          typeof input.arguments !== 'object')
        throw new CliError('invalid_request', 'Input must be a JSON object.');
    } else if (flags['input-file']) {
      throw new CliError('invalid_request', '--input-file requires run.');
    }
    const controller = new AbortController();
    let activeRequestId;
    onInterrupt = () => {
      if (activeRequestId) {
        cancellation = request(instance, 'cancel',
          { targetRequestId: activeRequestId }).catch(() => {});
      }
      controller.abort();
    };
    process.once('SIGINT', onInterrupt);
    timeout = setTimeout(() => {
      timedOut = true;
      onInterrupt();
    }, 670_000);
    const result = await request(instance, command.method, input, {
      signal: controller.signal,
      requestId: env.ABYA_CLI_REQUEST_ID,
      onRequestId: id => { activeRequestId = id; },
    });
    const output = await outputContent(result, flags);
    io.stdout.write(JSON.stringify(output, null, flags.json ? 0 : 2) + '\n');
    return output.success ? 0 :
      (exitCodes[output.data?.errorCode] || 6);
  } catch (error) {
    const code = error.name === 'AbortError' ?
      (timedOut ? 'timeout' : 'outcome_unknown') :
      error.code || 'request_failed';
    const message = error.message || 'CLI request failed.';
    if (flags.json)
      io.stdout.write(JSON.stringify({ success: false, errorCode: code, message }) + '\n');
    else io.stderr.write(`${code}: ${message}\n`);
    return exitCodes[code] || 6;
  } finally {
    if (onInterrupt) process.removeListener('SIGINT', onInterrupt);
    if (timeout) clearTimeout(timeout);
    if (cancellation) await cancellation;
  }
}
