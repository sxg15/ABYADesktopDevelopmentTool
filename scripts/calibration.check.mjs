import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { compareCalibration, checkImageIdentity } from '../.codex/skills/abya-game-development-task/references/game-art-design/references/product-interface-design/scripts/compare-calibration.mjs';
const script = path.resolve('.codex/skills/abya-game-development-task/references/game-art-design/references/product-interface-design/scripts/compare-calibration.mjs');
function fixture() {
  const context = { designVersion: 'design-1', scenario: 'idle camera-A', content: 'same text 13', layout: 'grid-A' };
  const fonts = [{ id: 'number', resourceId: 'fixture-font-hash', weight: 700, style: 'normal', sample: '13', source: 'synthetic observation' }];
  const measurements = [
    { id: 'left', kind: 'edge', axis: 'x', value: 100, source: 'synthetic edge' },
    { id: 'gap', kind: 'gap', axis: 'x', value: 12, source: 'synthetic gap' },
    { id: 'width', kind: 'size', axis: 'x', value: 112, source: 'synthetic size' },
    { id: 'glyph', kind: 'glyph-height', axis: 'y', value: 50, fontId: 'number', source: 'synthetic glyph' },
  ];
  return { schemaVersion: 1, tolerances: { status: 'specified', source: 'Synthetic test only', edgePx: 0, gapPx: 1, sizePx: 1, glyphHeightPercent: 5 },
    reference: { context, fonts, measurements, image: { width: 1280, height: 720 }, frame: { x: 0, y: 0, width: 1280, height: 720 } },
    actual: { context: structuredClone(context), implementationVersion: 'fixture-runtime', fonts: structuredClone(fonts),
      image: { width: 1950, height: 1110 }, frame: { x: 10, y: 20, width: 1920, height: 1080 },
      measurements: measurements.map(m => ({ ...m, value: m.value * 1.5 + (m.kind === 'edge' ? 10 : 0) })) } };
}
test('registered crop origins and uniform scale preserve actual geometric measurements', () => {
  const result = compareCalibration(fixture());
  assert.equal(result.measurementStatus, 'within-tolerance');
  assert.ok(result.checks.every(c => c.error === 0));
  assert.equal(result.visualAcceptance, 'not-evaluated');
});
test('smaller glyphs and changed net gaps remain differences even when functions work', () => {
  const input = fixture(); input.actual.measurements[1].value = 30; input.actual.measurements[3].value = 45;
  const result = compareCalibration(input);
  assert.equal(result.measurementStatus, 'differences');
  assert.equal(result.checks[1].error, 8); assert.equal(result.checks[3].error, 40);
});
test('matching numeric height cannot validate a different font, weight, style or sample', () => {
  for (const [key, value] of [['resourceId', 'different'], ['weight', 400], ['style', 'italic'], ['sample', '9']]) {
    const input = fixture(); input.actual.fonts[0][key] = value;
    const result = compareCalibration(input);
    assert.equal(result.fontChecks[0].status, 'different');
    assert.equal(result.checks[3].status, 'unverified');
  }
});
test('different data, missing version and unregistered frames do not produce plausible errors', () => {
  for (const change of [i => i.actual.context.content = 'other text', i => i.actual.context.layout = 'grid-B',
    i => i.actual.frame.height = 900, i => i.actual.implementationVersion = '']) {
    const input = fixture(); change(input); const result = compareCalibration(input);
    assert.equal(result.measurementStatus, 'unverified'); assert.ok(result.checks.every(c => c.error === null));
  }
});
test('proposed thresholds, omitted items, duplicate IDs and absent fonts never pass', () => {
  for (const change of [i => i.tolerances.status = 'proposed', i => i.tolerances.edgePx = null,
    i => i.actual.measurements.pop(), i => i.actual.fonts = []]) {
    const input = fixture(); change(input); assert.equal(compareCalibration(input).measurementStatus, 'unverified');
  }
  const input = fixture(); input.reference.measurements.push(input.reference.measurements[0]);
  assert.throws(() => compareCalibration(input), /duplicate/);
});
test('CLI verifies immutable image identity, missing templates and refuses to overwrite evidence', async t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'abya-calibration-'));
  t.after(() => { assert.equal(path.dirname(root), path.resolve(os.tmpdir())); fs.rmSync(root, { recursive: true }); });
  const image = path.join(root, 'capture.png'), bytes = Buffer.from('Synthetic capture identity, not runtime evidence');
  fs.writeFileSync(image, bytes);
  const input = fixture(), identity = { path: 'capture.png', sha256: crypto.createHash('sha256').update(bytes).digest('hex') };
  for (const side of ['reference', 'actual']) Object.assign(input[side].image, identity);
  assert.equal(await checkImageIdentity(input.actual, root), null);
  const source = path.join(root, 'input.json'); fs.writeFileSync(source, JSON.stringify(input));
  const run = out => spawnSync(process.execPath, [script, '--input', source, '--output', path.join(root, out)], { encoding: 'utf8' });
  assert.equal(run('report.json').status, 0);
  const saved = fs.readFileSync(path.join(root, 'report.json'));
  assert.equal(run('report.json').status, 2); assert.deepEqual(fs.readFileSync(path.join(root, 'report.json')), saved);
  fs.appendFileSync(image, 'changed'); assert.equal(run('changed.json').status, 2);
  assert.equal(JSON.parse(fs.readFileSync(path.join(root, 'changed.json'))).measurementStatus, 'unverified');
  const template = fs.readFileSync(path.resolve(path.dirname(script), '../assets/calibration.template.json'));
  fs.writeFileSync(source, template); assert.equal(run('missing.json').status, 2);
});
