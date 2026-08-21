use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, FileTimes};
use std::io::{BufRead, BufReader, ErrorKind, Read, Seek, Write};
use std::net::{TcpListener, TcpStream};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, WindowEvent};
use ::time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};

use crate::constants::*;
use crate::provider_protocol::ProviderProtocol;
use crate::router_dispatcher::DispatchCandidate;
use crate::*;
#[tauri::command]
pub fn read_provider_config() -> Result<ProviderConfigInput, String> {
    ensure_provider_config_file()?;
    let config = load_provider_config()?;
    let mut result = ProviderConfigInput::new();

    for (slug, value) in config.0 {
        let route = serde_json::from_value::<ProviderRouteFileItem>(value)
            .map_err(|error| format!("解析 provider 配置失败：{}，slug：{}", error, slug))?;
        result.insert(
            slug.clone(),
            ProviderConfigItemInput {
                display_name: if route.display_name.is_empty() {
                    slug
                } else {
                    route.display_name
                },
                base_url: route.base_url,
                api_key: route.api_key,
                real_model: route.real_model,
                context_window: normalize_positive_u64(route.context_window),
                max_context_window: normalize_positive_u64(route.max_context_window),
                effective_context_window_percent: normalize_percent(
                    route.effective_context_window_percent,
                ),
                proxy_mode: normalize_provider_proxy_mode(&route.proxy_mode),
                proxy_url: normalize_proxy_url(&route.proxy_url).unwrap_or_default(),
                protocol_type: normalize_protocol_type(&route.protocol_type),
                endpoint_path: normalize_endpoint_path(&route.endpoint_path),
                model_mappings: normalize_model_mappings(&route.model_mappings),
                priority: route.priority,
                weight: normalize_provider_weight(route.weight),
                enabled: route.enabled,
                active: route.active,
            },
        );
    }

    Ok(result)
}
#[tauri::command]
pub fn write_provider_config(config: ProviderConfigInput) -> Result<ProviderConfigInput, String> {
    let path = provider_config_path()?;
    ensure_parent_dir(&path)?;
    let value = serde_json::to_value(&config)
        .map_err(|error| format!("序列化 provider 配置失败：{}", error))?;
    let map = value
        .as_object()
        .cloned()
        .unwrap_or_default();
    let wrapped = RouterProviderConfig(map);
    let text = serde_json::to_string_pretty(&wrapped)
        .map_err(|error| format!("序列化 provider 配置失败：{}", error))?;
    fs::write(&path, text).map_err(|error| {
        format!(
            "写入 provider 配置失败：{}，路径：{}",
            error,
            path.display()
        )
    })?;
    read_provider_config()
}
#[tauri::command]
pub fn export_provider_config() -> Result<ProviderConfigExportResult, String> {
    ensure_provider_config_file()?;
    let source_path = provider_config_path()?;
    let backup_dir = model_config_backups_path()?;
    fs::create_dir_all(&backup_dir).map_err(|error| {
        format!(
            "创建模型备份目录失败：{}，路径：{}",
            error,
            backup_dir.display()
        )
    })?;

    let export_path = backup_dir.join(format!(
        "router_provider_config-{}.json",
        current_log_time()
    ));
    fs::copy(&source_path, &export_path).map_err(|error| {
        format!(
            "导出模型配置失败：{}，源路径：{}，目标路径：{}",
            error,
            source_path.display(),
            export_path.display()
        )
    })?;

    Ok(ProviderConfigExportResult {
        export_path: export_path.display().to_string(),
    })
}
#[tauri::command]
pub fn import_provider_config(
    request: ImportProviderConfigRequest,
) -> Result<ProviderConfigImportResult, String> {
    let source_path = PathBuf::from(request.source_path.trim());
    if source_path.as_os_str().is_empty() {
        return Err("请选择要导入的模型配置文件".to_string());
    }

    let metadata = fs::metadata(&source_path).map_err(|error| {
        format!(
            "读取导入文件失败：{}，路径：{}",
            error,
            source_path.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(format!("导入路径不是文件：{}", source_path.display()));
    }

    let source_text = fs::read_to_string(&source_path).map_err(|error| {
        format!(
            "读取导入文件失败：{}，路径：{}",
            error,
            source_path.display()
        )
    })?;
    let loaded: RouterProviderConfig = serde_json::from_str(&source_text).map_err(|error| {
        format!(
            "解析模型配置失败：{}，路径：{}",
            error,
            source_path.display()
        )
    })?;
    let value = serde_json::Value::Object(loaded.0.clone());
    let config: ProviderConfigInput = serde_json::from_value(value).map_err(|error| {
        format!(
            "解析模型配置失败：{}，路径：{}",
            error,
            source_path.display()
        )
    })?;

    let target_path = provider_config_path()?;
    let backup_path = backup_provider_config_before_import(&target_path)?;
    let saved = write_provider_config(config)?;

    Ok(ProviderConfigImportResult {
        config: saved,
        backup_path: backup_path.map(|path| path.display().to_string()),
    })
}
#[tauri::command]
pub fn fetch_provider_models(
    request: FetchProviderModelsRequest,
) -> Result<ProviderModelListResult, String> {
    let base_url = request.base_url.trim();
    let api_key = request.api_key.trim();

    if base_url.is_empty() {
        return Err("Base URL 不能为空".to_string());
    }

    if api_key.is_empty() {
        return Err("API Key 不能为空".to_string());
    }

    let protocol_type = normalize_protocol_type(&request.protocol_type);
    let models_url = build_upstream_models_url(base_url);
    let authorization = format!("Bearer {}", api_key);
    let effective_proxy_url = normalize_proxy_url(&request.proxy_url);
    let mut upstream_request = build_upstream_get_request(
        &models_url,
        effective_proxy_url.as_deref(),
        MODEL_LIST_TIMEOUT_SECONDS,
    );
    if is_anthropic_protocol(&protocol_type) {
        upstream_request = upstream_request
            .set(HEADER_ANTHROPIC_API_KEY, api_key)
            .set(HEADER_ANTHROPIC_VERSION, ANTHROPIC_VERSION_VALUE);
    } else {
        upstream_request = upstream_request.set(HEADER_AUTHORIZATION, &authorization);
    }
    let response = upstream_request.call();

    match response {
        Ok(response) => {
            let body = response
                .into_string()
                .map_err(|error| format!("读取模型列表响应失败：{}", error))?;
            Ok(ProviderModelListResult {
                models: parse_provider_model_ids(&body)?,
                url: models_url,
            })
        }
        Err(ureq::Error::Status(status_code, response)) => {
            let body = response.into_string().unwrap_or_default();
            Err(format!(
                "获取模型列表失败，上游返回状态码 {}：{}",
                status_code, body
            ))
        }
        Err(error) => Err(format!("获取模型列表请求失败：{}", error)),
    }
}
#[tauri::command]
pub async fn test_provider_model(
    request: TestProviderModelRequest,
) -> Result<ProviderModelTestResult, String> {
    tauri::async_runtime::spawn_blocking(move || test_provider_model_blocking(request))
        .await
        .map_err(|error| format!("模型测试任务执行失败：{}", error))?
}
#[tauri::command]
pub async fn test_provider_model_chat(
    request: TestProviderModelRequest,
) -> Result<ProviderModelChatTestResult, String> {
    tauri::async_runtime::spawn_blocking(move || test_provider_model_chat_blocking(request))
        .await
        .map_err(|error| format!("模型对话测试任务执行失败：{}", error))?
}
#[tauri::command]
pub async fn test_proxy_connection(request: ProxyTestRequest) -> Result<ProxyTestResult, String> {
    tauri::async_runtime::spawn_blocking(move || test_proxy_connection_blocking(request))
        .await
        .map_err(|error| format!("代理检测任务执行失败：{}", error))?
}
#[tauri::command]
pub async fn detect_proxy_connection() -> Result<ProxyTestResult, String> {
    tauri::async_runtime::spawn_blocking(detect_proxy_connection_blocking)
        .await
        .map_err(|error| format!("代理检测任务执行失败：{}", error))?
}
#[tauri::command]
pub fn preview_local_file(request: FilePreviewRequest) -> Result<FilePreviewResult, String> {
    let path = PathBuf::from(request.path);
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(FilePreviewResult {
                path: path.display().to_string(),
                exists: false,
                content: String::new(),
                truncated: false,
            });
        }
        Err(error) => {
            return Err(format!(
                "璇诲彇鏂囦欢澶辫触：{}锛岃矾寰勶細{}",
                error,
                path.display()
            ))
        }
    };
    let truncated = content.len() > FILE_PREVIEW_MAX_BYTES;
    let preview_content = if truncated {
        content.chars().take(FILE_PREVIEW_MAX_BYTES).collect()
    } else {
        content
    };

    Ok(FilePreviewResult {
        path: path.display().to_string(),
        exists: true,
        content: preview_content,
        truncated,
    })
}
#[tauri::command]
pub fn sync_enabled_models_to_catalog() -> Result<SyncCatalogResult, String> {
    sync_catalog_from_provider_config()
}
