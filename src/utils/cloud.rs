//! S3 云端清单读取。
//!
//! 与 `update` 使用同一个 bucket 与凭据；只读取 `dytes.json` / `packages.json` 这类小文件。
//! 注意：SSE / FFI 调用是同步的，网络请求会阻塞 cyctron 的调用线程，适合单次检查。

use crate::utils::error::{AppError, AppResult};
use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::{Config, Region};
use std::fs;
use std::path::Path;

const ENDPOINT_URL: &str = "https://s3.api.upyun.com";
const ACCESS_KEY: &str = "195cc6d12c2d45389a5e52e5138f7250";
const SECRET_KEY: &str = "7ab145ae97310ca9f65db3dff07beac2";
const BUCKET_NAME: &str = "packages";

fn runtime() -> AppResult<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| AppError::db(format!("创建 tokio 运行时失败: {error}")))
}

async fn s3_client() -> Client {
    let credentials = Credentials::new(ACCESS_KEY, SECRET_KEY, None, None, "custom");
    let config = aws_config::defaults(BehaviorVersion::latest())
        .endpoint_url(ENDPOINT_URL)
        .region(Region::new("us-east-1"))
        .credentials_provider(credentials)
        .load()
        .await;

    let config = Config::from(&config)
        .to_builder()
        .force_path_style(true)
        .build();

    Client::from_conf(config)
}

/// 读取 S3 对象的 UTF-8 文本。
pub fn fetch_text(object_name: &str) -> AppResult<String> {
    let runtime = runtime()?;

    runtime.block_on(async {
        let client = s3_client().await;
        let response = client
            .get_object()
            .bucket(BUCKET_NAME)
            .key(object_name)
            .send()
            .await
            .map_err(|error| AppError::db(format!("S3 读取 {object_name} 失败: {error:?}")))?;

        let bytes = response
            .body
            .collect()
            .await
            .map_err(|error| AppError::db(format!("读取 S3 响应失败: {error:?}")))?;

        String::from_utf8(bytes.into_bytes().to_vec())
            .map_err(|error| AppError::db(format!("{object_name} 不是 UTF-8: {error}")))
    })
}

/// 批量下载 S3 对象到本地文件（一次创建运行时与客户端）。
pub fn download_objects(items: &[(String, String)]) -> AppResult<()> {
    if items.is_empty() {
        return Ok(());
    }

    let runtime = runtime()?;

    runtime.block_on(async {
        let client = s3_client().await;

        for (object_name, dest_path) in items {
            let response = client
                .get_object()
                .bucket(BUCKET_NAME)
                .key(object_name)
                .send()
                .await
                .map_err(|error| AppError::db(format!("S3 下载 {object_name} 失败: {error:?}")))?;

            let bytes = response
                .body
                .collect()
                .await
                .map_err(|error| AppError::db(format!("读取 S3 响应失败: {error:?}")))?;

            if let Some(parent) = Path::new(dest_path).parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| AppError::db(format!("创建目录失败: {error}")))?;
            }

            fs::write(dest_path, bytes.into_bytes())
                .map_err(|error| AppError::db(format!("写入 {dest_path} 失败: {error}")))?;
        }

        Ok(())
    })
}
