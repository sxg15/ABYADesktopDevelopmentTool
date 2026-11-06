import preview from "../../../.codex/skills/abya-game-development-task/assets/art-library/preview.html?raw";
import effects from "../../../.codex/skills/abya-game-development-task/assets/art-library/effects.mjs?raw";
import catalog from "../../../.codex/skills/abya-game-development-task/assets/art-library/catalog.json";

/** 只渲染随包原创源码；导入资源不作为 WebView 脚本执行。 */
export function ArtRecipePreview({presetId}:{presetId:string}) {
  if(!catalog.entries.some(entry=>entry.id===presetId))return null;
  const source=preview.replace("import {sample,style} from './effects.mjs';",effects.split("export function ").join("function "))
    .replace("const catalog=await fetch('./catalog.json').then(response=>response.json());",`const catalog=${JSON.stringify(catalog)};`)
    .split("@@PRESET@@").join(presetId);
  return <iframe title="表现配方交互预览" sandbox="allow-scripts" srcDoc={source}
    style={{width:"100%",height:440,border:0,borderRadius:8}}/>;
}
