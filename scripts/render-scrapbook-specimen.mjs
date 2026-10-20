import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
import {buildScene} from '../.codex/skills/abya-art-template-scrapbook-collage/scripts/specimen-model.mjs';
const root=path.resolve('.codex/skills/abya-art-template-scrapbook-collage');
const spec=JSON.parse(fs.readFileSync(path.join(root,'assets/specimen.json')));
const tokens=JSON.parse(fs.readFileSync(path.join(root,'assets/design-tokens.json')));
if(!process.env.ABYA_SPECIMEN_FONT)throw Error('Provide the inspected local SourceHanSans font path; no font is distributed.');
const font=fs.readFileSync(process.env.ABYA_SPECIMEN_FONT).toString('base64');
const {chromium}=createRequire(import.meta.url)(process.env.ABYA_TEST_PLAYWRIGHT||'playwright');
const browser=await chromium.launch({headless:true,...(process.env.ABYA_TEST_BROWSER?{executablePath:process.env.ABYA_TEST_BROWSER}:{})});
const output=path.resolve(process.argv[2]||'logs/scrapbook-20261020/reference');fs.mkdirSync(output,{recursive:true});
const esc=x=>String(x).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('"','&quot;');
try {
  const page=await browser.newPage({deviceScaleFactor:1});
  for(const width of [1280,1920])for(const mode of ['3','4'])for(const state of spec.states){
    const scale=width/spec.canvas.width;
    await page.setViewportSize({width,height:width*9/16});
    // Compare stable endpoints. Transient success stars are verified from native video.
    const body=buildScene(spec,tokens,mode,state).filter(n=>n.kind!=='Button'&&!n.id.startsWith('star-')).map(n=>{
      const r=n.rect,style=`left:${r.x}px;top:${r.y}px;width:${r.width}px;height:${r.height}px;`;
      if(n.kind==='Text')return `<div class="text" style="${style}font-size:${n.fontSize}px;color:${n.color};text-align:${n.align.toLowerCase()};justify-content:${{Left:'flex-start',Center:'center',Right:'flex-end'}[n.align]};">${esc(n.text)}</div>`;
      if(n.asset==='solid')return `<div class="image" style="${style}background:${n.color}"></div>`;
      const png=fs.readFileSync(path.join(root,'assets/png',n.asset+'.png')).toString('base64');
      const rgb=[1,3,5].map(i=>parseInt(n.color.slice(i,i+2),16)/255);
      const filter=rgb.every(v=>v===1)?'':`<svg width="0" height="0"><filter id="tint-${n.id}" color-interpolation-filters="sRGB"><feComponentTransfer>${['R','G','B'].map((v,i)=>`<feFunc${v} type="linear" slope="${rgb[i]}"/>`).join('')}</feComponentTransfer></filter></svg>`;
      return `${filter}<img class="image" style="${style}${filter?`filter:url(#tint-${n.id});`:''}" src="data:image/png;base64,${png}"/>`;
    }).join('');
    await page.setContent(`<style>@font-face{font-family:Specimen;src:url(data:font/otf;base64,${font})}*{box-sizing:border-box}body{margin:0;background:${tokens.colors.paper}}main{position:relative;width:1280px;height:720px;transform:scale(${scale});transform-origin:0 0}.text,.image{position:absolute}.text{display:flex;align-items:center;white-space:nowrap;line-height:1;font-family:Specimen;font-weight:400}</style><main>${body}</main>`);
    const fonts=await page.evaluate(async()=>{await document.fonts.ready;const faces=await document.fonts.load('20px Specimen','目标9');return faces.map(f=>({family:f.family,status:f.status}));});
    if(!fonts.length||fonts.some(f=>f.status!=='loaded'))throw Error('Reference font did not load; do not compare fallback rendering');
    const cdp=await page.context().newCDPSession(page);
    await cdp.send('DOM.enable');await cdp.send('CSS.enable');
    const {root:document}=await cdp.send('DOM.getDocument');
    const {nodeIds}=await cdp.send('DOM.querySelectorAll',{nodeId:document.nodeId,selector:'.text'});
    const fontUse=[];
    for(const nodeId of nodeIds){
      const {fonts:used}=await cdp.send('CSS.getPlatformFontsForNode',{nodeId});
      if(!used.length||used.some(f=>!f.isCustomFont||f.postScriptName!=='SourceHanSansOLD-Medium'))
        throw Error('A sample used a fallback or unexpected font: '+JSON.stringify(used));
      fontUse.push(used);
    }
    fs.writeFileSync(path.join(output,`${width}-${mode}-${state}-fonts.json`),JSON.stringify(fontUse,null,2));
    await cdp.detach();
    await page.screenshot({path:path.join(output,`${width}-${mode}-${state}.png`)});
  }
} finally {await browser.close();}
console.log('Reference renders written; these are not native runtime evidence.');
