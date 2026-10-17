import { randomUUID } from 'node:crypto';
import { mkdir, readFile, writeFile, rename, open, unlink } from 'node:fs/promises';
import path from 'node:path';
import { loadConfig, buildTarget } from './build-config.mjs';
import { findUnity, assertProjectAvailable, runUnity } from './unity-process.mjs';

// 构建命令拥有独立退出码，不改变运行时能力命令的语义。
const codes = { invalid_request: 10, startup_failed: 20, timeout: 21, cancelled: 22, invalid_report: 23 };
export const buildHelp = `abya build|check --project <工程目录> --config <项目相对 JSON 路径>
  --unity <Unity 可执行文件>       覆盖 ABYA_UNITY_PATH 和安装目录发现
  --output-dir <目录>             覆盖配置中的输出目录
  --timeout <秒>                  整个 Unity 进程超时，默认 7200
  --auto-fix                      构建前自动修复；check 禁止使用
  --json                         stdout 输出一个 JSON，诊断写 stderr
  --help                         显示帮助，不启动 Unity
默认日志及结果：<工程>/Logs/AbyaBuildCli/<任务ID>/
`;

// 严格解析包装层选项，构建业务字段统一放入配置文件。
export function parseBuildArgs(args) {
  const flags = {};
  const switches = new Set(['json', 'auto-fix', 'help']);
  const values = new Set(['project', 'config', 'unity', 'output-dir', 'timeout']);
  for (let i = 0; i < args.length; i++) {
    if (!args[i].startsWith('--')) throw Error('无效参数：' + args[i]);
    const key = args[i].slice(2);
    if ((!switches.has(key) && !values.has(key)) || Object.hasOwn(flags, key))
      throw Error('未知或重复参数：' + args[i]);
    if (switches.has(key)) flags[key] = true;
    else {
      if (!args[i + 1] || args[i + 1].startsWith('--')) throw Error('参数缺少值：' + args[i]);
      flags[key] = args[++i];
    }
  }
  if (!flags.help && (!flags.project || !flags.config)) throw Error('必须指定 --project 和 --config。');
  const seconds = Number(flags.timeout ?? 7200);
  if (!Number.isFinite(seconds) || seconds <= 0 || seconds > 86400) throw Error('timeout 必须为 0 到 86400 秒之间的数值。');
  flags.timeoutMs = seconds * 1000;
  return flags;
}

// 任务 ID 与命令必须一致；成功报告不能覆盖非零进程退出码。
export function verifyReport(report, runId, command, processResult) {
  if (report?.schemaVersion !== 1 || report.runId !== runId || report.command !== command ||
      typeof report.success !== 'boolean' || !Array.isArray(report.issues) ||
      !Array.isArray(report.outputPaths) || !Number.isInteger(report.unityExitCode))
    throw Error('Unity 报告缺失、损坏或不属于本次任务。');
  if (![0, 2, 3, 4, 10].includes(report.unityExitCode) ||
      report.unityExitCode !== processResult.exitCode ||
      report.success !== (report.unityExitCode === 0))
    throw Error('Unity 退出码与结果报告不一致。');
  if (command === 'build' && report.success && !report.outputPaths.length)
    throw Error('构建成功报告缺少产物路径。');
  return report;
}

// 先写临时文件再改名，避免外部系统读取半份结果。
async function writeJson(file, value) {
  const temp = file + '.' + randomUUID() + '.tmp';
  await writeFile(temp, JSON.stringify(value, null, 2) + '\n', 'utf8');
  await rename(temp, file);
}

// 构建全程不连接运行时实例，失败也统一输出结果。
export async function runBuild(command, args, io = process, env = process.env, dependencies = {}) {
  const startedAt = new Date().toISOString();
  const runId = randomUUID();
  let flags = { json: args.includes('--json') };
  let result = { schemaVersion: 1, runId, command, success: false, stage: 'arguments',
    errorCode: null, issues: [], outputPaths: [], effectiveRequest: null,
    startedAt, unityExitCode: null, logPath: null };
  let resultFile;
  let lock;
  let lockPath;
  let exitCode = 10;
  const controller = new AbortController();
  const interrupt = () => controller.abort();
  process.once('SIGINT', interrupt);
  process.once('SIGTERM', interrupt);
  try {
    flags = parseBuildArgs(args);
    if (flags.help) { io.stdout.write(flags.json ? JSON.stringify({ help: buildHelp }) + '\n' : buildHelp); return 0; }
    if (command === 'check' && flags['auto-fix']) throw Error('check 不允许 --auto-fix。');
    const project = path.resolve(flags.project);
    const config = await loadConfig(project, flags.config,
      flags['output-dir'] ? { outputDirectory: flags['output-dir'] } : {});
    result.effectiveRequest = config;
    result.stage = 'startup';
    const directory = path.join(project, 'Logs/AbyaBuildCli', runId);
    await mkdir(directory, { recursive: true });
    resultFile = path.join(directory, 'result.json');
    result.logPath = path.join(directory, 'unity.log');
    lockPath = path.join(project, 'Logs/AbyaBuildCli/active.lock');
    try { lock = await open(lockPath, 'wx'); }
    catch { throw Error('已有 CLI 构建锁，请等待任务完成；异常遗留锁需确认没有构建进程后手动移除。'); }
    await lock.writeFile(JSON.stringify({ runId, pid: process.pid, startedAt }));
    await (dependencies.assertProjectAvailable || assertProjectAvailable)(project);
    const unity = await (dependencies.findUnity || findUnity)(project, flags.unity, env);
    if (controller.signal.aborted) {
      result.errorCode = 'cancelled';
      throw Error('启动前已取消本次构建。');
    }
    const configPath = path.join(directory, 'request.json');
    await writeJson(configPath, config);
    const unityReport = path.join(directory, 'unity-result.json');
    const unityArgs = ['-batchmode', '-projectPath', project, '-buildTarget', buildTarget(config),
      '-executeMethod', 'AbyaBuildCli.Run', '-config', configPath, '-runId', runId,
      '-resultFile', unityReport, '-logFile', result.logPath, '-checkOnly', String(command === 'check'),
      '-autoFix', String(Boolean(flags['auto-fix'])),
      '--UNITY_MCP_KEEP_CONNECTED=false', '--UNITY_MCP_START_SERVER=false'];
    result.stage = command;
    let processResult;
    try {
      processResult = await (dependencies.runUnity || runUnity)(unity.executable, unityArgs,
        { project, timeoutMs: flags.timeoutMs, signal: controller.signal,
          onDiagnostic: text => io.stderr.write(text) });
    } catch (error) {
      result.errorCode = 'startup_failed';
      throw error;
    }
    result.unityExitCode = processResult.exitCode;
    if (processResult.reason) {
      result.errorCode = processResult.reason;
      throw Error(processResult.reason === 'timeout' ? 'Unity 构建超时，已终止本次进程。' : '已取消本次构建。');
    }
    result.errorCode = 'invalid_report';
    const report = JSON.parse((await readFile(unityReport, 'utf8')).replace(/^\uFEFF/, ''));
    result = { ...result, ...verifyReport(report, runId, command, processResult), startedAt,
      logPath: result.logPath, unityVersion: unity.version };
    exitCode = result.unityExitCode;
  } catch (error) {
    result.success = false;
    result.errorCode ||= result.stage === 'arguments' ? 'invalid_request'
      : result.stage === 'startup' ? 'startup_failed' : 'invalid_report';
    result.message = error.message;
    exitCode = codes[result.errorCode] || 10;
  } finally {
    process.removeListener('SIGINT', interrupt);
    process.removeListener('SIGTERM', interrupt);
    if (lock) { await lock.close(); await unlink(lockPath).catch(() => {}); }
  }
  result.finishedAt = new Date().toISOString();
  result.duration = (Date.now() - Date.parse(startedAt)) / 1000;
  result.exitCode = exitCode;
  result.resultPath = resultFile || null;
  if (resultFile) {
    try { await writeJson(resultFile, result); }
    catch (error) {
      result.success = false; result.errorCode = 'invalid_report'; result.message = error.message;
      exitCode = result.exitCode = 23;
    }
  }
  if (flags.json) io.stdout.write(JSON.stringify(result) + '\n');
  else {
    io.stdout.write((result.success ? '成功：' : '失败：') + (result.message || command) + '\n');
    if (result.logPath) io.stdout.write('日志：' + result.logPath + '\n');
    if (resultFile) io.stdout.write('结果：' + resultFile + '\n');
  }
  return exitCode;
}
