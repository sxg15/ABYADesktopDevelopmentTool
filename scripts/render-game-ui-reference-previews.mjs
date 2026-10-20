// Authored structural diagrams, not game screenshots or an approved visual skin.
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const out=path.resolve('.codex/skills/abya-game-development-task/references/game-art-design/references/game-ui-design/assets/reference-previews');
const esc=v=>String(v).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('"','&quot;');
const text=(x,y,s,size=18,color='#263c48')=>`<text x="${x}" y="${y}" font-size="${size}" fill="${color}">${esc(s)}</text>`;
const box=(x,y,w,h,fill='#fff',stroke='#879da8')=>`<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="6" fill="${fill}" stroke="${stroke}" stroke-width="2"/>`;
const arrow=(x1,y1,x2,y2,dash='')=>`<path d="M${x1} ${y1} L${x2} ${y2}" fill="none" stroke="#527b8a" stroke-width="2" ${dash?`stroke-dasharray="${dash}"`:''} marker-end="url(#arrow)"/>`;
const svg=(title,body)=>`<svg xmlns="http://www.w3.org/2000/svg" width="960" height="600" viewBox="0 0 960 600"><defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto"><path d="M0 0L8 4L0 8z" fill="#527b8a"/></marker></defs><rect width="960" height="600" fill="#edf2f4"/><g font-family="Microsoft YaHei,Segoe UI,sans-serif">${text(40,42,title,26)}${text(40,72,'按玩法推导关系；位置、形状与尺寸均需重新设计',15,'#526a76')}${body}${text(40,570,'原创结构假设 · 非真实游戏截图 · 非字体规格或策划认可案例',15,'#526a76')}</g></svg>`;
const diagrams={
  'hud-action-zones':svg('HUD：信息与操作职责',
    box(40,92,880,370,'#dce6eb')+text(62,122,'游戏视野 / 真实镜头与内容',17)+
    box(62,142,206,62)+text(78,170,'持续信息',19)+text(78,191,'状态、资源等',15)+
    box(664,142,230,62)+text(680,170,'事件 / 方向信息',19)+text(680,191,'来源与重要性要清楚',15)+
    box(300,205,340,120,'#f8fbfc','#a1b4bd')+text(332,251,'主玩法注意力区',23)+text(332,285,'保护角色、目标与操作判断',17)+
    box(568,358,326,82)+text(588,389,'动作与即时反馈',20)+text(588,417,'按当前输入安排，避开遮挡',16)+
    arrow(470,345,568,383)+text(62,500,'不是固定四角布局：信息随对象、频率与镜头重新分配。',18)+
    text(62,532,'纸纹、阴影和装饰不得吞掉焦点、数值、目标或触控区域。',17)),
  'inventory-detail':svg('物品栏：选择与详情关联',
    box(40,98,425,342)+text(62,132,'物品集合 / 真实分类与容量',20)+
    Array.from({length:8},(_,i)=>{
      const x=62+(i%4)*96,y=155+Math.floor(i/4)*116;
      return box(x,y,76,88,i===1?'#d4e7f0':'#f5f8fa',i===1?'#2d7397':'#91a6b0')+
        `<circle cx="${x+38}" cy="${y+32}" r="15" fill="#7996a5"/>`+text(x+10,y+70,i===1?'已选物品':'物品轮廓',13);
    }).join('')+
    box(493,98,427,342)+text(517,132,'当前选择的详情',20)+text(517,175,'用途、差异、条件与可执行动作',18)+
    text(517,218,'已有图示要求与身份语义继续保留',17)+text(517,265,'比较关系由真实物品决定',17)+
    box(517,338,172,56,'#d4e7f0')+text(542,373,'使用 / 装备',18)+
    box(710,338,172,56)+text(746,373,'取消 / 返回',18)+arrow(465,240,493,240)+
    text(62,484,'手柄焦点、当前选择与鼠标悬停分别表达。',18)+
    text(62,520,'返回后保留选择与焦点；不以参考图决定固定网格或导航模型。',17)),
  'action-feedback':svg('组件反馈：输入、执行与结果',
    ['可用','输入回执','执行中','结果反馈','下一步'].map((name,i)=>{
      const x=40+i*180;
      return box(x,155,158,92,i===3?'#dcebdc':'#fff')+text(x+20,192,name,20)+
        text(x+20,224,['明确可操作','立即看见响应','只在实际需要时','成功 / 失败','继续 / 重试'][i],14)+
        (i<4?arrow(x+158,200,x+180,200):'');
    }).join('')+
    text(62,306,'失败说明可恢复的原因；取消与返回有可预测去向。',18)+
    box(220,367,520,72,'#f4f8fa')+text(246,398,'装饰与庆祝动画：按需播放、可取消或替换',18)+
    text(246,423,'玩法判定、数值更新和连续输入不等待它结束',16)+
    arrow(299,247,299,367,'6 5')+arrow(740,403,839,247,'6 5')+
    text(62,500,'这是关系示意；不要求所有控件都具有加载、失败或庆祝状态。',17)),
};
const {chromium}=createRequire(import.meta.url)(process.env.ABYA_TEST_PLAYWRIGHT||'playwright');
const browser=await chromium.launch({headless:true,...(process.env.ABYA_TEST_BROWSER?{executablePath:process.env.ABYA_TEST_BROWSER}:{})});
try{
  fs.mkdirSync(out,{recursive:true});
  const page=await browser.newPage({viewport:{width:960,height:600},deviceScaleFactor:1});
  for(const [name,source] of Object.entries(diagrams)){
    fs.writeFileSync(path.join(out,name+'.svg'),source);
    await page.setContent(`<style>body{margin:0}</style>${source}`);await page.evaluate(()=>document.fonts.ready);
    await page.screenshot({path:path.join(out,name+'.png')});
  }
}finally{await browser.close();}
console.log('Wrote 3 original structural diagrams; these are not runtime evidence.');
