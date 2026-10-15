// JSON.parse 会丢弃重复键；先扫描 JSON token，拒绝会改变构建意图的重复声明。
export function parseBuildJson(text) {
  text = text.replace(/^\uFEFF/, '');
  const tokens = text.match(/"(?:[^"\\]|\\.)*"|[{}\[\]:,]|[^\s{}\[\]:,]+/g) || [];
  const stack = [];
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    if (token === '{') stack.push(new Set());
    else if (token === '[') stack.push(null);
    else if (token === '}' || token === ']') stack.pop();
    else if (token.startsWith('"') && tokens[i + 1] === ':' && stack.at(-1)) {
      const key = JSON.parse(token);
      if (stack.at(-1).has(key)) throw Error('重复配置字段：' + key);
      stack.at(-1).add(key);
    }
  }
  return JSON.parse(text);
}
