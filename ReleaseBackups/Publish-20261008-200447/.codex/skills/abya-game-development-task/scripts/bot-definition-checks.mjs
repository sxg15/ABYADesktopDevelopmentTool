import { definitionsHash, normalizeDefinition, requireThat, same } from './bot-artifact-core.mjs';
export const difficulties = ['Easy', 'Medium', 'Hard', 'Hell'];

// 接口已做完整 Lua 校验；本地检查集合身份与易错参数，不伪装成 Lua 编译器。
export function definitionMap(envelope, archiveId) {
  requireThat(envelope?.archiveGuid === archiveId && Array.isArray(envelope.definitions), '定义存档身份不匹配');
  const map = new Map();
  for (const definition of envelope.definitions) {
    requireThat(typeof definition?.ID === 'string' && definition.ID.trim() && !map.has(definition.ID), '定义 ID 为空或重复');
    map.set(definition.ID, normalizeDefinition(definition));
  }
  return map;
}
export function validateCandidate(plan, before, current, candidate) {
  const original = definitionMap(before, plan.target.archiveId);
  definitionMap(current, plan.target.archiveId);
  const next = definitionMap(candidate, plan.target.archiveId);
  requireThat(definitionsHash(before) === definitionsHash(current), '定义已被其他写入修改，重新读取合并');
  const changed = new Set(plan.changedDefinitionIds), removed = new Set(plan.removedDefinitionIds);
  requireThat(![...changed].some(id => removed.has(id)), '同一 ID 不能同时修改和删除');
  requireThat(plan.enabled || (changed.size === 0 && removed.size === 0), '未启用人机开发时必须保留原定义');
  for (const [id, definition] of original) {
    if (removed.has(id)) requireThat(!next.has(id), `声明删除但仍存在: ${id}`);
    else requireThat(next.has(id) && (changed.has(id) || same(definition, next.get(id))), `无关定义被删除或修改: ${id}`);
  }
  for (const id of removed) requireThat(original.has(id), `删除不存在的 ID: ${id}`);
  requireThat(original.size === 0 || next.size > 0 || plan.allowRemoveAll, '清空全部定义需要明确 allowRemoveAll');
  for (const id of next.keys()) requireThat(original.has(id) || changed.has(id), `未声明的新增 ID: ${id}`);
  for (const id of changed) {
    const d = next.get(id);
    requireThat(d && typeof d.Name === 'string' && d.Name.trim(), `缺少候选定义或名称: ${id}`);
    requireThat(difficulties.includes(d.DefaultDifficulty) && Array.isArray(d.Difficulties)
      && d.Difficulties.length === 4 && d.Difficulties.every(p => p && typeof p === 'object')
      && new Set(d.Difficulties.map(p => p.Difficulty)).size === 4, `${id}: 缺少四档参数`);
    for (const p of d.Difficulties) requireThat(difficulties.includes(p.Difficulty)
      && Number.isFinite(p.ThinkInterval) && p.ThinkInterval >= .05
      && Number.isFinite(p.ReactionDelay) && p.ReactionDelay >= 0
      && Number.isFinite(p.Accuracy) && p.Accuracy >= 0 && p.Accuracy <= 1
      && Number.isInteger(p.Lookahead) && p.Lookahead >= 0, `${id}: 难度参数越界`);
    requireThat((typeof d.LuaSource === 'string' && d.LuaSource.trim())
      || (typeof d.LuaTextResourceID === 'string' && d.LuaTextResourceID.trim()), `${id}: 脚本来源为空`);
  }
  const botIds = plan.bots.map(bot => bot.definitionId);
  requireThat(new Set(botIds).size === botIds.length, '计划 Bot ID 重复');
  if (plan.enabled) requireThat(same([...botIds].sort(), [...next.keys()].sort()), '计划必须覆盖完整候选定义');
  for (const bot of plan.bots) if (bot.observationContract.source === 'message')
    requireThat(bot.observationContract.channel?.trim(), `${bot.definitionId}: 消息观察缺少通道`);
  return definitionsHash(candidate);
}
