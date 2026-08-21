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
pub fn load_mcp_servers() -> Result<McpServerListResult, String> {
    let config_path = codex_config_path()?;
    let items = read_mcp_servers_from_config(&config_path)?;

    Ok(McpServerListResult {
        total: items.len(),
        source_path: config_path.display().to_string(),
        items,
    })
}
#[tauri::command]
pub fn upsert_mcp_server(request: UpsertMcpServerRequest) -> Result<McpServerSummary, String> {
    let config_path = codex_config_path()?;
    let server = McpServerSummary {
        name: request.name.trim().to_string(),
        transport: request.transport,
        enabled: request.enabled,
        source_path: config_path.display().to_string(),
        command: request
            .command
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        args: request
            .args
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        url: request
            .url
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        headers: request.headers,
        environment: request.environment,
    };

    validate_mcp_server(&server)?;
    write_mcp_server_to_config(&config_path, &server)?;
    read_mcp_servers_from_config(&config_path)?
        .into_iter()
        .find(|item| item.name == server.name)
        .ok_or_else(|| "MCP 服务保存后未能重新读取。".to_string())
}

#[tauri::command]
pub fn set_mcp_server_enabled(request: SetMcpServerEnabledRequest) -> Result<McpServerSummary, String> {
    let config_path = codex_config_path()?;
    let mut server = read_mcp_servers_from_config(&config_path)?
        .into_iter()
        .find(|item| item.name == request.name)
        .ok_or_else(|| format!("MCP 服务不存在：{}", request.name))?;
    server.enabled = request.enabled;
    write_mcp_server_to_config(&config_path, &server)?;
    Ok(server)
}

#[tauri::command]
pub fn remove_mcp_server(request: RemoveMcpServerRequest) -> Result<McpServerListResult, String> {
    let config_path = codex_config_path()?;
    remove_mcp_server_from_config(&config_path, &request.name)?;
    load_mcp_servers()
}
