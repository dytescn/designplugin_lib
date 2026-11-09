//! 任务流业务：整体读写 designdb `taskflow` 集合里的数组。
//!
//! 整个集合只有一条文档，`taskflows` 字段就是数组，不提供单条操作。

use crate::utils::designdb;
use crate::utils::error::{AppError, AppResult};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Default, Deserialize)]
pub struct GetTaskflowReq {}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateTaskflowReq {
    #[serde(default)]
    pub taskflows: Vec<Value>,
}

pub fn get_taskflow(_req: GetTaskflowReq) -> AppResult<Value> {
    let taskflows = designdb::get_taskflow_doc()?
        .and_then(|doc| doc.get("taskflows").cloned())
        .unwrap_or_else(|| json!([]));
    Ok(json!({ "taskflows": taskflows }))
}

pub fn update_taskflow(req: UpdateTaskflowReq) -> AppResult<Value> {
    for (index, taskflow) in req.taskflows.iter().enumerate() {
        if !taskflow.is_object() {
            return Err(AppError::param(format!(
                "taskflows[{index}] 必须是对象"
            )));
        }
    }

    designdb::set_taskflow_doc(req.taskflows)?;
    get_taskflow(GetTaskflowReq {})
}
