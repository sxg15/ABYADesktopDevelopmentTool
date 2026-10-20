import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export function luminance(hex) {
  if (typeof hex !== 'string' || !/^#(?:[\da-f]{3}|[\da-f]{6})$/i.test(hex))
    throw new Error('Only opaque #RGB/#RRGGBB is supported; composite alpha and inspect image backgrounds separately.');
  const expanded = hex.length === 4 ? [...hex.slice(1)].map(x => x + x).join('') : hex.slice(1);
  const rgb = [0, 2, 4].map(i => parseInt(expanded.slice(i, i + 2), 16) / 255)
    .map(c => c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
}

export function contrastPair(pair) {
  try {
    const { fg, bg, fontSizePx, fontWeight = 400 } = pair;
    if (!Number.isFinite(fontSizePx) || fontSizePx <= 0 || !Number.isFinite(fontWeight)
      || fontWeight < 1 || fontWeight > 1000) throw new Error('Supply actual positive CSS fontSizePx and numeric fontWeight.');
    const a = luminance(fg), b = luminance(bg);
    const ratio = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
    const threshold = fontSizePx >= 24 || (fontSizePx >= 14 * 96 / 72 && fontWeight >= 700) ? 3 : 4.5;
    return { name: pair.name ?? '', ratio, threshold, status: ratio >= threshold ? 'pass' : 'fail' };
  } catch (error) {
    return { name: pair?.name ?? '', status: 'unverified', reason: error.message };
  }
}

const reference = /^\{([a-zA-Z0-9_.-]+)\}$/;
function leaf(value) {
  if (value && typeof value === 'object' && !Array.isArray(value)) value = value.hex ?? value.value;
  if (!['string', 'number'].includes(typeof value) || (typeof value === 'number' && !Number.isFinite(value)))
    throw new Error('Token must have a string/number value, hex or value leaf.');
  return String(value);
}
function resolve(value, primitive, seen = new Set()) {
  value = leaf(value);
  const match = reference.exec(value);
  if (!match) {
    if (/[;{}\r\n]/.test(value)) throw new Error('Invalid token value.');
    return value;
  }
  if (seen.has(match[1])) throw new Error(`Circular token reference: ${match[1]}`);
  let target = primitive;
  for (const key of match[1].split('.')) {
    if (!target || !Object.hasOwn(target, key)) throw new Error(`Unknown token: ${match[1]}`);
    target = target[key];
  }
  return resolve(target, primitive, new Set([...seen, match[1]]));
}
function flatten(node, prefix = [], out = []) {
  if (!node || typeof node !== 'object' || Array.isArray(node)) throw new Error('Expected token group.');
  for (const [key, value] of Object.entries(node)) {
    if (!/^[a-zA-Z0-9_-]+$/.test(key)) throw new Error(`Invalid token name: ${key}`);
    const names = [...prefix, key];
    if (value && typeof value === 'object' && !Array.isArray(value) && !('hex' in value || 'value' in value))
      flatten(value, names, out);
    else out.push([names, leaf(value)]);
  }
  return out;
}

export function emitTokens(tokens, format = 'css') {
  const primitive = tokens.primitive ?? {}, semantic = tokens.semantic ?? {};
  const entries = [...flatten(primitive), ...flatten(semantic)];
  const names = new Set();
  for (const [parts] of entries) {
    const name = parts.join('-');
    if (names.has(name)) throw new Error(`Duplicate emitted token: ${name}`);
    names.add(name);
  }
  if (format === 'css') return ':root {\n' + entries.map(([parts, value]) => {
    resolve(value, primitive);
    const match = reference.exec(value);
    return `  --${parts.join('-')}: ${match ? `var(--${match[1].replaceAll('.', '-')})` : value};`;
  }).join('\n') + '\n}\n';
  if (format !== 'v3') throw new Error('Supported formats: css, v3.');
  const namespaces = { color: 'colors', font: 'fontFamily', text: 'fontSize', space: 'spacing',
    radius: 'borderRadius', shadow: 'boxShadow', weight: 'fontWeight', leading: 'lineHeight' };
  const extend = Object.create(null);
  for (const [parts, value] of entries) {
    const [family, ...keys] = parts;
    if (!Object.hasOwn(namespaces, family) || !keys.length) throw new Error(`Unsupported v3 token: ${parts.join('.')}`);
    let target = extend[namespaces[family]] ??= Object.create(null);
    for (const key of keys.slice(0, -1)) target = target[key] ??= Object.create(null);
    target[keys.at(-1)] = resolve(value, primitive);
  }
  return `module.exports = ${JSON.stringify({ theme: { extend } }, null, 2)};\n`;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [command, input, format] = process.argv.slice(2);
    if (!input || process.argv.length > 5 || !['contrast', 'tokens'].includes(command)
      || (command === 'contrast' && format))
      throw new Error('Usage: node visual-tools.mjs contrast pairs.json | tokens tokens.json [css|v3]');
    const data = JSON.parse(fs.readFileSync(input, 'utf8').replace(/^\uFEFF/, ''));
    if (command === 'tokens') process.stdout.write(emitTokens(data, format));
    else {
      if (!Array.isArray(data.pairs) || !data.pairs.length) throw new Error('Provide a nonempty pairs array.');
      const results = data.pairs.map(contrastPair);
      console.log(JSON.stringify({ scope: 'declared opaque text/background pairs only', results }, null, 2));
      process.exitCode = results.some(x => x.status === 'unverified') ? 2 : results.some(x => x.status === 'fail') ? 1 : 0;
    }
  } catch (error) { console.error(error.message); process.exitCode = 2; }
}
