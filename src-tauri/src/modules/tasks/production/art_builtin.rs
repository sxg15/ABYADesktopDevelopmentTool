use super::*;
use sha2::{Digest, Sha256};

macro_rules! bundled {
    ($name:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../.codex/skills/abya-game-development-task/assets/art-library/",
            $name
        ))
    };
}

/// 随包配方提供源码和演示，不自动获得策划认可或原生验证。
pub(super) fn entries(c: &rusqlite::Connection) -> AppResult<Vec<Value>> {
    let catalog: Value = serde_json::from_slice(bundled!("catalog.json"))?;
    let root = super::art_library::directory(c)?;
    let payloads: Vec<(Value, Vec<u8>)> = [
        ("effects.mjs", bundled!("effects.mjs").as_slice(), "source"),
        (
            "preview.html",
            bundled!("preview.html").as_slice(),
            "preview",
        ),
        (
            "catalog.json",
            bundled!("catalog.json").as_slice(),
            "parameters",
        ),
        (
            "INTEGRATION.md",
            bundled!("INTEGRATION.md").as_slice(),
            "integration",
        ),
    ]
    .into_iter()
    .map(|(name, bytes, role)| {
        (
            json!({"name":name,"path":name,
        "sha256":format!("{:x}",Sha256::digest(bytes)),"role":role,
        "dependencies":if name=="preview.html" {vec!["effects.mjs", "catalog.json"]}else{vec![]}}),
            bytes.to_vec(),
        )
    })
    .collect();
    let mut entries = vec![];
    for preset in catalog["entries"].as_array().ok_or_else(conflict)? {
        let key = mutation::text(preset, "id")?;
        let id = format!("builtin.{key}");
        let resources = json!({"schema":"abya.art-resources/v1","kind":preset["kind"],
            "license":"ABYA 项目原创，可在 ABYA 项目中修改、组合和分发。",
            "compatibility":"Web JavaScript/CSS 演示；Abya 原生接入须查当前 Player 能力。",
            "lifecycle":"触发后采样；暂停保留进度，取消恢复初始值，销毁释放；业务提交独立。",
            "integration":"阅读 INTEGRATION.md，按当前游戏坐标、输入、网络和资源映射改造。",
            "verification":"提供源码与可运行 Web 演示；未在当前游戏或 Abya Player 验证。",
            "parameters":{"strength":{"type":"number","default":1,"minimum":0,"maximum":2},
                "distance":{"type":"number","default":160,"unit":"preview-pixels"}},
            "requiredCapabilities":[],"tests":["正常播放","重复触发","暂停继续","中途取消","销毁清理","原生接入后保存冷加载"],
            "files":payloads.iter().map(|(m,_)|m).collect::<Vec<_>>()});
        super::art_resources::validate(&resources)?;
        let resources =
            super::art_resources::copy(&root, &format!("{id}/1/files"), &resources, &payloads)?;
        entries.push(json!({"schema":"abya.art-case/v1","id":id,"version":1,"status":"available",
            "provenance":"builtin","presetId":key,"title":preset["title"],"tags":preset["tags"],
            "context":preset["purpose"],"plannerRequirements":"依据当前游戏有效要求选择；随包不等于采用。",
            "visualRules":"反馈服务玩法对象与动作因果，颜色与幅度按游戏调整。",
            "decisions":"独立参数采样与渲染适配，便于修改、组合及原生迁移。",
            "production":"effects.mjs + preview.html；源码与依赖随版本保存。",
            "applicability":preset["purpose"],"limitations":"Web 演示不证明原生输入、联机、性能或实机视觉通过。",
            "sources":"本项目原创表现配方","satisfaction":"尚无本任务策划认可",
            "designApproved":false,"runtimeVerified":false,"media":[],"resources":resources}));
    }
    Ok(entries)
}
