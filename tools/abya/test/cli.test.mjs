import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { findInstance, parseArgs, request, run } from '../src/cli.mjs';
import {loopbackFetch} from '../src/loopback-http.mjs';

test('runtime transport waits for delayed headers/body and respects explicit cancellation',async t=>{
  let calls=0;
  const server=createServer(async(req,res)=>{
    calls++;let raw='';for await(const c of req)raw+=c;
    await new Promise(r=>setTimeout(r,120));res.writeHead(200,{'Content-Type':'application/json'});
    res.write('{"value":');await new Promise(r=>setTimeout(r,120));res.end('1}');
  });
  await new Promise(r=>server.listen(0,'127.0.0.1',r));t.after(()=>{server.closeAllConnections();server.close();});
  const url=`http://127.0.0.1:${server.address().port}/invoke`;
  const r=await loopbackFetch(url,{body:'{}',signal:AbortSignal.timeout(1000)});
  assert.deepEqual(await r.json(),{value:1});assert.equal(calls,1);
  await assert.rejects(loopbackFetch(url,{body:'{}',signal:AbortSignal.timeout(30)}));
  assert.equal(calls,2);await assert.rejects(loopbackFetch('http://example.com/',{}),/loopback/);
});
test('runtime transport does not replay disconnected writes or follow redirects',async t=>{
 let count=0;const server=createServer((req,res)=>{count++;if(req.url==='/lost'){req.socket.destroy();return;}res.writeHead(302,{Location:'http://example.com/'});res.end('{}');});
 await new Promise(r=>server.listen(0,'127.0.0.1',r));t.after(()=>{server.closeAllConnections();server.close();});
 const url=`http://127.0.0.1:${server.address().port}`;
 await assert.rejects(loopbackFetch(url+'/lost',{body:'{}',signal:AbortSignal.timeout(1000)}));assert.equal(count,1);
 assert.equal((await loopbackFetch(url+'/redirect',{signal:AbortSignal.timeout(1000)})).status,302);assert.equal(count,2);
});

test('parser rejects unknown and repeated options', () => {
  assert.deepEqual(parseArgs(['capability', 'list', '--json']), {
    positional: ['capability', 'list'], flags: { json: true },
  });
  assert.throws(() => parseArgs(['status', '--json', '--json']), /repeated/);
  assert.throws(() => parseArgs(['status', '--secret', 'x']), /Unknown/);
});

test('--json emits machine-readable failures even during parsing', async () => {
  const lines = [];
  const io = { stdout: { write: line => lines.push(line) },
    stderr: { write: () => assert.fail('Unexpected stderr') } };
  const code = await run(['status', '--json', '--unknown'], io, {});
  assert.equal(code, 2);
  assert.equal(lines.length, 1);
  assert.equal(JSON.parse(lines[0]).errorCode, 'invalid_request');
});

test('instance discovery deduplicates identical registrations', async t => {
  const folder = await mkdtemp(path.join(os.tmpdir(), 'abya-cli-test-'));
  t.after(() => rm(folder, { recursive: true, force: true }));
  const descriptor = {
    processId: process.pid, projectPath: path.resolve('Project'),
    endpoint: 'http://127.0.0.1:47018', token: 'test-token',
  };
  await writeFile(path.join(folder, 'one.json'), JSON.stringify(descriptor));
  const env = { ABYA_CLI_INSTANCE_DIR: folder };
  assert.equal((await findInstance({ project: descriptor.projectPath }, env)).token,
    'test-token');
  await writeFile(path.join(folder, 'two.json'), JSON.stringify(descriptor));
  assert.equal((await findInstance({}, env)).endpoint, descriptor.endpoint);
  await assert.rejects(findInstance({ project: '/other' }, env), /No matching/);
});

test('request uses bearer authentication and checks response identity', async () => {
  const instance = { endpoint: 'http://127.0.0.1:1', token: 'test-token' };
  let captured;
  const fetchImpl = async (_, options) => {
    captured = options;
    const body = JSON.parse(options.body);
    return { ok: true, json: async () => ({
      version: 1, requestId: body.requestId, success: true, content: [],
    }) };
  };
  assert.equal((await request(instance, 'status', {}, { fetchImpl })).success, true);
  assert.equal(captured.headers.Authorization, 'Bearer test-token');
  await assert.rejects(request(instance, 'status', {}, {
    fetchImpl: async () => ({ ok: true, json: async () => ({
      version: 1, requestId: 'wrong', success: true,
    }) }),
  }), /mismatch/);
});

test('CLI reads the running instance and emits a single JSON result', async t => {
  const server = createServer(async (req, res) => {
    let raw = '';
    for await (const chunk of req) raw += chunk;
    const input = JSON.parse(raw);
    const data = req.url.endsWith('/status') ? {
      processId: process.pid, target: 'Player', projectPath: path.resolve('Project'),
    } : { target: 'Player', tools: [{ name: 'runtime_get_status' }] };
    res.setHeader('Content-Type', 'application/json');
    res.end(JSON.stringify({ version: 1, requestId: input.requestId,
      success: true, content: [{ type: 'text', text: JSON.stringify(data) }] }));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise(resolve => server.close(resolve)));
  const endpoint = `http://127.0.0.1:${server.address().port}`;
  const lines = [];
  const io = { stdout: { write: line => lines.push(line) },
    stderr: { write: () => assert.fail('Unexpected stderr') } };
  const code = await run(['capability', 'list', '--endpoint', endpoint, '--json'],
    io, { ABYA_CLI_TOKEN: 'test-token' });
  assert.equal(code, 0);
  assert.equal(lines.length, 1);
  assert.deepEqual(JSON.parse(lines[0]).data.tools,
    [{ name: 'runtime_get_status' }]);
});

test('run reads a JSON object from stdin without shell escaping', async t => {
  let inputArguments;
  const server = createServer(async (req, res) => {
    let raw = '';
    for await (const chunk of req) raw += chunk;
    const requestBody = JSON.parse(raw);
    if (req.url.endsWith('/invoke'))
      inputArguments = requestBody.arguments;
    const data = req.url.endsWith('/status') ? {
      processId: process.pid, target: 'Player', projectPath: path.resolve('Project'),
    } : { state: 'ready' };
    res.end(JSON.stringify({ version: 1, requestId: requestBody.requestId,
      success: true, content: [{ type: 'text', text: JSON.stringify(data) }] }));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise(resolve => server.close(resolve)));
  const cliPath = fileURLToPath(new URL('../bin/abya.mjs', import.meta.url));
  const child = spawn(process.execPath, [cliPath, 'capability', 'run',
    'runtime_get_game_state', '--input-file', '-', '--json',
    '--endpoint', `http://127.0.0.1:${server.address().port}`], {
    env: { ...process.env, ABYA_CLI_TOKEN: 'test-token' },
  });
  const exited = new Promise(resolve => child.once('close', resolve));
  child.stdin.end('{"query":"hello"}');
  let output = '';
  for await (const chunk of child.stdout) output += chunk;
  assert.equal(await exited, 0);
  assert.deepEqual(inputArguments, { query: 'hello' });
  assert.equal(JSON.parse(output).success, true);
});

test('non-allowlisted capability maps to exit code five', async t => {
  const server = createServer(async (req, res) => {
    let raw = '';
    for await (const chunk of req) raw += chunk;
    const input = JSON.parse(raw);
    const status = req.url.endsWith('/status');
    const data = status ? { processId: process.pid, target: 'Player',
      projectPath: path.resolve('Project') } :
      { success: false, errorCode: 'capability_unavailable' };
    res.end(JSON.stringify({ version: 1, requestId: input.requestId,
      success: status, content: [{ type: 'text', text: JSON.stringify(data) }] }));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise(resolve => server.close(resolve)));
  const lines = [];
  const io = { stdout: { write: line => lines.push(line) },
    stderr: { write: () => assert.fail('Unexpected stderr') } };
  const code = await run(['capability', 'describe', 'lua_execute', '--json',
    '--endpoint', `http://127.0.0.1:${server.address().port}`], io,
    { ABYA_CLI_TOKEN: 'test-token' });
  assert.equal(code, 5);
  assert.equal(JSON.parse(lines[0]).data.errorCode, 'capability_unavailable');
});
