import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { readFile, access } from 'node:fs/promises';
import path from 'node:path';
const exec = promisify(execFile);

// 要求完整 Unity 版本一致，包含中国版后缀。
export async function findUnity(project, explicit, env = process.env) {
  const text = await readFile(path.join(project, 'ProjectSettings/ProjectVersion.txt'), 'utf8');
  const version = text.match(/^m_EditorVersion:\s*(\S+)/m)?.[1];
  if (!version) throw Error('无法读取项目 Unity 版本。');
  const requested = explicit || env.ABYA_UNITY_PATH;
  const candidates = requested ? [path.resolve(requested)] : [];
  if (!requested) {
    const roots = process.platform === 'win32'
      ? [path.join(env.ProgramFiles || 'C:/Program Files', 'Unity/Hub/Editor'), 'D:/Unity Editors', 'D:/Unity Editor']
      : process.platform === 'darwin' ? ['/Applications/Unity/Hub/Editor']
        : [path.join(env.HOME || '', 'Unity/Hub/Editor')];
    for (const root of roots) {
      for (const folder of [version, 'Unity ' + version])
        candidates.push(path.join(root, folder, process.platform === 'darwin'
          ? 'Unity.app/Contents/MacOS/Unity' : process.platform === 'win32' ? 'Editor/Unity.exe' : 'Editor/Unity'));
    }
  }
  for (const candidate of candidates) {
    try {
      await access(candidate);
      const { stdout } = await exec(candidate, ['-version'], { timeout: 15000, windowsHide: true });
      if (stdout.trim().split(/\s+/).includes(version)) return { executable: candidate, version };
      if (requested) throw Error('指定的 Unity 与项目版本 ' + version + ' 不一致。');
    } catch (error) {
      if (requested) throw Error('无法验证 Unity：' + error.message);
    }
  }
  throw Error('找不到 Unity ' + version + '，请使用 --unity 或 ABYA_UNITY_PATH。');
}

// Windows 检查活动主编辑器，不输出可能含登录信息的完整命令行。
export async function assertProjectAvailable(project) {
  if (process.platform !== 'win32') return;
  const script = `[Console]::OutputEncoding=[Text.UTF8Encoding]::new(); $p=Get-CimInstance Win32_Process -Filter "name = 'Unity.exe'"; @($p | ForEach-Object { if ($_.CommandLine -notmatch 'AssetImportWorker') { $m=[regex]::Match($_.CommandLine,'(?i)-projectpath"?\\s+(?:"([^"]+)"|(\\S+))'); if($m.Success){ if($m.Groups[1].Success){$m.Groups[1].Value}else{$m.Groups[2].Value} } } }) | ConvertTo-Json -Compress`;
  const { stdout } = await exec('powershell.exe', ['-NoProfile', '-Command', script],
    { windowsHide: true, timeout: 15000, encoding: 'utf8' });
  const paths = JSON.parse(stdout.replace(/^\uFEFF/, '').trim() || '[]');
  for (const value of Array.isArray(paths) ? paths : [paths])
    if (path.resolve(value).toLowerCase() === path.resolve(project).toLowerCase())
      throw Error('项目已在 Unity 中打开，请使用独立构建工作区。');
}

// 仅终止本次启动的进程树，绝不按进程名称批量结束 Unity。
async function stopOwnedProcess(child) {
  if (!child.pid || child.exitCode !== null) return;
  if (process.platform === 'win32')
    await exec('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true }).catch(() => {});
  else {
    try { process.kill(-child.pid, 'SIGTERM'); } catch {}
    await new Promise(resolve => setTimeout(resolve, 1000));
    try { process.kill(-child.pid, 'SIGKILL'); } catch {}
  }
}

// 通过 close 事件等待子进程管道关闭后再交付结果。
export async function runUnity(executable, args, { project, timeoutMs, signal, onDiagnostic = () => {} }) {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, args, { cwd: project, shell: false, windowsHide: true,
      detached: process.platform !== 'win32', stdio: ['ignore', 'pipe', 'pipe'] });
    let reason;
    let stopping;
    const stop = value => { if (!reason) { reason = value; stopping = stopOwnedProcess(child); } };
    const abort = () => stop('cancelled');
    const timer = setTimeout(() => stop('timeout'), timeoutMs);
    signal?.addEventListener('abort', abort, { once: true });
    child.stdout.on('data', chunk => onDiagnostic(chunk.toString()));
    child.stderr.on('data', chunk => onDiagnostic(chunk.toString()));
    const cleanup = () => { clearTimeout(timer); signal?.removeEventListener('abort', abort); };
    child.once('error', error => { cleanup(); reject(error); });
    child.once('close', async (exitCode, exitSignal) => {
      cleanup();
      if (stopping) await stopping;
      resolve({ exitCode, exitSignal, reason });
    });
    if (signal?.aborted) abort();
  });
}
