import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { importTemplate } from "../.codex/skills/abya-import-task-template/scripts/import-template.mjs";

for (const kind of ["task", "art"]) test(`${kind}: imports complete resources, updates idempotently and protects collisions`, () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "abya-template-test-"));
  try {
    const source = path.join(root, "source");
    const destination = path.join(root, "tool");
    fs.mkdirSync(path.join(source, "references"), { recursive: true });
    fs.mkdirSync(path.join(source, "agents"));
    const original = '---\nname: example\ndescription: |\n  reusable workflow\nmetadata:\n  short-description: retained\n---\n# Original\n[Rules](references/rules.md)\n';
    fs.writeFileSync(path.join(source, "SKILL.md"), original);
    fs.writeFileSync(path.join(source, "references/rules.md"), "original rules");
    fs.writeFileSync(path.join(source, "references/image.png"), Buffer.from([0, 137, 255, 13, 10]));
    fs.writeFileSync(path.join(source, "agents/openai.yaml"), 'interface:\n  default_prompt: "Use $example"\n');
    for (const provider of [".codex", ".grok"]) {
      const general = path.join(destination, provider, "skills/abya-game-development-task");
      fs.mkdirSync(general, { recursive: true });
      fs.writeFileSync(path.join(general, "SKILL.md"), "general");
    }
    const options = { kind, source, destination, slug: "example", displayName: "示例模板", description: "示例流程" };
    const result = importTemplate(options);
    for (const target of result.targets) {
      assert.deepEqual(fs.readFileSync(path.join(target, "references/image.png")), Buffer.from([0, 137, 255, 13, 10]));
      assert.equal(fs.readFileSync(path.join(target, "references/rules.md"), "utf8"), "original rules");
      assert.match(fs.readFileSync(path.join(target, "SKILL.md"), "utf8"), /short-description: retained/);
      assert.match(fs.readFileSync(path.join(target, "agents/openai.yaml"), "utf8"), new RegExp(`\\$abya-${kind}-template-example`));
      assert.equal(JSON.parse(fs.readFileSync(path.join(target, `abya-${kind}-template.json`))).displayName, "示例模板");
    }
    fs.writeFileSync(path.join(source, "references/rules.md"), "updated rules");
    fs.unlinkSync(path.join(source, "agents/openai.yaml"));
    importTemplate(options);
    for (const target of result.targets) {
      assert.equal(fs.readFileSync(path.join(target, "references/rules.md"), "utf8"), "updated rules");
      assert.equal(fs.existsSync(path.join(target, "agents/openai.yaml")), false);
    }
    fs.unlinkSync(path.join(result.targets[1], `abya-${kind}-template.json`));
    fs.writeFileSync(path.join(source, "references/rules.md"), "must not install");
    assert.throws(() => importTemplate(options), /unmanaged Skill/);
    assert.equal(fs.readFileSync(path.join(result.targets[0], "references/rules.md"), "utf8"), "updated rules");
    assert.equal(fs.readFileSync(path.join(source, "SKILL.md"), "utf8"), original);
    assert.throws(() => importTemplate({ ...options, slug: "../escape" }), /slug/);
    assert.throws(() => importTemplate({ ...options, destination: source }), /missing the bundled workflow/);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});
