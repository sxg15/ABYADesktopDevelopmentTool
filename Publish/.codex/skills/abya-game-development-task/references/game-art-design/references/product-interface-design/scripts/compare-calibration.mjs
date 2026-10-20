import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const text = value => typeof value === 'string' && value.trim().length > 0;
const number = value => typeof value === 'number' && Number.isFinite(value);
const positive = value => number(value) && value > 0;
const unknown = (id, reason) => ({ id, status: 'unverified', reason });
function index(items, label) {
  if (!Array.isArray(items) || items.length > 500) throw new Error(`${label}: expected up to 500 items`);
  const result = new Map();
  for (const item of items) {
    if (!item || !text(item.id) || result.has(item.id)) throw new Error(`${label}: missing/duplicate ID`);
    result.set(item.id, item);
  }
  return result;
}
function frameValid(capture) {
  const frame = capture?.frame, image = capture?.image;
  return frame && image && positive(image.width) && positive(image.height)
    && positive(frame.width) && positive(frame.height)
    && number(frame.x) && number(frame.y) && frame.x >= 0 && frame.y >= 0
    && frame.x + frame.width <= image.width && frame.y + frame.height <= image.height;
}
export function compareCalibration(input) {
  if (input?.schemaVersion !== 1) throw new Error('Unsupported calibration schema');
  const { reference, actual, tolerances } = input;
  const expected = index(reference?.measurements, 'reference.measurements');
  const observed = index(actual?.measurements, 'actual.measurements');
  const expectedFonts = index(reference?.fonts, 'reference.fonts');
  const observedFonts = index(actual?.fonts, 'actual.fonts');
  const blockers = [];
  if (!text(actual?.implementationVersion)) blockers.push('Actual implementation version is missing');
  for (const key of ['designVersion', 'scenario', 'content', 'layout']) {
    if (!text(reference?.context?.[key]) || reference.context[key] !== actual?.context?.[key])
      blockers.push(`Comparable ${key} is missing or different`);
  }
  let scale = 1;
  if (!frameValid(reference) || !frameValid(actual)) blockers.push('Valid capture dimensions and registered frames are required');
  else {
    scale = reference.frame.width / actual.frame.width;
    if (Math.abs(actual.frame.height * scale - reference.frame.height) > 0.01)
      blockers.push('Frames require uniform scaling; use the matching layout baseline');
  }
  const comparable = blockers.length === 0;
  if (tolerances?.status !== 'specified' || !text(tolerances?.source))
    blockers.push('Tolerance is provisional or lacks a documented source');
  const fontChecks = [...expectedFonts.values()].map(font => {
    const actualFont = observedFonts.get(font.id);
    const fields = ['resourceId', 'weight', 'style', 'sample', 'source'];
    if (fields.filter(key => key !== 'weight').some(key => !text(font[key]) || !text(actualFont?.[key]))
      || !positive(font.weight) || !positive(actualFont?.weight) || font.weight > 1000 || actualFont.weight > 1000)
      return unknown(font.id, 'Font resource, weight, style, sample or observation source is missing');
    const changed = fields.filter(key => key !== 'source' && font[key] !== actualFont[key]);
    return { id: font.id, status: changed.length ? 'different' : 'same-metadata', changed };
  });
  const checks = [...expected.values()].map(target => {
    const measured = observed.get(target.id);
    const key = { edge: 'edgePx', gap: 'gapPx', size: 'sizePx', 'glyph-height': 'glyphHeightPercent' }[target.kind];
    if (!key || !['x', 'y'].includes(target.axis) || !number(target.value) || !text(target.source))
      return unknown(target.id, 'Reference measurement kind, axis, value or source is missing');
    if (!measured || measured.kind !== target.kind || measured.axis !== target.axis || !number(measured.value) || !text(measured.source))
      return unknown(target.id, 'Matching runtime measurement is missing');
    if (target.value < 0 || measured.value < 0 || (target.kind === 'glyph-height' && (!positive(target.value) || !positive(measured.value))))
      return unknown(target.id, 'Invalid measurement magnitude');
    let observedValue = measured.value * scale, expectedValue = target.value;
    if (target.kind === 'edge' && frameValid(reference) && frameValid(actual)) {
      observedValue = (measured.value - actual.frame[target.axis]) * scale;
      expectedValue = target.value - reference.frame[target.axis];
    }
    const deltaPx = observedValue - expectedValue;
    const error = target.kind === 'glyph-height' ? Math.abs(deltaPx) / target.value * 100 : Math.abs(deltaPx);
    const tolerance = tolerances?.[key];
    const reasons = [...blockers];
    if (!number(tolerance) || tolerance < 0) reasons.push(`Explicit ${key} is required`);
    if (target.kind === 'glyph-height') {
      const font = fontChecks.find(item => item.id === target.fontId);
      if (!font || font.status !== 'same-metadata' || target.fontId !== measured.fontId)
        reasons.push('Glyph size cannot validate a missing/different font or sample');
    }
    return { id: target.id, kind: target.kind, expectedValue, observedValue: comparable ? observedValue : null,
      deltaPx: comparable ? deltaPx : null, error: comparable ? error : null,
      unit: target.kind === 'glyph-height' ? 'percent' : 'reference-pixels', tolerance: tolerance ?? null,
      status: reasons.length ? 'unverified' : error <= tolerance + 1e-9 ? 'within-tolerance' : 'outside-tolerance', reasons };
  });
  const measurementStatus = blockers.length || !checks.length || checks.some(c => c.status === 'unverified') || fontChecks.some(c => c.status === 'unverified')
    ? 'unverified' : checks.some(c => c.status === 'outside-tolerance') || fontChecks.some(c => c.status === 'different') ? 'differences' : 'within-tolerance';
  return { schemaVersion: 1, measurementStatus, blockers, checks, fontChecks,
    unpairedActual: [...observed.keys()].filter(id => !expected.has(id)),
    visualAcceptance: 'not-evaluated', note: 'Measured inputs only; no OCR, automatic geometry detection, aesthetic score or human approval.' };
}

export async function checkImageIdentity(capture, base) {
  const image = capture?.image;
  if (!text(image?.path) || !/^[a-f\d]{64}$/i.test(image?.sha256 ?? '')) return 'Image path or expected SHA-256 is missing';
  try {
    const file = path.resolve(base, image.path), stat = fs.statSync(file);
    if (!stat.isFile() || stat.size <= 0 || stat.size > 64 * 1024 * 1024) return 'Image must be a regular file within 64 MiB';
    const hash = crypto.createHash('sha256');
    for await (const bytes of fs.createReadStream(file)) hash.update(bytes);
    if (hash.digest('hex') !== image.sha256.toLowerCase()) return 'Image SHA-256 differs from the recorded capture';
    return null;
  } catch { return 'Recorded image cannot be read'; }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [inputFlag, file, outputFlag, output, ...extra] = process.argv.slice(2);
    if (inputFlag !== '--input' || !file || outputFlag !== '--output' || !output || extra.length)
      throw new Error('Usage: node compare-calibration.mjs --input measurements.json --output new-report.json');
    const inputPath = path.resolve(file);
    if (fs.statSync(inputPath).size > 2 * 1024 * 1024) throw new Error('Measurement input exceeds 2 MiB');
    const input = JSON.parse(fs.readFileSync(inputPath, 'utf8').replace(/^\uFEFF/, ''));
    const report = compareCalibration(input);
    report.captureChecks = [];
    for (const side of ['reference', 'actual']) {
      const reason = await checkImageIdentity(input[side], path.dirname(inputPath));
      report.captureChecks.push({ side, status: reason ? 'unverified' : 'hash-matched', reason });
      if (reason) report.measurementStatus = 'unverified';
    }
    fs.writeFileSync(output, JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
    console.log(JSON.stringify({ measurementStatus: report.measurementStatus, visualAcceptance: report.visualAcceptance, output: path.resolve(output) }));
    process.exitCode = { 'within-tolerance': 0, differences: 1, unverified: 2 }[report.measurementStatus];
  } catch (error) { console.error(error.message); process.exitCode = 2; }
}
