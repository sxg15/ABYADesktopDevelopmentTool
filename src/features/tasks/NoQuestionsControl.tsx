import {useState} from "react";
import {productionApi,type ProductionRecord,type ProductionView} from "../../shared/production";
import {errorMessage} from "../../shared/api";
export function NoQuestionsControl({taskId,record,active,onChanged}:{taskId:string;record:ProductionRecord;active:boolean;onChanged:(v:ProductionView)=>void}){
  const [busy,setBusy]=useState(false),[error,setError]=useState("");
  const mode=record.noQuestions;
  async function change(action:"start"|"off"|"stop"){
    if(busy)return;
    setBusy(true);setError("");
    try{onChanged(await productionApi.noQuestions(taskId,action));}
    catch(e){setError(errorMessage(e));}finally{setBusy(false);}
  }
  return <section className="no-questions-control" aria-label="不提问模式">
    <div><strong>{mode?.enabled?"不提问模式进行中":"不提问模式"}</strong>
      <p>{mode?.enabled?"AI 自行审阅与推进，产出新试玩版后停止。":"使用当前客户端，自行审阅需求、美术和方案，继续制作到新试玩版。"}</p>
      {mode?.message&&<small role="status">{mode.message}</small>}</div>
    <div className="production-actions">
      {mode?.enabled?<><button className="secondary-button" disabled={!active||busy} onClick={()=>void change("off")}>关闭模式</button>
        <button className="secondary-button" disabled={!active||busy} onClick={()=>void change("stop")}>停止当前执行</button></>:
        <button className="primary-button" disabled={!active||busy} onClick={()=>void change("start")}>{busy?"正在开启…":"开始不提问模式"}</button>}
    </div>{error&&<p className="inline-error" role="alert">{error}</p>}
  </section>;
}
