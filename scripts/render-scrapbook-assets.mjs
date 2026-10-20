import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const root=path.resolve('.codex/skills/abya-art-template-scrapbook-collage');
const {chromium}=createRequire(import.meta.url)(process.env.ABYA_TEST_PLAYWRIGHT||'playwright');
const browser=await chromium.launch({headless:true,...(process.env.ABYA_TEST_BROWSER?{executablePath:process.env.ABYA_TEST_BROWSER}:{})});
try {
  const page=await browser.newPage({deviceScaleFactor:1});
  const out=path.join(root,'assets/png');fs.mkdirSync(out,{recursive:true});
  for(const name of fs.readdirSync(path.join(root,'assets/source')).filter(x=>x.endsWith('.svg'))) {
    const body=fs.readFileSync(path.join(root,'assets/source',name),'utf8');
    const [width,height]=[...body.matchAll(/(?:width|height)="(\d+)"/g)].slice(0,2).map(x=>Number(x[1]));
    await page.setViewportSize({width,height});
    await page.setContent(`<style>html,body{margin:0;background:transparent}svg{display:block}</style>${body}`);
    await page.screenshot({path:path.join(out,name.replace('.svg','.png')),omitBackground:true});
  }
} finally {await browser.close();}
