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
pub fn start_codex_account_login() -> Result<CodexAccountOperationResult, String> {
    let code_verifier = random_base64_url(96)?;
    let state = random_base64_url(24)?;
    let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));
    let redirect_uri = oauth_redirect_uri();
    let login_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&prompt=login&id_token_add_organizations=true&codex_cli_simplified_flow=true&state={}",
        OAUTH_AUTHORIZE_URL,
        OAUTH_CLIENT_ID,
        url_encode_component(&redirect_uri),
        url_encode_component(OAUTH_SCOPE),
        code_challenge,
        state,
    );

    let oauth_state = codex_oauth_state();
    let mut pending = oauth_state.lock().map_err(|error| error.to_string())?;
    *pending = Some(CodexOAuthLoginState {
        state,
        code_verifier,
        created_at: Instant::now(),
    });
    drop(pending);
    set_codex_oauth_last_result(CodexOAuthLoginStatus {
        status: "waiting".to_string(),
        message: "等待浏览器授权回调。".to_string(),
        account_key: None,
        account_email: None,
    });

    ensure_codex_oauth_callback_listener()?;

    Ok(CodexAccountOperationResult {
        message: "已生成 OAuth 登录链接。浏览器登录完成后会回调到本地并保存账号。".to_string(),
        path: Some(login_url),
        scan: scan_codex_accounts()?,
    })
}
#[tauri::command]
pub fn codex_oauth_login_status() -> CodexOAuthLoginStatus {
    codex_oauth_last_result()
        .lock()
        .ok()
        .and_then(|status| status.clone())
        .unwrap_or(CodexOAuthLoginStatus {
            status: "idle".to_string(),
            message: "尚未开始 OAuth 登录。".to_string(),
            account_key: None,
            account_email: None,
        })
}

#[tauri::command]
pub async fn start_codex_client_login() -> Result<CodexAccountOperationResult, String> {
    tauri::async_runtime::spawn_blocking(|| {
        backup_current_auth_file()?;
        let auth_path = codex_auth_path()?;
        if auth_path.exists() {
            fs::remove_file(&auth_path).map_err(|error| {
                format!(
                    "清理当前 Codex 登录状态失败：{}，路径：{}",
                    error,
                    auth_path.display()
                )
            })?;
        }

        let restart_message = restart_codex_process();

        Ok(CodexAccountOperationResult {
            message: format!(
                "已退出当前 Codex 客户端登录状态，并尝试打开客户端登录页。{}",
                restart_message
            ),
            path: Some(auth_path.display().to_string()),
            scan: scan_codex_accounts()?,
        })
    })
    .await
    .map_err(|error| format!("重启 Codex 客户端失败：{}", error))?
}

#[tauri::command]
pub fn codex_oauth_callback_listener_status() -> CodexOAuthCallbackListenerStatus {
    let result = CODEX_OAUTH_CALLBACK_LISTENER.get();
    let running = matches!(result, Some(Ok(())));
    let port = CODEX_OAUTH_CALLBACK_LISTENER_PORT
        .get()
        .copied()
        .unwrap_or_else(configured_oauth_callback_port);
    let message = match result {
        Some(Ok(())) => "OAuth 回调监听正常。".to_string(),
        Some(Err(error)) => error.clone(),
        None => "OAuth 回调监听尚未启动。".to_string(),
    };

    CodexOAuthCallbackListenerStatus {
        running,
        host: ROUTER_HOST.to_string(),
        port,
        callback_url: oauth_redirect_uri(),
        message,
    }
}
