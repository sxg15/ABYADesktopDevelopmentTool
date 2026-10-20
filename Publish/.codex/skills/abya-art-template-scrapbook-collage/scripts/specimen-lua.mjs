const literal=value=>value===null?'nil':Array.isArray(value)?`{${value.map(literal).join(',')}}`:
  typeof value==='object'?`{${Object.entries(value).map(([k,v])=>`[${JSON.stringify(k)}]=${literal(v)}`).join(',')}}`:
  typeof value==='string'?JSON.stringify(value):String(value);
export function stateScript(spec,tokens,nodeMap,adapter,command) {
  if(!['mode3','mode4',...spec.states].includes(command))throw Error('Unknown control');
  const header={schemaVersion:1,apiVersion:'3.0',name:'scrapbook-'+command,description:'Isolated native UI state specimen; no gameplay authority or multiplayer acceptance',context:'runtime',side:'client',entry:'main',requires:[],parameters:[{name:'arg1',type:'UIButton',required:true,description:'Native clicked button in this specimen root.'}],returns:[]};
  return `--[[ABYA-LUA
${JSON.stringify(header)}
ABYA-LUA]]
local ids=${literal(nodeMap)}
local grids=${literal(spec.grids)}
local colors=${literal(tokens.colors)}
local feedback=${literal(Object.fromEntries(['successDurationMs','errorDurationMs','starCountPerCell','startScale','endScale','risePixels'].map(k=>[k,tokens.feedback[k]])))}
local sx=${adapter.scale.x}
local sy=${adapter.scale.y}
function main(input)
  local root=input.arg1.runtime_root_id
  local function comp(name) return abya.ui.component(ids[name],root) end
  local function txt(name) return abya.ui.text(ids[name],root) end
  local function img(name) return abya.ui.image(ids[name],root) end
  local function geometry(name,x,y,w,h)
    local n=comp(name)
    n:set_dimension_rule("offset_x",{follow="None",pixels=x*sx,percent=0})
    n:set_dimension_rule("offset_y",{follow="None",pixels=-y*sy,percent=0})
    n:set_dimension_rule("size_x",{follow="None",pixels=w*sx,percent=0})
    n:set_dimension_rule("size_y",{follow="None",pixels=h*sy,percent=0})
  end
  local token=txt("state-token")
  local revision=(tonumber(token.content) or 0)+1
  token.content=tostring(revision)
  local mode=comp("cell-9").active and "4" or "3"
  local command=${literal(command)}
  local state=command
  if command=="mode3" then mode="3";state="default" end
  if command=="mode4" then mode="4";state="default" end
  local g=grids[mode]
  for _,i in ipairs(${literal(spec.selectedCells)}) do
    for j=0,feedback.starCountPerCell-1 do
      local n=comp("star-"..i.."-"..j)
      n:stop_visual_animations();n:tween_alpha(1,0)
      n.visual_scale=abya.vector2(1,1);n.visual_offset=abya.vector2(0,0);n.active=false
    end
  end
  for i=0,15 do
    local active=i<#g.values
    local textName=(mode=="4" and i<9) and "cell4-"..i or "cell-"..i
    comp("cell-"..i).active=active and (mode=="3" or i>=9)
    if i<9 then comp("cell4-"..i).active=mode=="4" end
    for _,name in ipairs({"cell-"..i.."-border","cell-"..i.."-fill",textName}) do
      local n=comp(name);n.active=active;n:stop_visual_animations();n.visual_offset=abya.vector2(0,0)
    end
    if active then
      local x=g.x+(i%g.columns)*(g.cell+g.gap)
      local y=g.y+math.floor(i/g.columns)*(g.cell+g.gap)
      geometry("cell-"..i.."-border",x,y,g.cell,g.cell)
      geometry("cell-"..i.."-fill",x+${spec.cellBorderPixels},y+${spec.cellBorderPixels},g.cell-${2*spec.cellBorderPixels},g.cell-${2*spec.cellBorderPixels})
      geometry(textName,x,y,g.cell,g.cell)
      local chosen=false
      for _,selected in ipairs(${literal(spec.selectedCells)}) do if selected==i then chosen=state~="default" end end
      local border=chosen and (state=="error" and colors.error or state=="success" and colors.success or colors.ink) or colors.line
      img("cell-"..i.."-border").color=border
      img("cell-"..i.."-fill").color=chosen and colors.selected or colors.cell
      txt(textName).content=tostring(g.values[i+1])
    end
  end
  txt("target-label").content=state=="settlement" and "本轮完成" or "目标"
  txt("target").content=state=="settlement" and "完成" or g.target
  txt("timer").content=state=="settlement" and "0" or state=="error" and "10" or "30"
  if state=="error" then
    for _,phase in ipairs({{${tokens.feedback.errorShakePixels},0.25},{-${tokens.feedback.errorShakePixels},0.5},{0,0.25}}) do
      if token.content~=tostring(revision) then return end
      local duration=feedback.errorDurationMs/1000*phase[2]
      for _,i in ipairs(${literal(spec.selectedCells)}) do
        local textName=(mode=="4" and i<9) and "cell4-"..i or "cell-"..i
        for _,name in ipairs({"cell-"..i.."-border","cell-"..i.."-fill",textName}) do
        comp(name):tween_visual_offset(abya.vector2(phase[1]*sx,0),duration,{wait=false,ease="linear"})
      end end
      abya.time.wait(duration)
    end
  end
  if state=="success" then
    for _,i in ipairs(${literal(spec.selectedCells)}) do
      local x=g.x+(i%g.columns)*(g.cell+g.gap)
      local y=g.y+math.floor(i/g.columns)*(g.cell+g.gap)
      for j=0,feedback.starCountPerCell-1 do
        local name="star-"..i.."-"..j;geometry(name,x+${tokens.feedback.starStartX}+j*${tokens.feedback.starStepX},y${tokens.feedback.starStartY>=0?'+':''}${tokens.feedback.starStartY}+j*${tokens.feedback.starStepY},${tokens.feedback.starSize},${tokens.feedback.starSize})
        local n=comp(name);n.active=true;n.visual_scale=abya.vector2(feedback.startScale,feedback.startScale)
        n:tween_visual_scale(abya.vector2(feedback.endScale,feedback.endScale),feedback.successDurationMs/1000,{wait=false,ease="out_cubic"})
        n:tween_visual_offset(abya.vector2(0,feedback.risePixels*sy),feedback.successDurationMs/1000,{wait=false,ease="out_cubic"})
        n:tween_alpha(0,feedback.successDurationMs/1000)
      end
    end
    abya.time.wait(feedback.successDurationMs/1000)
    if token.content==tostring(revision) then
      for _,i in ipairs(${literal(spec.selectedCells)}) do for j=0,feedback.starCountPerCell-1 do comp("star-"..i.."-"..j).active=false end end
    end
  end
end
`;
}
