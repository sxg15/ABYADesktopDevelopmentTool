import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
export function makeAssets(tokens) {
  const c=tokens.colors;
  let seed=tokens.decoration.seed;
  const random=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed/4294967296;};
  const svg=(w,h,body)=>`<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${body}</svg>`;
  const texture=(w,h,n,color)=>Array.from({length:n},()=>`<path d="M${(random()*w).toFixed(2)} ${(random()*h).toFixed(2)}l${(1+random()*3).toFixed(2)} .4" stroke="${color}" opacity=".09" stroke-width=".6"/>`).join('');
  const starPoints=Array.from({length:10},(_,i)=>{const a=i*Math.PI/5-Math.PI/2,r=i%2?13:27;return `${32+Math.cos(a)*r},${32+Math.sin(a)*r}`;}).join(' ');
  return {
    'solid.svg':svg(16,16,'<path fill="#fff" d="M0 0h16v16H0z"/>'),
    'hit.svg':svg(16,16,'<path fill="#fff" fill-opacity="0" d="M0 0h16v16H0z"/>'),
    'paper.svg':svg(512,512,`<path fill="${c.paper}" d="M0 0h512v512H0z"/>${texture(512,512,1600,c.ink)}`),
    'panel.svg':svg(480,596,`<path d="M6 7L473 4 476 589 9 592Z" fill="${c.kraft}" opacity=".34"/><path d="M3 3L471 1 473 584 5 588Z" fill="${c.cream}" stroke="${c.line}" stroke-width="2"/>${texture(470,584,400,c.ink)}<path d="M16 18H456V571H19Z" fill="none" stroke="${c.kraft}" stroke-width="1" stroke-dasharray="4 5"/>`),
    'torn-paper.svg':svg(288,408,`<path d="M4 8L15 4 28 9 39 3 55 7 71 4 86 9 102 3 119 7 138 4 152 8 169 3 182 8 201 5 219 9 236 3 251 7 269 4 283 8V397L270 402 252 398 237 405 219 399 201 402 182 398 160 405 141 400 121 403 102 397 85 402 69 398 47 404 29 398 12 403 4 399Z" fill="${c.cream}" stroke="${c.kraft}" stroke-width="1.5"/>${texture(280,390,180,c.ink)}`),
    'tape.svg':svg(140,44,`<path d="M6 3L15 6 23 2 35 6 47 3 63 5 77 2 91 5 105 3 118 6 133 2 137 40 123 37 109 42 95 39 81 41 68 38 53 42 41 38 24 42 11 38Z" fill="${c.tape}" fill-opacity=".82"/>${texture(135,42,100,c.ink)}`),
    'star.svg':svg(64,64,`<polygon points="${starPoints}" fill="#E8BA62" stroke="${c.ink}" stroke-width="2.5" stroke-linejoin="round"/><path d="M24 28l3-7M38 37l5-1" stroke="${c.cream}" stroke-width="3" stroke-linecap="round"/>`),
    'sticker.svg':svg(120,88,`<path d="M8 13Q26 1 47 9Q67 1 88 10Q114 14 112 38Q120 62 98 75Q76 88 55 78Q30 88 14 72Q0 62 7 42Q-1 23 8 13Z" fill="${c.blue}" stroke="${c.cream}" stroke-width="5"/><path d="M31 47l14 13 33-34" fill="none" stroke="${c.success}" stroke-width="6" stroke-linecap="round" stroke-linejoin="round"/>`)
  };
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  const out=path.join(root,'assets/source');fs.mkdirSync(out,{recursive:true});
  const tokens=JSON.parse(fs.readFileSync(path.join(root,'assets/design-tokens.json'),'utf8'));
  for(const [name,body] of Object.entries(makeAssets(tokens))) fs.writeFileSync(path.join(out,name),body);
  console.log('Wrote original vector assets. Raster delivery is generated separately; no external images or fonts copied.');
}
