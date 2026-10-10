import { useEffect, useRef, useState } from "react";
import { productionApi, type TaskControlState } from "../../shared/production";
export function useTaskControl(taskId:string,conversationId:string|undefined,provider:string) {
  const [state,setState]=useState<TaskControlState>();
  const sequence=useRef(0);
  useEffect(()=>{
    setState(undefined); let alive=true;
    async function refresh(){
      if(!conversationId||provider!=="codex")return;
      const request=++sequence.current;
      try{const next=await productionApi.control(taskId,conversationId);
        if(alive&&request===sequence.current)setState(next);
      }catch{/* Connection/operation errors are surfaced by their originating action. */}
    }
    void refresh();const timer=window.setInterval(()=>void refresh(),1500);
    window.addEventListener("abya:control-changed",refresh);
    return()=>{alive=false;sequence.current++;window.clearInterval(timer);window.removeEventListener("abya:control-changed",refresh);};
  },[taskId,conversationId,provider]);
  return state?.conversationId===conversationId?state:undefined;
}
export function controlText(state:TaskControlState|undefined) {
  if(!state)return "正在核对任务状态";
  const text:Record<string,string>={queued:"操作已保存，当前回复结束后继续",running:"AI 正在处理当前任务",waitingForResponse:"等待模型响应，可随时暂停",
    pausing:"正在暂停任务",paused:"任务已暂停，答案和历史已保存",waitingForAnswers:"等待你回答阶段问题",blocked:"当前阶段遇到问题，可查看处理方案或重新检查",
    waitingForApproval:"等待你确认文档版本",needsReview:"执行结果需要核对，未自动重发",disconnected:"连接已断开，需要核对后台状态",
    failed:"本轮遇到错误，可核对记录后继续",ready:"本轮回复已结束，可继续当前阶段",notStarted:"尚未开始执行，请先完成终端连接并输入需求",closed:"任务已完成或归档；恢复任务后才能继续执行"};
  const elapsed=state.lastEventAt?Math.max(0,Math.floor((Date.now()-Date.parse(state.lastEventAt))/1000)):0;
  return (text[state.state]??"正在核对任务状态")+(["running","waitingForResponse","queued"].includes(state.state)&&elapsed>=30?` · 上次进展 ${elapsed} 秒前`:"");
}
export function controlLabel(state:TaskControlState|undefined) {
  const labels:Record<string,string>={running:"运行中",waitingForResponse:"等待响应",queued:"已排队",paused:"已暂停",pausing:"暂停中",
    blocked:"遇到问题",waitingForAnswers:"等待回答",waitingForApproval:"等待确认",needsReview:"需要核对",failed:"执行失败",disconnected:"连接断开",ready:"可继续",notStarted:"待开始",closed:"任务已结束"};
  return state ? labels[state.state]??"核对中" : "核对中";
}
