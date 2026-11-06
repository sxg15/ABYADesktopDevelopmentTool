import catalog from "../../../.codex/skills/abya-game-development-task/assets/art-library/catalog.json";

/** 配方结构示意，交互详情使用同版本实际演示。 */
export function ArtRecipeThumbnail({presetId}:{presetId:string}) {
 const color=catalog.entries.find(p=>p.id===presetId)?.color??"#e6b86a";
 return <svg viewBox="0 0 200 110" aria-hidden="true" style={{width:"100%",height:"100%",background:"#202a35"}}>
  {presetId==="reward"?<>
   <path d="M32 80 Q100 8 164 56" fill="none" stroke={color} strokeWidth="2" strokeDasharray="4 5"/>
   <circle cx="95" cy="43" r="10" fill={color}/><rect x="150" y="54" width="24" height="25" rx="4" fill="none" stroke={color}/>
  </>:<>
   {presetId==="aura"&&<><circle cx="100" cy="55" r="38" fill="none" stroke={color} strokeWidth="7" opacity=".2"/><circle cx="100" cy="55" r="30" fill="none" stroke={color}/></>}
   {presetId==="hit"&&<path d="M100 8 L107 31 L126 15 L120 39 L148 38 L127 53 L148 72 L120 69 L123 94 L104 77 L90 98 L89 73 L62 82 L76 59 L52 49 L78 40 L70 18 L91 33 Z" fill={color} opacity=".55"/>}
   <rect x={presetId==="panel"?56:65} y={presetId==="press"?39:33} width={presetId==="panel"?88:70} height={presetId==="press"?36:44}
    rx="8" fill="#283747" stroke={color} strokeWidth={presetId==="selection"?4:2}/>
   <path d="M82 54 H118" stroke={color} strokeWidth="4" strokeLinecap="round"/>
   {presetId==="panel"&&<path d="M100 92 V81 M95 86 L100 81 L105 86" fill="none" stroke={color} strokeWidth="2"/>}
  </>}
 </svg>;
}
