export function buildScene(spec,tokens,mode='3',state='default') {
  if(!spec.grids[mode]||!spec.states.includes(state))throw Error('Unknown specimen mode/state');
  const c=tokens.colors,nodes=[];
  const image=(id,asset,x,y,width,height,color='#FFFFFF',visible=true)=>nodes.push({id,kind:'Image',asset,rect:{x,y,width,height},color,visible});
  const rect=(id,x,y,w,h,color)=>image(id,'solid',x,y,w,h,color);
  const text=(id,value,x,y,width,height,fontSize,align='Center',color=c.ink)=>nodes.push({id,kind:'Text',text:String(value),rect:{x,y,width,height},fontSize,align,color,visible:true});
  const button=(id,label,x,y,width,height)=>{rect(id+'-paper',x,y,width,height,c.cream);text(id+'-label',label,x,y,width,height,spec.controls.fontSize);nodes.push({id,kind:'Button',asset:'hit',rect:{x,y,width,height},visible:true});};
  image('background','paper',0,0,spec.canvas.width,spec.canvas.height);
  // Fixed authored accents; no random positions or rotating interactive targets.
  for(const n of spec.accents)image(n.id,n.asset,...n.rect,n.color?c[n.color]:'#FFFFFF');
  for(const n of spec.notes)text(n.id,n.text,...n.rect,n.fontSize,'Center',c[n.color||'ink']);
  image('board','panel',spec.board.x,spec.board.y,spec.board.width,spec.board.height);
  image('header-tape','tape',spec.header.x,spec.header.y,spec.header.width,spec.header.height);
  text('title',spec.header.text,spec.header.x,spec.header.y,spec.header.width,spec.header.height,spec.header.fontSize);
  const g=spec.grids[mode],target=spec.target,label=spec.targetLabel;
  text('target-label',state==='settlement'?'本轮完成':label.text,label.x,label.y,label.width,label.height,label.fontSize);
  text('target',state==='settlement'?'完成':g.target,target.x,target.y,target.width,target.height,target.fontSize);
  for(let i=0;i<g.values.length;i++){
    const x=g.x+(i%g.columns)*(g.cell+g.gap),y=g.y+Math.floor(i/g.columns)*(g.cell+g.gap);
    const chosen=state!=='default'&&spec.selectedCells.includes(i);
    const border=chosen?(state==='error'?c.error:state==='success'?c.success:c.ink):c.line;
    rect('cell-'+i+'-border',x,y,g.cell,g.cell,border);
    const b=spec.cellBorderPixels,f=tokens.feedback;
    rect('cell-'+i+'-fill',x+b,y+b,g.cell-2*b,g.cell-2*b,chosen?c.selected:c.cell);
    text((mode==='4'&&i<9?'cell4-':'cell-')+i,g.values[i],x,y,g.cell,g.cell,g.fontSize);
    if(state==='success'&&chosen)for(let j=0;j<tokens.feedback.starCountPerCell;j++)
      image(`star-${i}-${j}`,'star',x+f.starStartX+j*f.starStepX,y+f.starStartY+j*f.starStepY,f.starSize,f.starSize);
  }
  const s=spec.score,l=spec.scoreLayout;
  const offset=(r,y=s.y)=>[s.x+r[0],y+r[1],r[2],r[3]];
  image('score','torn-paper',s.x,s.y,s.width,s.height);
  image('score-tape','tape',...offset(l.tape));
  text('timer-label',spec.timer.label,...offset(l.label),l.label[4]);
  text('timer',state==='settlement'?'0':state==='error'?'10':spec.timer.value,...offset(l.timer),spec.timer.fontSize);
  text('timer-unit','秒',...offset(l.unit),l.unit[4]);
  for(let i=0;i<s.names.length;i++){
    const y=s.y+l.rowsTop+i*s.rowHeight;
    rect('row-'+i+'-line',...offset(l.line,y),c.line);
    rect('row-'+i+'-mark',...offset(l.mark,y),c.players[i]);
    text('row-'+i+'-name',s.names[i],...offset(l.name,y),s.nameFontSize,'Left');
    text('row-'+i+'-score',s.values[i],...offset(l.value,y),s.valueFontSize,'Right');
  }
  const labels=['3×3','4×4','默认','选中','成功','错误','结算'],ids=['mode3','mode4','default','selected','success','error','settlement'];
  labels.forEach((value,i)=>button('control-'+ids[i],value,spec.controls.x+i*(spec.controls.width+spec.controls.gap),spec.controls.y,spec.controls.width,spec.controls.height));
  return nodes;
}
