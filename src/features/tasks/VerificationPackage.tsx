import {useEffect, useRef, useState} from "react";
import {productionApi,type ProductionRecord,type ProductionView} from "../../shared/production";
import {errorMessage} from "../../shared/api";
import {Modal} from "../../app/Modal";
import {playableIssues,playableState} from "./playableState";
import "./playablePackage.css";
import {PlaytestSeats} from "./PlaytestSeats";
import {PreviewPlayerRecovery} from "./PreviewPlayerRecovery";

export function VerificationPackage({record:r,active,en,embedded=false,onRepair,onUpdated}:{record:ProductionRecord;active:boolean;en:boolean;embedded?:boolean;onRepair?:()=>Promise<unknown>;onUpdated?:(view:ProductionView)=>void}) {
  const [error,setError]=useState(""),[busy,setBusy]=useState(false),[confirm,setConfirm]=useState(false),[notice,setNotice]=useState("");
  const locked=useRef(false), pack=r.workbench?.execution?.verificationPackage, state=playableState(r), issues=playableIssues(r,en);
  const [seats, setSeats] = useState(2);
  useEffect(() => setSeats(2), [r.taskId, r.playerMode]);
  const selectedSeats = r.playerMode === "multiplayer" ? seats : undefined;
  const [recoverPlayer,setRecoverPlayer]=useState(false);
  const seatSelector = r.playerMode === "multiplayer"
    ? <PlaytestSeats value={seats} onChange={setSeats} disabled={!active || busy} en={en}/>
    : null;
  const text=(zh:string,english:string)=>en?english:zh;
  async function preview(version:string){
    try {
      const result=await productionApi.startPreview(r.taskId,version,selectedSeats);
      setRecoverPlayer(false);
      setNotice(result.message||text("试玩窗口已打开。","Playtest window opened."));
    } catch(reason) {
      const code=(reason as {code?:string})?.code;
      setRecoverPlayer(code==="player_pin_mismatch"||code==="app_player_mismatch"||code==="preview_preflight_required");
      throw reason;
    }
  }
  const recovery=recoverPlayer&&<PreviewPlayerRecovery taskId={r.taskId} active={active&&!busy} en={en}
    reuse={r.currentStage==="review"||!!r.currentVersion} onVerified={()=>{
      setRecoverPlayer(false);setError("");setNotice(text("当前 PB 预检通过，可以重新启动试玩。","Preflight passed. Start the playtest again."));
    }}/>;
  const packageClass=`playable-package${embedded?" playable-package-embedded":""}`, Heading=embedded?"h4":"h2", Issues=embedded?"details":"div";
  async function run(action:()=>Promise<unknown>){if(locked.current)return;locked.current=true;setBusy(true);setError("");setNotice("");try{await action();}catch(e){setError(errorMessage(e));}finally{locked.current=false;setBusy(false);}}
  async function enter(){const next=await productionApi.workbench(r.taskId,r.revision,"enter-playtest",{version:pack!.version});onUpdated?.(next);setConfirm(false);}
  if (r.currentStage === "review") {
    const version = r.currentVersion;
    return <section className={packageClass} aria-label={text("当前修改版复验", "Review current changes")}>
      <div className="playable-package-heading"><div>
        <Heading>{text("当前修改版", "Current candidate")} <small>{version ?? text("尚未保存", "Not saved")}</small></Heading>
        <p>{text("启动当前已保存版本的独立副本，核对本轮修改。", "Open an isolated copy of the current saved version to review these changes.")}</p>
      </div><span className="playable-status">{text("待试玩复验", "Awaiting playtest")}</span></div>
      {issues.length > 0 && <Issues className="playable-gaps">{embedded?<summary>{text("目前仍需核对的问题", "Remaining checks")} · {issues.length}</summary>:<h3>{text("目前仍需核对的问题", "Remaining checks")}</h3>}
        {issues.map(issue => <article key={issue.id}><strong><span>{issue.confirmed?text("已确认","Confirmed"):text("原因待确认","Cause unconfirmed")}</span>{issue.title}</strong><p style={{whiteSpace:"pre-wrap"}}>{issue.description}</p></article>)}
      </Issues>}
      <div className="playable-actions">{seatSelector}<button className="primary-button" disabled={!active || busy || !version}
        onClick={() => version && void run(()=>preview(version))}>{text("启动当前修改版试玩", "Play current changes")}</button></div>
      {recovery}
      {pack?.path && pack.version !== version && <p className="playable-muted">
        {text("历史试玩包：", "Previous playable package: ")}{pack.version}
        {text("。本轮复验使用上方的当前修改版。", ". Use the current candidate above for this review.")}
      </p>}
      {error && <p role="alert" className="inline-error playable-note">{error}</p>}
      {notice && <p role="status">{notice}</p>}
    </section>;
  }
  if(!pack?.path)return null;
  return <section className={packageClass} aria-label={text("试玩版本与剩余问题","Playable version and remaining issues")}>
    <div className="playable-package-heading"><div><Heading>{text("试玩版本","Playable version")} <small>{pack.version}</small></Heading>
      <p>{state.current?text("核心玩法已有验证记录。剩余问题会随版本保留。","Core gameplay has verification records. Remaining issues stay attached to this version."):text("这是之前的可试玩版本，尚不包含当前候选的最新修改。","This earlier playable version does not include the latest candidate changes.")}</p></div>
      <span className="playable-status">{state.available?text("可试玩 · 尚未验收","Playable · not accepted"):text("试玩包待核对","Package needs verification")}</span></div>
    {issues.length>0?<Issues className="playable-gaps">{embedded?<summary>{text("可能影响试玩的问题","Possible gameplay issues")} · {issues.length}</summary>:<h3>{text("可能影响试玩的问题","Possible gameplay issues")} <small>{issues.length} {text("项待处理","open")}</small></h3>}
      {issues.map(i=><article key={i.id}><strong><span>{i.confirmed?text("已确认","Confirmed"):text("原因待确认","Cause unconfirmed")}</span>{i.title}</strong><p style={{whiteSpace:"pre-wrap"}}>{i.description}</p></article>)}
      {issues.some(i=>!i.confirmed)&&<p className="playable-muted">{text("待确认不代表已经发生故障，AI 仍需补充验证。","Unconfirmed does not mean broken; AI still needs to verify the behavior.")}</p>}</Issues>
      :<p className="playable-empty">{text("暂未登记未解决问题；试玩不等于已通过全部检查。","No unresolved issues recorded. Playability does not imply all checks passed.")}</p>}
    <div className="playable-actions">{seatSelector}<button className="secondary-button" disabled={busy} onClick={()=>void run(()=>productionApi.revealDocument(r.taskId,pack.path))}>{text("定位试玩包","Show package")}</button>
      <button className="primary-button" disabled={!active||busy||!state.available} onClick={()=>void run(()=>preview(pack.version))}>{text("启动此版本试玩","Play this version")}</button>
      {onRepair&&<button className="secondary-button playable-repair" disabled={!active||busy} onClick={()=>void run(async()=>{await onRepair();setNotice(text("诊断请求已提交，请查看执行状态；若仍缺少条件，AI 会说明原因。","Diagnosis requested. Check execution status for progress or missing conditions."));})}>{text("诊断并修复上述缺陷","Diagnose and repair these issues")}</button>}</div>
    <p className="playable-note">{text("试玩使用独立副本，不自动切换阶段。试玩期间请停止 AI 自动操作；发现问题后可提交反馈。","Playtest uses an isolated copy and does not switch stages. Stop AI input during manual play and submit feedback afterward.")}</p>
    {r.currentStage==="implementation"&&<div className="playable-handoff"><div><strong>{text("想先边玩边改？","Ready to play and iterate?")}</strong><p>{text("可以带着待处理项进入下一阶段，后续继续跟进，不视为问题已解决。","Carry open items into the next stage without marking them resolved.")}</p></div><button className="secondary-button" disabled={!active||busy||!state.available||!state.current} onClick={()=>setConfirm(true)}>{text("进入试玩与修改阶段 →","Enter playtest & changes →")}</button></div>}
    {recovery}{error&&<p role="alert" className="inline-error playable-note">{error}</p>}{notice&&<p role="status" className="playable-note">{notice}</p>}
    {confirm&&<Modal title={text("进入试玩与修改阶段？","Enter playtest & changes?")} onClose={()=>setConfirm(false)}><p>{text("将以此版本作为本轮试玩版本，现有问题和待验证项会一并带入。","Use this version for playtesting and carry forward all unresolved issues and checks.")}</p><p>{pack.version}</p><p>{text("切换阶段不会把未通过检查改为通过，也不代表你已验收游戏。最终交付仍需你确认。","This does not pass failed checks or accept the game. Final delivery still requires your approval.")}</p>{error&&<p role="alert">{error}</p>}<div className="playable-actions"><button className="secondary-button" disabled={busy} onClick={()=>setConfirm(false)}>{text("留在 Lua 实现","Stay in implementation")}</button><button className="primary-button" disabled={busy} onClick={()=>void run(enter)}>{text("带着待处理项进入","Enter with open items")}</button></div></Modal>}
  </section>;
}
