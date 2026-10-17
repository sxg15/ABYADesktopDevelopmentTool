// Same real task fixture at two local builds; not a model-quality or human-approval test.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const [before, after, directory] = process.argv.slice(2);
if (!before || !after || !directory || process.argv.length !== 5) throw new Error('Usage: <baseline-url> <current-url> <output-directory>');
for (const value of [before, after]) if (new URL(value).hostname !== '127.0.0.1') throw new Error('Local fixtures only.');
const output = path.resolve(directory); fs.mkdirSync(output, { recursive: true });
const { chromium } = createRequire(import.meta.url)(process.env.ABYA_TEST_PLAYWRIGHT || 'playwright');
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const observations = [], errors = [];
try {
  for (const [label, origin] of [['before', before], ['after', after]]) {
    const page = await browser.newPage({ viewport: { width: 1440, height: 960 } });
    page.on('pageerror', error => errors.push({ label, error: error.message }));
    await page.goto(`${origin}/scripts/workspace-refinement-preview.html`);
    await page.getByRole('tab', { name: /^制作流程/ }).click();
    await page.getByText('要求与审阅', { exact: true }).waitFor();
    for (const text of ['任务说明', '美术方案已更新，待你审阅', '资源美术', '不要用数字显示库存，直接显示剩余物品。'])
      assert.ok(await page.getByText(text, { exact: false }).first().isVisible(), `${label}: retained ${text}`);
    if (label === 'after') {
      await page.getByText('当前记录的视觉约束', { exact: true }).click();
      await page.locator('.art-proposal-context').getByText('清楚的物品轮廓与稳定身份色。', { exact: true }).waitFor();
    }
    const top = () => page.evaluate(() => { for (const element of document.querySelectorAll('*')) if (element.scrollTop) element.scrollTop = 0; });
    await top();
    await page.screenshot({ path: path.join(output, `${label}-workflow-wide.png`), fullPage: true });
    for (const viewport of [{ width: 1100, height: 760 }, { width: 960, height: 720 }]) {
      await page.setViewportSize(viewport);
      await top();
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), `${label}: body overflows`);
      await page.screenshot({ path: path.join(output, `${label}-workflow-${viewport.width}.png`), fullPage: true });
    }
    await page.setViewportSize({ width: 1440, height: 960 });
    await page.getByRole('tab', { name: 'Skill 库', exact: true }).click();
    await page.getByRole('tab', { name: '美术设计库', exact: true }).click();
    await page.getByRole('button', { name: /物品库存 · 纸质桌游 HUD/ }).click();
    await page.locator('.art-case-detail img').first().waitFor();
    assert.ok((await page.locator('.art-case-detail').innerText()).includes('认可物品数量的表达方式'));
    if (label === 'after') {
      const summary = page.locator('.art-case-context');
      assert.ok((await summary.innerText()).includes('大规模库存'));
      assert.ok(await summary.evaluate(element => !!(element.compareDocumentPosition(document.querySelector('.art-case-media')) & Node.DOCUMENT_POSITION_FOLLOWING)));
    }
    await page.locator('.art-case-detail').scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.join(output, `${label}-library-wide.png`), fullPage: true });
    await page.setViewportSize({ width: 960, height: 720 });
    await page.locator('.art-case-detail h3').scrollIntoViewIfNeeded();
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
    await page.screenshot({ path: path.join(output, `${label}-library-960.png`), fullPage: true });
    observations.push({ build: label, retainedRequirements: true, retainedNavigation: true, viewports: ['1440x960', '1100x760', '960x720'], horizontalOverflow: false });
    await page.close();
  }
  const page = await browser.newPage({ viewport: { width: 1100, height: 1150 } });
  await page.goto(`${after}/scripts/visual-refinement-hud.html`);
  for (const scene of ['before', 'after']) {
    assert.equal(await page.locator(`.${scene} .identity span`).count(), 4);
    assert.equal(await page.locator(`.${scene} .items b`).count(), 5);
    assert.equal(await page.locator(`.${scene} .items .spent`).count(), 2);
  }
  await page.screenshot({ path: path.join(output, 'hud-controlled-comparison.png'), fullPage: true });
  assert.deepEqual(errors, []);
  fs.writeFileSync(path.join(output, 'report.json'), JSON.stringify({ scope: 'controlled real-component and static HUD fixtures; no model A/B, real game or planner approval', observations, errors }, null, 2));
  console.log(JSON.stringify({ output, observations, errors }));
} finally { await browser.close(); }
