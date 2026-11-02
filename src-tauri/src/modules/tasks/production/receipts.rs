use super::*;
use sha2::{Digest, Sha256};
const ALLOWED: &[&str] = &[
    "register-acceptance-check",
    "revise-work-unit",
    "classify-work-unit",
    "reconcile-work-unit",
    "record-work-signal",
    "record-validation-run",
    "save-self-test",
    "save-art-realization",
    "register-evidence",
    "reuse-evidence",
    "register-artifact",
    "register-visual",
    "save-issue",
    "record-issue-handling",
    "save-work-unit",
    "report-work-unit",
    "update-stage",
    "update-workbench-request",
];
impl TaskService {
    pub fn production_receipt(&self, task: &str, action: &str) -> AppResult<Value> {
        self.get_readonly(task)?;
        self.database.with_connection(|c| {
            Ok(load(c, task)?
                .and_then(|r| r.workbench.execution.receipts.get(action).cloned())
                .unwrap_or(Value::Null))
        })
    }
    pub fn production_batch(
        &self,
        task: &str,
        expected: u64,
        batch: &str,
        items: &[Value],
    ) -> AppResult<Value> {
        uuid::Uuid::parse_str(batch).map_err(|_| AppError::validation("batchId 必须是 UUID。"))?;
        if items.is_empty() || items.len() > 64 {
            return Err(AppError::validation("批次应包含 1–64 项。"));
        }
        let mut revision = expected;
        let mut results = vec![];
        let mut stopped = false;
        for (index, item) in items.iter().enumerate() {
            let id = format!("{batch}:{index}");
            if stopped {
                results.push(json!({"actionId":id,"status":"notExecuted"}));
                continue;
            }
            match self.production_action(task, revision, &id, item) {
                Ok(receipt) => {
                    revision = receipt["resultRevision"].as_u64().unwrap_or(revision);
                    results.push(receipt);
                }
                Err(e) => {
                    results.push(json!({"actionId":id,"status":"rejected","code":e.code,"message":e.message}));
                    stopped = true;
                }
            }
        }
        // A failed export does not erase or repeat committed items; receipts remain queryable.
        let package_warning = if !stopped {
            self.production_auto_package(task)
        } else {
            None
        };
        if let Some(record) = self.production_get(task)?.record {
            revision = record.revision;
        }
        let export_error = self.production_export(task).err().map(|e| e.message);
        Ok(
            json!({"batchId":batch,"revision":revision,"complete":!stopped,"results":results,"exportWarning":export_error,"packageWarning":package_warning}),
        )
    }
    fn production_action(
        &self,
        task: &str,
        expected: u64,
        id: &str,
        input: &Value,
    ) -> AppResult<Value> {
        let op = mutation::text(input, "operation")?;
        if !ALLOWED.contains(&op) || !input["data"].is_object() {
            return Err(AppError::validation(
                "此操作不支持批次登记。批准、反馈关闭和外部游戏写入不走此接口。",
            ));
        }
        let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(input)?));
        let existing = self.production_receipt(task, id)?;
        if !existing.is_null() {
            return self.receipt_result(task, existing, &hash);
        }
        let root = self.production_workspace(task)?;
        self.production_validate_mutation_files(task, op, &input["data"])?;
        let result=self.production_commit(task,expected,|r| {
            if r.workbench.execution.receipts.len()>=4000 {return Err(AppError::validation("操作收据容量已满。"));}
            mutation::apply(r,&root,op,&input["data"])?;
            r.workbench.execution.receipts.insert(id.into(),json!({"actionId":id,"inputHash":hash,"operation":op,
                "status":"applied","resultRevision":expected+1,"affectedId":input["data"]["id"],"at":now()}));
            Ok(())
        });
        if let Err(error) = result {
            let existing = self.production_receipt(task, id)?;
            if !existing.is_null() {
                return self.receipt_result(task, existing, &hash);
            }
            return Err(error);
        }
        self.receipt_result(task, self.production_receipt(task, id)?, &hash)
    }
    fn receipt_result(&self, task: &str, mut receipt: Value, hash: &str) -> AppResult<Value> {
        if receipt["inputHash"] != hash {
            return Err(AppError::new(
                "conflict",
                "相同 actionId 的输入不同，未覆盖原结果。",
                "",
            ));
        }
        let r = self
            .database
            .with_connection(|c| load(c, task))?
            .ok_or_else(conflict)?;
        receipt["currentRevision"] = json!(r.revision);
        Ok(receipt)
    }
}
