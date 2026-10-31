use super::*;

fn verified_report(value: &Value) -> AppResult<()> {
    let report = &value["report"];
    if report["state"] != "passed"
        || report["assertions"]
            .as_array()
            .is_none_or(|a| a.is_empty() || a.iter().any(|x| x["passed"] != true))
    {
        return Err(AppError::validation(
            "基线须包含真实完成且有通过断言的运行报告。",
        ));
    }
    Ok(())
}

impl RuntimeBridgeService {
    pub fn qualify_player(
        &self,
        task: &str,
        instance: &str,
        test_ids: &[String],
        session: &str,
    ) -> AppResult<Value> {
        if test_ids.is_empty() || test_ids.len() > 16 {
            return Err(AppError::validation("提供 1–16 个实际运行测试 ID。"));
        }
        let unique: std::collections::BTreeSet<_> = test_ids.iter().collect();
        if unique.len() != test_ids.len()
            || test_ids.iter().any(|id| {
                id.is_empty()
                    || id.len() > 128
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            })
        {
            return Err(AppError::validation(
                "运行测试 ID 必须唯一且符合实际标识格式。",
            ));
        }
        let owned = self.instances.read(instance)?;
        if owned.task_id.as_deref() != Some(task) {
            return Err(AppError::validation("实例不属于当前任务。"));
        }
        let player = PathBuf::from(
            owned
                .executable_path
                .ok_or_else(|| AppError::validation("缺少实际运行包。"))?,
        );
        let before = self.tasks.baseline_build_hash(task, &player)?;
        let capabilities = self.list_tools(instance)?;
        if !capabilities.is_array() {
            return Err(AppError::validation("能力目录不可用。"));
        }
        let mut reports = Vec::new();
        for id in test_ids {
            let result = self.call_tool_scoped(
                instance,
                "runtime_test_get_report",
                json!({"testId":id}),
                session,
                &uuid::Uuid::new_v4().to_string(),
            )?;
            if result.is_error {
                return Err(AppError::validation("运行测试报告读取失败。"));
            }
            let report = result
                .content
                .iter()
                .filter_map(|c| c["text"].as_str())
                .find_map(|text| serde_json::from_str::<Value>(text).ok())
                .ok_or_else(|| AppError::validation("运行报告不是结构化结果。"))?;
            if report["testId"] != *id {
                return Err(AppError::validation("运行报告身份不匹配。"));
            }
            verified_report(&report)?;
            reports.push(report);
        }
        if self.tasks.baseline_build_hash(task, &player)? != before {
            return Err(AppError::validation("核验期间构建改变。"));
        }
        self.tasks.save_runtime_baseline(task, &before, json!({
            "schema":"abya.player-baseline/v1","playerBuildHash":before,"sourceTaskId":task,
            "instanceId":instance,"observedCapabilities":capabilities,"reports":reports,
            "recordedAt":chrono::Utc::now().to_rfc3339(),"scope":"declared-native-checks-only",
            "note":"能力目录表示已发现，不表示所有能力都已验收。仅复用报告明确验证的范围，不替代当前游戏验收。"
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn baseline_rejects_running_empty_or_failed_reports() {
        for report in [
            json!({"state":"running","assertions":[{"passed":true}]}),
            json!({"state":"passed","assertions":[]}),
            json!({"state":"passed","assertions":[{"passed":false}]}),
        ] {
            assert!(verified_report(&json!({"report":report})).is_err());
        }
        assert!(
            verified_report(&json!({"report":{"state":"passed","assertions":[{"passed":true}]}}))
                .is_ok()
        );
    }
}
