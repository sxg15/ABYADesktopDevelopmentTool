import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const skillRoot = provider => path.join(root, provider, 'skills', 'abya-game-development-task');
const json = file => JSON.parse(fs.readFileSync(file, 'utf8').replace(/^\uFEFF/, ''));
const unique = values => assert.equal(new Set(values).size, values.length);

for (const provider of ['.codex', '.grok']) {
  test(`${provider}: production policy retains the agreed gates and complete pilot coverage`, () => {
    const policy = json(path.join(skillRoot(provider), 'assets/production/workflow-policy.json'));
    unique(policy.stages.map(stage => stage.id));
    unique(policy.dimensions.map(dimension => dimension.id));
    unique(policy.questions.map(question => question.id));
    assert.deepEqual(policy.stages.flatMap(stage => stage.approval ? [stage.approval] : []),
      ['requirements', 'plan', 'delivery']);
    assert.equal(policy.rounds.count, 12);
    assert.deepEqual(policy.rounds.questionRounds, [9, 10, 11, 12]);
    assert.deepEqual(policy.rounds.milestones, ['R0', 'R8', 'R12']);
    assert.equal(policy.rounds.allDimensionsEveryRound, true);
    assert.equal(policy.rounds.allQuestionsEachQuestionRound, true);
    for (const family of ['functional', 'visual']) {
      assert.equal(policy.questions.filter(question => question.family === family).length, 8);
    }
    assert.ok(policy.questions.every(question => question.text.trim()));
    assert.equal(policy.dimensions.length, 9);
  });

  test(`${provider}: new task and round templates contain no fabricated acceptance or evidence`, () => {
    const directory = path.join(skillRoot(provider), 'assets/production');
    const policy = json(path.join(directory, 'workflow-policy.json'));
    const workflow = json(path.join(directory, 'workflow.template.json'));
    const round = json(path.join(directory, 'round.template.json'));
    assert.equal(workflow.workflowVersion, policy.workflowVersion);
    assert.equal(round.workflowVersion, policy.workflowVersion);
    assert.deepEqual(Object.keys(workflow.stages), policy.stages.map(stage => stage.id));
    assert.ok(Object.values(workflow.stages).every(status => status === 'not-started'));
    assert.ok(Object.values(workflow.productStatus).every(status => status === 'not-started'));
    for (const key of ['taskId', 'provider', 'questionMode', 'currentVersion']) assert.equal(workflow[key], null);
    for (const key of ['approvals', 'evidence', 'roundFiles', 'checks']) assert.deepEqual(workflow[key], []);
    assert.equal(workflow.currentRound, 0);
    assert.equal(round.number, null);
    assert.equal(round.closedAt, null);
    assert.equal(round.status, 'not-started');
    assert.deepEqual(round.checks, []);
    assert.deepEqual(round.questions, []);
    assert.deepEqual(round.evidenceIds, []);
  });

  test(`${provider}: staged reading references resolve inside the distributed bundle`, () => {
    const base = skillRoot(provider);
    const sources = ['SKILL.md', 'references/production-stages.md', 'references/production-records.md',
      'references/runtime-authoring.md', 'references/whole-experience-review.md'];
    for (const relative of sources) {
      const file = path.join(base, relative);
      const text = fs.readFileSync(file, 'utf8');
      for (const match of text.matchAll(/\]\(([^)#]+)(?:#[^)]*)?\)/g)) {
        if (/^[a-z]+:/i.test(match[1])) continue;
        const target = path.resolve(path.dirname(file), match[1]);
        assert.ok(target.startsWith(base + path.sep), `${relative}: external resource ${match[1]}`);
        assert.ok(fs.statSync(target).isFile(), `${relative}: missing ${match[1]}`);
      }
    }
  });
}
