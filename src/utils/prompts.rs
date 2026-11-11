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

/// 50 个汉字按 UTF-8 编码约 150 字节；英文/数字/标点按实际字节计入。
pub const MAX_PROMPT_CONTENT_BYTES: usize = 150;

/// 校验 prompt 列表：元素必须是对象，`content` 去首尾空白后不超过 150 字节。
pub fn validate_prompt_list(prompts: &[Value]) -> AppResult<()> {
    for (index, prompt) in prompts.iter().enumerate() {
        if !prompt.is_object() {
            return Err(AppError::param(format!(
                "prompts[{index}] 必须是对象"
            )));
        }
        let content = prompt
            .get("content")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if content.as_bytes().len() > MAX_PROMPT_CONTENT_BYTES {
            return Err(AppError::param(format!(
                "prompts[{index}].content 不能超过 {MAX_PROMPT_CONTENT_BYTES} 字节（50 个汉字）"
            )));
        }
    }
    Ok(())
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
    validate_prompt_list(&req.prompts)?;

    let plugin_uuid = req.plugin_uuid.trim();
    designdb::set_prompts_doc(plugin_uuid, req.prompts)?;
    get_prompts(GetPromptsReq {
        plugin_uuid: plugin_uuid.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_content_allows_50_chinese_chars() {
        let content = "汉".repeat(50);
        assert!(validate_prompt_list(&[json!({ "content": content })]).is_ok());
    }

    #[test]
    fn prompt_content_rejects_51_chinese_chars() {
        let content = "汉".repeat(51);
        let error = validate_prompt_list(&[json!({ "content": content })]).unwrap_err();
        assert_eq!(error.code, 400103);
        assert!(error.msg.contains("prompts[0].content"));
    }

    #[test]
    fn prompt_content_counts_utf8_bytes() {
        assert!(validate_prompt_list(&[json!({ "content": "a".repeat(150) })]).is_ok());
        let error =
            validate_prompt_list(&[json!({ "content": "a".repeat(151) })]).unwrap_err();
        assert_eq!(error.code, 400103);
    }

    #[test]
    fn prompt_content_rejects_non_object() {
        let error = validate_prompt_list(&[json!("plain string")]).unwrap_err();
        assert_eq!(error.code, 400103);
    }
}
