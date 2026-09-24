// One-time repair for ABYA-owned native conversation IDs. Does not resume or archive them.
import { spawn, spawnSync } from 'node:child_process';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import readline from 'node:readline';
if (!process.env.ABYA_TEST_CODEX || !process.env.ABYA_DESKTOP_EXECUTABLE) throw new Error('Set ABYA_TEST_CODEX and ABYA_DESKTOP_EXECUTABLE');
const listed = spawnSync(process.env.ABYA_DESKTOP_EXECUTABLE, ['task', 'list', '--json'], { encoding: 'utf8', windowsHide: true });
if (listed.status !== 0) throw new Error('Desktop task listing failed');
const result = JSON.parse(listed.stdout);
if (!result.success || !Array.isArray(result.data)) throw new Error('Invalid Desktop task list');
// Explicit historical roots permit organizing retained histories after a Desktop task was removed.
// Never recreate task database records or discover arbitrary user directories.
for (const workspacePath of JSON.parse(process.env.ABYA_HISTORY_WORKSPACES ?? '[]')) {
  if (!path.isAbsolute(workspacePath)) throw new Error('Historical workspace must be absolute');
  const root=path.join(workspacePath,'conversations');
  const entries=await readdir(root,{withFileTypes:true});
  const ids=new Set();
  for (const entry of entries.filter(entry=>entry.isDirectory())) {
    const value=JSON.parse(await readFile(path.join(root,entry.name,'conversation.json'),'utf8'));
    if (/^[0-9a-f-]{36}$/i.test(value.taskId??'')) ids.add(value.taskId);
  }
  if (ids.size!==1) throw new Error('Ambiguous historical task identity');
  const id=[...ids][0];
  if (!result.data.some(task=>task.id===id)) result.data.push({id,title:path.basename(workspacePath).replace(/-[0-9a-f-]{8,}$/i,''),workspacePath});
}
const server = spawn(process.env.ABYA_TEST_CODEX, ['app-server'], { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
server.stderr.resume();
let sequence = 0;
const pending = new Map();
readline.createInterface({ input: server.stdout }).on('line', line => {
  let message; try { message = JSON.parse(line); } catch { return; }
  if (message.id !== undefined && pending.has(message.id)) { pending.get(message.id)(message); pending.delete(message.id); }
});
async function call(method, params) {
  const id = ++sequence;
  const response = await Promise.race([
    new Promise(resolve => { pending.set(id, resolve); server.stdin.write(JSON.stringify({ id, method, params }) + '\n'); }),
    new Promise((_, reject) => setTimeout(() => reject(new Error(`${method} timeout`)), 15000).unref()),
  ]);
  if (response.error) throw new Error(`${method}: ${response.error.message}`);
  return response.result;
}
try {
  await call('initialize', { clientInfo: { name: 'ABYA conversation organization', version: '1' }, capabilities: { experimentalApi: true } });
  server.stdin.write(JSON.stringify({ method: 'initialized', params: {} }) + '\n');
  const projects = []; let cursor;
  do { const page = await call('project/list', { limit: 100, cursor }); projects.push(...page.data); cursor = page.nextCursor; } while (cursor);
  for (const task of result.data) {
    const root = path.join(task.workspacePath, 'conversations');
    let entries; try { entries = await readdir(root, { withFileTypes: true }); } catch (error) { if (error.code === 'ENOENT') continue; throw error; }
    const conversations = [];
    for (const entry of entries.filter(entry => entry.isDirectory())) {
      const conversation = JSON.parse(await readFile(path.join(root, entry.name, 'conversation.json'), 'utf8'));
      if (conversation.taskId === task.id && /^[0-9a-f-]{36}$/i.test(conversation.nativeSessionId ?? '')) conversations.push(conversation);
    }
    if (!conversations.length) continue;
    // Recover older native sessions from this exact workspace, without scanning unrelated threads.
    let threadCursor;
    for (let pageIndex=0;pageIndex<100;pageIndex++) {
      const page=await call('thread/list',{cwd:task.workspacePath,archived:false,limit:100,cursor:threadCursor,modelProviders:[]});
      for (const thread of page.data) if (!conversations.some(c=>c.nativeSessionId===thread.id)) conversations.push({nativeSessionId:thread.id,title:thread.name??'历史对话'});
      threadCursor=page.nextCursor; if (!threadCursor) break;
    }
    let project = projects.find(project => project.metadata.abyaTaskId === task.id || project.roots.some(root => path.resolve(root.path) === path.resolve(task.workspacePath)));
    if (!project) {
      project = (await call('project/create', { idempotencyKey: `abya-task-${task.id}`, name: `ABYA · ${task.title}`, roots: [{ path: task.workspacePath }], metadata: { abyaTaskId: task.id, origin: 'ABYA Desktop Development Tool' } })).project;
      projects.push(project);
    }
    let organized=0;
    for (const conversation of conversations) {
      const threadId = conversation.nativeSessionId;
      try {
        await call('thread/name/set', { threadId, name: `ABYA · ${task.title} · ${conversation.title}` });
        await call('thread/metadata/update', { threadId, projectId: project.id });
        organized++;
      } catch { console.log(JSON.stringify({threadId,status:'skippedUnavailableHistory'})); }
    }
    console.log(JSON.stringify({ taskId: task.id, projectId: project.id, name: project.name, conversations: organized }));
  }
} finally {
  spawnSync('taskkill', ['/PID', String(server.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' });
}
