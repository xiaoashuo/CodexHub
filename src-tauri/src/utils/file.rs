use crate::*;
use crate::constants::*;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn ensure_json_file(path: &PathBuf) -> Result<(), String> {
    ensure_parent_dir(path)?;

    if !path.exists() {
        fs::write(path, EMPTY_JSON_OBJECT_CONTENT).map_err(|error| {
            format!(
                "鍒涘缓 JSON 閰嶇疆鏂囦欢澶辫触：{}锛岃矾寰勶細{}",
                error,
                path.display()
            )
        })?;
    }

    Ok(())
}

pub(crate) fn provider_config_path() -> Result<PathBuf, String> {
    build_user_relative_path(CONFIG_RELATIVE_PATH)
}

pub(crate) fn workspace_backup_sessions_path() -> Result<PathBuf, String> {
    Ok(workspace_backup_path()?.join("sessions"))
}

pub(crate) fn model_config_backups_path() -> Result<PathBuf, String> {
    Ok(workspace_backup_path()?.join("models"))
}

pub(crate) fn backup_provider_config_before_import(path: &Path) -> Result<Option<PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let backup_dir = model_config_backups_path()?;
    fs::create_dir_all(&backup_dir).map_err(|error| {
        format!(
            "创建模型备份目录失败：{}，路径：{}",
            error,
            backup_dir.display()
        )
    })?;
    let backup_path = backup_dir.join(format!(
        "router_provider_config-before-import-{}.json",
        current_log_time()
    ));
    fs::copy(path, &backup_path).map_err(|error| {
        format!(
            "备份当前模型配置失败：{}，源路径：{}，目标路径：{}",
            error,
            path.display(),
            backup_path.display()
        )
    })?;

    Ok(Some(backup_path))
}

pub(crate) fn workspace_backup_path() -> Result<PathBuf, String> {
    build_user_relative_path(WORKSPACE_BACKUP_RELATIVE_PATH)
}

pub(crate) fn ensure_workspace_layout() -> Result<(), String> {
    for relative_path in [
        WORKSPACE_RELATIVE_PATH,
        WORKSPACE_CONFIG_RELATIVE_PATH,
        WORKSPACE_LOGS_APP_RELATIVE_PATH,
        WORKSPACE_LOGS_ROUTER_RELATIVE_PATH,
        WORKSPACE_BACKUP_RELATIVE_PATH,
        WORKSPACE_CACHE_RELATIVE_PATH,
        WORKSPACE_RUNTIME_RELATIVE_PATH,
        WORKSPACE_ACCOUNTS_RELATIVE_PATH,
    ] {
        let path = build_user_relative_path(relative_path)?;
        fs::create_dir_all(&path).map_err(|error| {
            format!(
                "创建 ai-router workspace 目录失败：{}，路径：{}",
                error,
                path.display()
            )
        })?;
    }

    fs::create_dir_all(workspace_backup_sessions_path()?)
        .map_err(|error| format!("创建会话备份目录失败：{}", error))?;
    fs::create_dir_all(codex_accounts_backups_path()?)
        .map_err(|error| format!("创建账号备份目录失败：{}", error))?;
    fs::create_dir_all(codex_accounts_snapshots_path()?)
        .map_err(|error| format!("创建账号快照目录失败：{}", error))?;

    Ok(())
}

pub(crate) fn models_cache_path() -> Result<PathBuf, String> {
    build_user_relative_path(MODELS_CACHE_RELATIVE_PATH)
}

pub(crate) fn catalog_base_config_path() -> Result<PathBuf, String> {
    build_user_relative_path(CATALOG_BASE_RELATIVE_PATH)
}

pub(crate) fn catalog_config_path() -> Result<PathBuf, String> {
    build_user_relative_path(CATALOG_RELATIVE_PATH)
}

pub(crate) fn app_log_path() -> Result<PathBuf, String> {
    build_user_relative_path(APP_LOG_RELATIVE_PATH)
}

pub(crate) fn account_proxy_log_path() -> Result<PathBuf, String> {
    build_user_relative_path(ACCOUNT_PROXY_LOG_RELATIVE_PATH)
}

pub(crate) fn router_log_path() -> Result<PathBuf, String> {
    build_user_relative_path(ROUTER_LOG_RELATIVE_PATH)
}

pub(crate) fn router_debug_log_path() -> Result<PathBuf, String> {
    build_user_relative_path(ROUTER_DEBUG_LOG_RELATIVE_PATH)
}

pub(crate) fn router_full_debug_log_path() -> Result<PathBuf, String> {
    build_user_relative_path(ROUTER_FULL_DEBUG_LOG_RELATIVE_PATH)
}

pub(crate) fn app_settings_path() -> Result<PathBuf, String> {
    build_user_relative_path(APP_SETTINGS_RELATIVE_PATH)
}

pub(crate) fn codex_config_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_CONFIG_RELATIVE_PATH)
}

pub(crate) fn codex_auth_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_AUTH_RELATIVE_PATH)
}

pub(crate) fn codex_plugins_cache_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_PLUGINS_CACHE_RELATIVE_PATH)
}

pub(crate) fn codex_plugin_state_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_PLUGIN_STATE_RELATIVE_PATH)
}

#[allow(dead_code)]
pub(crate) fn codex_global_state_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_GLOBAL_STATE_RELATIVE_PATH)
}

#[allow(dead_code)]
pub(crate) fn codex_global_state_backup_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_GLOBAL_STATE_BACKUP_RELATIVE_PATH)
}

pub(crate) fn codex_accounts_registry_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_ACCOUNTS_REGISTRY_RELATIVE_PATH)
}

pub(crate) fn codex_accounts_backups_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_ACCOUNTS_BACKUPS_RELATIVE_PATH)
}

pub(crate) fn codex_accounts_snapshots_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_ACCOUNTS_SNAPSHOTS_RELATIVE_PATH)
}

pub(crate) fn codex_sessions_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_SESSIONS_RELATIVE_PATH)
}

pub(crate) fn codex_archived_sessions_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_ARCHIVED_SESSIONS_RELATIVE_PATH)
}

pub(crate) fn codex_session_index_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_SESSION_INDEX_RELATIVE_PATH)
}

pub(crate) fn codex_state_db_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_STATE_DB_RELATIVE_PATH)
}

pub(crate) fn codex_skills_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_SKILLS_RELATIVE_PATH)
}

pub(crate) fn skill_backups_path() -> Result<PathBuf, String> {
    Ok(workspace_backup_path()?.join("skills"))
}

pub(crate) fn canonicalize_existing_dir(path: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Ok(path.to_path_buf());
    }

    path.canonicalize()
        .map_err(|error| format!("解析目录路径失败：{}，路径：{}", error, path.display()))
}

pub(crate) fn codex_config_backup_path() -> Result<PathBuf, String> {
    build_user_relative_path(CODEX_CONFIG_BACKUP_RELATIVE_PATH)
}

pub(crate) fn build_user_relative_path(relative_path: &[&str]) -> Result<PathBuf, String> {
    let mut path = user_home_path()?;

    for segment in relative_path {
        path.push(segment);
    }

    Ok(path)
}

pub(crate) fn remove_dir_contents(dir: &Path, label: &str) -> Result<CleanCount, String> {
    assert_safe_cleanup_root(dir)?;

    if !dir.exists() {
        return Ok(CleanCount::default());
    }

    let mut result = CleanCount::default();
    let entries = fs::read_dir(dir)
        .map_err(|error| format!("读取{}失败：{}，路径：{}", label, error, dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("读取{}条目失败：{}", label, error))?;
        let path = entry.path();
        let size = path_total_size(&path);

        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(|error| {
                format!("清理{}失败：{}，路径：{}", label, error, path.display())
            })?;
        } else {
            fs::remove_file(&path).map_err(|error| {
                format!("清理{}失败：{}，路径：{}", label, error, path.display())
            })?;
        }

        result.count += 1;
        result.bytes += size;
    }

    Ok(result)
}

pub(crate) fn remove_file_counted(path: &Path, label: &str) -> Result<CleanCount, String> {
    let bytes = fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    fs::remove_file(path)
        .map_err(|error| format!("清理{}失败：{}，路径：{}", label, error, path.display()))?;
    Ok(CleanCount { count: 1, bytes })
}

pub(crate) fn path_total_size(path: &Path) -> u64 {
    let Ok(metadata) = fs::metadata(path) else {
        return 0;
    };

    if metadata.is_file() {
        return metadata.len();
    }

    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| path_total_size(&entry.path()))
        .sum()
}

pub(crate) fn assert_safe_cleanup_root(path: &Path) -> Result<(), String> {
    let workspace = workspace_backup_path()?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "无法解析工作区目录".to_string())?;
    let codex_snapshots = codex_accounts_snapshots_path()?;
    let allowed_roots = [workspace, codex_snapshots];
    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    if allowed_roots.iter().any(|root| {
        let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        canonical_path.starts_with(canonical_root)
    }) {
        return Ok(());
    }

    Err(format!("拒绝清理非应用工作区路径：{}", path.display()))
}

pub(crate) fn read_json_file_optional(path: &Path) -> Option<serde_json::Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str::<serde_json::Value>(&text).ok()
}

pub(crate) fn user_home_path() -> Result<PathBuf, String> {
    env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| "无法读取用户目录环境变量 USERPROFILE/HOME".to_string())
}

pub(crate) fn copy_dir_all(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target)
        .map_err(|error| format!("创建目录失败：{}，路径：{}", error, target.display()))?;
    let entries = fs::read_dir(source)
        .map_err(|error| format!("读取目录失败：{}，路径：{}", error, source.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("读取目录项失败：{}", error))?;
        let path = entry.path();
        let dest = target.join(entry.file_name());
        if path.is_dir() {
            copy_dir_all(&path, &dest)?;
        } else {
            fs::copy(&path, &dest).map_err(|error| {
                format!(
                    "复制文件失败：{}，来源：{}，目标：{}",
                    error,
                    path.display(),
                    dest.display()
                )
            })?;
        }
    }
    Ok(())
}

pub(crate) fn ensure_parent_dir(path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("创建目录失败：{}，路径：{}", error, parent.display()))?;
    }

    Ok(())
}

