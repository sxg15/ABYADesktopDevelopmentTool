// 公共动作只映射实际已实现能力；调用时仍须通过当前 Player 的 schema 核验。
export const actions={
 'state.read':{toolName:'runtime_get_game_state',readOnly:true},
 'players.read':{toolName:'runtime_get_players',readOnly:true},
 'entities.read':{toolName:'runtime_list_entities',readOnly:true},
 'entity.read':{toolName:'runtime_get_entity',readOnly:true},
 'key.down':{toolName:'runtime_input_key_press'},
 'key.up':{toolName:'runtime_input_key_release'},
 'action.down':{toolName:'runtime_input_press'},
 'action.up':{toolName:'runtime_input_release'},
 'axis.set':{toolName:'runtime_input_set_axis'},
 'pointer.move':{toolName:'runtime_input_mouse_set_position'},
 'pointer.down':{toolName:'runtime_input_mouse_press'},
 'pointer.up':{toolName:'runtime_input_mouse_release'},
 'pointer.click':{toolName:'runtime_input_mouse_click'},
 'input.reset':{toolName:'runtime_input_reset'},
 'ui.tree':{toolName:'custom_ui_runtime_get_tree',readOnly:true},
 'ui.node':{toolName:'custom_ui_runtime_get_node',readOnly:true},
 'ui.event-click':{toolName:'ui_click_button',inputLayer:'event-only'},
 'ui.text':{toolName:'ui_set_input_text'},
 'ui.toggle':{toolName:'ui_set_toggle'},
 'wait.frames':{toolName:'runtime_wait_frames'},
 'wait.condition':{toolName:'runtime_wait_until'},
 'archive.save':{toolName:'editor_save_archive'},
 'level.restart':{toolName:'runtime_restart_level'},
 'test.begin':{toolName:'runtime_test_begin'},
 'test.assert':{toolName:'runtime_test_assert'},
 'test.capture':{toolName:'runtime_test_capture'},
 'test.end':{toolName:'runtime_test_end'},
 'test.report':{toolName:'runtime_test_get_report',readOnly:true},
 'screenshot':{toolName:'ui_capture_screenshot',readOnly:true},
 'logs.read':{toolName:'runtime_get_logs',readOnly:true},
};
export function expand(s){
 if(s.schema!=='abya.runtime-scenario/v3')return s;
 if(!s.roles||typeof s.roles!=='object'||Array.isArray(s.roles))throw Error('Scenario roles are required');
 const expanded=s.steps.flatMap(step=>{
  if(step.action==='key.tap')return [
   {...step,id:step.id+'-down',action:'key.down',assertions:[]},
   {id:step.id+'-hold',role:step.role,action:'wait.frames',arguments:{frames:step.holdFrames??2},assertions:[]},
   {...step,action:'key.up'}];
  if(step.action==='pointer.drag'){
   if(!Array.isArray(step.points)||step.points.length<2||step.points.length>128)throw Error('Drag needs 2–128 positions');
   return [
    {id:step.id+'-start',role:step.role,action:'pointer.move',arguments:{...step.arguments,...step.points[0]},assertions:[]},
    {id:step.id+'-down',role:step.role,action:'pointer.down',arguments:step.arguments,assertions:[]},
    ...step.points.slice(1).flatMap((point,i)=>[
     {id:`${step.id}-move-${i}`,role:step.role,action:'pointer.move',arguments:{...step.arguments,...point},assertions:[]},
     {id:`${step.id}-frame-${i}`,role:step.role,action:'wait.frames',arguments:{frames:1},assertions:[]}]),
    {...step,action:'pointer.up'}];
  }
  return [step];
 });
 const steps=expanded.map(step=>{
  const definition=step.action?actions[step.action]:null;
  if(step.action&&!definition)throw Error(`Missing reusable action: ${step.action}; add its implementation and tests before use`);
  const instanceId=s.roles[step.role];
  if(typeof instanceId!=='string'||!instanceId)throw Error(`Unknown role: ${step.role}`);
  if(step.instanceId||definition&&step.toolName)throw Error('Role/action mappings cannot be overridden');
  return {...definition,...step,toolName:definition?.toolName??step.toolName,instanceId};
 });
 return {...s,steps};
}
