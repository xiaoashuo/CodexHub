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
pub fn token_usage_summary() -> Result<TokenUsageSummary, String> {
    read_request_log_token_usage_summary()
}
#[tauri::command]
pub fn scan_codex_accounts() -> Result<CodexAccountScanResult, String> {
    ensure_workspace_layout()?;
    ensure_accounts_registry_file()?;
    normalize_imported_accounts_registry_paths()?;
    let mut registry_root = read_accounts_registry()?;
    let synced_count = sync_accounts_registry_from_snapshot_dir(&mut registry_root)?;
    let deduplicated = dedupe_registry_accounts_by_email(&mut registry_root)?;
    complete_missing_account_snapshot_id_tokens(&registry_root)?;
    if synced_count > 0 || deduplicated {
        write_accounts_registry(&registry_root)?;
    }
    let auth_root = read_json_file_optional(&codex_auth_path()?);
    let current_account_id = registry_root
        .get("activeAccountKey")
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .or_else(|| auth_root.as_ref().and_then(find_codex_account_id));
    let mut accounts = Vec::new();

    collect_accounts_from_registry(&registry_root, &current_account_id, &mut accounts);

    Ok(CodexAccountScanResult {
        api_healthy: !accounts.is_empty(),
        accounts,
        current_account_id,
        scanned_at: current_log_time(),
    })
}
#[tauri::command]
pub async fn switch_codex_account(
    request: CodexAccountKeyRequest,
) -> Result<CodexAccountOperationResult, String> {
    tauri::async_runtime::spawn_blocking(move || switch_codex_account_blocking(request))
        .await
        .map_err(|error| format!("切换账号任务执行失败：{}", error))?
}
#[tauri::command]
pub fn remove_codex_account_snapshot(
    request: CodexAccountKeyRequest,
) -> Result<CodexAccountOperationResult, String> {
    let mut registry = read_accounts_registry()?;
    let removed_snapshot_path = remove_registry_account(&mut registry, &request.account_key)?;

    if let Some(path_text) = removed_snapshot_path.as_ref() {
        let path = PathBuf::from(path_text);
        if path.exists() {
            fs::remove_file(&path).map_err(|error| {
                format!("删除账号快照失败：{}，路径：{}", error, path.display())
            })?;
        }
    }

    write_accounts_registry(&registry)?;

    Ok(CodexAccountOperationResult {
        message: "已从账号管理中移除该账号快照。".to_string(),
        path: removed_snapshot_path,
        scan: scan_codex_accounts()?,
    })
}

#[tauri::command]
pub fn import_current_codex_account() -> Result<CodexAccountOperationResult, String> {
    let mut registry = read_accounts_registry()?;
    let account_key = upsert_current_codex_auth_account(&mut registry)?;
    write_accounts_registry(&registry)?;

    let refreshed_usage =
        refresh_account_usage_from_backend_api(&mut registry, &account_key, false);
    if refreshed_usage {
        write_accounts_registry(&registry)?;
    }

    Ok(CodexAccountOperationResult {
        message: if refreshed_usage {
            "已保存当前 Codex 登录，并拉取该账号的最新额度。".to_string()
        } else {
            "已保存当前 Codex 登录。额度可稍后点击刷新额度更新。".to_string()
        },
        path: find_registry_account(&registry, &account_key)
            .and_then(|account| json_string_field(&account, "snapshotPath")),
        scan: scan_codex_accounts()?,
    })
}
#[tauri::command]
pub fn import_cpa_account(request: CpaImportRequest) -> Result<CodexAccountOperationResult, String> {
    let raw_root = serde_json::from_str::<serde_json::Value>(request.cpa_json.trim())
        .map_err(|error| format!("解析 CPA JSON 失败：{}", error))?;
    // Try to unwrap from common outer wrapper fields
    let auth_root = unwrap_cpa_auth_value(&raw_root);
    // Normalize identity fields: email, name, account_id, plan from JWT claims
    let enriched = enrich_codex_auth_identity(auth_root.clone());
    let mut registry = read_accounts_registry()?;
    let account_key = upsert_codex_auth_value_account(&mut registry, &enriched, false)?;
    write_accounts_registry(&registry)?;

    let mut refreshed_registry = read_accounts_registry()?;
    if refresh_account_usage_from_backend_api(&mut refreshed_registry, &account_key, false) {
        write_accounts_registry(&refreshed_registry)?;
    }

    Ok(CodexAccountOperationResult {
        message: "已通过 CPA JSON 保存账号。".to_string(),
        path: find_registry_account(&refreshed_registry, &account_key)
            .and_then(|account| json_string_field(&account, "snapshotPath")),
        scan: scan_codex_accounts()?,
    })
}

#[tauri::command]
pub fn export_codex_accounts() -> Result<CodexAccountOperationResult, String> {
    let registry = read_accounts_registry()?;
    let export_dir = codex_accounts_backups_path()?.join(format!("export-{}", current_log_time()));
    let export_snapshots_dir = export_dir.join("snapshots");
    fs::create_dir_all(&export_snapshots_dir).map_err(|error| {
        format!(
            "创建账号导出目录失败：{}，路径：{}",
            error,
            export_snapshots_dir.display()
        )
    })?;

    let registry_path = codex_accounts_registry_path()?;
    fs::copy(&registry_path, export_dir.join("registry.json"))
        .map_err(|error| format!("导出账号 registry 失败：{}", error))?;

    if let Some(items) = registry.get("items").and_then(|value| value.as_array()) {
        for item in items {
            let Some(snapshot_path_text) = json_string_field(item, "snapshotPath") else {
                continue;
            };
            let snapshot_path = PathBuf::from(snapshot_path_text);
            if !snapshot_path.exists() {
                continue;
            }
            let Some(file_name) = snapshot_path.file_name() else {
                continue;
            };
            let _ = fs::copy(&snapshot_path, export_snapshots_dir.join(file_name));
        }
    }

    Ok(CodexAccountOperationResult {
        message: "账号 registry 与快照已导出。".to_string(),
        path: Some(export_dir.display().to_string()),
        scan: scan_codex_accounts()?,
    })
}

#[tauri::command]
pub fn update_codex_account_expiration(
    request: CodexAccountExpirationRequest,
) -> Result<CodexAccountOperationResult, String> {
    let expires_at = request
        .expires_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_expiration_iso)
        .transpose()?;

    let mut registry = read_accounts_registry()?;
    let root = registry
        .as_object_mut()
        .ok_or_else(|| "账号注册表格式无效。".to_string())?;
    let items = root
        .get_mut("items")
        .and_then(|value| value.as_array_mut())
        .ok_or_else(|| "账号注册表缺少 items。".to_string())?;
    let item = items
        .iter_mut()
        .find(|item| json_string_field(item, "accountKey").as_deref() == Some(&request.account_key))
        .ok_or_else(|| "未找到要设置到期时间的账号。".to_string())?;
    let account = item
        .as_object_mut()
        .ok_or_else(|| "账号注册表条目格式无效。".to_string())?;

    account.insert(
        "subscriptionExpiresAt".to_string(),
        expires_at
            .as_ref()
            .map(|value| serde_json::Value::String(value.clone()))
            .unwrap_or(serde_json::Value::Null),
    );
    root.insert(
        "updatedAt".to_string(),
        serde_json::Value::Number(current_log_time().parse::<i64>().unwrap_or_default().into()),
    );
    write_accounts_registry(&registry)?;

    Ok(CodexAccountOperationResult {
        message: if expires_at.is_some() {
            "账号到期时间已保存。".to_string()
        } else {
            "账号到期时间已清空。".to_string()
        },
        path: None,
        scan: scan_codex_accounts()?,
    })
}

const EXPIRATION_PARSE_ERROR: &str = "到期时间格式无效，请重新选择日期。";

fn normalize_expiration_iso(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(EXPIRATION_PARSE_ERROR.to_string());
    }

    let candidate = if !trimmed.contains('T') && trimmed.len() == 10 && trimmed.as_bytes()[4] == b'-' && trimmed.as_bytes()[7] == b'-' {
        format!("{}T23:59:59Z", trimmed)
    } else {
        strip_expiration_fractional(trimmed)
    };

    match OffsetDateTime::parse(&candidate, &Rfc3339) {
        Ok(date) => date.format(&Rfc3339).map_err(|_| EXPIRATION_PARSE_ERROR.to_string()),
        Err(_) => Err(EXPIRATION_PARSE_ERROR.to_string()),
    }
}

fn strip_expiration_fractional(value: &str) -> String {
    let dot = match value.find('.') {
        Some(index) => index,
        None => return value.to_string(),
    };
    let offset_index = value.find('Z').or_else(|| value.rfind('+'));
    match offset_index {
        Some(offset) if offset > dot => format!("{}{}", &value[..dot], &value[offset..]),
        _ => value.to_string(),
    }
}
