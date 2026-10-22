import { useState, memo } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArtifactMedia } from "./ArtifactPreview";
import { productionApi, type ProductionArtifact } from "../../shared/production";
import { errorMessage } from "../../shared/api";
import "./readableText.css";

function external(url:string) {try{return ["https:","http:"].includes(new URL(url).protocol);}catch{return false;}}
function reference(url:string,basePath:string,artifacts:ProductionArtifact[]) {
  if(!url || /^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(url))return undefined;
  try {
    const origin="https://task.invalid/";
    const path=decodeURIComponent(new URL(url.replace(/\\/g,"/"),origin+basePath).pathname).slice(1);
    const authored=decodeURIComponent(url.replace(/\\/g,"/").split(/[?#]/)[0]).replace(/^\//,"");
    return artifacts.find(a=>[path,authored].includes(a.path.replace(/\\/g,"/")) && a.status!=="withdrawn");
  }catch{return undefined;}
}
export const ReadableText = memo(function ReadableText({text,taskId,basePath="",artifacts=[]}:{text:string;taskId?:string;basePath?:string;artifacts?:ProductionArtifact[]}) {
  const [error,setError]=useState("");
  async function open(url:string) {
    try {const artifact=reference(url,basePath,artifacts);
      if(taskId&&artifact)await productionApi.open(taskId,artifact.path);
      else if(external(url))await openUrl(url);
    }catch(e){setError(errorMessage(e));}
  }
  return <div className="readable-text markdown-body"><Markdown remarkPlugins={[remarkGfm]} components={{
    h1:({children})=><h2>{children}</h2>,h2:({children})=><h3>{children}</h3>,h3:({children})=><h4>{children}</h4>,
    h4:({children})=><h5>{children}</h5>,h5:({children})=><h6>{children}</h6>,h6:({children})=><h6>{children}</h6>,
    table:({children})=><div className="markdown-table-scroll" tabIndex={0} role="region" aria-label="Markdown table"><table>{children}</table></div>,
    a:({href,children})=>href&&(external(href)||taskId&&reference(href,basePath,artifacts))
      ? <a href={href} onClick={e=>{e.preventDefault();void open(href);}}>{children}</a> : <span>{children}</span>,
    img:({src,alt})=>{const url=typeof src==="string"?src:"",artifact=reference(url,basePath,artifacts);
      return taskId&&artifact?.mediaType?.startsWith("image/") ? <span className="markdown-image"><ArtifactMedia taskId={taskId} artifact={artifact}/></span>
        : external(url)?<a href={url} onClick={e=>{e.preventDefault();void open(url);}}>{alt||"图片"} ↗</a>:<span className="markdown-image-unavailable">{alt||"图片"}（未登记或不可用）</span>;},
  }}>{text}</Markdown>{error&&<p role="alert">{error}</p>}</div>;
},(before,after)=>before.text===after.text && before.taskId===after.taskId && before.basePath===after.basePath && JSON.stringify(before.artifacts??[])===JSON.stringify(after.artifacts??[]));
