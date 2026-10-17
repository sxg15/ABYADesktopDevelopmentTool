import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import test from 'node:test';
import { contrastPair, emitTokens } from '../.codex/skills/abya-game-development-task/references/game-art-design/references/product-interface-design/scripts/visual-tools.mjs';

test('contrast uses actual font size and weight, never treating 18px normal text as large', () => {
  const text = { fg: '#888', bg: '#fff', fontSizePx: 18 };
  assert.equal(contrastPair(text).status, 'fail');
  assert.equal(contrastPair({ ...text, fontSizePx: 24 }).status, 'pass');
  assert.equal(contrastPair({ ...text, fontSizePx: 14 * 96 / 72, fontWeight: 700 }).status, 'pass');
  assert.equal(contrastPair({ ...text, fontSizePx: 18.5, fontWeight: 700 }).status, 'fail');
  assert.equal(contrastPair({ ...text, fg: '#000' }).ratio, 21);
  assert.equal(contrastPair({ ...text, fg: '#fff' }).status, 'fail');
  assert.equal(contrastPair({ fg: '#000', bg: '#fff' }).status, 'unverified');
  for (const fg of ['#ffffff80', 'var(--text)', 'red', null])
    assert.equal(contrastPair({ ...text, fg }).status, 'unverified');
});

test('token export retains font quotes, object colors and semantic aliases without executing input', () => {
  const font = "Inter, 'Segoe UI', sans-serif";
  const tokens = { primitive: { color: { primary: { 500: { hex: '#16705b' } } }, font: { sans: font } },
    semantic: { color: { action: '{color.primary.500}' } } };
  const css = emitTokens(tokens);
  assert.match(css, /--color-primary-500: #16705b;/);
  assert.match(css, /--color-action: var\(--color-primary-500\);/);
  const sandbox = { module: { exports: {} } };
  vm.runInNewContext(emitTokens(tokens, 'v3'), sandbox, { timeout: 100 });
  assert.equal(sandbox.module.exports.theme.extend.fontFamily.sans, font);
  assert.equal(sandbox.module.exports.theme.extend.colors.action, '#16705b');
  assert.throws(() => emitTokens({ primitive: { a: '{missing}' } }), /Unknown/);
  assert.throws(() => emitTokens({ primitive: { a: '{b}', b: '{a}' } }), /Circular/);
  assert.throws(() => emitTokens({ primitive: { a: { b: 1 }, 'a-b': 2 } }), /Duplicate/);
});

test('all portable design references and scripts resolve within both distributed providers', () => {
  for (const provider of ['.codex', '.grok']) {
    const root = path.resolve(provider, 'skills/abya-game-development-task/references/game-art-design');
    function walk(dir) {
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const file = path.join(dir, entry.name);
        if (entry.isDirectory()) { walk(file); continue; }
        if (!file.endsWith('.md')) continue;
        for (const match of fs.readFileSync(file, 'utf8').matchAll(/\]\(([^)#]+)(?:#[^)]*)?\)/g)) {
          if (/^[a-z]+:/i.test(match[1])) continue;
          const target = path.resolve(path.dirname(file), match[1]);
          assert.ok(target.startsWith(root + path.sep), file);
          assert.ok(fs.statSync(target).isFile(), `${file}: ${match[1]}`);
        }
      }
    }
    walk(root);
  }
});
