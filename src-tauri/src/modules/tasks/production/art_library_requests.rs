use super::*;
use uuid::Uuid;

impl TaskService {
    pub(super) fn request_art_case_work(&self, input: ProductionMutation) -> AppResult<Value> {
        let kind = art_design::text(&input.data, "kind", 20)?;
        if !["reference", "draft", "revision"].contains(&kind.as_str()) {
            return Err(AppError::validation("无效案例处理类型。"));
        }
        let entry = if kind != "draft" {
            let id = art_design::text(&input.data, "caseId", 100)?;
            let version = input.data["version"]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| AppError::validation("请选择具体案例版本。"))?;
            let entry = self.art_library_get(&id, version)?;
            if kind == "reference" && entry["status"] != "accepted" {
                return Err(AppError::validation("该案例版本已停止推荐，请重新选择。"));
            }
            entry
        } else {
            Value::Null
        };
        let prompt = match kind.as_str() {
            "reference" => format!("策划请求评估案例 {} 第 {} 版。通过 production get 阅读当前要求，通过 art get 读取该案例上下文，通过 inspect-art-case 获取并实际查看该版本媒体，然后判断适用性；若决定借鉴，用 use-art-case 记录 borrow/adapt/avoid。当前策划要求优先，不直接套用布局，旧案例验证不代表本任务验收。", entry["id"], entry["version"]),
            "revision" => format!("请为当前任务整理案例修订草稿，关联 parentCaseId={}、parentVersion={}。核对本次设计和证据，保留旧版，不自动入库，不代替策划声明满意。", entry["id"], entry["version"]),
            _ => "请读取当前美术方案与反馈，使用 save-art-case-draft 整理可复用成果、上下文、适用条件、制作方法和限制。草稿不是认可或入库，不得代替策划声明满意。".into(),
        };
        self.production_commit(&input.task_id, input.expected_revision, |record| {
            if !art_design_sources::enabled(record) {
                return Err(AppError::validation(
                    "此任务尚未启用美术设计流程，请显式升级后再请求。",
                ));
            }
            let request = json!({"kind":kind,"caseId":entry["id"],"version":entry["version"],
                "cycle":record.cycle,"designHash":art_design_sources::design_hash(record)});
            if record
                .art_design
                .case_requests
                .iter()
                .any(|old| old["selection"] == request)
            {
                return Ok(());
            }
            if record.art_design.case_requests.len() >= 128 {
                return Err(AppError::validation(
                    "此任务的案例请求记录已达到128条上限，请在后续任务中处理更多案例。",
                ));
            }
            record
                .art_design
                .case_requests
                .push(json!({"id":Uuid::new_v4().to_string(),
                "selection":request,"prompt":prompt,"at":now(),"origin":"app-user"}));
            Ok(())
        })?;
        self.production_export(&input.task_id)?;
        let view = self.production_get(&input.task_id)?;
        Ok(json!({"revision":view.record.map(|record|record.revision),"prompt":prompt}))
    }
}
