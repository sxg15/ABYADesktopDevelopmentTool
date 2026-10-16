import type { ArtCase, ArtDesign } from "./artDesign";
import type { ProductionView } from "./production";

export const artCaseFixture = ():ArtCase=>({
  id:"case-fixture",version:1,status:"accepted",title:"用物品轮廓表达剩余次数",tags:["固定镜头","鼠标","少量库存"],
  context:"桌面休闲游戏；玩家在瞄准时持续观察剩余可发射物品。",plannerRequirements:"不使用数字库存标签，玩家身份颜色保持稳定。",
  visualRules:"实体轮廓优先，身份色只表达玩家归属。",decisions:"库存靠近操作区，避免视线在角落与目标间往返。",
  production:"独立物品图层、状态绑定和操作热区；在最终显示尺寸复验。",applicability:"剩余数量少且需要逐件辨认的玩法。",
  limitations:"大规模库存和远距离电视操作需要重新设计；此条目为隔离测试资料。",sources:"本地程序绘制的测试示意。",
  satisfaction:"认可物品数量的表达方式，不代表认可所有画面。",designApproved:true,runtimeVerified:false,provenance:"local-planner",
  media:[{path:"media/0.png",sha256:"fixture",mediaType:"image/png",title:"隔离测试示意",sourceType:"mockup",source:"fixture"}],
});
export const artDesignFixture=():ArtDesign=>({
  sources:[{id:"task-description",kind:"task",text:"不要用数字显示库存，直接显示剩余物品。保留玩家身份颜色。",hash:"source-1"}],
  requirements:[{id:"ART-01",sourceId:"task-description",sourceHash:"source-1",sourceText:"不要用数字显示库存，直接显示剩余物品。保留玩家身份颜色。",
    quote:"不要用数字显示库存，直接显示剩余物品。",kind:"must",interpretation:"用待发射物品的轮廓呈现剩余次数，逐件消耗。",scope:"游戏进行中 HUD / 剩余物品区",acceptance:"从五枚逐次消耗至最后一枚，画面数量与状态一致，无数字标签。"},
    {id:"ART-02",sourceId:"task-description",sourceHash:"source-1",sourceText:"保留玩家身份颜色。",quote:"保留玩家身份颜色。",kind:"must",
      interpretation:"玩家颜色不随排名或回合变化。",scope:"玩家标记和结算页",acceptance:"交换排名后核对身份色。"}],
  sourceReviews:[{id:"task-description",sourceHash:"source-1",requirementIds:["ART-01","ART-02"],reason:"已保留库存表达和玩家身份两条独立要求。"}],
  referenceAnalyses:[],direction:{playerExperience:"轻松瞄准，快速读懂还剩多少机会。",tasksAndInformation:"瞄准、发射、观察剩余次数。",platformAndCamera:"桌面鼠标，固定镜头。",
    visualRules:"清楚的物品轮廓与稳定身份色。",representativeScene:"游戏进行中及最后一次操作。",assetProduction:"复用物品图像并独立绑定状态。",feasibility:"仅为隔离界面测试，未运行游戏。",acceptance:"逐件消耗与跨状态比对。",librarySearch:"借鉴库存表达，重新适配布局。",inputHash:"input-1"},
  coverage:[{id:"ART-01",artifactIds:["layout"],bindings:{"artifacts/layout.png":"layout-hash"},designLocation:"游戏主画面下部待发射物品区；无数字标签。",status:"addressed",inputHash:"input-1"}],
  realizations:[],caseDrafts:[{...artCaseFixture(),id:"draft-1",version:undefined,status:undefined,designHash:"design-1",artifactIds:["layout"],media:undefined}],
  caseUses:[],plannerNotes:[],conversationInputs:[],
});
export const artViewFixture=():ProductionView=>({
  policy:{workflowVersion:"2.2.0",stages:[{id:"resources",name:"美术需求",approval:"resources"}],dimensions:[],questions:[]},
  artDesignStatus:{enabled:true,inputHash:"input-1",designHash:"design-1"},warnings:[],reportPaths:[],availableUpdate:false,
  actions:{review:{resources:"等待 AI 补齐逐条设计"},approve:{resources:"ART-02 尚未关联设计"},feedback:{},recheck:{}},
  record:{taskId:"art-design-test",revision:4,workflowVersion:"2.2.0",questionMode:"ask",currentStage:"resources",currentRound:0,cycle:1,
    stages:{resources:"in-progress"},documents:{},artDesign:artDesignFixture(),artifacts:[{id:"layout",path:"artifacts/layout.png",title:"游戏进行中 · 设计示意",
      stage:"resources",status:"draft",cycle:1,sourceType:"mockup",groupId:"layout",visualVersion:"1",mediaType:"image/png",sha256:"layout-hash"}],
    approvals:[],versionDetails:{},issues:[],evidence:[],rounds:[],milestones:{},knowledge:[],skillPins:[],updatedAt:"fixture"},
});
