use super::*;

// Resolve against the latest formally submitted answer, including its revision
// even when two successive submissions contain identical wording.
fn answer<'a>(
    r: &'a ProductionRecord,
    source: &Value,
) -> Option<(
    &'a QuestionGroup,
    &'a IntakeQuestion,
    &'a AnswerRevision,
    &'a str,
)> {
    let (group_id, question_id) = source.as_str()?.strip_prefix("answer:")?.split_once(':')?;
    let g = r.question_groups.iter().find(|g| {
        g.id == group_id && g.stage == "resources" && g.cycle == r.cycle && g.status == "submitted"
    })?;
    let q = g.questions.iter().find(|q| q.id == question_id)?;
    let rev = g.answers.last()?;
    Some((g, q, rev, rev.answers.get(question_id)?.as_str()))
}

fn binding(
    r: &ProductionRecord,
    original: &Value,
    adopted: &Value,
    revision: u64,
    human: bool,
) -> Option<Value> {
    let (group, question, current, text) = answer(r, &adopted["sourceId"])?;
    let quote = adopted["quote"].as_str()?;
    let latest_linked = r.question_groups.iter().rev().find(|g| {
        g.cycle == r.cycle
            && g.status != "cancelled"
            && g.questions
                .iter()
                .any(|q| q.clarifies_requirement_id.as_deref() == original["id"].as_str())
    });
    if original["kind"] != "clarify"
        || latest_linked.is_some_and(|g| g.id != group.id)
        || original["withdrawn"] == true
        || original["id"] == adopted["id"]
        || adopted["kind"] == "clarify"
        || adopted["withdrawn"] == true
        || current.revision != revision
        || quote.trim().is_empty()
        || !text.contains(quote)
        || !r
            .art_design
            .sources
            .iter()
            .any(|s| s["id"] == adopted["sourceId"] && s["hash"] == adopted["sourceHash"])
        || !(question.clarifies_requirement_id.as_deref() == original["id"].as_str()
            || human && question.clarifies_requirement_id.is_none())
    {
        return None;
    }
    Some(
        json!({"requirementId":adopted["id"],"groupId":group.id,"questionId":question.id,
        "answerRevision":revision,"sourceHash":adopted["sourceHash"],"originalSourceHash":original["sourceHash"]}),
    )
}

pub(super) fn resolved(r: &ProductionRecord, q: &Value) -> bool {
    let link = &q["clarificationAnswer"];
    let Some(adopted) = r
        .art_design
        .requirements
        .iter()
        .find(|a| a["id"] == link["requirementId"])
    else {
        return false;
    };
    binding(
        r,
        q,
        adopted,
        link["answerRevision"].as_u64().unwrap_or(0),
        true,
    )
    .as_ref()
        == Some(link)
}

pub(super) fn connect(
    r: &mut ProductionRecord,
    id: &str,
    data: &Value,
    human: bool,
) -> AppResult<()> {
    let item = r
        .art_design
        .requirements
        .iter()
        .find(|q| q["id"] == id)
        .unwrap();
    let explicit = data.get("answerRequirementId");
    let pair = if let Some(target) = explicit {
        if !human {
            return Err(AppError::validation("旧题目的补充关联需要策划核对。"));
        }
        let adopted = r
            .art_design
            .requirements
            .iter()
            .find(|a| a["id"] == *target)
            .ok_or_else(|| AppError::validation("明确要求不存在。"))?;
        Some((item, adopted))
    } else if let Some((_, q, _, _)) = answer(r, &item["sourceId"])
        && let Some(old) = &q.clarifies_requirement_id
        && item["kind"] != "clarify"
    {
        Some((
            r.art_design
                .requirements
                .iter()
                .find(|a| a["id"] == *old)
                .ok_or_else(|| AppError::validation("澄清要求不存在。"))?,
            item,
        ))
    } else {
        None
    };
    if let Some((original, adopted)) = pair {
        let rev = data["answerRevision"]
            .as_u64()
            .or_else(|| {
                let saved = &original["clarificationAnswer"];
                (saved["requirementId"] == adopted["id"])
                    .then(|| saved["answerRevision"].as_u64())
                    .flatten()
            })
            .ok_or_else(|| AppError::validation("请核对并提供当前正式答案的 answerRevision。"))?;
        let link = binding(r, original, adopted, rev, human)
            .ok_or_else(|| AppError::validation("请关联同周期、对应题目最新正式答案中的原句。"))?;
        let old = original["id"].clone();
        r.art_design
            .requirements
            .iter_mut()
            .find(|q| q["id"] == old)
            .unwrap()["clarificationAnswer"] = link;
    }
    Ok(())
}
