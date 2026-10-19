import { invoke } from "@tauri-apps/api/core";
export interface MemorySample { at:string; privateBytes:number; workingSetBytes:number; availableBytes:number; commitBytes:number; commitLimitBytes:number }
export interface MemoryStatus { instanceId:string; latest?:MemorySample; peakPrivateBytes:number; pressured:boolean; autoStop:boolean; incidentId?:string; reportPath?:string; error:string }
export interface MemoryPolicy { privateLimitMib:number; growthLimitMib:number; availableFloorMib:number; commitLimitPercent:number }
export const memoryApi = {
  status:(id:string)=>invoke<MemoryStatus>("instance_memory_status",{id}),
  policy:()=>invoke<MemoryPolicy>("instance_memory_policy"),
  save:(policy:MemoryPolicy)=>invoke<void>("set_instance_memory_policy",{policy}),
  arm:(id:string,enabled:boolean)=>invoke<void>("arm_instance_memory_stop",{id,enabled}),
  reports:(id?:string)=>invoke<void>("open_memory_reports",{id}),
};
