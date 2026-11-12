// 参数化表现采样；业务结果、权限和输入状态由接入游戏维护。
export function sample(id,progress,{strength=1,distance=160}={}){
 const t=Math.max(0,Math.min(1,progress)),pulse=Math.sin(Math.PI*t);
 const base={x:0,y:0,scale:1,opacity:1,glow:0,rotation:0};
 switch(id){
  case 'press':return {...base,scale:1-0.08*strength*pulse,y:3*strength*pulse};
  case 'selection':return {...base,glow:(8+8*pulse)*strength};
  case 'panel':return {...base,y:distance*0.2*(1-t)**3,opacity:t};
  case 'reward':return {...base,x:distance*(1-(1-t)**3)-distance/2,y:-45*pulse,scale:1-0.35*t,opacity:t===1?0:1};
  case 'hit':return {...base,scale:1+0.25*strength*pulse,rotation:3*strength*pulse*Math.sin(t*Math.PI*4),glow:25*strength*(1-t),opacity:1};
  case 'aura':return {...base,scale:1+0.06*strength*pulse,glow:(12+12*pulse)*strength};
  default:throw Error('Unknown effect preset');
 }
}
export function style(frame,color){
 return {transform:`translate(${frame.x}px, ${frame.y}px) rotate(${frame.rotation}deg) scale(${frame.scale})`,
  opacity:String(frame.opacity),boxShadow:`0 0 ${frame.glow}px ${color}`};
}
