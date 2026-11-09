//! 提示词业务：按 `plugin_uuid` 整体读写提示词数组。
//!
//! 提示词不使用 SQLite 表，直接存放在 designdb `prompts` 集合的文档里；
//! 每个 `plugin_uuid` 一条文档，`prompts` 字段就是数组，不提供单条操作。

use crate::utils::designdb;
use crate::utils::error::{AppError, AppResult};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Default, Deserialize)]
pub struct GetPromptsReq {
    #[serde(default)]
    pub plugin_uuid: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdatePromptsReq {
    #[serde(default)]
    pub plugin_uuid: String,
    #[serde(default)]
    pub prompts: Vec<Value>,
}

fn require_plugin_uuid(value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::param("plugin_uuid is required"));
    }
    Ok(())
}

pub fn get_prompts(req: GetPromptsReq) -> AppResult<Value> {
    require_plugin_uuid(&req.plugin_uuid)?;
    let plugin_uuid = req.plugin_uuid.trim();
    let prompts = designdb::get_prompts_doc(plugin_uuid)?
        .and_then(|doc| doc.get("prompts").cloned())
        .unwrap_or_else(|| json!([]));
    Ok(json!({ "plugin_uuid": plugin_uuid, "prompts": prompts }))
}

pub fn update_prompts(req: UpdatePromptsReq) -> AppResult<Value> {
    require_plugin_uuid(&req.plugin_uuid)?;
    for (index, prompt) in req.prompts.iter().enumerate() {
        if !prompt.is_object() {
            return Err(AppError::param(format!(
                "prompts[{index}] 必须是对象"
            )));
        }
    }

    let plugin_uuid = req.plugin_uuid.trim();
    designdb::set_prompts_doc(plugin_uuid, req.prompts)?;
    get_prompts(GetPromptsReq {
        plugin_uuid: plugin_uuid.to_string(),
    })
}
