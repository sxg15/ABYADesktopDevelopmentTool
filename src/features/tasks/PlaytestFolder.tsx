import {useState} from "react";
import {FolderOpen} from "lucide-react";
import {productionApi} from "../../shared/production";
import {errorMessage} from "../../shared/api";
export function PlaytestFolder({taskId,version,preview=false,en=false}:{taskId:string;version?:string;preview?:boolean;en?:boolean}){
  const [busy,setBusy]=useState(false),[error,setError]=useState("");
  async function open(){
    if(!version||busy)return;
    setBusy(true);setError("");
    try{await productionApi.openPlayableArchive(taskId,version,preview);}
    catch(e){setError(errorMessage(e));}finally{setBusy(false);}
  }
  return <div className="playtest-folder">
    <button className="secondary-button" disabled={!version||busy} onClick={()=>void open()}><FolderOpen size={15}/>{en?"Open this save in Explorer":"打开对应存档位置"}</button>
    {error&&<p role="alert" className="inline-error">{error}</p>}
  </div>;
}
