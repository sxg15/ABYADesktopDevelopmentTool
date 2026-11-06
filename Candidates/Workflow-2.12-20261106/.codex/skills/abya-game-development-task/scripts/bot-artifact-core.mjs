import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

// 规范化对象键；数组保留语义顺序，定义集合另按稳定 ID 排序。
export function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') return Object.fromEntries(
    Object.keys(value).sort().map(key => [key, canonical(value[key])]));
  return value;
}
export const digest = value => createHash('sha256').update(JSON.stringify(canonical(value))).digest('hex');
// 分块读取大型 Unity 数据文件，避免整个资源包常驻内存。
export function fileDigest(file) {
  const hash = createHash('sha256'), buffer = Buffer.allocUnsafe(1024 * 1024), fd = fs.openSync(file, 'r');
  try { let count; while ((count = fs.readSync(fd, buffer, 0, buffer.length, null)) > 0) hash.update(buffer.subarray(0, count)); }
  finally { fs.closeSync(fd); }
  return hash.digest('hex');
}
export const readJson = file => JSON.parse(fs.readFileSync(file, 'utf8').replace(/^\uFEFF/, ''));
export const same = (a, b) => JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));
export function requireThat(condition, message) { if (!condition) throw new Error(message); }
// 运行时读回可能为枚举序号，写入 schema 使用名称；两种表示具有同一内容摘要。
export function normalizeDefinition(definition) {
  const names = ['Easy', 'Medium', 'Hard', 'Hell'];
  const name = value => Number.isInteger(value) && value >= 0 && value < names.length ? names[value] : value;
  return { ...definition, LuaSource: definition.LuaSource ?? '', LuaTextResourceID: definition.LuaTextResourceID ?? '',
    DefaultDifficulty: name(definition.DefaultDifficulty),
    Difficulties: Array.isArray(definition.Difficulties) ? definition.Difficulties.map(p => p &&
      ({ ...p, Difficulty: name(p.Difficulty) })) : definition.Difficulties };
}
export function definitionsHash(envelope) {
  return digest({ archiveGuid: envelope.archiveGuid,
    definitions: envelope.definitions.map(normalizeDefinition).sort((a, b) => a.ID < b.ID ? -1 : a.ID > b.ID ? 1 : 0) });
}

// 证据必须来自本次产物目录，解析真实路径后仍不能越界。
export function evidencePath(root, relative) {
  requireThat(typeof relative === 'string' && relative.length > 0 && !path.isAbsolute(relative)
    && !relative.includes('\\') && !relative.split('/').includes('..'), '证据路径必须是目录内相对路径');
  const base = fs.realpathSync(root);
  const result = fs.realpathSync(path.join(base, relative));
  const relation = path.relative(base, result);
  requireThat(relation !== '' && !relation.startsWith('..') && !path.isAbsolute(relation)
    && fs.statSync(result).isFile(), '证据真实路径越界或不是文件');
  return result;
}

// 仅实现随包 Schema 实际使用的关键字，不作为通用 JSON Schema 引擎。
export function validateSchema(value, schema, root = schema, label = '$') {
  if (schema.$ref) {
    requireThat(schema.$ref.startsWith('#/$defs/'), '仅支持本地 $defs 引用');
    const target = root.$defs?.[schema.$ref.slice(8)];
    requireThat(target, `缺少 Schema 引用 ${schema.$ref}`);
    return validateSchema(value, target, root, label);
  }
  const kinds = { object: v => v !== null && typeof v === 'object' && !Array.isArray(v),
    array: Array.isArray, string: v => typeof v === 'string', boolean: v => typeof v === 'boolean',
    number: v => typeof v === 'number' && Number.isFinite(v), integer: Number.isInteger };
  if (schema.type) requireThat(kinds[schema.type]?.(value), `${label}: 类型应为 ${schema.type}`);
  if ('const' in schema) requireThat(same(value, schema.const), `${label}: 常量不匹配`);
  if (schema.enum) requireThat(schema.enum.some(item => same(item, value)), `${label}: 不在枚举中`);
  if (schema.type === 'object') {
    for (const key of schema.required ?? []) requireThat(Object.hasOwn(value, key), `${label}: 缺少 ${key}`);
    for (const [key, entry] of Object.entries(value)) {
      if (schema.properties?.[key]) validateSchema(entry, schema.properties[key], root, `${label}.${key}`);
      else requireThat(schema.additionalProperties !== false, `${label}: 未知字段 ${key}`);
    }
  }
  if (schema.type === 'array') {
    if (schema.minItems !== undefined) requireThat(value.length >= schema.minItems, `${label}: 数组过短`);
    if (schema.uniqueItems) requireThat(new Set(value.map(v => JSON.stringify(canonical(v)))).size === value.length,
      `${label}: 存在重复项`);
    value.forEach((entry, index) => validateSchema(entry, schema.items ?? {}, root, `${label}[${index}]`));
  }
  if (schema.type === 'string') {
    if (schema.minLength) requireThat(value.trim().length >= schema.minLength, `${label}: 文本为空`);
    if (schema.pattern) requireThat(new RegExp(schema.pattern).test(value), `${label}: 格式错误`);
  }
  if (schema.minimum !== undefined) requireThat(value >= schema.minimum, `${label}: 数值过小`);
  if (schema.maximum !== undefined) requireThat(value <= schema.maximum, `${label}: 数值过大`);
}

// 使用 JSON Pointer 读取原始捕获，不从报告的文字结论推断断言结果。
export function pointer(document, location) {
  requireThat(typeof location === 'string' && (location === '' || location.startsWith('/')), '无效 JSON Pointer');
  let value = document;
  for (const token of location === '' ? [] : location.slice(1).split('/')) {
    const key = token.replace(/~1/g, '/').replace(/~0/g, '~');
    requireThat(value !== null && typeof value === 'object' && Object.hasOwn(value, key), `断言路径不存在: ${location}`);
    value = value[key];
  }
  return value;
}

// 保存内容整体摘要；拒绝链接，避免用不完整清单或另一目录冒充已验收版本。
export function archiveHash(directory) {
  const root = fs.realpathSync(directory), entries = [];
  requireThat(fs.existsSync(path.join(root, 'Main.PBArc')), '验收目录缺少 Main.PBArc');
  function walk(folder) {
    for (const entry of fs.readdirSync(folder, { withFileTypes: true })) {
      const file = path.join(folder, entry.name);
      requireThat(!fs.lstatSync(file).isSymbolicLink(), '存档摘要不接受符号链接');
      if (entry.isDirectory()) walk(file);
      else if (entry.isFile()) entries.push([path.relative(root, file).split(path.sep).join('/'), fileDigest(file)]);
    }
  }
  walk(root);
  return digest(entries.sort((a, b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
}

// Unity 的 exe 可能不随游戏逻辑变化，构建身份必须包含 Data 与运行库。
export function playerHash(executable) {
  const exe = path.resolve(executable), folder = path.dirname(exe), entries = [];
  const data = `${path.basename(exe, path.extname(exe))}_Data`;
  requireThat(fs.existsSync(path.join(folder, data)), 'Player 缺少配套 _Data 目录');
  function add(file) {
    requireThat(!fs.lstatSync(file).isSymbolicLink(), 'Player 摘要不接受符号链接');
    if (fs.statSync(file).isDirectory()) {
      for (const name of fs.readdirSync(file)) add(path.join(file, name));
    } else entries.push([path.relative(folder, file).split(path.sep).join('/'), fileDigest(file)]);
  }
  add(exe); add(path.join(folder, data));
  for (const name of ['UnityPlayer.dll', 'GameAssembly.dll', 'MonoBleedingEdge']) {
    const file = path.join(folder, name); if (fs.existsSync(file)) add(file);
  }
  return digest(entries.sort((a, b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
}
