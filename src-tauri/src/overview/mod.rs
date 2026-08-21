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
pub fn dashboard_quick_counts() -> Result<DashboardQuickCounts, String> {
    let account_count = quick_account_count()?;
    let skill_count = quick_skill_count(&codex_skills_path()?)?;
    let mcp_items = read_mcp_servers_from_config(&codex_config_path()?)?;
    let mcp_total = mcp_items.len();
    let mcp_enabled = mcp_items.iter().filter(|item| item.enabled).count();

    Ok(DashboardQuickCounts {
        account_count,
        skill_count,
        mcp_total,
        mcp_enabled,
    })
}
