import type {GameInstance} from "../../shared/types";

export function playVersionInfo(instance:Pick<GameInstance,"id"|"name"|"mode">, sessions:Map<string,string>, dates:Record<string,string>, latest?:string) {
  const linked=sessions.get(instance.id);
  if(linked)return linked;
  // Older launch records used the generated window name before session IDs were retained.
  const match=/^(?:验收|预览) (.+) · (Host \/ 单人|Client [1-9]\d*)$/.exec(instance.name);
  if(!match || (match[1]!==latest&&!Object.prototype.hasOwnProperty.call(dates,match[1])))return;
  if(match[2].startsWith("Client")?instance.mode!=="lan-client":!["lan-host","offline"].includes(instance.mode??""))return;
  return match[1];
}

export function PlayVersionLabel({version,latest,at,en}:{version:string;latest:boolean;at?:string;en:boolean}) {
  const date=at ? new Date(at) : undefined;
  const valid=date && Number.isFinite(date.getTime());
  return <span className="play-version-label">
    {latest&&<span className="play-version-latest">{en?"Latest version":"最新版本"}</span>}
    <span>{version}</span>
    <span>{en?"Published ":"发布时间 "}{valid?<time dateTime={at}>{date.toLocaleString(en?"en-US":"zh-CN",{year:"numeric",month:"2-digit",day:"2-digit",hour:"2-digit",minute:"2-digit",hour12:false})}</time>:(en?"unknown":"未记录")}</span>
  </span>;
}
