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
use crate::router_config::{RouterConfig, load_router_config_file, save_router_config_file};
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
pub fn clean_maintenance_data() -> Result<MaintenanceCleanResult, String> {
    let backup_root = workspace_backup_path()?;
    let cache_root = build_user_relative_path(WORKSPACE_CACHE_RELATIVE_PATH)?;
    let mut backup = CleanCount::default();
    let mut cache = CleanCount::default();
    let invalid_snapshots = clean_invalid_account_snapshots()?;

    if backup_root.exists() {
        backup = remove_dir_contents(&backup_root, "备份目录")?;
    }

    if cache_root.exists() {
        cache = remove_dir_contents(&cache_root, "缓存目录")?;
    }

    let models_cache = models_cache_path()?;
    if models_cache.exists() && models_cache.is_file() {
        let deleted = remove_file_counted(&models_cache, "模型缓存")?;
        cache.count += deleted.count;
        cache.bytes += deleted.bytes;
    }

    ensure_workspace_layout()?;

    let total_count = backup.count + cache.count + invalid_snapshots.count;
    let total_bytes = backup.bytes + cache.bytes + invalid_snapshots.bytes;

    Ok(MaintenanceCleanResult {
        message: format!(
            "已清理 {} 项，占用 {}。",
            total_count,
            format_bytes(total_bytes)
        ),
        backup_deleted_count: backup.count,
        backup_deleted_bytes: backup.bytes,
        cache_deleted_count: cache.count,
        cache_deleted_bytes: cache.bytes,
        invalid_snapshot_deleted_count: invalid_snapshots.count,
        invalid_snapshot_deleted_bytes: invalid_snapshots.bytes,
    })
}

#[tauri::command]
pub fn inspect_migration_backup(
    request: ImportMigrationBackupRequest,
) -> Result<MigrationBackupInspectionResult, String> {
    let source_path = PathBuf::from(request.source_path.trim());
    if source_path.as_os_str().is_empty() {
        return Err("请选择要检查的迁移备份 ZIP。".to_string());
    }
    if !source_path.exists() || !source_path.is_file() {
        return Err(format!("迁移备份文件不存在：{}", source_path.display()));
    }

    let mut archive = SimpleZipArchive::open(&source_path)?;
    let mut session_count = 0usize;
    let mut project_sessions: HashMap<String, usize> = HashMap::new();

    while let Some(entry) = archive.next_entry()? {
        if !migration_entry_is_session_file(&entry.name) {
            continue;
        }
        session_count += 1;
        if let Some(cwd) = extract_session_cwd_from_jsonl_bytes(&entry.bytes) {
            *project_sessions.entry(cwd).or_insert(0) += 1;
        }
    }

    let mut missing_projects = project_sessions
        .iter()
        .filter(|(cwd, _)| !Path::new(cwd.as_str()).exists())
        .map(|(cwd, count)| MigrationMissingProjectSummary {
            cwd: cwd.clone(),
            session_count: *count,
        })
        .collect::<Vec<_>>();
    missing_projects.sort_by(|left, right| left.cwd.cmp(&right.cwd));
    let affected_session_count = missing_projects
        .iter()
        .map(|item| item.session_count)
        .sum::<usize>();
    let missing_project_count = missing_projects.len();
    let project_count = project_sessions.len();

    Ok(MigrationBackupInspectionResult {
        session_count,
        project_count,
        missing_project_count,
        affected_session_count,
        missing_projects,
        message: if missing_project_count == 0 {
            format!(
                "会话项目目录检查通过：{} 个会话，{} 个项目。",
                session_count, project_count
            )
        } else {
            format!(
                "发现 {} 个项目目录不存在，影响 {} 个会话。",
                missing_project_count, affected_session_count
            )
        },
    })
}
#[tauri::command]
pub fn import_migration_backup(
    request: ImportMigrationBackupRequest,
) -> Result<MigrationRestoreResult, String> {
    let source_path = PathBuf::from(request.source_path.trim());
    if source_path.as_os_str().is_empty() {
        return Err("\u{8bf7}\u{9009}\u{62e9}\u{8981}\u{5bfc}\u{5165}\u{7684}\u{8fc1}\u{79fb}\u{5907}\u{4efd} ZIP\u{3002}".to_string());
    }
    if !source_path.exists() || !source_path.is_file() {
        return Err(format!(
            "\u{8fc1}\u{79fb}\u{5907}\u{4efd}\u{6587}\u{4ef6}\u{4e0d}\u{5b58}\u{5728}\u{ff1a}{}",
            source_path.display()
        ));
    }

    import_migration_backup_transactional(&source_path)
}
#[tauri::command]
pub fn check_latest_version() -> Result<LatestVersionCheckResult, String> {
    let mut settings = read_app_settings()?;
    let current_version = default_system_version();
    let latest_asset = fetch_latest_codexhub_msi_asset()?;
    let latest_version = latest_asset.version.clone();
    let update_available = is_version_newer(&latest_version, &current_version);
    let message = if update_available {
        format!(
            "检测到新版本 {}，当前记录版本 {}。",
            latest_version, current_version
        )
    } else {
        format!("当前已是最新版本 {}。", current_version)
    };

    if settings.system_version != current_version {
        settings.system_version = current_version.clone();
        save_app_settings(&settings)?;
    }

    Ok(LatestVersionCheckResult {
        current_version,
        latest_version,
        update_available,
        asset_name: Some(latest_asset.asset_name),
        download_url: Some(latest_asset.download_url),
        release_page_url: Some(latest_asset.release_page_url),
        message,
    })
}
#[tauri::command]
pub fn read_app_settings() -> Result<AppSettings, String> {
    ensure_app_settings_file()?;
    let mut settings = load_app_settings()?;
    let normalized_activation_time = normalize_app_activation_time(&settings.activation_time)
        .unwrap_or_else(current_app_activation_time);
    if settings.activation_time != normalized_activation_time {
        settings.activation_time = normalized_activation_time;
        save_app_settings(&settings)?;
    }
    if settings.system_version != default_system_version() {
        settings.system_version = default_system_version();
        save_app_settings(&settings)?;
    }
    let normalized_refresh_seconds =
        normalize_account_usage_refresh_seconds(settings.account_usage_refresh_seconds);
    if settings.account_usage_refresh_seconds != normalized_refresh_seconds {
        settings.account_usage_refresh_seconds = normalized_refresh_seconds;
        save_app_settings(&settings)?;
    }

    let normalized_router_port = normalize_port(settings.router_port, default_router_port());
    let normalized_oauth_callback_port =
        normalize_port(settings.oauth_callback_port, default_oauth_callback_port());
    let normalized_account_proxy = normalize_account_proxy_settings(
        settings.account_proxy.clone(),
        normalized_oauth_callback_port,
    );
    if settings.router_port != normalized_router_port
        || settings.oauth_callback_port != normalized_oauth_callback_port
        || settings.account_proxy.account_proxy_url != normalized_account_proxy.account_proxy_url
        || settings.account_proxy.api_key != normalized_account_proxy.api_key
    {
        settings.router_port = normalized_router_port;
        settings.oauth_callback_port = normalized_oauth_callback_port;
        settings.account_proxy = normalized_account_proxy;
        save_app_settings(&settings)?;
    }

    if refresh_invalid_codex_exe_path(&mut settings)? {
        save_app_settings(&settings)?;
    }

    Ok(settings)
}

#[tauri::command]
pub fn load_router_config_command() -> Result<RouterConfig, String> {
    load_router_config_file()
}

#[tauri::command]
pub fn save_router_config_command(config: RouterConfig) -> Result<RouterConfig, String> {
    save_router_config_file(config)
}
#[tauri::command]
pub fn open_external_url(request: OpenExternalUrlRequest) -> Result<(), String> {
    let url = request.url.trim();
    if !url.starts_with("https://auth.openai.com/oauth/authorize?")
        && !url.starts_with("https://chatgpt.com/")
        && url != "https://github.com/xiaoashuo/CodexHub"
        && !url.starts_with("https://github.com/xiaoashuo/CodexHub/releases")
    {
        return Err("拒绝打开未允许的外部链接。".to_string());
    }

    let escaped_url = url.replace('`', "``").replace('\'', "''");
    let mut command = hidden_command("powershell");
    command.args([
        "-NoProfile",
        "-WindowStyle",
        "Hidden",
        "-Command",
        &format!("Start-Process -FilePath '{}'", escaped_url),
    ]);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    command
        .spawn()
        .map_err(|error| format!("打开系统浏览器失败：{}", error))?;
    Ok(())
}
#[tauri::command]
pub async fn download_and_install_update(
    app_handle: tauri::AppHandle,
    request: UpdateInstallRequest,
) -> Result<UpdateInstallResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        download_and_install_update_blocking(app_handle, request)
    })
    .await
    .map_err(|error| format!("update task failed: {}", error))?
}
#[tauri::command]
pub fn cancel_update_download(app_handle: tauri::AppHandle) -> Result<(), String> {
    UPDATE_DOWNLOAD_CANCELED.store(true, Ordering::SeqCst);
    emit_update_progress(&app_handle, "canceled", 0, None, None, "Cancel requested");
    Ok(())
}
#[tauri::command]
pub fn write_app_settings(settings: AppSettingsInput) -> Result<AppSettings, String> {
    let activation_time = load_app_settings()
        .ok()
        .and_then(|current| normalize_app_activation_time(&current.activation_time))
        .or_else(|| normalize_app_activation_time(&settings.activation_time))
        .unwrap_or_else(current_app_activation_time);
    let normalized = AppSettings {
        system_version: default_system_version(),
        activation_time,
        codex_exe_path: settings.codex_exe_path.trim().to_string(),
        app_restart_target: normalize_restart_target(&settings.app_restart_target),
        official_proxy_url: settings.official_proxy_url.trim().to_string(),
        account_usage_refresh_seconds: normalize_account_usage_refresh_seconds(
            settings.account_usage_refresh_seconds,
        ),
        token_auto_renew_enabled: settings.token_auto_renew_enabled,
        router_port: normalize_port(settings.router_port, default_router_port()),
        router_concurrency_limit: normalize_router_concurrency_limit(
            settings.router_concurrency_limit,
        ),
        oauth_callback_port: normalize_port(
            settings.oauth_callback_port,
            default_oauth_callback_port(),
        ),
        router_debug_mode: settings.router_debug_mode,
        image_generation_compat_mode: settings.image_generation_compat_mode,
        account_proxy: normalize_account_proxy_settings(
            settings.account_proxy,
            normalize_port(settings.oauth_callback_port, default_oauth_callback_port()),
        ),
        router_name: settings.router_name.trim().to_string(),
        router_base_url: settings.router_base_url.trim().to_string(),
        router_auth_method: normalize_router_auth_method(&settings.router_auth_method),
        router_auth_external_token: settings.router_auth_external_token.trim().to_string(),
        router_auth_env_key: settings.router_auth_env_key.trim().to_string(),
        router_model_catalog_json: settings.router_model_catalog_json.trim().to_string(),
        router_default_model: settings.router_default_model.trim().to_string(),
        router_mode: normalize_router_mode(&settings.router_mode),
        router_auto_restart: settings.router_auto_restart,
        audit_request_enabled: settings.audit_request_enabled,
        audit_response_enabled: settings.audit_response_enabled,
    };
    save_app_settings(&normalized)?;
    read_app_settings()
}

#[tauri::command]
pub fn create_migration_backup() -> Result<MigrationBackupResult, String> {
    ensure_workspace_layout()?;

    let backup_dir = workspace_backup_path()?.join("migration");
    fs::create_dir_all(&backup_dir).map_err(|error| {
        format!(
            "创建迁移备份目录失败：{}，路径：{}",
            error,
            backup_dir.display()
        )
    })?;

    let backup_path = backup_dir.join(format!("codex-migration-{}.zip", current_log_time()));
    let backup_tmp_path = backup_path.with_extension("zip.tmp");
    if backup_tmp_path.exists() {
        let _ = fs::remove_file(&backup_tmp_path);
    }
    let mut backup_tmp_guard = TempPathGuard::new(backup_tmp_path.clone());
    let mut writer = SimpleZipWriter::create(&backup_tmp_path)?;
    let mut skipped_items = Vec::new();
    let mut included_sections = Vec::new();
    let mut file_times = HashMap::new();

    add_migration_section_file(
        &mut writer,
        &mut skipped_items,
        &mut file_times,
        "accounts/current-auth.json",
        codex_auth_path()?,
    )?;
    add_migration_section_file(
        &mut writer,
        &mut skipped_items,
        &mut file_times,
        "accounts/registry.json",
        codex_accounts_registry_path()?,
    )?;
    add_migration_section_dir(
        &mut writer,
        &mut skipped_items,
        &mut file_times,
        "accounts/snapshots",
        codex_accounts_snapshots_path()?,
    )?;
    included_sections.push("\u{8d26}\u{6237}".to_string());

    add_migration_section_file(
        &mut writer,
        &mut skipped_items,
        &mut file_times,
        "models/router_provider_config.json",
        provider_config_path()?,
    )?;
    included_sections.push("\u{6a21}\u{578b}".to_string());

    add_migration_section_dir(
        &mut writer,
        &mut skipped_items,
        &mut file_times,
        "sessions/sessions",
        codex_sessions_path()?,
    )?;
    add_migration_section_dir(
        &mut writer,
        &mut skipped_items,
        &mut file_times,
        "sessions/archived_sessions",
        codex_archived_sessions_path()?,
    )?;
    add_migration_section_file(
        &mut writer,
        &mut skipped_items,
        &mut file_times,
        "sessions/session_index.jsonl",
        codex_session_index_path()?,
    )?;
    add_migration_section_file(
        &mut writer,
        &mut skipped_items,
        &mut file_times,
        "sessions/codex-global-state.json",
        codex_global_state_path()?,
    )?;
    add_migration_section_file(
        &mut writer,
        &mut skipped_items,
        &mut file_times,
        "sessions/codex-global-state.json.bak",
        codex_global_state_backup_path()?,
    )?;
    included_sections.push("\u{4f1a}\u{8bdd}".to_string());

    add_migration_section_dir(
        &mut writer,
        &mut skipped_items,
        &mut file_times,
        "skills/installed",
        codex_skills_path()?,
    )?;
    add_migration_section_dir(
        &mut writer,
        &mut skipped_items,
        &mut file_times,
        "skills/backups",
        skill_backups_path()?,
    )?;
    included_sections.push("\u{6280}\u{80fd}".to_string());

    add_migration_mcp_config(&mut writer, &mut skipped_items)?;
    included_sections.push("MCP".to_string());

    add_migration_section_file(
        &mut writer,
        &mut skipped_items,
        &mut file_times,
        "app/app_settings.json",
        app_settings_path()?,
    )?;

    let file_times_text = serde_json::to_string_pretty(&file_times)
        .map_err(|error| format!("生成迁移备份文件时间元数据失败：{}", error))?;
    writer.add_bytes("metadata/file_times.json", file_times_text.as_bytes())?;

    let manifest = serde_json::json!({
        "createdAt": current_log_time(),
        "app": "codex-proxy",
        "formatVersion": 2,
        "includedSections": included_sections.clone(),
        "sensitive": true,
        "warning": "\u{6b64}\u{8fc1}\u{79fb}\u{5305}\u{53ef}\u{80fd}\u{5305}\u{542b} auth token\u{3001}Provider API Key\u{3001}MCP headers/env \u{4e0e}\u{4f1a}\u{8bdd}\u{5185}\u{5bb9}\u{ff0c}\u{8bf7}\u{53ea}\u{4fdd}\u{5b58}\u{5230}\u{53ef}\u{4fe1}\u{4f4d}\u{7f6e}\u{3002}",
        "restoreNote": "\u{6062}\u{590d}\u{65f6}\u{4f1a}\u{6309}\u{767d}\u{540d}\u{5355}\u{5bfc}\u{5165}\u{5e76}\u{5408}\u{5e76} MCP\u{ff0c}\u{4e0d}\u{8986}\u{76d6}\u{5b8c}\u{6574} config.toml\u{3002}"
    });
    let manifest_text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("生成迁移备份 manifest 失败：{}", error))?;
    writer.add_bytes("manifest.json", manifest_text.as_bytes())?;

    let stats = match writer.finish() {
        Ok(stats) => stats,
        Err(error) => {
            let _ = fs::remove_file(&backup_tmp_path);
            return Err(error);
        }
    };
    fs::rename(&backup_tmp_path, &backup_path).map_err(|error| {
        let _ = fs::remove_file(&backup_tmp_path);
        format!(
            "提交迁移备份 ZIP 失败：{}，路径：{}",
            error,
            backup_path.display()
        )
    })?;
    backup_tmp_guard.keep();

    Ok(MigrationBackupResult {
        backup_path: backup_path.display().to_string(),
        file_count: stats.file_count,
        total_bytes: stats.total_bytes,
        included_sections,
        skipped_items,
        message: format!(
            "\u{8fc1}\u{79fb}\u{5907}\u{4efd}\u{5df2}\u{751f}\u{6210}\u{ff1a}{} \u{4e2a}\u{6587}\u{4ef6}\u{ff0c}{}\u{3002}",
            stats.file_count,
            format_bytes(stats.total_bytes)
        ),
    })
}

#[tauri::command]
pub fn local_config_paths() -> Result<LocalConfigPaths, String> {
    Ok(LocalConfigPaths {
        user_home_path: user_home_path()?.display().to_string(),
        codex_config_path: codex_config_path()?.display().to_string(),
        catalog_path: catalog_config_path()?.display().to_string(),
        provider_config_path: provider_config_path()?.display().to_string(),
        app_settings_path: app_settings_path()?.display().to_string(),
        router_config_path: build_user_relative_path(ROUTER_CONFIG_RELATIVE_PATH)?.display().to_string(),
        app_log_path: app_log_path()?.display().to_string(),
        router_debug_log_path: router_debug_log_path()?.display().to_string(),
    })
}
