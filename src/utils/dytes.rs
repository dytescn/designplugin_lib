//! 调用外部 `dytes` 可执行文件。
//!
//! 插件安装与更新检查都交给 `dytes` 完成，本库不再直接访问 UpYun / S3。
//! 可执行文件按以下顺序查找：
//!
//! 1. `DYTES_BIN` 环境变量；
//! 2. 安装根目录下的 `dytes.exe` / `dytes`；
//! 3. `PATH` 中的 `dytes.exe` / `dytes`。

use crate::utils::error::{AppError, AppResult};
use std::path::{Path, PathBuf};
use std::process::Command;

const NAMES: [&str; 2] = ["dytes.exe", "dytes"];

/// 查找 `dytes` 可执行文件。
pub fn executable(root: &Path) -> AppResult<PathBuf> {
    if let Ok(value) = std::env::var("DYTES_BIN") {
        let value = value.trim();
        if !value.is_empty() {
            let path = PathBuf::from(value);
            if path.is_file() {
                return Ok(path);
            }
            return Err(AppError::file(format!(
                "DYTES_BIN 指向的文件不存在: {}",
                path.display()
            )));
        }
    }

    for name in NAMES {
        let path = root.join(name);
        if path.is_file() {
            return Ok(path);
        }
    }

    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for name in NAMES {
                let path = dir.join(name);
                if path.is_file() {
                    return Ok(path);
                }
            }
        }
    }

    Err(AppError::file(
        "未找到 dytes 可执行文件；请将 dytes 加入 PATH 或设置 DYTES_BIN",
    ))
}

/// 一次 `dytes` 调用的结果。
pub struct Output {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// 在安装根目录执行 `dytes` 命令。
pub fn run(root: &Path, args: &[&str]) -> AppResult<Output> {
    let exe = executable(root)?;
    let output = Command::new(&exe)
        .args(args)
        .current_dir(root)
        // plugin_lib 自己会在 install 后调用 scanner::scan；避免 dytes 再回调
        // 正在处理当前请求的 cyctron，造成嵌套等待。
        .env("DYTES_SKIP_PLUGIN_SCAN", "1")
        .output()
        .map_err(|error| {
            AppError::file(format!("执行 {} 失败: {error}", exe.display()))
        })?;

    Ok(Output {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// 解析 `--json` 输出；允许前面夹杂日志行。
pub fn parse_json(output: &Output) -> AppResult<serde_json::Value> {
    let text = output.stdout.trim();
    if let Ok(value) = serde_json::from_str(text) {
        return Ok(value);
    }
    for line in output.stdout.lines().rev() {
        let line = line.trim();
        if line.starts_with('{')
            && let Ok(value) = serde_json::from_str(line)
        {
            return Ok(value);
        }
    }

    Err(AppError::file(format!(
        "dytes 未返回 JSON: stdout={} stderr={}",
        text,
        output.stderr.trim()
    )))
}
