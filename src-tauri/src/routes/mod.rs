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
pub async fn start_router() -> Result<RouterCommandResult, String> {
    tauri::async_runtime::spawn_blocking(start_router_blocking)
        .await
        .map_err(|error| format!("启动 Router 任务执行失败：{}", error))?
}
#[tauri::command]
pub async fn stop_router() -> Result<RouterCommandResult, String> {
    tauri::async_runtime::spawn_blocking(stop_router_blocking)
        .await
        .map_err(|error| format!("停止 Router 任务执行失败：{}", error))?
}
#[tauri::command]
pub fn router_status() -> Result<RouterCommandResult, String> {
    let state = router_state();
    let runtime = state.lock().map_err(|error| error.to_string())?;
    let status = if runtime.started {
        "running"
    } else {
        "stopped"
    };

    Ok(build_router_result(&runtime, status))
}
#[tauri::command]
pub fn check_router_port_occupancy() -> PortOccupancyInfo {
    build_port_occupancy_info()
}
#[tauri::command]
pub fn ensure_required_config_files() -> Result<Vec<String>, String> {
    ensure_workspace_layout()?;
    ensure_catalog_base_config_file()?;
    ensure_catalog_config_file()?;
    ensure_provider_config_file()?;
    ensure_app_settings_file()?;

    Ok(vec![
        catalog_base_config_path()?.display().to_string(),
        catalog_config_path()?.display().to_string(),
        provider_config_path()?.display().to_string(),
        app_settings_path()?.display().to_string(),
    ])
}
#[tauri::command]
pub async fn prepare_router_startup(
    request: RouterStartupPreparationRequest,
) -> Result<RouterStartupPreparationResult, String> {
    tauri::async_runtime::spawn_blocking(move || prepare_router_startup_blocking(request))
        .await
        .map_err(|error| format!("Router 启动准备任务执行失败：{}", error))?
}
#[tauri::command]
pub async fn restart_router() -> Result<RouterCommandResult, String> {
    tauri::async_runtime::spawn_blocking(restart_router_blocking)
        .await
        .map_err(|error| format!("重启 Router 任务执行失败：{}", error))?
}
#[tauri::command]
pub fn write_codex_router_config() -> Result<(), String> {
    ensure_codex_config_backup()?;
    upsert_codex_router_config()
}
