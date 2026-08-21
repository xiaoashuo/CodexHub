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
pub async fn scan_codex_threads() -> Result<ThreadScanResult, String> {
    tauri::async_runtime::spawn_blocking(scan_codex_threads_blocking)
        .await
        .map_err(|error| format!("会话扫描任务执行失败：{}", error))?
}
#[tauri::command]
pub async fn quick_codex_thread_summary() -> Result<ScanSummary, String> {
    tauri::async_runtime::spawn_blocking(quick_codex_thread_summary_blocking)
        .await
        .map_err(|error| format!("quick thread summary task failed: {}", error))?
}
#[tauri::command]
pub async fn delete_codex_thread_files(
    request: DeleteCodexThreadFilesRequest,
) -> Result<ThreadScanResult, String> {
    tauri::async_runtime::spawn_blocking(move || delete_codex_thread_files_blocking(request))
        .await
        .map_err(|error| format!("会话删除任务执行失败：{}", error))?
}
#[tauri::command]
pub async fn restore_codex_thread_index(
    request: serde_json::Value,
) -> Result<RestoreCodexThreadIndexResult, String> {
    let request = parse_restore_codex_thread_index_request(request)?;
    tauri::async_runtime::spawn_blocking(move || restore_codex_thread_index_blocking(request))
        .await
        .map_err(|error| format!("会话索引恢复任务执行失败：{}", error))?
}
#[tauri::command]
pub async fn check_restore_codex_thread_index(
    request: serde_json::Value,
) -> Result<RestoreCodexThreadIndexCheckResult, String> {
    let request = parse_restore_codex_thread_index_request(request)?;
    tauri::async_runtime::spawn_blocking(move || check_restore_codex_thread_index_blocking(request))
        .await
        .map_err(|error| format!("会话恢复预检查执行失败：{}", error))?
}
