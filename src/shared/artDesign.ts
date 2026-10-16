import { invoke } from "@tauri-apps/api/core";
export interface ArtSource {
  id:string;kind:string;text:string;hash:string;asset?:{id:string;path:string;sha256:string;title:string}|null;
}
export interface ArtRequirement {
  id:string;sourceId:string;sourceHash:string;sourceText:string;quote:string;kind:"must"|"preference"|"prohibited"|"clarify";
  interpretation:string;scope:string;acceptance:string;withdrawn?:boolean;withdrawReason?:string;
}
export interface ArtCoverage {
  id:string;artifactIds:string[];designLocation:string;status:string;inputHash:string;bindings:Record<string,string>;
}
export interface ArtRealization {
  id:string;evidenceIds:string[];target:string;comparison:string;differences:string;status:string;designHash:string;version:string;cycle:number;
}
export interface ArtCase {
  id:string;version?:number;status?:string;title:string;context:string;plannerRequirements:string;visualRules:string;
  decisions:string;production:string;applicability:string;limitations:string;sources:string;tags:string[];
  satisfaction?:string;designApproved?:boolean;runtimeVerified?:boolean;provenance?:string;designHash?:string;
  artifactIds?:string[];referenceIds?:string[];parentCaseId?:string;parentVersion?:number;
  media?:{path:string;sha256:string;mediaType:string;title:string;sourceType:string;source:string}[];
}
export interface ArtDesign {
  sources:ArtSource[];requirements:ArtRequirement[];
  sourceReviews:{id:string;sourceHash:string;requirementIds:string[];reason:string}[];
  referenceAnalyses:{id:string;sha256:string;role:string;observations:string;borrow:string;avoid:string;application:string;conflicts:string}[];
  direction:Record<string,string>|null;coverage:ArtCoverage[];realizations:ArtRealization[];caseDrafts:ArtCase[];
  caseUses:{id:string;caseId:string;version:number;title:string;borrow:string;adapt:string;avoid:string;paths:string[]}[];
  plannerNotes:{id:string;text:string;at:string}[];
  conversationInputs?:{id:string;text:string;locator:string}[];
}
export const artLibraryApi = {
  list:(query:string,history=false)=>invoke<ArtCase[]>("list_art_cases",{query,history}),
  get:(id:string,version:number)=>invoke<ArtCase>("read_art_case",{id,version}),
  media:(id:string,version:number,index:number)=>invoke<string>("read_art_case_media",{id,version,index}),
  action:(taskId:string,expectedRevision:number,operation:string,data:unknown)=>
    invoke<ArtCase & {path?:string}>("mutate_art_library",{input:{taskId,expectedRevision,operation,data}}),
};
export const artDirectionFields = [
  ["playerExperience","玩家体验","Player experience"],["tasksAndInformation","玩法与信息","Tasks and information"],
  ["platformAndCamera","平台与镜头","Platform and camera"],["visualRules","视觉规则","Visual rules"],
  ["representativeScene","代表场景","Representative scene"],["assetProduction","资产制作规格","Asset production"],
  ["feasibility","能力与未验证项","Feasibility"],["acceptance","验收方法","Acceptance"],
  ["librarySearch","案例检索与取舍","Library search"],
] as const;
export const artCaseFields = [
  ["context","玩法与使用环境","Game and platform"],["plannerRequirements","策划要求","Planner requirements"],
  ["visualRules","视觉规则","Visual rules"],["decisions","设计决定与原因","Design decisions"],
  ["production","制作与还原方法","Production"],["applicability","适用条件","Applicability"],
  ["limitations","不适用情况与限制","Limitations"],["sources","素材来源与使用范围","Sources"],
] as const;
