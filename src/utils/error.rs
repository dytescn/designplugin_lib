//! 业务错误与 FFI 错误码。

/// 业务错误：`code` 写入响应 JSON，`msg` 为可读说明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppError {
    pub code: i32,
    pub msg: String,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: i32, msg: impl Into<String>) -> Self {
        Self {
            code,
            msg: msg.into(),
        }
    }

    /// 4001xx：参数 / JSON / UTF-8。
    pub fn param(msg: impl Into<String>) -> Self {
        Self::new(400103, msg)
    }

    /// 4004xx：未找到。
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(400404, msg)
    }

    /// 4004xx：唯一键冲突 / 已存在。
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::new(400409, msg)
    }

    /// 5002xx：数据库加载 / 查询 / 写入失败。
    pub fn db(msg: impl Into<String>) -> Self {
        Self::new(500201, msg)
    }

    /// 5003xx：能力库 / 文件相关失败（M4 使用）。
    pub fn file(msg: impl Into<String>) -> Self {
        Self::new(500300, msg)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.msg)
    }
}

impl std::error::Error for AppError {}
