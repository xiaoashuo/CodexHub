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
pub fn load_installed_skills() -> Result<SkillListResult, String> {
    let skills_dir = codex_skills_path()?;
    let items = scan_installed_skills(&skills_dir)?;

    Ok(SkillListResult {
        total: items.len(),
        root_path: skills_dir.display().to_string(),
        items,
    })
}
#[tauri::command]
pub fn load_codex_plugins() -> Result<PluginListResult, String> {
    load_codex_plugins_result()
}
#[tauri::command]
pub fn set_codex_plugin_enabled(
    request: SetCodexPluginEnabledRequest,
) -> Result<PluginListResult, String> {
    let mut state = read_codex_plugin_state()?;
    if request.enabled {
        state.disabled_plugins.remove(&request.id);
    } else {
        state.disabled_plugins.insert(request.id);
    }
    write_codex_plugin_state(&state)?;
    load_codex_plugins_result()
}
#[tauri::command]
pub fn set_codex_plugin_skill_enabled(
    request: SetCodexPluginSkillEnabledRequest,
) -> Result<PluginListResult, String> {
    let mut state = read_codex_plugin_state()?;
    if request.enabled {
        state.disabled_skills.remove(&request.full_name);
    } else {
        state.disabled_skills.insert(request.full_name);
    }
    write_codex_plugin_state(&state)?;
    load_codex_plugins_result()
}
#[tauri::command]
pub fn load_skill_backups() -> Result<SkillBackupListResult, String> {
    let backup_dir = skill_backups_path()?;
    let items = scan_skill_backups(&backup_dir)?;

    Ok(SkillBackupListResult {
        total: items.len(),
        root_path: backup_dir.display().to_string(),
        items,
    })
}
#[tauri::command]
pub fn import_skill(request: ImportSkillRequest) -> Result<SkillImportResult, String> {
    let skills_dir = codex_skills_path()?;
    let backup_dir = skill_backups_path()?;
    fs::create_dir_all(&skills_dir).map_err(|error| {
        format!(
            "创建 Skills 目录失败：{}，路径：{}",
            error,
            skills_dir.display()
        )
    })?;
    fs::create_dir_all(&backup_dir).map_err(|error| {
        format!(
            "创建 Skills 备份目录失败：{}，路径：{}",
            error,
            backup_dir.display()
        )
    })?;

    let source = resolve_skill_source(Path::new(&request.source_path))?;
    let target = skills_dir.join(
        source
            .file_name()
            .ok_or_else(|| "无法识别 Skill 目录名。".to_string())?,
    );
    let source_canonical = fs::canonicalize(&source).unwrap_or_else(|_| source.clone());
    let target_canonical = fs::canonicalize(&target).unwrap_or_else(|_| target.clone());

    if source_canonical == target_canonical {
        let skill = load_skill_summary(&target.join("SKILL.md"), &skills_dir)
            .ok_or_else(|| "Skill 来源无效。".to_string())?;
        return Ok(SkillImportResult {
            skill,
            replaced_existing: false,
            backup: None,
        });
    }

    let replaced_existing = target.exists();
    let backup = if replaced_existing {
        let backup = backup_skill_directory(&target, &skills_dir, &backup_dir, "replace")?;
        fs::remove_dir_all(&target)
            .map_err(|error| format!("移除旧 Skill 失败：{}，路径：{}", error, target.display()))?;
        Some(backup)
    } else {
        None
    };

    copy_dir_all(&source, &target)?;
    let skill = load_skill_summary(&target.join("SKILL.md"), &skills_dir)
        .ok_or_else(|| "导入后的 Skill 缺少 SKILL.md。".to_string())?;

    Ok(SkillImportResult {
        skill,
        replaced_existing,
        backup,
    })
}

#[tauri::command]
pub fn remove_skill(request: SkillIdRequest) -> Result<SkillRemoveResult, String> {
    let skills_dir = codex_skills_path()?;
    let backup_dir = skill_backups_path()?;
    let skill = scan_installed_skills(&skills_dir)?
        .into_iter()
        .find(|item| item.id == request.id)
        .ok_or_else(|| format!("Skill 不存在：{}", request.id))?;
    let dir = PathBuf::from(&skill.directory_path);
    let backup = backup_skill_directory(&dir, &skills_dir, &backup_dir, "remove")?;

    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|error| format!("移除 Skill 失败：{}，路径：{}", error, dir.display()))?;
    }

    Ok(SkillRemoveResult {
        removed_skill_id: request.id,
        backup,
        remaining_installed_count: scan_installed_skills(&skills_dir)?.len(),
    })
}
#[tauri::command]
pub fn restore_skill_backup(request: SkillIdRequest) -> Result<SkillRestoreResult, String> {
    let skills_dir = codex_skills_path()?;
    let backup_dir = skill_backups_path()?;
    let backup_root = backup_dir.join(&request.id);
    let metadata_path = backup_root.join("metadata.json");
    let metadata_text = fs::read_to_string(&metadata_path).map_err(|error| {
        format!(
            "读取 Skill 备份元数据失败：{}，路径：{}",
            error,
            metadata_path.display()
        )
    })?;
    let metadata: SkillBackupMetadata = serde_json::from_str(&metadata_text)
        .map_err(|error| format!("解析 Skill 备份元数据失败：{}", error))?;
    let staged = backup_root.join("skill");
    if !staged.exists() {
        return Err(format!("Skill 备份已损坏：{}", request.id));
    }

    let target = skills_dir.join(&metadata.relative_path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!("创建 Skill 目录失败：{}，路径：{}", error, parent.display())
        })?;
    }

    let rollback_backup = if target.exists() {
        let backup = backup_skill_directory(&target, &skills_dir, &backup_dir, "restore-rollback")?;
        fs::remove_dir_all(&target).map_err(|error| {
            format!("移除当前 Skill 失败：{}，路径：{}", error, target.display())
        })?;
        Some(backup)
    } else {
        None
    };

    copy_dir_all(&staged, &target)?;
    let restored_skill = load_skill_summary(&target.join("SKILL.md"), &skills_dir)
        .ok_or_else(|| "恢复后的 Skill 缺少 SKILL.md。".to_string())?;
    let backup = SkillBackupSummary {
        id: metadata.backup_id,
        skill_id: metadata.skill_id,
        name: metadata.name,
        title: metadata.title,
        relative_path: metadata.relative_path,
        backup_path: staged.display().to_string(),
        created_at: metadata.created_at,
    };

    Ok(SkillRestoreResult {
        restored_skill,
        backup,
        rollback_backup,
    })
}

#[tauri::command]
pub fn delete_skill_backup(request: SkillIdRequest) -> Result<SkillBackupListResult, String> {
    let backup_dir = skill_backups_path()?;
    let target = backup_dir.join(&request.id);
    if target.exists() {
        fs::remove_dir_all(&target).map_err(|error| {
            format!("删除 Skill 备份失败：{}，路径：{}", error, target.display())
        })?;
    }
    load_skill_backups()
}
