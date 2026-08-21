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
pub fn router_request_logs() -> Result<Vec<RouterLogEntry>, String> {
    read_router_log_entries(REQUEST_LOG_LIMIT)
}
#[tauri::command]
pub fn clear_router_request_logs() -> Result<Vec<RouterLogEntry>, String> {
    let mut logs = router_logs().lock().map_err(|error| error.to_string())?;
    logs.clear();
    let path = router_log_path()?;
    ensure_parent_dir(&path)?;
    fs::write(&path, "").map_err(|error| {
        format!(
            "clear router log failed: {}, path: {}",
            error,
            path.display()
        )
    })?;

    Ok(Vec::new())
}
#[tauri::command]
pub fn account_proxy_request_logs() -> Result<Vec<AccountProxyLogEntry>, String> {
    read_account_proxy_log_entries(REQUEST_LOG_LIMIT)
}
#[tauri::command]
pub fn clear_account_proxy_request_logs() -> Result<Vec<AccountProxyLogEntry>, String> {
    let path = account_proxy_log_path()?;
    ensure_parent_dir(&path)?;
    fs::write(&path, "").map_err(|error| {
        format!(
            "clear account proxy log failed: {}, path: {}",
            error,
            path.display()
        )
    })?;
    Ok(Vec::new())
}
#[tauri::command]
pub fn append_app_log(log: AppOperationLogInput) -> Result<Vec<AppOperationLogEntry>, String> {
    let log_entry = AppOperationLogEntry {
        id: current_log_millis().to_string(),
        time: current_log_time(),
        level: normalize_log_level(&log.level),
        module: log.module,
        action: log.action,
        message: log.message,
        detail: log.detail.filter(|detail| !detail.trim().is_empty()),
    };

    append_app_log_entry(&log_entry)?;
    search_app_logs(AppLogQuery {
        keyword: String::new(),
        level: "all".to_string(),
        limit: APP_LOG_DEFAULT_LIMIT,
    })
}
#[tauri::command]
pub fn search_app_logs(query: AppLogQuery) -> Result<Vec<AppOperationLogEntry>, String> {
    rotate_app_log_if_needed()?;
    let path = app_log_path()?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(format!(
                "璇诲彇搴旂敤鏃ュ織澶辫触：{}锛岃矾寰勶細{}",
                error,
                path.display()
            ))
        }
    };
    let keyword = query.keyword.trim().to_lowercase();
    let level = query.level.trim().to_lowercase();
    let limit = query.limit.clamp(1, APP_LOG_MAX_LIMIT);
    let mut logs = Vec::new();

    for line in text.lines().rev() {
        if line.trim().is_empty() {
            continue;
        }

        let Ok(log) = serde_json::from_str::<AppOperationLogEntry>(line) else {
            continue;
        };

        if level != "all" && log.level != level {
            continue;
        }

        if !keyword.is_empty() && !app_log_matches_keyword(&log, &keyword) {
            continue;
        }

        logs.push(log);

        if logs.len() >= limit {
            break;
        }
    }

    Ok(logs)
}
#[tauri::command]
pub fn clear_app_logs() -> Result<Vec<AppOperationLogEntry>, String> {
    let path = app_log_path()?;
    ensure_parent_dir(&path)?;
    fs::write(&path, "").map_err(|error| {
        format!(
            "娓呯┖搴旂敤鏃ュ織澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })?;

    Ok(Vec::new())
}
#[tauri::command]
pub fn app_log_file_info() -> Result<AppLogFileInfo, String> {
    rotate_app_log_if_needed()?;
    let path = app_log_path()?;
    let size = fs::metadata(&path)
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    let count = search_app_logs(AppLogQuery {
        keyword: String::new(),
        level: "all".to_string(),
        limit: APP_LOG_MAX_LIMIT,
    })?
    .len();

    Ok(AppLogFileInfo {
        path: path.display().to_string(),
        size,
        max_size: APP_LOG_MAX_SIZE_BYTES,
        count,
    })
}
