use crate::*;
mod login;
mod list;
mod detail;
pub use login::*;
pub use list::*;
pub use detail::*;

#[tauri::command]
pub fn detect_codex_exe_path_for_settings() -> Result<String, String> {
    detect_codex_exe_path().ok_or_else(|| {
        "未检测到 ChatGPT.exe 或 codex.exe，请确认客户端已安装，或手动填写完整路径。".to_string()
    })
}
#[tauri::command]
pub async fn refresh_codex_accounts_usage() -> Result<CodexAccountOperationResult, String> {
    tauri::async_runtime::spawn_blocking(refresh_codex_accounts_usage_blocking)
        .await
        .map_err(|error| format!("额度刷新任务执行失败：{}", error))?
}
#[tauri::command]
pub async fn refresh_codex_account_usage(
    request: CodexAccountKeyRequest,
) -> Result<CodexAccountOperationResult, String> {
    tauri::async_runtime::spawn_blocking(move || refresh_codex_account_usage_blocking(request))
        .await
        .map_err(|error| format!("额度刷新任务执行失败：{}", error))?
}
#[tauri::command]
pub async fn refresh_codex_account_token(
    request: CodexAccountKeyRequest,
) -> Result<CodexAccountOperationResult, String> {
    tauri::async_runtime::spawn_blocking(move || refresh_codex_account_token_blocking(request))
        .await
        .map_err(|error| format!("Token 刷新任务执行失败：{}", error))?
}
#[tauri::command]
pub async fn import_chatgpt_session_account(
    request: ChatGptSessionImportRequest,
) -> Result<CodexAccountOperationResult, String> {
    tauri::async_runtime::spawn_blocking(move || import_chatgpt_session_account_blocking(request))
        .await
        .map_err(|error| format!("web_session 登录任务执行失败：{}", error))?
}

pub(crate) fn detect_codex_exe_path() -> Option<String> {
    find_app_path_by_appx_package("ChatGPT", "ChatGPT.exe")
        .or_else(|| find_app_path_by_appx_package("Codex", "Codex.exe"))
        .or_else(find_codex_path_by_where)
        .or_else(find_codex_path_by_powershell)
        .or_else(find_codex_path_from_common_locations)
}

pub(crate) fn read_accounts_registry() -> Result<serde_json::Value, String> {
    ensure_accounts_registry_file()?;
    let path = codex_accounts_registry_path()?;
    let text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "璇诲彇璐﹀彿 registry 澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })?;
    serde_json::from_str::<serde_json::Value>(&text).map_err(|error| {
        format!(
            "瑙ｆ瀽璐﹀彿 registry 澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })
}

pub(crate) fn write_accounts_registry(registry: &serde_json::Value) -> Result<(), String> {
    let path = codex_accounts_registry_path()?;
    ensure_parent_dir(&path)?;
    let text = serde_json::to_string_pretty(registry)
        .map_err(|error| format!("搴忓垪鍖栬处鍙?registry 澶辫触：{}", error))?;
    fs::write(&path, text).map_err(|error| {
        format!(
            "鍐欏叆璐﹀彿 registry 澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })
}

pub(crate) fn refresh_codex_accounts_usage_blocking() -> Result<CodexAccountOperationResult, String> {
    normalize_imported_accounts_registry_paths()?;
    let mut registry = match read_accounts_registry() {
        Ok(registry) => registry,
        Err(_) => {
            return Ok(CodexAccountOperationResult {
                message: "未找到账号 registry，已执行普通扫描。".to_string(),
                path: None,
                scan: scan_codex_accounts()?,
            });
        }
    };

    let refreshed_count = refresh_accounts_usage_from_backend_api(&mut registry);
    if refreshed_count > 0 {
        write_accounts_registry(&registry)?;
        return Ok(CodexAccountOperationResult {
            message: format!(
                "已通过账号 token 拉取 {} 个账号的真实额度。",
                refreshed_count
            ),
            path: None,
            scan: scan_codex_accounts()?,
        });
    }

    Err("通过账号 token 拉取额度失败。".to_string())
}

pub(crate) fn refresh_codex_account_usage_blocking(
    request: CodexAccountKeyRequest,
) -> Result<CodexAccountOperationResult, String> {
    normalize_imported_accounts_registry_paths()?;
    let mut registry = read_accounts_registry()?;
    let manual = request.manual.unwrap_or(true);

    if refresh_account_usage_from_backend_api(&mut registry, &request.account_key, manual) {
        write_accounts_registry(&registry)?;
        return Ok(CodexAccountOperationResult {
            message: "已通过当前账号 token 拉取真实额度。".to_string(),
            path: None,
            scan: scan_codex_accounts()?,
        });
    }

    Err(format!(
        "额度刷新异常：{}",
        take_account_usage_last_error().unwrap_or_else(|| "请查看应用日志".to_string())
    ))
}

pub(crate) fn refresh_codex_account_token_blocking(
    request: CodexAccountKeyRequest,
) -> Result<CodexAccountOperationResult, String> {
    normalize_imported_accounts_registry_paths()?;
    let in_flight = ACCOUNT_TOKEN_REFRESH_IN_FLIGHT.get_or_init(|| Mutex::new(HashSet::new()));
    {
        let mut keys = in_flight
            .lock()
            .map_err(|_| "Token 刷新状态锁异常".to_string())?;
        if keys.contains(&request.account_key) {
            return Err("该账号 Token 正在刷新，请稍后再试。".to_string());
        }
        keys.insert(request.account_key.clone());
    }

    let result = refresh_codex_account_token_inner(&request.account_key);
    if let Ok(mut keys) = in_flight.lock() {
        keys.remove(&request.account_key);
    }

    result
}

pub(crate) fn import_chatgpt_session_account_blocking(
    request: ChatGptSessionImportRequest,
) -> Result<CodexAccountOperationResult, String> {
    let session_root = serde_json::from_str::<serde_json::Value>(request.session_json.trim())
        .map_err(|error| format!("解析 ChatGPT session JSON 失败：{}", error))?;
    let auth_root = build_codex_auth_from_chatgpt_session(&session_root)?;
    let mut registry = read_accounts_registry()?;
    let account_key = upsert_codex_auth_value_account(&mut registry, &auth_root, false)?;
    write_accounts_registry(&registry)?;

    let mut refreshed_registry = read_accounts_registry()?;
    if refresh_account_usage_from_backend_api(&mut refreshed_registry, &account_key, false) {
        write_accounts_registry(&refreshed_registry)?;
    }

    Ok(CodexAccountOperationResult {
        message: "已通过 ChatGPT session 保存账号。".to_string(),
        path: find_registry_account(&refreshed_registry, &account_key)
            .and_then(|account| json_string_field(&account, "snapshotPath")),
        scan: scan_codex_accounts()?,
    })
}
