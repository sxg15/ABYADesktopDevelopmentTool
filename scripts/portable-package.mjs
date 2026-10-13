import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const inventoryName = 'package-files.json';
const required = ['ABYA Desktop Development Tool.exe', 'abya-desktop.exe', 'build-manifest.json',
  'runtime/node.exe', 'tools/abya/bin/abya.mjs', 'tools/ffmpeg/source.json',
  'tools/ffmpeg/runtime/bin/ffmpeg.exe', 'tools/ffmpeg/runtime/bin/ffprobe.exe', 'tools/ffmpeg/runtime/LICENSE.txt',
  '.codex/skills/abya-game-development-task/SKILL.md', '.grok/skills/abya-game-development-task/SKILL.md'];
const readJson = file => JSON.parse(fs.readFileSync(file, 'utf8').replace(/^\uFEFF/, ''));
function checkedPath(root, relative) {
  if (typeof relative !== 'string' || !relative || relative.includes('\\') || path.posix.isAbsolute(relative)
    || relative.split('/').some(p => !p || p === '.' || p === '..') || relative.includes(':')) throw new Error(`Invalid package path: ${relative}`);
  const leaf = path.posix.basename(relative);
  if (/^(auth\.json|cli-connection\.json|id_rsa|id_ed25519|\.env.*)$/i.test(leaf) || /\.(db|sqlite)(?:$|[.-])/i.test(leaf)) throw new Error(`Private data cannot be distributed: ${relative}`);
  const allowed = ['ABYA Desktop Development Tool.exe','abya-desktop.exe','build-manifest.json',inventoryName,'.codex/README.md','.grok/README.md'].includes(relative)
    || ['runtime/','tools/abya/','tools/ffmpeg/','.codex/skills/','.grok/skills/'].some(prefix => relative.startsWith(prefix));
  if (!allowed) throw new Error(`Unexpected package content: ${relative}`);
  let current = root;
  for (const part of relative.split('/')) {
    current = path.join(current, part);
    if (fs.lstatSync(current).isSymbolicLink()) throw new Error(`Package links are not allowed: ${relative}`);
  }
  if (!fs.statSync(current).isFile()) throw new Error(`Not a regular package file: ${relative}`);
  return current;
}
async function hashFile(file) {
  const hash = crypto.createHash('sha256');
  for await (const chunk of fs.createReadStream(file)) hash.update(chunk);
  return hash.digest('hex');
}
function filesIn(root, skipBrowserState = false) {
  const result = [];
  function visit(folder) {
    for (const entry of fs.readdirSync(folder, {withFileTypes:true})) {
      if (skipBrowserState && folder === root && entry.name === 'ABYA Desktop Development Tool.exe.WebView2') continue;
      const full = path.join(folder, entry.name);
      if (entry.isSymbolicLink()) throw new Error(`Package link found: ${full}`);
      if (entry.isDirectory()) visit(full);
      else if (entry.isFile()) result.push(path.relative(root, full).split(path.sep).join('/'));
      else throw new Error(`Unsupported package item: ${full}`);
    }
  }
  visit(root); return result.filter(file => file !== inventoryName).sort();
}
export async function verifyPackage(root, {write = false, quick = false, tracked = false} = {}) {
  root = path.resolve(root);
  if (fs.lstatSync(root).isSymbolicLink()) throw new Error('Package root must not be a link.');
  const files = filesIn(root, !write);
  for (const file of required) if (!files.includes(file)) throw new Error(`Missing runtime file: ${file}`);
  const release = readJson(path.join(root, 'build-manifest.json'));
  for (const [file, expected] of [[release.executable,release.sha256],[release.cliExecutable,release.cliSha256]]) {
    const absolute = checkedPath(root, file);
    if (!expected || (await hashFile(absolute)).toLowerCase() !== expected.toLowerCase()) throw new Error(`Release hash mismatch: ${file}`);
  }
  let inventory;
  if (write) {
    const entries = [];
    for (const file of files) {
      const absolute = checkedPath(root, file), size = fs.statSync(absolute).size;
      if (size >= 100 * 1024 * 1024) throw new Error(`Package file exceeds normal Git hosting limit: ${file}`);
      entries.push({path:file,bytes:size,sha256:await hashFile(absolute)});
    }
    inventory = {schemaVersion:1,releaseLabel:release.releaseLabel,totalBytes:entries.reduce((sum,e)=>sum+e.bytes,0),files:entries};
    fs.writeFileSync(path.join(root, inventoryName), JSON.stringify(inventory,null,2)+'\n','utf8');
  } else {
    inventory = readJson(path.join(root, inventoryName));
    if (inventory.schemaVersion !== 1 || inventory.releaseLabel !== release.releaseLabel || !Array.isArray(inventory.files)) throw new Error('Package inventory identity mismatch.');
    const listed = inventory.files.map(e=>e.path).sort();
    if (JSON.stringify(listed) !== JSON.stringify(files)) throw new Error('Package inventory differs from files on disk.');
    for (const entry of inventory.files) {
      const absolute = checkedPath(root, entry.path);
      if (fs.statSync(absolute).size !== entry.bytes || (!quick && await hashFile(absolute) !== entry.sha256)) throw new Error(`Incomplete or modified package file: ${entry.path}`);
    }
  }
  if (tracked) {
    const repository = execFileSync('git',['-C',root,'rev-parse','--show-toplevel'],{encoding:'utf8'}).trim();
    const trackedFiles = new Set(execFileSync('git',['-C',repository,'ls-files','-z'],{encoding:'utf8',maxBuffer:16*1024*1024}).split('\0'));
    for (const file of [...files,inventoryName]) {
      const relative = path.relative(repository,path.join(root,file)).split(path.sep).join('/');
      if (!trackedFiles.has(relative)) throw new Error(`Package file is not tracked by Git: ${relative}`);
    }
  }
  return {release:release.releaseLabel,files:files.length,totalBytes:inventory.totalBytes,verified:quick ? 'core hashes and complete file inventory' : 'all file hashes',tracked};
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  let root = path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../Publish');
  const options = {};
  for (let index=0;index<args.length;index++) {
    const flag=args[index];
    if (flag === '--package' && args[index+1]) root=args[++index];
    else if (flag === '--write') options.write=true;
    else if (flag === '--quick') options.quick=true;
    else if (flag === '--require-tracked') options.tracked=true;
    else throw new Error(`Unknown option: ${flag}`);
  }
  try { console.log(JSON.stringify(await verifyPackage(root,options),null,2)); }
  catch (error) { console.error(`Portable package check failed: ${error.message}`); process.exitCode=1; }
}
