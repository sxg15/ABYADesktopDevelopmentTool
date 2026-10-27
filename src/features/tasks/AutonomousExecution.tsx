import type {ProductionRecord,TaskControlState} from "../../shared/production";
const labels:Record<string,string>={repairRequired:"尚未形成可玩包，等待修复恢复",externalBlocked:"存在明确外部条件，查看下一步",ready:"准备执行",running:"AI 正在自主推进",waitingDependencies:"可执行工作已收敛，等待相关条件",settledWithGaps:"本轮执行已结束，存在未完成或未验证项",settled:"本轮工作已收敛",paused:"已暂停",pending:"等待依赖 / 待执行",succeeded:"工作项已完成",failed:"自动恢复中",deferred:"已暂缓","awaiting-condition":"结果待对账 / 等待条件"};
export function AutonomousExecution({record,en,control}:{record:ProductionRecord;en:boolean;control?:TaskControlState}) {
 const ledger=record.workbench?.execution;
 if(!ledger||!ledger.units.length)return null;
 const observed=control?.taskId===record.taskId?control.state:undefined;
 const stopped:Record<string,string>={interrupted:"子流程被中断，工作队列已保留",paused:"已暂停，工作队列已保留",pausing:"正在暂停",stopFailed:"停止尚未确认",disconnected:"原会话待恢复",waitingForTerminal:"原生终端待处理"};
 const status=observed&&stopped[observed] ? (en?observed:stopped[observed]) : ledger.state==="running"&&!["running","waitingForResponse","queued"].includes(observed??"") ? (en?"Checking execution state":"正在核对执行状态") : en?ledger.state:labels[ledger.state]??ledger.state;
 const units=ledger.units.filter(u=>u.cycle===record.cycle&&(!ledger.scopeHash||u.scopeHash===ledger.scopeHash)),active=units.find(u=>u.id===ledger.activeUnitId);
 return <section className="goal-execution" aria-label={en?"Autonomous implementation":"Lua 自主执行"}>
  <strong>{status}</strong>
  <p>{active?`${active.title} · ${en?"Route":"当前路线"} ${active.route+1} / ${active.routes.length}`:en?"Results and deferred work are preserved. Convergence does not imply acceptance.":"已有成果、替代方案和暂缓项均保留；收敛不代表验收通过。"}</p>
  <details><summary>{en?"Work queue & remaining items":"工作队列与待补项"} · {units.filter(u=>u.status==="succeeded").length}/{units.length}</summary>
   {units.map(u=><div key={u.id}><p><strong>{u.title}</strong> · {en?u.status:labels[u.status]??u.status}</p>{u.reason&&<p>{u.reason}</p>}{u.dependsOn.length>0&&<small>{en?"Dependencies":"依赖工作项"}：{u.dependsOn.join("、")}</small>}</div>)}
  </details>
 </section>;
}
