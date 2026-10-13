// Isolated real-xterm regression fixture: no native process, model or user session.
import {Terminal} from '@xterm/xterm';
import {installTerminalComposition} from '../src/features/terminal/terminalComposition';
import '@xterm/xterm/css/xterm.css';
import '../src/App.css';
const root=document.getElementById('root')!;
root.style.cssText='padding:20px;overflow:auto;height:100%;';
root.innerHTML='<h2>中文组合输入与终端重绘验证</h2><p>隔离测试：模拟 AI 更新日志时移动光标，并检查输入法定位和中文提交次数。</p><button id="old">复现旧行为</button> <button id="fixed">验证修复</button><pre id="result" role="status">等待测试</pre><div class="terminal-host" style="height:360px;margin-top:16px;background:#171c1e"></div>';
const host=root.querySelector<HTMLElement>('.terminal-host')!;
const result=root.querySelector<HTMLElement>('#result')!;
const terminal=new Terminal({cols:90,rows:22,fontSize:13,lineHeight:1.15,cursorBlink:false,theme:{background:'#171c1e',foreground:'#d7e2de'}});
terminal.open(host);
let guard:ReturnType<typeof installTerminalComposition>|undefined;
let busy=false;
const wait=(ms:number)=>new Promise(resolve=>setTimeout(resolve,ms));
const write=(data:string)=>new Promise<void>(resolve=>terminal.write(data,resolve));
async function run(fixed:boolean) {
  if(busy)return;
  busy=true;guard?.dispose();guard=undefined;
  const sent:string[]=[];
  const input=terminal.onData(data=>sent.push(data));
  try {
    terminal.reset();
    await write('\x1b[2J\x1b[1;1HAI 正在输出……\x1b[18;1H> ');
    terminal.focus();await wait(40);
    if(fixed)guard=installTerminalComposition(host);
    const area=terminal.textarea!;
    area.value='';
    area.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));
    area.value='nihao';
    area.dispatchEvent(new CompositionEvent('compositionupdate',{bubbles:true,data:'nihao'}));
    await wait(30);
    const anchor=area.getBoundingClientRect();
    const positions:Array<{x:number;y:number}>=[];
    for(let i=0;i<12;i++) {
      await write(`\x1b[${i%2?18:3+i};${i%2?3:12}H${i%2?'':`AI 日志 ${i}`}`);
      await wait(30);
      const rect=area.getBoundingClientRect();positions.push({x:rect.x,y:rect.y});
    }
    const outputVisible=terminal.buffer.active.getLine(2)?.translateToString().includes('AI 日志');
    area.value='你好';
    area.dispatchEvent(new CompositionEvent('compositionupdate',{bubbles:true,data:'你好'}));
    await wait(10);
    area.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true,data:'你好'}));
    await wait(30);
    const moves=positions.filter(p=>Math.abs(p.x-anchor.x)>0.5||Math.abs(p.y-anchor.y)>0.5).length;
    const passed=fixed?moves===0&&sent.join('')==='你好'&&sent.length===1&&!guard!.isComposing()&&outputVisible:moves>0;
    result.textContent=JSON.stringify({mode:fixed?'修复后':'旧行为',passed,redraws:12,anchorMoves:moves,committed:sent,outputContinued:!!outputVisible,released:!guard?.isComposing()},null,2);
    if(!passed)throw new Error('IME redraw regression failed');
  }catch(error){result.textContent+='\n'+String(error);}
  finally{input.dispose();busy=false;}
}
root.querySelector('#old')!.addEventListener('click',()=>void run(false));
root.querySelector('#fixed')!.addEventListener('click',()=>void run(true));
