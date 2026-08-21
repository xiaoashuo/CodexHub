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

mod app;
mod codex_protocol;
mod constants;
mod provider_protocol;
mod router_dispatcher;
mod router_config;
use constants::*;
use provider_protocol::ProviderProtocol;
use router_dispatcher::DispatchCandidate;
use utils::sqlite::insert_audit_log;

mod accounts;
mod logs;
mod mcps;
mod models;
mod overview;
mod routes;
mod sessions;
mod settings;
mod skills;
mod utils;

pub use accounts::*;
pub use logs::*;
pub use mcps::*;
pub use models::*;
pub use overview::*;
pub use routes::*;
pub use sessions::*;
pub use settings::*;
pub use skills::*;
pub use utils::file::*;
pub use utils::http::*;
pub use utils::json::*;
pub use utils::mask::*;
pub use utils::time::*;

pub use utils::url::*;

struct RouterRuntime {
    started: bool,
    started_at: Option<Instant>,
    stop_signal: Option<Arc<Mutex<bool>>>,
    handle: Option<JoinHandle<()>>,
    pid: Option<u32>,
    port: u16,
}

struct CodexOAuthLoginState {
    state: String,
    code_verifier: String,
    created_at: Instant,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexOAuthLoginStatus {
    status: String,
    message: String,
    account_key: Option<String>,
    account_email: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexOAuthCallbackListenerStatus {
    running: bool,
    host: String,
    port: u16,
    callback_url: String,
    message: String,
}

#[derive(serde::Serialize)]
struct RouterCommandResult {
    status: String,
    service: String,
    version: String,
    host: String,
    port: u16,
    pid: Option<u32>,
    health_path: String,
    health_url: String,
    uptime_seconds: u64,
    started: bool,
    forwarding_enabled: bool,
    concurrency_limit: usize,
}

#[derive(serde::Serialize)]
struct PortOccupancyInfo {
    occupied: bool,
    host: String,
    port: u16,
    pid: Option<u32>,
    process_name: String,
    process_path: String,
}

#[derive(serde::Serialize)]
struct RouterStartupPreparationResult {
    router_mode: i32,
    codex_config_path: String,
    catalog_path: String,
    provider_config_path: String,
    sync_catalog_result: SyncCatalogResult,
    port_occupancy: PortOccupancyInfo,
    killed_port_owner: bool,
}

#[derive(serde::Deserialize)]
struct RouterStartupPreparationRequest {
    router_mode: i32,
}

#[derive(serde::Serialize)]
struct LocalConfigPaths {
    user_home_path: String,
    codex_config_path: String,
    catalog_path: String,
    provider_config_path: String,
    app_settings_path: String,
    router_config_path: String,
    app_log_path: String,
    router_debug_log_path: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpServerSummary {
    name: String,
    transport: String,
    enabled: bool,
    source_path: String,
    command: Option<String>,
    args: Vec<String>,
    url: Option<String>,
    headers: HashMap<String, String>,
    environment: HashMap<String, String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct McpServerListResult {
    total: usize,
    source_path: String,
    items: Vec<McpServerSummary>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpsertMcpServerRequest {
    name: String,
    transport: String,
    enabled: bool,
    command: Option<String>,
    args: Vec<String>,
    url: Option<String>,
    headers: HashMap<String, String>,
    environment: HashMap<String, String>,
}

#[derive(serde::Deserialize)]
struct SetMcpServerEnabledRequest {
    name: String,
    enabled: bool,
}

#[derive(serde::Deserialize)]
struct RemoveMcpServerRequest {
    name: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportMigrationBackupRequest {
    source_path: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct InstalledSkillSummary {
    id: String,
    name: String,
    title: Option<String>,
    summary: Option<String>,
    relative_path: String,
    directory_path: String,
    skill_file_path: String,
    updated_at: Option<i64>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillListResult {
    total: usize,
    root_path: String,
    items: Vec<InstalledSkillSummary>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginSkillSummary {
    id: String,
    name: String,
    full_name: String,
    title: Option<String>,
    summary: Option<String>,
    enabled: bool,
    relative_path: String,
    directory_path: String,
    skill_file_path: String,
    updated_at: Option<i64>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexPluginSummary {
    id: String,
    name: String,
    display_name: String,
    source: String,
    version: String,
    description: Option<String>,
    short_description: Option<String>,
    developer_name: Option<String>,
    category: Option<String>,
    enabled: bool,
    directory_path: String,
    manifest_path: String,
    skill_count: usize,
    skills: Vec<PluginSkillSummary>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginListResult {
    total: usize,
    root_path: String,
    items: Vec<CodexPluginSummary>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetCodexPluginEnabledRequest {
    id: String,
    enabled: bool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetCodexPluginSkillEnabledRequest {
    full_name: String,
    enabled: bool,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexPluginState {
    #[serde(default)]
    disabled_plugins: HashSet<String>,
    #[serde(default)]
    disabled_skills: HashSet<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillBackupMetadata {
    backup_id: String,
    skill_id: String,
    name: String,
    title: Option<String>,
    relative_path: String,
    created_at: i64,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillBackupSummary {
    id: String,
    skill_id: String,
    name: String,
    title: Option<String>,
    relative_path: String,
    backup_path: String,
    created_at: i64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillBackupListResult {
    total: usize,
    root_path: String,
    items: Vec<SkillBackupSummary>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportSkillRequest {
    source_path: String,
}

#[derive(serde::Deserialize)]
struct SkillIdRequest {
    id: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillImportResult {
    skill: InstalledSkillSummary,
    replaced_existing: bool,
    backup: Option<SkillBackupSummary>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillRemoveResult {
    removed_skill_id: String,
    backup: SkillBackupSummary,
    remaining_installed_count: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillRestoreResult {
    restored_skill: InstalledSkillSummary,
    backup: SkillBackupSummary,
    rollback_backup: Option<SkillBackupSummary>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MaintenanceCleanResult {
    message: String,
    backup_deleted_count: usize,
    backup_deleted_bytes: u64,
    cache_deleted_count: usize,
    cache_deleted_bytes: u64,
    invalid_snapshot_deleted_count: usize,
    invalid_snapshot_deleted_bytes: u64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationBackupResult {
    backup_path: String,
    file_count: usize,
    total_bytes: u64,
    included_sections: Vec<String>,
    skipped_items: Vec<String>,
    message: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationRestoreResult {
    restored_count: usize,
    restored_bytes: u64,
    backup_path: String,
    restored_sections: Vec<String>,
    skipped_items: Vec<String>,
    message: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationBackupInspectionResult {
    session_count: usize,
    project_count: usize,
    missing_project_count: usize,
    affected_session_count: usize,
    missing_projects: Vec<MigrationMissingProjectSummary>,
    message: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationMissingProjectSummary {
    cwd: String,
    session_count: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LatestVersionCheckResult {
    current_version: String,
    latest_version: String,
    update_available: bool,
    asset_name: Option<String>,
    download_url: Option<String>,
    release_page_url: Option<String>,
    message: String,
}

struct ReleaseMsiAsset {
    version: String,
    asset_name: String,
    download_url: String,
    release_page_url: String,
}

#[derive(Default)]
struct CleanCount {
    count: usize,
    bytes: u64,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct RouterLogEntry {
    time: String,
    source_ip: String,
    method: String,
    path: String,
    status: String,
    target_provider: String,
    cost: String,
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
    #[serde(default)]
    usage_source: String,
    error_detail: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct AccountProxyLogEntry {
    time: String,
    source_ip: String,
    method: String,
    path: String,
    protocol: String,
    model: String,
    stream: bool,
    status: String,
    cost: String,
    account: String,
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
    #[serde(default)]
    usage_source: String,
    error_detail: String,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct TokenUsage {
    input_tokens: u64,
    output_tokens: u64,
    cached_input_tokens: u64,
    reasoning_tokens: u64,
    total_tokens: u64,
}

#[derive(Default, serde::Serialize)]
struct TokenUsageSummary {
    router_input_tokens: u64,
    router_output_tokens: u64,
    router_cached_input_tokens: u64,
    account_proxy_input_tokens: u64,
    account_proxy_output_tokens: u64,
    account_proxy_cached_input_tokens: u64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardQuickCounts {
    account_count: usize,
    skill_count: usize,
    mcp_total: usize,
    mcp_enabled: usize,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AppOperationLogEntry {
    id: String,
    time: String,
    level: String,
    module: String,
    action: String,
    message: String,
    detail: Option<String>,
}

struct RecentAppOperationLog {
    level: String,
    module: String,
    action: String,
    message: String,
    detail: Option<String>,
    at: Instant,
}

enum RouterLogWriteTask {
    RouterLog(RouterLogEntry),
    AccountProxy(AccountProxyLogEntry),
}

#[derive(serde::Deserialize)]
struct AppOperationLogInput {
    level: String,
    module: String,
    action: String,
    message: String,
    detail: Option<String>,
}

#[derive(serde::Deserialize)]
struct AppLogQuery {
    keyword: String,
    level: String,
    limit: usize,
}

#[derive(serde::Serialize)]
struct AppLogFileInfo {
    path: String,
    size: u64,
    max_size: u64,
    count: usize,
}

#[derive(serde::Serialize)]
struct FilePreviewResult {
    path: String,
    exists: bool,
    content: String,
    truncated: bool,
}

#[derive(serde::Deserialize)]
struct FilePreviewRequest {
    path: String,
}

#[derive(serde::Deserialize)]
struct DeleteCodexThreadFilesRequest {
    #[serde(rename = "filePaths")]
    file_paths: Vec<String>,
}

#[derive(Clone, serde::Deserialize)]
struct RestoreCodexThreadIndexRequest {
    #[serde(rename = "filePaths")]
    file_paths: Vec<String>,
    #[serde(rename = "restoreAll", default)]
    restore_all: bool,
    #[serde(rename = "allowCodexRestart", default)]
    allow_codex_restart: bool,
    #[serde(rename = "moveToRecent", default)]
    move_to_recent: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RestoreCodexThreadIndexCheckResult {
    restore_count: usize,
    skipped_count: usize,
    requires_codex_restart: bool,
    codex_running: bool,
    project_roots: Vec<String>,
    message: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RestoreCodexThreadIndexResult {
    restored_count: usize,
    skipped_count: usize,
    backup_path: Option<String>,
    message: String,
    scan: ThreadScanResult,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct AccountProxySettings {
    #[serde(default)]
    account_proxy_enabled: bool,
    #[serde(default)]
    account_proxy_url: String,
    #[serde(default = "default_account_proxy_api_key")]
    api_key: String,
}

impl Default for AccountProxySettings {
    fn default() -> Self {
        Self {
            account_proxy_enabled: false,
            account_proxy_url: default_account_proxy_url(default_oauth_callback_port()),
            api_key: default_account_proxy_api_key(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AppSettings {
    #[serde(default = "default_system_version")]
    system_version: String,
    #[serde(default)]
    activation_time: String,
    #[serde(default)]
    codex_exe_path: String,
    #[serde(default = "default_restart_target")]
    app_restart_target: String,
    #[serde(default)]
    official_proxy_url: String,
    #[serde(default = "default_account_usage_refresh_seconds")]
    account_usage_refresh_seconds: u64,
    #[serde(default)]
    token_auto_renew_enabled: bool,
    #[serde(default = "default_router_port")]
    router_port: u16,
    #[serde(default = "default_router_concurrency_limit")]
    router_concurrency_limit: usize,
    #[serde(default = "default_oauth_callback_port")]
    oauth_callback_port: u16,
    #[serde(default)]
    router_debug_mode: bool,
    #[serde(default)]
    image_generation_compat_mode: bool,
    #[serde(default)]
    account_proxy: AccountProxySettings,
    #[serde(default = "default_router_name")]
    router_name: String,
    #[serde(default)]
    router_base_url: String,
    #[serde(default = "default_router_auth_method")]
    router_auth_method: String,
    #[serde(default)]
    router_auth_external_token: String,
    #[serde(default)]
    router_auth_env_key: String,
    #[serde(default)]
    router_model_catalog_json: String,
    #[serde(default)]
    router_default_model: String,
    #[serde(default = "default_router_mode")]
    router_mode: String,
    #[serde(default)]
    router_auto_restart: bool,
}

#[derive(serde::Deserialize)]
struct AppSettingsInput {
    #[serde(default)]
    activation_time: String,
    codex_exe_path: String,
    #[serde(default = "default_restart_target")]
    app_restart_target: String,
    #[serde(default)]
    official_proxy_url: String,
    #[serde(default = "default_account_usage_refresh_seconds")]
    account_usage_refresh_seconds: u64,
    #[serde(default)]
    token_auto_renew_enabled: bool,
    #[serde(default = "default_router_port")]
    router_port: u16,
    #[serde(default = "default_router_concurrency_limit")]
    router_concurrency_limit: usize,
    #[serde(default = "default_oauth_callback_port")]
    oauth_callback_port: u16,
    #[serde(default)]
    router_debug_mode: bool,
    #[serde(default)]
    image_generation_compat_mode: bool,
    #[serde(default)]
    account_proxy: AccountProxySettings,
    #[serde(default = "default_router_name")]
    router_name: String,
    #[serde(default)]
    router_base_url: String,
    #[serde(default = "default_router_auth_method")]
    router_auth_method: String,
    #[serde(default)]
    router_auth_external_token: String,
    #[serde(default)]
    router_auth_env_key: String,
    #[serde(default)]
    router_model_catalog_json: String,
    #[serde(default)]
    router_default_model: String,
    #[serde(default = "default_router_mode")]
    router_mode: String,
    #[serde(default)]
    router_auto_restart: bool,
}

fn default_restart_target() -> String {
    RESTART_TARGET_CHATGPT.to_string()
}

fn normalize_restart_target(value: &str) -> String {
    if value.trim().eq_ignore_ascii_case(RESTART_TARGET_CODEX) {
        RESTART_TARGET_CODEX.to_string()
    } else {
        RESTART_TARGET_CHATGPT.to_string()
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            system_version: default_system_version(),
            activation_time: current_app_activation_time(),
            codex_exe_path: String::new(),
            app_restart_target: default_restart_target(),
            official_proxy_url: String::new(),
            account_usage_refresh_seconds: default_account_usage_refresh_seconds(),
            token_auto_renew_enabled: false,
            router_port: default_router_port(),
            router_concurrency_limit: default_router_concurrency_limit(),
            oauth_callback_port: default_oauth_callback_port(),
            router_debug_mode: false,
            image_generation_compat_mode: false,
            account_proxy: AccountProxySettings::default(),
            router_name: default_router_name(),
            router_base_url: String::new(),
            router_auth_method: default_router_auth_method(),
            router_auth_external_token: String::new(),
            router_auth_env_key: String::new(),
            router_model_catalog_json: String::new(),
            router_default_model: String::new(),
            router_mode: default_router_mode(),
            router_auto_restart: false,
        }
    }
}

fn default_router_name() -> String {
    CODEX_MODEL_PROVIDER_NAME.to_string()
}

fn default_router_auth_method() -> String {
    "native".to_string()
}

fn default_router_mode() -> String {
    "system".to_string()
}

fn normalize_router_mode(value: &str) -> String {
    if value.trim().eq_ignore_ascii_case("third") {
        "third".to_string()
    } else {
        "system".to_string()
    }
}

fn normalize_router_auth_method(value: &str) -> String {
    match value.trim() {
        "native" | "external" | "env" => value.trim().to_string(),
        _ => "native".to_string(),
    }
}

#[derive(serde::Serialize)]
struct SyncCatalogResult {
    source_path: String,
    target_path: String,
    synced_count: usize,
    total_count: usize,
    synced_slugs: Vec<String>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ThreadSession {
    id: String,
    title: String,
    file_path: String,
    source: String,
    archived: bool,
    indexed: bool,
    missing_from_index: bool,
    sidebar_missing: bool,
    state_needs_repair: bool,
    cwd: Option<String>,
    project_name: String,
    originator: Option<String>,
    cli_version: Option<String>,
    thread_source: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    file_size: u64,
    message_count: usize,
    first_user_text: Option<String>,
    parse_errors: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectGroup {
    project_name: String,
    cwd: Option<String>,
    thread_count: usize,
    total_size: u64,
    active_days: usize,
    sessions: Vec<ThreadSession>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanSummary {
    total_threads: usize,
    total_size: u64,
    active_days: usize,
    average_threads_per_day: f64,
    indexed_threads: usize,
    missing_from_index: usize,
    archived_threads: usize,
    project_count: usize,
    scanned_at: String,
}

#[derive(serde::Serialize)]
struct ThreadScanResult {
    summary: ScanSummary,
    projects: Vec<ProjectGroup>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexAccount {
    id: String,
    account_key: String,
    email: String,
    name: String,
    plan: String,
    auth_mode: String,
    subscription_status: String,
    workspace_name: String,
    access_token_mask: String,
    is_current: bool,
    five_hour_percent: Option<u8>,
    weekly_percent: Option<u8>,
    five_hour_reset_at: Option<String>,
    weekly_reset_at: Option<String>,
    expires_at: Option<String>,
    auto_renew: bool,
    snapshot_path: Option<String>,
    last_used_at: Option<String>,
    last_usage_at: Option<String>,
    usage_windows: Vec<CodexAccountUsageWindow>,
    token_expires_at: Option<String>,
    token_needs_refresh: bool,
    token_expired: bool,
    token_refresh_permanently_failed: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexAccountUsageWindow {
    remaining_percent: u8,
    reset_at: Option<String>,
    limit_window_seconds: Option<u64>,
    reset_after_seconds: Option<u64>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexAccountScanResult {
    accounts: Vec<CodexAccount>,
    current_account_id: Option<String>,
    api_healthy: bool,
    scanned_at: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountKeyRequest {
    account_key: String,
    manual: Option<bool>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountExpirationRequest {
    account_key: String,
    expires_at: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptSessionImportRequest {
    session_json: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CpaImportRequest {
    cpa_json: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenExternalUrlRequest {
    url: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInstallRequest {
    download_url: String,
    asset_name: String,
    latest_version: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UpdateDownloadProgress {
    phase: String,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    percent: Option<u8>,
    message: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInstallResult {
    installer_path: String,
    message: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountOperationResult {
    message: String,
    path: Option<String>,
    scan: CodexAccountScanResult,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexRestartResult {
    success: bool,
    message: String,
}

#[derive(Clone)]
struct CodexUsageWindow {
    used_percent: u8,
    resets_at: Option<String>,
    limit_window_seconds: Option<u64>,
    reset_after_seconds: Option<u64>,
}

#[derive(Clone)]
struct CodexUsageSnapshot {
    primary: Option<CodexUsageWindow>,
    secondary: Option<CodexUsageWindow>,
    plan: Option<String>,
    user_id: Option<String>,
    account_id: Option<String>,
}

#[derive(Default)]
struct SessionIndexInfo {
    ids: HashSet<String>,
    titles: HashMap<String, String>,
    sidebar_ids: HashSet<String>,
    prompt_history: HashMap<String, String>,
    sqlite_threads: HashMap<String, SqliteThreadState>,
}

#[derive(Clone, Default)]
struct SqliteThreadState {
    title: String,
    first_user_message: String,
    cwd: String,
    rollout_path: String,
    archived: i64,
}

#[derive(Clone)]
struct SidebarThreadRow {
    id: String,
    title: String,
    cwd: String,
    prompt_history_text: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ProviderConfigItemInput {
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "baseUrl")]
    base_url: String,
    #[serde(rename = "apiKey")]
    api_key: String,
    #[serde(rename = "realModel")]
    real_model: String,
    #[serde(rename = "contextWindow", default)]
    context_window: Option<u64>,
    #[serde(rename = "maxContextWindow", default)]
    max_context_window: Option<u64>,
    #[serde(rename = "effectiveContextWindowPercent", default)]
    effective_context_window_percent: Option<u64>,
    #[serde(rename = "proxyMode", default = "default_provider_proxy_mode")]
    proxy_mode: String,
    #[serde(rename = "proxyUrl", default)]
    proxy_url: String,
    #[serde(rename = "protocolType", default = "default_protocol_type")]
    protocol_type: String,
    #[serde(rename = "endpointPath", default)]
    endpoint_path: String,
    #[serde(rename = "modelMappings", alias = "modelAliases", default)]
    model_mappings: Vec<String>,
    #[serde(default)]
    priority: i32,
    #[serde(default = "default_provider_weight")]
    weight: u32,
    enabled: bool,
    #[serde(default)]
    active: bool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportProviderConfigRequest {
    source_path: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderConfigExportResult {
    export_path: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderConfigImportResult {
    config: ProviderConfigInput,
    backup_path: Option<String>,
}

#[derive(serde::Deserialize)]
struct FetchProviderModelsRequest {
    #[serde(rename = "baseUrl")]
    base_url: String,
    #[serde(rename = "apiKey")]
    api_key: String,
    #[serde(rename = "protocolType", default = "default_protocol_type")]
    protocol_type: String,
    #[serde(rename = "proxyUrl", default)]
    proxy_url: String,
}

#[derive(serde::Deserialize)]
struct TestProviderModelRequest {
    slug: String,
}

#[derive(serde::Serialize)]
struct ProviderModelTestResult {
    slug: String,
    success: bool,
    status_code: u16,
    latency_ms: u128,
    latency: String,
    url: String,
    message: String,
}

#[derive(serde::Serialize)]
struct ProviderModelChatTestResult {
    slug: String,
    success: bool,
    status_code: u16,
    latency_ms: u128,
    latency: String,
    url: String,
    protocol_type: String,
    request_body: String,
    message: String,
    response_text: String,
}

#[derive(serde::Serialize)]
struct ProviderModelListResult {
    models: Vec<String>,
    url: String,
}

type ProviderConfigInput = std::collections::BTreeMap<String, ProviderConfigItemInput>;

struct RouterProviderConfig(serde_json::Map<String, serde_json::Value>);

impl serde::Serialize for RouterProviderConfig {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut items = Vec::new();
        for (slug, value) in self.0.iter() {
            let mut object = match value {
                serde_json::Value::Object(map) => map.clone(),
                _ => serde_json::Map::new(),
            };
            object.insert("slug".to_string(), serde_json::Value::String(slug.clone()));
            items.push(serde_json::Value::Object(object));
        }
        items.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for RouterProviderConfig {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        let mut map = serde_json::Map::new();
        match value {
            serde_json::Value::Array(items) => {
                for item in items {
                    if let serde_json::Value::Object(mut object) = item {
                        let slug = match object.get("slug").and_then(|value| value.as_str()) {
                            Some(slug) if !slug.is_empty() => slug.to_string(),
                            _ => continue,
                        };
                        object.remove("slug");
                        map.insert(slug, serde_json::Value::Object(object));
                    }
                }
            }
            serde_json::Value::Object(object) => map = object,
            _ => {}
        }
        Ok(RouterProviderConfig(map))
    }
}

#[derive(Clone)]
struct ProviderRoute {
    provider: String,
    base_url: String,
    api_key: String,
    real_model: String,
    proxy_mode: String,
    proxy_url: String,
    protocol_type: String,
    endpoint_path: String,
    priority: i32,
    weight: u32,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ProviderRouteFileItem {
    #[serde(rename = "displayName")]
    #[serde(default)]
    display_name: String,
    #[serde(rename = "baseUrl")]
    base_url: String,
    #[serde(rename = "apiKey")]
    api_key: String,
    #[serde(rename = "realModel")]
    real_model: String,
    #[serde(rename = "contextWindow", default)]
    context_window: Option<u64>,
    #[serde(rename = "maxContextWindow", default)]
    max_context_window: Option<u64>,
    #[serde(rename = "effectiveContextWindowPercent", default)]
    effective_context_window_percent: Option<u64>,
    #[serde(rename = "proxyMode", default = "default_provider_proxy_mode")]
    proxy_mode: String,
    #[serde(rename = "proxyUrl", default)]
    proxy_url: String,
    #[serde(rename = "protocolType", default = "default_protocol_type")]
    protocol_type: String,
    #[serde(rename = "endpointPath", default)]
    endpoint_path: String,
    #[serde(rename = "modelMappings", alias = "modelAliases", default)]
    model_mappings: Vec<String>,
    #[serde(default)]
    priority: i32,
    #[serde(default = "default_provider_weight")]
    weight: u32,
    #[serde(default = "default_provider_enabled")]
    enabled: bool,
    #[serde(default)]
    active: bool,
}

struct EnabledProviderRoute {
    slug: String,
    route: ProviderRouteFileItem,
}

impl From<ProviderConfigItemInput> for ProviderRouteFileItem {
    fn from(value: ProviderConfigItemInput) -> Self {
        Self {
            display_name: value.display_name,
            base_url: value.base_url,
            api_key: value.api_key,
            real_model: value.real_model,
            context_window: normalize_positive_u64(value.context_window),
            max_context_window: normalize_positive_u64(value.max_context_window),
            effective_context_window_percent: normalize_percent(
                value.effective_context_window_percent,
            ),
            proxy_mode: normalize_provider_proxy_mode(&value.proxy_mode),
            proxy_url: normalize_proxy_url(&value.proxy_url).unwrap_or_default(),
            protocol_type: normalize_protocol_type(&value.protocol_type),
            endpoint_path: normalize_endpoint_path(&value.endpoint_path),
            model_mappings: normalize_model_mappings(&value.model_mappings),
            priority: value.priority,
            weight: normalize_provider_weight(value.weight),
            enabled: value.enabled,
            active: value.active,
        }
    }
}

fn default_protocol_type() -> String {
    "cpamc".to_string()
}

fn default_provider_enabled() -> bool {
    true
}

fn default_provider_weight() -> u32 { 1 }

fn normalize_provider_weight(weight: u32) -> u32 { weight.max(1) }

fn normalize_model_mappings(mappings: &[String]) -> Vec<String> {
    let mut normalized = mappings
        .iter()
        .map(|alias| alias.trim().to_string())
        .filter(|alias| !alias.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn default_provider_proxy_mode() -> String {
    "default".to_string()
}

fn normalize_positive_u64(value: Option<u64>) -> Option<u64> {
    value.filter(|item| *item > 0)
}

fn normalize_percent(value: Option<u64>) -> Option<u64> {
    value.filter(|item| *item > 0).map(|item| item.min(100))
}

struct ParsedRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

struct RouterResponse {
    status_code: u16,
    content_type: String,
    body: String,
    target_provider: String,
    error_detail: String,
    usage: Option<TokenUsage>,
    usage_source: String,
    flush_headers_before_body: bool,
}

#[derive(serde::Serialize)]
struct ProxyTestResult {
    success: bool,
    proxy_url: String,
    latency_ms: u128,
    latency: String,
    status_code: u16,
    message: String,
}

#[derive(serde::Deserialize)]
struct ProxyTestRequest {
    #[serde(rename = "proxyUrl")]
    proxy_url: String,
}

struct CodexAuthCredentials {
    access_token: String,
    account_id: String,
}

struct OfficialCodexForwardSettings {
    proxy_url: Option<String>,
}



fn start_router_blocking() -> Result<RouterCommandResult, String> {
    let router_port = configured_router_port();
    {
        let state = router_state();
        let runtime = state.lock().map_err(|error| error.to_string())?;

        if runtime.started {
            return Ok(build_router_result(&runtime, "running"));
        }
    }

    ensure_provider_config_file()?;
    let listener = TcpListener::bind((ROUTER_HOST, router_port))
        .map_err(|error| format_listener_bind_error(error, router_port))?;

    ensure_codex_config_backup()?;
    upsert_codex_router_config()?;

    let started_at = Instant::now();
    let stop_signal = Arc::new(Mutex::new(false));
    let thread_stop_signal = Arc::clone(&stop_signal);
    let concurrency_limit = configured_router_concurrency_limit();
    let connection_slots = Arc::new((Mutex::new(0usize), Condvar::new()));

    let handle = thread::spawn(move || loop {
        if is_stop_requested(&thread_stop_signal) {
            break;
        }

        match listener.accept() {
            Ok((stream, _)) => {
                let permit = acquire_connection_slot(
                    Arc::clone(&connection_slots),
                    concurrency_limit,
                    &thread_stop_signal,
                );
                if is_stop_requested(&thread_stop_signal) {
                    drop(permit);
                    break;
                }
                thread::spawn(move || {
                    let _permit = permit;
                    match handle_connection(stream, started_at, router_port) {
                        Ok(log_entry) => push_router_log(log_entry),
                        Err(error) => eprintln!("connection error: {}", error),
                    }
                });
            }
            Err(error) => eprintln!("listener error: {}", error),
        }
    });

    let state = router_state();
    let mut runtime = state.lock().map_err(|error| error.to_string())?;
    runtime.started = true;
    runtime.started_at = Some(started_at);
    runtime.stop_signal = Some(stop_signal);
    runtime.handle = Some(handle);
    runtime.pid = Some(std::process::id());
    runtime.port = router_port;

    Ok(build_router_result(&runtime, "started"))
}



fn stop_router_blocking() -> Result<RouterCommandResult, String> {
    let runtime = stop_router_runtime_blocking()?;
    restore_official_model_catalog()?;
    remove_router_models_from_models_cache()?;
    remove_codex_router_config()?;
    Ok(build_router_result(&runtime, "stopped"))
}

fn stop_router_runtime_blocking() -> Result<std::sync::MutexGuard<'static, RouterRuntime>, String> {
    let (handle, router_port) = {
        let state = router_state();
        let mut runtime = state.lock().map_err(|error| error.to_string())?;

        if let Some(stop_signal) = runtime.stop_signal.take() {
            let mut requested = stop_signal.lock().map_err(|error| error.to_string())?;
            *requested = true;
        }

        (runtime.handle.take(), runtime.port)
    };

    wake_router_listener(router_port);

    if let Some(handle) = handle {
        handle
            .join()
            .map_err(|_| "router thread stopped unexpectedly".to_string())?;
    }

    let state = router_state();
    let mut runtime = state.lock().map_err(|error| error.to_string())?;
    runtime.started = false;
    runtime.started_at = None;
    runtime.pid = None;
    Ok(runtime)
}

fn wake_router_listener(router_port: u16) {
    let _ = TcpStream::connect((ROUTER_HOST, router_port));
}



















fn append_internal_app_log(
    level: &str,
    module: &str,
    action: &str,
    message: &str,
    detail: Option<String>,
) {
    let log_entry = AppOperationLogEntry {
        id: current_log_millis().to_string(),
        time: current_log_time(),
        level: normalize_log_level(level),
        module: module.to_string(),
        action: action.to_string(),
        message: message.to_string(),
        detail: detail.filter(|value| !value.trim().is_empty()),
    };

    let _ = append_app_log_entry(&log_entry);
}















struct ZipStats {
    file_count: usize,
    total_bytes: u64,
}

struct ZipEntryRecord {
    name: String,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    local_header_offset: u32,
}

struct SimpleZipWriter {
    file: std::fs::File,
    entries: Vec<ZipEntryRecord>,
    total_bytes: u64,
}

struct SimpleZipEntry {
    name: String,
    bytes: Vec<u8>,
}

struct SimpleZipArchive {
    file: std::fs::File,
    finished: bool,
}

struct TempPathGuard {
    path: PathBuf,
    keep: bool,
}

impl TempPathGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, keep: false }
    }

    fn keep(&mut self) {
        self.keep = true;
    }
}

impl Drop for TempPathGuard {
    fn drop(&mut self) {
        if !self.keep && self.path.exists() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[derive(Clone)]
struct MigrationRestorePlan {
    entry_name: String,
    section: String,
    target_path: PathBuf,
    staging_path: PathBuf,
    byte_len: u64,
    modified_unix_ms: Option<i64>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MigrationFileTimeMetadata {
    modified_unix_ms: i64,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationRestoreJournalEntry {
    entry_name: String,
    target_path: String,
    staging_path: String,
    backup_path: Option<String>,
    existed_before: bool,
}

impl SimpleZipWriter {
    fn create(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::create(path).map_err(|error| {
            format!("创建迁移备份 ZIP 失败：{}，路径：{}", error, path.display())
        })?;
        Ok(Self {
            file,
            entries: Vec::new(),
            total_bytes: 0,
        })
    }

    fn add_file(&mut self, name: &str, path: &Path) -> Result<(), String> {
        let mut file = std::fs::File::open(path)
            .map_err(|error| format!("读取备份文件失败：{}，路径：{}", error, path.display()))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|error| {
            format!("读取备份文件内容失败：{}，路径：{}", error, path.display())
        })?;
        self.add_bytes(name, &bytes)
    }

    fn add_bytes(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let normalized_name = normalize_zip_entry_name(name)?;
        let name_bytes = normalized_name.as_bytes();
        let name_len = checked_u16(name_bytes.len(), "ZIP 文件名过长")?;
        let size = checked_u32(bytes.len(), "ZIP 文件过大")?;
        let offset = checked_u32(
            self.file
                .stream_position()
                .map_err(|error| format!("读取 ZIP 写入位置失败：{}", error))?,
            "ZIP 文件过大",
        )?;
        let crc32 = crc32(bytes);

        write_u32(&mut self.file, 0x04034b50)?;
        write_u16(&mut self.file, 20)?;
        write_u16(&mut self.file, 0)?;
        write_u16(&mut self.file, 0)?;
        write_u16(&mut self.file, 0)?;
        write_u16(&mut self.file, 0)?;
        write_u32(&mut self.file, crc32)?;
        write_u32(&mut self.file, size)?;
        write_u32(&mut self.file, size)?;
        write_u16(&mut self.file, name_len)?;
        write_u16(&mut self.file, 0)?;
        self.file
            .write_all(name_bytes)
            .map_err(|error| format!("写入 ZIP 文件名失败：{}", error))?;
        self.file
            .write_all(bytes)
            .map_err(|error| format!("写入 ZIP 文件内容失败：{}", error))?;

        self.entries.push(ZipEntryRecord {
            name: normalized_name,
            crc32,
            compressed_size: size,
            uncompressed_size: size,
            local_header_offset: offset,
        });
        self.total_bytes += bytes.len() as u64;
        Ok(())
    }

    fn finish(mut self) -> Result<ZipStats, String> {
        let central_directory_offset = checked_u32(
            self.file
                .stream_position()
                .map_err(|error| format!("读取 ZIP 中央目录位置失败：{}", error))?,
            "ZIP 文件过大",
        )?;

        for entry in &self.entries {
            let name_bytes = entry.name.as_bytes();
            let name_len = checked_u16(name_bytes.len(), "ZIP 文件名过长")?;
            write_u32(&mut self.file, 0x02014b50)?;
            write_u16(&mut self.file, 20)?;
            write_u16(&mut self.file, 20)?;
            write_u16(&mut self.file, 0)?;
            write_u16(&mut self.file, 0)?;
            write_u16(&mut self.file, 0)?;
            write_u16(&mut self.file, 0)?;
            write_u32(&mut self.file, entry.crc32)?;
            write_u32(&mut self.file, entry.compressed_size)?;
            write_u32(&mut self.file, entry.uncompressed_size)?;
            write_u16(&mut self.file, name_len)?;
            write_u16(&mut self.file, 0)?;
            write_u16(&mut self.file, 0)?;
            write_u16(&mut self.file, 0)?;
            write_u16(&mut self.file, 0)?;
            write_u32(&mut self.file, 0)?;
            write_u32(&mut self.file, entry.local_header_offset)?;
            self.file
                .write_all(name_bytes)
                .map_err(|error| format!("写入 ZIP 中央目录失败：{}", error))?;
        }

        let central_directory_end = checked_u32(
            self.file
                .stream_position()
                .map_err(|error| format!("读取 ZIP 结束位置失败：{}", error))?,
            "ZIP 文件过大",
        )?;
        let central_directory_size = central_directory_end
            .checked_sub(central_directory_offset)
            .ok_or_else(|| "ZIP 中央目录位置异常。".to_string())?;
        let entry_count = checked_u16(self.entries.len(), "ZIP 文件数量过多")?;

        write_u32(&mut self.file, 0x06054b50)?;
        write_u16(&mut self.file, 0)?;
        write_u16(&mut self.file, 0)?;
        write_u16(&mut self.file, entry_count)?;
        write_u16(&mut self.file, entry_count)?;
        write_u32(&mut self.file, central_directory_size)?;
        write_u32(&mut self.file, central_directory_offset)?;
        write_u16(&mut self.file, 0)?;
        self.file
            .flush()
            .map_err(|error| format!("刷新 ZIP 文件失败：{}", error))?;

        Ok(ZipStats {
            file_count: self.entries.len(),
            total_bytes: self.total_bytes,
        })
    }
}

impl SimpleZipArchive {
    fn open(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|error| {
            format!("打开迁移备份 ZIP 失败：{}，路径：{}", error, path.display())
        })?;
        Ok(Self {
            file,
            finished: false,
        })
    }

    fn next_entry(&mut self) -> Result<Option<SimpleZipEntry>, String> {
        if self.finished {
            return Ok(None);
        }

        let mut signature_bytes = [0u8; 4];
        match self.file.read_exact(&mut signature_bytes) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
                self.finished = true;
                return Ok(None);
            }
            Err(error) => return Err(format!("读取 ZIP 条目失败：{}", error)),
        }
        let signature = u32::from_le_bytes(signature_bytes);
        if signature == 0x02014b50 || signature == 0x06054b50 {
            self.finished = true;
            return Ok(None);
        }
        if signature != 0x04034b50 {
            return Err("迁移备份 ZIP 格式不受支持。".to_string());
        }

        let _version_needed = read_zip_u16(&mut self.file)?;
        let flags = read_zip_u16(&mut self.file)?;
        let method = read_zip_u16(&mut self.file)?;
        let _mtime = read_zip_u16(&mut self.file)?;
        let _mdate = read_zip_u16(&mut self.file)?;
        let expected_crc32 = read_zip_u32(&mut self.file)?;
        let compressed_size = read_zip_u32(&mut self.file)?;
        let uncompressed_size = read_zip_u32(&mut self.file)?;
        let name_len = read_zip_u16(&mut self.file)? as usize;
        let extra_len = read_zip_u16(&mut self.file)? as usize;

        if flags & 0x0008 != 0 {
            return Err("迁移备份 ZIP 使用 data descriptor，当前不支持导入。".to_string());
        }
        if method != 0 {
            return Err(
                "迁移备份 ZIP 使用压缩格式，当前只支持本工具生成的未压缩迁移包。".to_string(),
            );
        }
        if compressed_size != uncompressed_size {
            return Err("迁移备份 ZIP 条目大小异常。".to_string());
        }

        let mut name_bytes = vec![0u8; name_len];
        self.file
            .read_exact(&mut name_bytes)
            .map_err(|error| format!("读取 ZIP 条目名称失败：{}", error))?;
        if extra_len > 0 {
            let mut extra = vec![0u8; extra_len];
            self.file
                .read_exact(&mut extra)
                .map_err(|error| format!("读取 ZIP 额外字段失败：{}", error))?;
        }
        let name = String::from_utf8(name_bytes)
            .map_err(|error| format!("ZIP 条目名称不是 UTF-8：{}", error))?;
        let name = normalize_zip_entry_name(&name)?;
        if name.ends_with('/') {
            return Ok(Some(SimpleZipEntry {
                name,
                bytes: Vec::new(),
            }));
        }

        let mut bytes = vec![0u8; compressed_size as usize];
        self.file
            .read_exact(&mut bytes)
            .map_err(|error| format!("读取 ZIP 条目内容失败：{}", error))?;
        let actual_crc32 = crc32(&bytes);
        if actual_crc32 != expected_crc32 {
            return Err(format!("ZIP 条目校验失败：{}", name));
        }

        Ok(Some(SimpleZipEntry { name, bytes }))
    }
}

fn add_migration_section_file(
    writer: &mut SimpleZipWriter,
    skipped_items: &mut Vec<String>,
    file_times: &mut HashMap<String, MigrationFileTimeMetadata>,
    entry_name: &str,
    path: PathBuf,
) -> Result<(), String> {
    if path.exists() && path.is_file() {
        writer.add_file(entry_name, &path)?;
        record_migration_file_time(file_times, entry_name, &path);
    } else {
        skipped_items.push(format!("{}：文件不存在", entry_name));
    }
    Ok(())
}

fn add_migration_section_dir(
    writer: &mut SimpleZipWriter,
    skipped_items: &mut Vec<String>,
    file_times: &mut HashMap<String, MigrationFileTimeMetadata>,
    entry_prefix: &str,
    path: PathBuf,
) -> Result<(), String> {
    if !path.exists() {
        skipped_items.push(format!("{}：目录不存在", entry_prefix));
        return Ok(());
    }
    if !path.is_dir() {
        skipped_items.push(format!("{}：不是目录", entry_prefix));
        return Ok(());
    }

    let mut files = Vec::new();
    collect_files_for_zip(&path, &mut files)?;
    files.sort();
    for file_path in files {
        let relative = file_path.strip_prefix(&path).map_err(|error| {
            format!(
                "计算备份文件相对路径失败：{}，路径：{}",
                error,
                file_path.display()
            )
        })?;
        let entry_name = format!(
            "{}/{}",
            entry_prefix.trim_end_matches('/'),
            path_to_zip_name(relative)?
        );
        writer.add_file(&entry_name, &file_path)?;
        record_migration_file_time(file_times, &entry_name, &file_path);
    }
    Ok(())
}

fn record_migration_file_time(
    file_times: &mut HashMap<String, MigrationFileTimeMetadata>,
    entry_name: &str,
    path: &Path,
) {
    let Some(modified_unix_ms) = fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(system_time_to_unix_millis)
    else {
        return;
    };

    file_times.insert(
        entry_name.replace('\\', "/"),
        MigrationFileTimeMetadata { modified_unix_ms },
    );
}

fn add_migration_mcp_config(
    writer: &mut SimpleZipWriter,
    skipped_items: &mut Vec<String>,
) -> Result<(), String> {
    let path = codex_config_path()?;
    if !path.exists() {
        skipped_items.push("mcp/mcp_servers.toml：config.toml 不存在".to_string());
        return Ok(());
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("读取 MCP 配置失败：{}，路径：{}", error, path.display()))?;
    let mcp_text = extract_mcp_blocks(&text);
    if mcp_text.trim().is_empty() {
        skipped_items.push("mcp/mcp_servers.toml：未找到 MCP 配置块".to_string());
        return Ok(());
    }
    writer.add_bytes("mcp/mcp_servers.toml", mcp_text.as_bytes())
}

fn migration_entry_is_session_file(name: &str) -> bool {
    (name.starts_with("sessions/sessions/") || name.starts_with("sessions/archived_sessions/"))
        && name.ends_with(".jsonl")
}

fn extract_session_cwd_from_jsonl_bytes(bytes: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        if value.get("type").and_then(|item| item.as_str()) != Some("session_meta") {
            continue;
        }
        let cwd = value
            .get("payload")
            .and_then(|payload| json_string_field(payload, "cwd"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if cwd.is_some() {
            return cwd;
        }
    }
    None
}

fn import_migration_backup_transactional(
    source_path: &Path,
) -> Result<MigrationRestoreResult, String> {
    ensure_workspace_layout()?;
    let tx_id = current_log_time();
    let restore_backup_root = workspace_backup_path()?
        .join("migration-restore")
        .join(&tx_id);
    let staging_root = build_user_relative_path(WORKSPACE_RUNTIME_RELATIVE_PATH)?
        .join("migration-staging")
        .join(&tx_id);
    fs::create_dir_all(&restore_backup_root).map_err(|error| {
        format!(
            "创建恢复事务备份目录失败：{}，路径：{}",
            error,
            restore_backup_root.display()
        )
    })?;
    fs::create_dir_all(&staging_root).map_err(|error| {
        format!(
            "创建恢复事务暂存目录失败：{}，路径：{}",
            error,
            staging_root.display()
        )
    })?;

    let result = (|| {
        let mut archive = SimpleZipArchive::open(source_path)?;
        let mut archive_entries = Vec::new();
        let mut file_times: HashMap<String, MigrationFileTimeMetadata> = HashMap::new();

        while let Some(entry) = archive.next_entry()? {
            if entry.name == "metadata/file_times.json" {
                file_times = serde_json::from_slice::<HashMap<String, MigrationFileTimeMetadata>>(
                    &entry.bytes,
                )
                .map_err(|error| format!("解析迁移备份文件时间元数据失败：{}", error))?;
                continue;
            }
            archive_entries.push(entry);
        }

        let mut plans = Vec::new();
        let mut skipped_items = Vec::new();
        let mut seen_targets = HashSet::new();

        for entry in archive_entries {
            let name = entry.name.clone();
            if name == "manifest.json" || name.ends_with('/') {
                continue;
            }

            if name == "mcp/mcp_servers.toml" {
                let modified_unix_ms = file_times.get(&name).map(|item| item.modified_unix_ms);
                let text = String::from_utf8(entry.bytes)
                    .map_err(|error| format!("MCP 配置片段不是 UTF-8：{}", error))?;
                let target_path = codex_config_path()?;
                let original = fs::read_to_string(&target_path).unwrap_or_default();
                let without_mcp = remove_mcp_blocks(&original);
                let next = if without_mcp.trim().is_empty() {
                    text.trim().to_string() + "\n"
                } else {
                    format!("{}\n{}\n", without_mcp.trim_end(), text.trim())
                };
                let staging_path = staging_root.join("mcp").join("config.toml");
                write_staging_file(&staging_path, next.as_bytes())?;
                add_restore_plan(
                    &mut plans,
                    &mut seen_targets,
                    name,
                    "MCP".to_string(),
                    target_path,
                    staging_path,
                    next.len() as u64,
                    modified_unix_ms,
                )?;
                continue;
            }

            let Some((target_path, section)) = migration_entry_target_path(&name)? else {
                skipped_items.push(format!("{}：已跳过", name));
                continue;
            };
            validate_migration_entry_payload(&name, &entry.bytes)?;
            let modified_unix_ms = file_times.get(&name).map(|item| item.modified_unix_ms);
            let staging_path = staging_root
                .join("entries")
                .join(zip_entry_to_relative_path(&name)?);
            write_staging_file(&staging_path, &entry.bytes)?;
            add_restore_plan(
                &mut plans,
                &mut seen_targets,
                name,
                section,
                target_path,
                staging_path,
                entry.bytes.len() as u64,
                modified_unix_ms,
            )?;
        }

        normalize_staged_accounts_registry_paths(&mut plans)?;
        let should_sync_catalog = plans
            .iter()
            .any(|plan| plan.entry_name == "models/router_provider_config.json");
        let (journal, committed_count, committed_bytes) =
            commit_migration_restore_plans(&plans, &restore_backup_root)?;
        let catalog_sync_result = if should_sync_catalog {
            match sync_catalog_from_provider_config() {
                Ok(result) => Some(serde_json::json!({
                    "success": true,
                    "syncedCount": result.synced_count,
                    "totalCount": result.total_count,
                    "targetPath": result.target_path,
                    "syncedSlugs": result.synced_slugs
                })),
                Err(error) => {
                    skipped_items.push(format!("模型 catalog 自动同步失败：{}", error));
                    Some(serde_json::json!({
                        "success": false,
                        "error": error
                    }))
                }
            }
        } else {
            None
        };

        let mut restored_sections: Vec<String> = plans
            .iter()
            .map(|plan| plan.section.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        restored_sections.sort();

        let result_path = restore_backup_root.join("restore-result.json");
        let result_json = serde_json::json!({
            "committedAt": current_log_time(),
            "restoredCount": committed_count,
            "restoredBytes": committed_bytes,
            "restoredSections": restored_sections,
            "skippedItems": skipped_items,
            "catalogSync": catalog_sync_result,
            "journalEntries": journal.len()
        });
        fs::write(
            &result_path,
            serde_json::to_string_pretty(&result_json)
                .map_err(|error| format!("生成恢复结果失败：{}", error))?,
        )
        .map_err(|error| {
            format!(
                "写入恢复结果失败：{}，路径：{}",
                error,
                result_path.display()
            )
        })?;

        Ok(MigrationRestoreResult {
            restored_count: committed_count,
            restored_bytes: committed_bytes,
            backup_path: restore_backup_root.display().to_string(),
            restored_sections,
            skipped_items,
            message: if committed_count == 0 {
                "\u{672a}\u{4ece}\u{8fc1}\u{79fb}\u{5305}\u{4e2d}\u{6062}\u{590d}\u{4efb}\u{4f55}\u{6587}\u{4ef6}\u{3002}".to_string()
            } else {
                format!(
                    "\u{8fc1}\u{79fb}\u{5305}\u{6062}\u{590d}\u{5b8c}\u{6210}\u{ff1a}{} \u{4e2a}\u{6587}\u{4ef6}\u{ff0c}{}\u{3002}\u{6062}\u{590d}\u{524d}\u{5907}\u{4efd}\u{5df2}\u{4fdd}\u{5b58}\u{3002}",
                    committed_count,
                    format_bytes(committed_bytes)
                )
            },
        })
    })();

    let _ = fs::remove_dir_all(&staging_root);
    result
}

fn add_restore_plan(
    plans: &mut Vec<MigrationRestorePlan>,
    seen_targets: &mut HashSet<PathBuf>,
    entry_name: String,
    section: String,
    target_path: PathBuf,
    staging_path: PathBuf,
    byte_len: u64,
    modified_unix_ms: Option<i64>,
) -> Result<(), String> {
    if !seen_targets.insert(target_path.clone()) {
        return Err(format!("迁移包包含重复恢复目标：{}", target_path.display()));
    }
    plans.push(MigrationRestorePlan {
        entry_name,
        section,
        target_path,
        staging_path,
        byte_len,
        modified_unix_ms,
    });
    Ok(())
}

fn write_staging_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    ensure_parent_dir(path)?;
    fs::write(path, bytes)
        .map_err(|error| format!("写入恢复暂存文件失败：{}，路径：{}", error, path.display()))
}

fn validate_migration_entry_payload(name: &str, bytes: &[u8]) -> Result<(), String> {
    if name.ends_with(".json") {
        serde_json::from_slice::<serde_json::Value>(bytes)
            .map_err(|error| format!("迁移包 JSON 文件无效：{}，{}", name, error))?;
    }
    Ok(())
}

fn zip_entry_to_relative_path(name: &str) -> Result<PathBuf, String> {
    let normalized = normalize_zip_entry_name(name)?;
    let mut path = PathBuf::new();
    for part in normalized.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(format!("ZIP 条目名称不安全：{}", name));
        }
        path.push(part);
    }
    Ok(path)
}

fn normalize_staged_accounts_registry_paths(
    plans: &mut [MigrationRestorePlan],
) -> Result<(), String> {
    let Some(registry_index) = plans
        .iter()
        .position(|plan| plan.entry_name == "accounts/registry.json")
    else {
        return Ok(());
    };

    let snapshots_dir = codex_accounts_snapshots_path()?;
    let mut planned_snapshots: HashMap<String, PathBuf> = HashMap::new();
    for plan in plans.iter() {
        if let Some(file_name) = plan
            .entry_name
            .strip_prefix("accounts/snapshots/")
            .filter(|value| !value.trim().is_empty())
        {
            planned_snapshots.insert(file_name.to_string(), snapshots_dir.join(file_name));
        }
    }

    let registry_path = plans[registry_index].staging_path.clone();
    let text = fs::read_to_string(&registry_path).map_err(|error| {
        format!(
            "读取暂存账号 registry 失败：{}，路径：{}",
            error,
            registry_path.display()
        )
    })?;
    let mut registry = serde_json::from_str::<serde_json::Value>(&text)
        .map_err(|error| format!("解析暂存账号 registry 失败：{}", error))?;
    rewrite_registry_snapshot_paths_value(&mut registry, &planned_snapshots)?;
    let next_text = serde_json::to_string_pretty(&registry)
        .map_err(|error| format!("生成暂存账号 registry 失败：{}", error))?;
    fs::write(&registry_path, &next_text).map_err(|error| {
        format!(
            "写入暂存账号 registry 失败：{}，路径：{}",
            error,
            registry_path.display()
        )
    })?;
    plans[registry_index].byte_len = next_text.len() as u64;
    Ok(())
}

fn rewrite_registry_snapshot_paths_value(
    registry: &mut serde_json::Value,
    planned_snapshots: &HashMap<String, PathBuf>,
) -> Result<(), String> {
    let snapshots_dir = codex_accounts_snapshots_path()?;
    let Some(items) = registry
        .get_mut("items")
        .and_then(|value| value.as_array_mut())
    else {
        return Ok(());
    };

    for item in items {
        let Some(map) = item.as_object_mut() else {
            continue;
        };
        let Some(account_key) = map
            .get("accountKey")
            .and_then(|value| value.as_str())
            .map(str::to_string)
        else {
            continue;
        };
        let existing_file_name = map
            .get("snapshotPath")
            .and_then(|value| value.as_str())
            .and_then(|path| Path::new(path).file_name())
            .and_then(|file_name| file_name.to_str())
            .map(str::to_string);
        let snapshot_path = existing_file_name
            .as_ref()
            .and_then(|file_name| planned_snapshots.get(file_name))
            .cloned()
            .unwrap_or_else(|| {
                snapshots_dir.join(format!(
                    "{}.json",
                    sanitize_account_key_for_filename(&account_key)
                ))
            });
        map.insert(
            "snapshotPath".to_string(),
            serde_json::Value::String(snapshot_path.display().to_string()),
        );
    }
    Ok(())
}

fn commit_migration_restore_plans(
    plans: &[MigrationRestorePlan],
    restore_backup_root: &Path,
) -> Result<(Vec<MigrationRestoreJournalEntry>, usize, u64), String> {
    let mut journal = Vec::new();
    for plan in plans {
        let backup_path =
            backup_restore_target_for_transaction(&plan.target_path, restore_backup_root)?;
        journal.push(MigrationRestoreJournalEntry {
            entry_name: plan.entry_name.clone(),
            target_path: plan.target_path.display().to_string(),
            staging_path: plan.staging_path.display().to_string(),
            existed_before: backup_path.is_some(),
            backup_path: backup_path.map(|path| path.display().to_string()),
        });
    }
    write_restore_journal(restore_backup_root, &journal)?;

    let mut committed = Vec::new();
    for (index, plan) in plans.iter().enumerate() {
        if let Err(error) = copy_staged_file_to_target(plan) {
            rollback_migration_restore(&journal, &committed)?;
            return Err(format!("恢复事务提交失败，已尝试回滚：{}", error));
        }
        committed.push(index);
        write_restore_journal(restore_backup_root, &journal)?;
    }

    let committed_count = plans.len();
    let committed_bytes = plans.iter().map(|plan| plan.byte_len).sum();
    Ok((journal, committed_count, committed_bytes))
}

fn backup_restore_target_for_transaction(
    target_path: &Path,
    restore_backup_root: &Path,
) -> Result<Option<PathBuf>, String> {
    if !target_path.exists() || !target_path.is_file() {
        return Ok(None);
    }
    let user_home = user_home_path()?;
    let relative = target_path
        .strip_prefix(&user_home)
        .unwrap_or(target_path)
        .to_path_buf();
    let backup_path = restore_backup_root.join("before").join(relative);
    ensure_parent_dir(&backup_path)?;
    fs::copy(target_path, &backup_path).map_err(|error| {
        format!(
            "创建恢复前备份失败：{}，源：{}，目标：{}",
            error,
            target_path.display(),
            backup_path.display()
        )
    })?;
    Ok(Some(backup_path))
}

fn copy_staged_file_to_target(plan: &MigrationRestorePlan) -> Result<(), String> {
    ensure_parent_dir(&plan.target_path)?;
    fs::copy(&plan.staging_path, &plan.target_path).map_err(|error| {
        format!(
            "写入恢复文件失败：{}，源：{}，目标：{}",
            error,
            plan.staging_path.display(),
            plan.target_path.display()
        )
    })?;
    if let Some(modified) = plan.modified_unix_ms.and_then(unix_millis_to_system_time) {
        restore_file_modified_time(&plan.target_path, modified)?;
    }
    Ok(())
}

fn rollback_migration_restore(
    journal: &[MigrationRestoreJournalEntry],
    committed_indexes: &[usize],
) -> Result<(), String> {
    let mut errors = Vec::new();
    for index in committed_indexes.iter().rev() {
        let Some(entry) = journal.get(*index) else {
            continue;
        };
        let target_path = PathBuf::from(&entry.target_path);
        if let Some(backup_path) = entry.backup_path.as_ref() {
            if let Err(error) = fs::copy(backup_path, &target_path) {
                errors.push(format!("{}: {}", target_path.display(), error));
            }
        } else if target_path.exists() {
            if let Err(error) = fs::remove_file(&target_path) {
                errors.push(format!("{}: {}", target_path.display(), error));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("恢复事务回滚不完整：{}", errors.join("; ")))
    }
}

fn write_restore_journal(
    restore_backup_root: &Path,
    journal: &[MigrationRestoreJournalEntry],
) -> Result<(), String> {
    let path = restore_backup_root.join("restore-journal.json");
    let text = serde_json::to_string_pretty(journal)
        .map_err(|error| format!("生成恢复事务日志失败：{}", error))?;
    fs::write(&path, text)
        .map_err(|error| format!("写入恢复事务日志失败：{}，路径：{}", error, path.display()))
}

fn extract_mcp_blocks(text: &str) -> String {
    let mut output = Vec::new();
    let mut in_mcp_block = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = trimmed.trim_start_matches('[').trim_end_matches(']');
            in_mcp_block = section == "mcp_servers" || section.starts_with("mcp_servers.");
        }
        if in_mcp_block {
            output.push(line.to_string());
        }
    }
    if output.is_empty() {
        String::new()
    } else {
        output.join("\n") + "\n"
    }
}

fn collect_files_for_zip(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root)
        .map_err(|error| format!("读取备份目录失败：{}，路径：{}", error, root.display()))?
    {
        let entry = entry.map_err(|error| format!("读取备份目录项失败：{}", error))?;
        let path = entry.path();
        let metadata = entry.metadata().map_err(|error| {
            format!("读取备份文件信息失败：{}，路径：{}", error, path.display())
        })?;
        if metadata.is_dir() {
            collect_files_for_zip(&path, files)?;
        } else if metadata.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn path_to_zip_name(path: &Path) -> Result<String, String> {
    let parts: Vec<String> = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect();
    normalize_zip_entry_name(&parts.join("/"))
}

fn normalize_zip_entry_name(name: &str) -> Result<String, String> {
    let normalized = name.replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.contains("..")
        || normalized.chars().any(|ch| ch == '\0')
    {
        return Err(format!("ZIP 条目名称不安全：{}", name));
    }
    Ok(normalized)
}

fn checked_u16(value: usize, message: &str) -> Result<u16, String> {
    u16::try_from(value).map_err(|_| message.to_string())
}

fn checked_u32<T>(value: T, message: &str) -> Result<u32, String>
where
    T: TryInto<u32>,
{
    value.try_into().map_err(|_| message.to_string())
}

fn write_u16(writer: &mut std::fs::File, value: u16) -> Result<(), String> {
    writer
        .write_all(&value.to_le_bytes())
        .map_err(|error| format!("写入 ZIP 失败：{}", error))
}

fn write_u32(writer: &mut std::fs::File, value: u32) -> Result<(), String> {
    writer
        .write_all(&value.to_le_bytes())
        .map_err(|error| format!("写入 ZIP 失败：{}", error))
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn read_zip_u16(reader: &mut std::fs::File) -> Result<u16, String> {
    let mut bytes = [0u8; 2];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| format!("读取 ZIP 字段失败：{}", error))?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_zip_u32(reader: &mut std::fs::File) -> Result<u32, String> {
    let mut bytes = [0u8; 4];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| format!("读取 ZIP 字段失败：{}", error))?;
    Ok(u32::from_le_bytes(bytes))
}

fn migration_entry_target_path(name: &str) -> Result<Option<(PathBuf, String)>, String> {
    if matches!(
        name,
        "models/models_cache.json"
            | "models/codex_router_catalog.json"
            | "models/codex_router_catalog_base.json"
            | "sessions/state_5.sqlite"
    ) {
        return Ok(None);
    }

    let path = match name {
        "accounts/current-auth.json" => Some((codex_auth_path()?, "账户".to_string())),
        "accounts/registry.json" => Some((codex_accounts_registry_path()?, "账户".to_string())),
        "models/router_provider_config.json" => Some((provider_config_path()?, "模型".to_string())),
        "models/codex_router_catalog.json" => Some((catalog_config_path()?, "模型".to_string())),
        "models/codex_router_catalog_base.json" => {
            Some((catalog_base_config_path()?, "模型".to_string()))
        }
        "models/models_cache.json" => Some((models_cache_path()?, "模型".to_string())),
        "sessions/session_index.jsonl" => Some((codex_session_index_path()?, "会话".to_string())),
        "sessions/state_5.sqlite" => Some((codex_state_db_path()?, "会话".to_string())),
        "sessions/codex-global-state.json" => {
            Some((codex_global_state_path()?, "会话".to_string()))
        }
        "sessions/codex-global-state.json.bak" => {
            Some((codex_global_state_backup_path()?, "会话".to_string()))
        }
        "app/app_settings.json" => Some((app_settings_path()?, "应用设置".to_string())),
        _ if name.starts_with("accounts/snapshots/") => Some((
            codex_accounts_snapshots_path()?.join(&name["accounts/snapshots/".len()..]),
            "账户".to_string(),
        )),
        _ if name.starts_with("sessions/sessions/") => Some((
            codex_sessions_path()?.join(&name["sessions/sessions/".len()..]),
            "会话".to_string(),
        )),
        _ if name.starts_with("sessions/archived_sessions/") => Some((
            codex_archived_sessions_path()?.join(&name["sessions/archived_sessions/".len()..]),
            "会话".to_string(),
        )),
        _ if name.starts_with("skills/installed/") => Some((
            codex_skills_path()?.join(&name["skills/installed/".len()..]),
            "技能".to_string(),
        )),
        _ if name.starts_with("skills/backups/") => Some((
            skill_backups_path()?.join(&name["skills/backups/".len()..]),
            "技能".to_string(),
        )),
        _ => None,
    };

    if let Some((target, section)) = path {
        ensure_safe_restore_target(&target)?;
        Ok(Some((target, section)))
    } else {
        Ok(None)
    }
}

fn ensure_safe_restore_target(path: &Path) -> Result<(), String> {
    let user_home = user_home_path()?;
    let workspace = build_user_relative_path(WORKSPACE_RELATIVE_PATH)?;
    if path.starts_with(&user_home) || path.starts_with(&workspace) {
        return Ok(());
    }
    Err(format!("恢复目标路径不安全：{}", path.display()))
}

fn remove_mcp_blocks(text: &str) -> String {
    let mut output = Vec::new();
    let mut in_mcp_block = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = trimmed.trim_start_matches('[').trim_end_matches(']');
            in_mcp_block = section == "mcp_servers" || section.starts_with("mcp_servers.");
        }
        if !in_mcp_block {
            output.push(line.to_string());
        }
    }
    output.join("\n")
}

fn normalize_imported_accounts_registry_paths() -> Result<(), String> {
    let registry_path = codex_accounts_registry_path()?;
    if !registry_path.exists() {
        return Ok(());
    }

    let mut registry = read_accounts_registry()?;
    let snapshots_dir = codex_accounts_snapshots_path()?;
    let Some(items) = registry
        .get_mut("items")
        .and_then(|value| value.as_array_mut())
    else {
        return Ok(());
    };

    let mut changed = false;
    for item in items {
        let Some(map) = item.as_object_mut() else {
            continue;
        };
        let Some(account_key) = map
            .get("accountKey")
            .and_then(|value| value.as_str())
            .map(str::to_string)
        else {
            continue;
        };

        let existing_file_name = map
            .get("snapshotPath")
            .and_then(|value| value.as_str())
            .and_then(|path| Path::new(path).file_name())
            .and_then(|file_name| file_name.to_str())
            .map(str::to_string);

        let mut candidates = Vec::new();
        if let Some(file_name) = existing_file_name {
            candidates.push(snapshots_dir.join(file_name));
        }
        candidates.push(snapshots_dir.join(format!(
            "{}.json",
            sanitize_account_key_for_filename(&account_key)
        )));

        let snapshot_path = candidates
            .into_iter()
            .find(|path| path.exists() && path.is_file())
            .or_else(|| find_snapshot_path_by_account_key(&snapshots_dir, &account_key));

        if let Some(snapshot_path) = snapshot_path {
            let next = snapshot_path.display().to_string();
            if map
                .get("snapshotPath")
                .and_then(|value| value.as_str())
                .map(|current| current != next)
                .unwrap_or(true)
            {
                map.insert("snapshotPath".to_string(), serde_json::Value::String(next));
                changed = true;
            }
        }
    }

    if changed {
        write_accounts_registry(&registry)?;
    }
    Ok(())
}

fn find_snapshot_path_by_account_key(snapshots_dir: &Path, account_key: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(snapshots_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| !extension.eq_ignore_ascii_case("json"))
            .unwrap_or(true)
        {
            continue;
        }
        let root = read_json_file_optional(&path)?;
        if build_codex_account_key(&root).as_deref() == Some(account_key) {
            return Some(path);
        }
    }
    None
}















fn test_provider_model_blocking(
    request: TestProviderModelRequest,
) -> Result<ProviderModelTestResult, String> {
    let config = load_provider_config()?;
    let route = select_provider_route_by_slug(&config, request.slug.trim())
        .ok_or_else(|| format!("模型 {} 不存在或已禁用", request.slug.trim()))?;
    let protocol_type = normalize_protocol_type(&route.protocol_type);
    let models_url = build_upstream_models_url(&route.base_url);
    let authorization = format!("Bearer {}", route.api_key);
    let effective_proxy_url = resolve_provider_route_proxy_url(&route);
    let started_at = Instant::now();
    let mut upstream_request = build_upstream_get_request(
        &models_url,
        effective_proxy_url.as_deref(),
        MODEL_TEST_TIMEOUT_SECONDS,
    );
    if is_anthropic_protocol(&protocol_type) {
        upstream_request = upstream_request
            .set(HEADER_ANTHROPIC_API_KEY, &route.api_key)
            .set(HEADER_ANTHROPIC_VERSION, ANTHROPIC_VERSION_VALUE);
    } else {
        upstream_request = upstream_request.set(HEADER_AUTHORIZATION, &authorization);
    }
    let response = upstream_request.call();
    let latency_ms = started_at.elapsed().as_millis();
    let latency = format_latency(latency_ms);

    match response {
        Ok(response) => {
            let status_code = response.status();
            let body = response.into_string().unwrap_or_default();
            let models = parse_provider_model_ids(&body)?;
            let success = models.iter().any(|model| model == &route.real_model);
            let message = if success {
                format!(
                    "模型 {} 连通正常，上游模型列表包含该模型。",
                    route.real_model
                )
            } else {
                format!("接口连通，但上游模型列表未包含 {}。", route.real_model)
            };

            Ok(ProviderModelTestResult {
                slug: request.slug,
                success,
                status_code,
                latency_ms,
                latency,
                url: models_url,
                message,
            })
        }
        Err(ureq::Error::Status(status_code, response)) => {
            let body = response.into_string().unwrap_or_default();
            Ok(ProviderModelTestResult {
                slug: request.slug,
                success: false,
                status_code,
                latency_ms,
                latency,
                url: models_url,
                message: format!("上游返回状态码 {}：{}", status_code, body),
            })
        }
        Err(error) => Ok(ProviderModelTestResult {
            slug: request.slug,
            success: false,
            status_code: 0,
            latency_ms,
            latency,
            url: models_url,
            message: format!("请求失败：{}", error),
        }),
    }
}



fn test_provider_model_chat_blocking(
    request: TestProviderModelRequest,
) -> Result<ProviderModelChatTestResult, String> {
    let config = load_provider_config()?;
    let route = select_provider_route_by_slug(&config, request.slug.trim())
        .ok_or_else(|| format!("模型 {} 不存在或已禁用", request.slug.trim()))?;
    let protocol_type = normalize_protocol_type(&route.protocol_type);
    let (url, body) = build_provider_model_chat_test_request(&route, &protocol_type)?;
    let request_body = body.clone();
    let authorization = format!("Bearer {}", route.api_key);
    let effective_proxy_url = resolve_provider_route_proxy_url(&route);
    let started_at = Instant::now();
    let mut upstream_request = build_upstream_post_request(
        &url,
        effective_proxy_url.as_deref(),
        MODEL_TEST_TIMEOUT_SECONDS,
    )
    .set(HEADER_CONTENT_TYPE, HEADER_JSON);

    if is_anthropic_protocol(&protocol_type) {
        upstream_request = upstream_request
            .set(HEADER_ANTHROPIC_API_KEY, &route.api_key)
            .set(HEADER_ANTHROPIC_VERSION, ANTHROPIC_VERSION_VALUE);
    } else {
        upstream_request = upstream_request.set(HEADER_AUTHORIZATION, &authorization);
    }

    let response = upstream_request.send_string(&body);
    let latency_ms = started_at.elapsed().as_millis();
    let latency = format_latency(latency_ms);

    match response {
        Ok(response) => {
            let status_code = response.status();
            let response_body = response.into_string().unwrap_or_default();
            let response_text = extract_model_chat_test_text(&response_body, &protocol_type);
            let success = status_code < 400 && !response_text.trim().is_empty();
            let message = if success {
                "hello 消息已收到模型响应。".to_string()
            } else {
                "上游返回成功状态码，但没有解析到模型文本响应。".to_string()
            };

            Ok(ProviderModelChatTestResult {
                slug: request.slug,
                success,
                status_code,
                latency_ms,
                latency,
                url,
                protocol_type,
                request_body,
                message,
                response_text,
            })
        }
        Err(ureq::Error::Status(status_code, response)) => {
            let response_body = response.into_string().unwrap_or_default();
            Ok(ProviderModelChatTestResult {
                slug: request.slug,
                success: false,
                status_code,
                latency_ms,
                latency,
                url,
                protocol_type,
                request_body,
                message: format!("上游返回状态码 {}：{}", status_code, response_body),
                response_text: String::new(),
            })
        }
        Err(error) => Ok(ProviderModelChatTestResult {
            slug: request.slug,
            success: false,
            status_code: 0,
            latency_ms,
            latency,
            url,
            protocol_type,
            request_body,
            message: format!("请求失败：{}", error),
            response_text: String::new(),
        }),
    }
}



fn test_proxy_connection_blocking(request: ProxyTestRequest) -> Result<ProxyTestResult, String> {
    let proxy_url = request.proxy_url.trim();

    if proxy_url.is_empty() {
        return Err("代理地址不能为空".to_string());
    }

    test_proxy_url(proxy_url)
}



fn detect_proxy_connection_blocking() -> Result<ProxyTestResult, String> {
    let (sender, receiver) = std::sync::mpsc::channel();

    for proxy_url in PROXY_DETECT_CANDIDATES {
        let sender = sender.clone();
        let proxy_url = proxy_url.to_string();
        thread::spawn(move || {
            let _ = sender.send(test_proxy_url(&proxy_url));
        });
    }
    drop(sender);

    for result in receiver.iter().take(PROXY_DETECT_CANDIDATES.len()) {
        if let Ok(result) = result {
            if result.success {
                return Ok(result);
            }
        }
    }

    Err("未检测到可用代理，请确认代理客户端已启动，或手动填写代理地址。".to_string())
}

fn test_proxy_url(proxy_url: &str) -> Result<ProxyTestResult, String> {
    let proxy =
        ureq::Proxy::new(proxy_url).map_err(|error| format!("代理地址格式无效：{}", error))?;
    let agent = ureq::builder().proxy(proxy).build();
    let started_at = Instant::now();
    let response = agent
        .get(PROXY_TEST_URL)
        .timeout(Duration::from_secs(PROXY_TEST_TIMEOUT_SECONDS))
        .call();
    let latency_ms = started_at.elapsed().as_millis();
    let latency = format_latency(latency_ms);

    match response {
        Ok(response) => {
            let status_code = response.status();
            Ok(ProxyTestResult {
                success: true,
                proxy_url: proxy_url.to_string(),
                latency_ms,
                latency,
                status_code,
                message: format!("代理可用，已连通 chatgpt.com，状态码 {}。", status_code),
            })
        }
        Err(ureq::Error::Status(status_code, _response)) => Ok(ProxyTestResult {
            success: true,
            proxy_url: proxy_url.to_string(),
            latency_ms,
            latency,
            status_code,
            message: format!("代理已连通 chatgpt.com，上游返回状态码 {}。", status_code),
        }),
        Err(error) => Ok(ProxyTestResult {
            success: false,
            proxy_url: proxy_url.to_string(),
            latency_ms,
            latency,
            status_code: 0,
            message: format!("代理不可用：{}", error),
        }),
    }
}





fn refresh_invalid_codex_exe_path(settings: &mut AppSettings) -> Result<bool, String> {
    let configured_path = settings.codex_exe_path.trim();
    if !configured_path.is_empty() && is_usable_codex_start_command(configured_path) {
        return Ok(false);
    }

    let Some(detected_path) = detect_codex_exe_path() else {
        return Ok(false);
    };

    if configured_path == detected_path {
        return Ok(false);
    }

    settings.codex_exe_path = detected_path;
    Ok(true)
}

fn is_usable_codex_start_command(command: &str) -> bool {
    if command.trim().is_empty() {
        return false;
    }

    if command.contains(['\\', '/', ':']) {
        return PathBuf::from(command).exists();
    }

    true
}

fn is_usable_start_command_for_target(command: &str, target: &str) -> bool {
    if !is_usable_codex_start_command(command) {
        return false;
    }

    let normalized = command.to_ascii_lowercase();
    if target.eq_ignore_ascii_case(RESTART_TARGET_CHATGPT) && normalized.contains("codex") {
        return false;
    }

    true
}





fn quick_account_count() -> Result<usize, String> {
    ensure_accounts_registry_file()?;
    let registry = read_accounts_registry()?;
    let Some(items) = registry.get("items").and_then(|value| value.as_array()) else {
        return Ok(0);
    };

    let mut account_keys = HashSet::new();
    for item in items {
        if let Some(account_key) = json_string_field(item, "accountKey") {
            account_keys.insert(account_key);
        }
    }

    Ok(account_keys.len())
}



fn switch_codex_account_blocking(
    request: CodexAccountKeyRequest,
) -> Result<CodexAccountOperationResult, String> {
    normalize_imported_accounts_registry_paths()?;
    let mut registry = read_accounts_registry()?;
    let account = find_registry_account(&registry, &request.account_key)
        .ok_or_else(|| "未找到要切换的账号".to_string())?;
    let snapshot_path_text = json_string_field(&account, "snapshotPath")
        .ok_or_else(|| "账号缺少 snapshotPath，无法切换".to_string())?;
    let snapshot_path = PathBuf::from(&snapshot_path_text);

    if !snapshot_path.exists() {
        return Err(format!("账号快照不存在：{}", snapshot_path.display()));
    }

    backup_current_auth_file()?;
    let auth_path = codex_auth_path()?;
    ensure_parent_dir(&auth_path)?;
    let snapshot_root = read_json_file_optional(&snapshot_path)
        .ok_or_else(|| format!("无法解析账号快照：{}", snapshot_path.display()))?;
    let snapshot_root = prepare_account_snapshot_for_switch(&snapshot_path, &snapshot_root)?;
    let auth_root = build_codex_auth_file_from_snapshot(&snapshot_root)?;
    let auth_text = serde_json::to_string_pretty(&auth_root)
        .map_err(|error| format!("序列化 Codex auth.json 失败：{}", error))?;
    fs::write(&auth_path, auth_text).map_err(|error| {
        format!(
            "写入 Codex auth.json 失败：{}，路径：{}",
            error,
            auth_path.display()
        )
    })?;

    update_registry_active_account(&mut registry, &request.account_key)?;
    write_accounts_registry(&registry)?;

    Ok(CodexAccountOperationResult {
        message: "已切换账号。若 Codex 进程已启动，建议重启后生效。".to_string(),
        path: Some(snapshot_path.display().to_string()),
        scan: scan_codex_accounts()?,
    })
}









fn refresh_codex_accounts_usage_blocking() -> Result<CodexAccountOperationResult, String> {
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



fn refresh_codex_account_usage_blocking(
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

    Err("额度刷新异常，详细信息请查看应用日志".to_string())
}



fn refresh_codex_account_token_blocking(
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

fn refresh_codex_account_token_inner(
    account_key: &str,
) -> Result<CodexAccountOperationResult, String> {
    let mut registry = read_accounts_registry()?;
    let item = find_registry_account(&registry, account_key)
        .ok_or_else(|| "未找到要刷新 Token 的账号。".to_string())?;
    let snapshot_path_text = json_string_field(&item, "snapshotPath")
        .ok_or_else(|| "账号缺少 snapshotPath，无法刷新 Token。".to_string())?;
    let snapshot_path = PathBuf::from(&snapshot_path_text);
    let snapshot_root = read_json_file_optional(&snapshot_path)
        .ok_or_else(|| "账号快照无法读取或解析，请重新授权。".to_string())?;
    let Some(refresh_token) =
        find_string_by_keys(&snapshot_root, &["refresh_token", "refreshToken"])
    else {
        if let Some(session_token) = find_chatgpt_session_cookie_token(&snapshot_root) {
            return refresh_codex_web_session_account_token_inner(
                account_key,
                &mut registry,
                &snapshot_path,
                &snapshot_root,
                &session_token,
            );
        }
        let _ = set_registry_account_token_refresh_failed(&mut registry, account_key, true);
        let _ = write_accounts_registry(&registry);
        return Err("账号快照缺少 refresh_token 或 session_token，请重新授权。".to_string());
    };
    if refresh_token.trim().is_empty() {
        if let Some(session_token) = find_chatgpt_session_cookie_token(&snapshot_root) {
            return refresh_codex_web_session_account_token_inner(
                account_key,
                &mut registry,
                &snapshot_path,
                &snapshot_root,
                &session_token,
            );
        }
        let _ = set_registry_account_token_refresh_failed(&mut registry, account_key, true);
        let _ = write_accounts_registry(&registry);
        return Err("账号快照缺少 refresh_token 或 session_token，请重新授权。".to_string());
    }

    append_internal_app_log(
        "info",
        "accounts",
        "refresh-token",
        "开始刷新账号 Token",
        Some(format!("accountKey={}", mask_secret(account_key))),
    );

    let token_root = match exchange_codex_oauth_refresh_token(&refresh_token) {
        Ok(token_root) => token_root,
        Err(error) => {
            let _ = set_registry_account_token_refresh_failed(
                &mut registry,
                account_key,
                error.permanently_failed,
            );
            let _ = write_accounts_registry(&registry);
            append_internal_app_log(
                "warn",
                "accounts",
                "refresh-token",
                "刷新账号 Token 失败",
                Some(format!(
                    "accountKey={}, permanent={}, error={}",
                    mask_secret(account_key),
                    error.permanently_failed,
                    error.message
                )),
            );
            return Err(error.message);
        }
    };

    let mut merged_token_root = token_root;
    ensure_oauth_refresh_token_field(&mut merged_token_root, &refresh_token);
    let mut refreshed_auth = build_codex_auth_from_oauth_token(&merged_token_root)?;
    preserve_refreshed_auth_metadata(&mut refreshed_auth, &snapshot_root);
    let refreshed_auth = enrich_codex_auth_identity(refreshed_auth);

    upsert_codex_auth_value_account_with_key(&mut registry, &refreshed_auth, account_key, false)?;
    set_registry_account_token_refresh_failed(&mut registry, account_key, false)?;
    write_accounts_registry(&registry)?;

    let current_account_key = read_accounts_registry()
        .ok()
        .and_then(|root| json_string_field(&root, "activeAccountKey"));
    if current_account_key.as_deref() == Some(account_key) {
        let auth_path = codex_auth_path()?;
        ensure_parent_dir(&auth_path)?;
        let auth_root = build_codex_auth_file_from_snapshot(&refreshed_auth)?;
        let auth_text = serde_json::to_string_pretty(&auth_root)
            .map_err(|error| format!("序列化 Codex auth.json 失败：{}", error))?;
        fs::write(&auth_path, auth_text)
            .map_err(|error| format!("写入 Codex auth.json 失败：{}", error))?;
    }

    append_internal_app_log(
        "info",
        "accounts",
        "refresh-token",
        "刷新账号 Token 成功",
        Some(format!("accountKey={}", mask_secret(account_key))),
    );

    Ok(CodexAccountOperationResult {
        message: "Token 已刷新。".to_string(),
        path: Some(snapshot_path.display().to_string()),
        scan: scan_codex_accounts()?,
    })
}

fn refresh_codex_web_session_account_token_inner(
    account_key: &str,
    registry: &mut serde_json::Value,
    snapshot_path: &Path,
    snapshot_root: &serde_json::Value,
    session_token: &str,
) -> Result<CodexAccountOperationResult, String> {
    append_internal_app_log(
        "info",
        "accounts",
        "refresh-token",
        "开始刷新 web_session 账号 Token",
        Some(format!("accountKey={}", mask_secret(account_key))),
    );

    let session_root = match fetch_chatgpt_session_with_cookie(session_token) {
        Ok(session_root) => session_root,
        Err(error) => {
            let _ = set_registry_account_token_refresh_failed(
                registry,
                account_key,
                error.permanently_failed,
            );
            let _ = write_accounts_registry(registry);
            append_internal_app_log(
                "warn",
                "accounts",
                "refresh-token",
                "刷新 web_session 账号 Token 失败",
                Some(format!(
                    "accountKey={}, permanent={}, error={}",
                    mask_secret(account_key),
                    error.permanently_failed,
                    error.message
                )),
            );
            return Err(error.message);
        }
    };

    let mut refreshed_auth = build_codex_auth_from_chatgpt_session(&session_root)?;
    ensure_chatgpt_session_token_field(&mut refreshed_auth, session_token);
    preserve_refreshed_auth_metadata(&mut refreshed_auth, snapshot_root);
    let refreshed_auth = enrich_codex_auth_identity(refreshed_auth);

    upsert_codex_auth_value_account_with_key(registry, &refreshed_auth, account_key, false)?;
    set_registry_account_token_refresh_failed(registry, account_key, false)?;
    write_accounts_registry(registry)?;

    let current_account_key = read_accounts_registry()
        .ok()
        .and_then(|root| json_string_field(&root, "activeAccountKey"));
    if current_account_key.as_deref() == Some(account_key) {
        let auth_path = codex_auth_path()?;
        ensure_parent_dir(&auth_path)?;
        let auth_root = build_codex_auth_file_from_snapshot(&refreshed_auth)?;
        let auth_text = serde_json::to_string_pretty(&auth_root)
            .map_err(|error| format!("序列化 Codex auth.json 失败：{}", error))?;
        fs::write(&auth_path, auth_text)
            .map_err(|error| format!("写入 Codex auth.json 失败：{}", error))?;
    }

    append_internal_app_log(
        "info",
        "accounts",
        "refresh-token",
        "web_session 账号 Token 刷新成功",
        Some(format!("accountKey={}", mask_secret(account_key))),
    );

    Ok(CodexAccountOperationResult {
        message: "web_session Token 已刷新。".to_string(),
        path: Some(snapshot_path.display().to_string()),
        scan: scan_codex_accounts()?,
    })
}

#[allow(dead_code)]
fn start_codex_account_login_legacy() -> Result<CodexAccountOperationResult, String> {
    Err("旧版 Codex 登录入口已停用，请使用当前账号登录流程。".to_string())
}











fn import_chatgpt_session_account_blocking(
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



/// Try to extract the auth object from common CPA wrapper fields.
/// If the root already looks like an auth value (has tokens/access_token),
/// return it as-is. Otherwise, look inside known wrapper keys.
fn unwrap_cpa_auth_value(raw: &serde_json::Value) -> &serde_json::Value {
    // Already looks like an auth value
    if raw.get("tokens").is_some() || find_codex_access_token(raw).is_some() {
        return raw;
    }
    // Common wrapper keys
    for key in &["auth", "authJson", "codexAuth", "data"] {
        if let Some(inner) = raw.get(*key) {
            if inner.is_object()
                && (inner.get("tokens").is_some() || find_codex_access_token(inner).is_some())
            {
                return inner;
            }
        }
    }
    raw
}











fn download_and_install_update_blocking(
    app_handle: tauri::AppHandle,
    request: UpdateInstallRequest,
) -> Result<UpdateInstallResult, String> {
    UPDATE_DOWNLOAD_CANCELED.store(false, Ordering::SeqCst);
    let asset_name = request.asset_name.trim();
    let version = parse_codex_companion_msi_version(asset_name).ok_or_else(|| {
        "Update asset name must match CodexHub_version_arch_locale.msi.".to_string()
    })?;
    if version != normalize_system_version(request.latest_version.clone()) {
        return Err("Update asset version does not match latest version.".to_string());
    }
    if asset_name.contains('/') || asset_name.contains('\\') {
        return Err("Update asset name contains an invalid path separator.".to_string());
    }

    let download_url = request.download_url.trim();
    if !download_url.starts_with("https://github.com/xiaoashuo/CodexHub/releases/download/") {
        return Err("Update download URL is not allowed.".to_string());
    }

    emit_update_progress(
        &app_handle,
        "downloading",
        0,
        None,
        Some(0),
        "Preparing download",
    );

    let update_dir =
        build_user_relative_path(&[".codex", "ai-router-workspace", "cache", "updates"])?;
    fs::create_dir_all(&update_dir)
        .map_err(|error| format!("create update cache directory failed: {}", error))?;
    let installer_path = update_dir.join(asset_name);
    let partial_path = update_dir.join(format!("{}.download", asset_name));

    let proxy_url = load_app_settings()
        .ok()
        .and_then(|settings| normalize_proxy_url(&settings.official_proxy_url));
    let request_builder = match proxy_url.as_deref() {
        Some(proxy_url) => {
            let proxy = ureq::Proxy::new(proxy_url)
                .map_err(|error| format!("update proxy URL is invalid: {}", error))?;
            ureq::builder().proxy(proxy).build().get(download_url)
        }
        None => ureq::get(download_url),
    };
    let response = request_builder
        .set(HEADER_USER_AGENT, "CodexHub")
        .timeout(Duration::from_secs(600))
        .call()
        .map_err(|error| format!("download update failed: {}", error))?;
    let total_bytes = response
        .header("content-length")
        .and_then(|value| value.parse::<u64>().ok());

    let mut reader = response.into_reader();
    let mut writer = std::fs::File::create(&partial_path)
        .map_err(|error| format!("create update file failed: {}", error))?;
    let mut buffer = [0u8; 64 * 1024];
    let mut downloaded_bytes = 0u64;
    let mut last_percent = None;

    loop {
        if UPDATE_DOWNLOAD_CANCELED.load(Ordering::SeqCst) {
            drop(writer);
            let _ = fs::remove_file(&partial_path);
            emit_update_progress(
                &app_handle,
                "canceled",
                downloaded_bytes,
                total_bytes,
                last_percent,
                "Download canceled",
            );
            return Err("Update download canceled.".to_string());
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("read update stream failed: {}", error))?;
        if read == 0 {
            break;
        }
        writer
            .write_all(&buffer[..read])
            .map_err(|error| format!("write update file failed: {}", error))?;
        downloaded_bytes = downloaded_bytes.saturating_add(read as u64);
        let percent = total_bytes
            .filter(|total| *total > 0)
            .map(|total| ((downloaded_bytes.saturating_mul(100) / total).min(100)) as u8);
        if percent != last_percent {
            last_percent = percent;
            emit_update_progress(
                &app_handle,
                "downloading",
                downloaded_bytes,
                total_bytes,
                percent,
                "Downloading update",
            );
        }
    }
    writer
        .flush()
        .map_err(|error| format!("flush update file failed: {}", error))?;
    drop(writer);

    if let Some(total) = total_bytes {
        if downloaded_bytes != total {
            return Err(format!(
                "Downloaded size mismatch: expected {}, got {} bytes.",
                total, downloaded_bytes
            ));
        }
    }

    fs::rename(&partial_path, &installer_path)
        .map_err(|error| format!("finalize update file failed: {}", error))?;
    emit_update_progress(
        &app_handle,
        "installing",
        downloaded_bytes,
        total_bytes,
        Some(100),
        "Starting installer",
    );

    Command::new("msiexec")
        .args(["/i", &installer_path.display().to_string()])
        .spawn()
        .map_err(|error| format!("start installer failed: {}", error))?;

    let exit_handle = app_handle.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(800));
        exit_handle.exit(0);
    });

    Ok(UpdateInstallResult {
        installer_path: installer_path.display().to_string(),
        message: "Installer started.".to_string(),
    })
}

fn emit_update_progress(
    app_handle: &tauri::AppHandle,
    phase: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    percent: Option<u8>,
    message: &str,
) {
    let _ = app_handle.emit(
        "update-download-progress",
        UpdateDownloadProgress {
            phase: phase.to_string(),
            downloaded_bytes,
            total_bytes,
            percent,
            message: message.to_string(),
        },
    );
}

































fn quick_codex_thread_summary_blocking() -> Result<ScanSummary, String> {
    let mut total_threads = 0usize;
    let mut total_size = 0u64;
    let mut archived_threads = 0usize;
    let mut active_days = HashSet::new();

    collect_quick_thread_summary(
        &codex_sessions_path()?,
        false,
        &mut total_threads,
        &mut total_size,
        &mut archived_threads,
        &mut active_days,
    )?;
    collect_quick_thread_summary(
        &codex_archived_sessions_path()?,
        true,
        &mut total_threads,
        &mut total_size,
        &mut archived_threads,
        &mut active_days,
    )?;

    let active_day_count = active_days.len();
    Ok(ScanSummary {
        total_threads,
        total_size,
        active_days: active_day_count,
        average_threads_per_day: if active_day_count == 0 {
            0.0
        } else {
            total_threads as f64 / active_day_count as f64
        },
        indexed_threads: 0,
        missing_from_index: 0,
        archived_threads,
        project_count: 0,
        scanned_at: current_log_time(),
    })
}

fn collect_quick_thread_summary(
    root: &Path,
    archived: bool,
    total_threads: &mut usize,
    total_size: &mut u64,
    archived_threads: &mut usize,
    active_days: &mut HashSet<String>,
) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(root).map_err(|error| {
        format!(
            "read thread directory failed: {}, path: {}",
            error,
            root.display()
        )
    })?;

    for entry_result in entries {
        let Ok(entry) = entry_result else {
            continue;
        };
        let path = entry.path();
        if path.is_dir() {
            collect_quick_thread_summary(
                &path,
                archived,
                total_threads,
                total_size,
                archived_threads,
                active_days,
            )?;
            continue;
        }

        let is_jsonl = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("jsonl"))
            .unwrap_or(false);
        if !is_jsonl {
            continue;
        }

        *total_threads += 1;
        if archived {
            *archived_threads += 1;
        }
        if let Ok(metadata) = fs::metadata(&path) {
            *total_size += metadata.len();
        }
        if let Some(day) = rollout_file_date(
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default(),
        ) {
            active_days.insert(day);
        }
    }

    Ok(())
}

fn scan_codex_threads_blocking() -> Result<ThreadScanResult, String> {
    let index_info = load_session_index_info().unwrap_or_default();
    let mut sessions = Vec::new();

    scan_thread_root(
        &codex_sessions_path()?,
        "sessions",
        false,
        &index_info,
        &mut sessions,
    )?;
    scan_thread_root(
        &codex_archived_sessions_path()?,
        "archived_sessions",
        true,
        &index_info,
        &mut sessions,
    )?;
    sessions.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.title.cmp(&right.title))
    });

    let mut project_map: HashMap<String, Vec<ThreadSession>> = HashMap::new();
    for session in sessions {
        let key = if is_dialog_project_name(&session.project_name) {
            "对话".to_string()
        } else {
            format!(
                "{}:{}",
                session.project_name,
                session.cwd.clone().unwrap_or_default()
            )
        };
        project_map.entry(key).or_default().push(session);
    }

    let mut projects = Vec::new();
    let mut total_threads = 0usize;
    let mut total_size = 0u64;
    let mut indexed_threads = 0usize;
    let mut missing_from_index = 0usize;
    let mut archived_threads = 0usize;
    let mut all_active_days = HashSet::new();

    for project_sessions in project_map.into_values() {
        let Some(first_session) = project_sessions.first().cloned() else {
            continue;
        };
        let mut project_days = HashSet::new();
        let mut project_size = 0u64;

        for session in &project_sessions {
            total_threads += 1;
            total_size += session.file_size;
            project_size += session.file_size;

            if session.indexed {
                indexed_threads += 1;
            }
            if session.missing_from_index {
                missing_from_index += 1;
            }
            if session.archived {
                archived_threads += 1;
            }
            if let Some(day) = session_active_day(session) {
                all_active_days.insert(day.clone());
                project_days.insert(day);
            }
        }

        let is_dialog_project = is_dialog_project_name(&first_session.project_name);
        projects.push(ProjectGroup {
            project_name: if is_dialog_project {
                "对话".to_string()
            } else {
                first_session.project_name
            },
            cwd: if is_dialog_project {
                Some("Documents\\Codex".to_string())
            } else {
                first_session.cwd
            },
            thread_count: project_sessions.len(),
            total_size: project_size,
            active_days: project_days.len(),
            sessions: project_sessions,
        });
    }

    projects.sort_by(|left, right| {
        left.project_name
            .cmp(&right.project_name)
            .then_with(|| right.thread_count.cmp(&left.thread_count))
    });

    let active_days = all_active_days.len();
    let average_threads_per_day = if active_days == 0 {
        0.0
    } else {
        total_threads as f64 / active_days as f64
    };

    Ok(ThreadScanResult {
        summary: ScanSummary {
            total_threads,
            total_size,
            active_days,
            average_threads_per_day,
            indexed_threads,
            missing_from_index,
            archived_threads,
            project_count: projects.len(),
            scanned_at: current_log_time(),
        },
        projects,
    })
}



fn delete_codex_thread_files_blocking(
    request: DeleteCodexThreadFilesRequest,
) -> Result<ThreadScanResult, String> {
    if request.file_paths.is_empty() {
        return scan_codex_threads_blocking();
    }

    let allowed_roots = vec![
        canonicalize_existing_dir(&codex_sessions_path()?)?,
        canonicalize_existing_dir(&codex_archived_sessions_path()?)?,
    ];
    for file_path in request.file_paths {
        let path = PathBuf::from(&file_path);
        let canonical_path = path.canonicalize().map_err(|error| {
            format!(
                "浼氳瘽鏂囦欢涓嶅瓨鍦ㄦ垨鏃犳硶璁块棶：{}锛岃矾寰勶細{}",
                error,
                path.display()
            )
        })?;
        let is_allowed = allowed_roots
            .iter()
            .any(|root| canonical_path.starts_with(root));

        if !is_allowed {
            return Err(format!(
                "拒绝删除非Codex 会话目录内的文件：{}",
                canonical_path.display()
            ));
        }

        if canonical_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("jsonl"))
            .unwrap_or(false)
        {
            fs::remove_file(&canonical_path).map_err(|error| {
                format!(
                    "鍒犻櫎浼氳瘽鏂囦欢澶辫触：{}锛岃矾寰勶細{}",
                    error,
                    canonical_path.display()
                )
            })?;
        } else {
            return Err(format!(
                "鎷掔粷鍒犻櫎闈?jsonl 浼氳瘽鏂囦欢：{}",
                canonical_path.display()
            ));
        }
    }

    scan_codex_threads_blocking()
}





fn parse_restore_codex_thread_index_request(
    value: serde_json::Value,
) -> Result<RestoreCodexThreadIndexRequest, String> {
    let request_value = value.get("request").cloned().unwrap_or(value);
    let Some(request_object) = request_value.as_object() else {
        return Err("解析会话恢复参数失败：request 必须是对象。".to_string());
    };

    let file_paths = match request_object.get("filePaths") {
        Some(serde_json::Value::Array(values)) => values
            .iter()
            .map(|value| {
                value.as_str().map(|path| path.to_string()).ok_or_else(|| {
                    "解析会话恢复参数失败：filePaths 只能包含字符串路径。".to_string()
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => return Err("解析会话恢复参数失败：filePaths 必须是字符串数组。".to_string()),
        None => Vec::new(),
    };

    Ok(RestoreCodexThreadIndexRequest {
        file_paths,
        restore_all: parse_optional_bool_request_field(request_object.get("restoreAll")),
        allow_codex_restart: parse_optional_bool_request_field(
            request_object.get("allowCodexRestart"),
        ),
        move_to_recent: parse_optional_bool_request_field(request_object.get("moveToRecent")),
    })
}

fn parse_optional_bool_request_field(value: Option<&serde_json::Value>) -> bool {
    match value {
        Some(serde_json::Value::Bool(value)) => *value,
        Some(serde_json::Value::String(value)) => value.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

fn check_restore_codex_thread_index_blocking(
    request: RestoreCodexThreadIndexRequest,
) -> Result<RestoreCodexThreadIndexCheckResult, String> {
    let (restore_items, skipped_count) = collect_restore_items_for_request(&request)?;
    let project_roots = restore_project_roots_for_display(&restore_items);
    let codex_running = !restore_items.is_empty() && is_codex_process_running();
    let requires_codex_restart = codex_running && !restore_items.is_empty();
    let message = if requires_codex_restart {
        "本次恢复需要关闭并重启 Codex，避免运行中的 Codex 用内存状态覆盖恢复结果。".to_string()
    } else {
        "Codex 当前未运行，本次恢复可直接写入。".to_string()
    };

    Ok(RestoreCodexThreadIndexCheckResult {
        restore_count: restore_items.len(),
        skipped_count,
        requires_codex_restart,
        codex_running,
        project_roots,
        message,
    })
}

fn collect_restore_items_for_request(
    request: &RestoreCodexThreadIndexRequest,
) -> Result<(Vec<ThreadSession>, usize), String> {
    let index_info = load_session_index_info().unwrap_or_default();
    let sidebar_thread_ids = if request.restore_all {
        load_sidebar_thread_ids_from_global_state().unwrap_or_default()
    } else {
        HashSet::new()
    };
    let mut candidates = Vec::new();

    if request.file_paths.is_empty() {
        if !request.restore_all {
            return Err(
                "未收到要恢复的会话文件，请先勾选具体会话，或使用恢复缺失会话。".to_string(),
            );
        }
        scan_thread_root(
            &codex_sessions_path()?,
            "sessions",
            false,
            &index_info,
            &mut candidates,
        )?;
    } else {
        let sessions_root = canonicalize_existing_dir(&codex_sessions_path()?)?;
        let archived_sessions_root = canonicalize_existing_dir(&codex_archived_sessions_path()?)?;
        let allowed_roots = vec![sessions_root.clone(), archived_sessions_root.clone()];

        for file_path in &request.file_paths {
            let path = PathBuf::from(file_path);
            let canonical_path = path.canonicalize().map_err(|error| {
                format!(
                    "会话文件不存在或无法访问：{}，路径：{}",
                    error,
                    path.display()
                )
            })?;
            let is_allowed = allowed_roots
                .iter()
                .any(|root| canonical_path.starts_with(root));
            if !is_allowed {
                return Err(format!(
                    "拒绝恢复非 Codex 会话目录内的文件：{}",
                    canonical_path.display()
                ));
            }
            if !canonical_path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case("jsonl"))
                .unwrap_or(false)
            {
                return Err(format!(
                    "拒绝恢复非 jsonl 会话文件：{}",
                    canonical_path.display()
                ));
            }

            let archived = canonical_path.starts_with(&archived_sessions_root);
            let source = if archived {
                "archived_sessions"
            } else {
                "sessions"
            };
            if let Ok(session) =
                parse_thread_session_file(&canonical_path, source, archived, &index_info)
            {
                candidates.push(session);
            }
        }
    }

    let affected_restore_all_projects = if request.restore_all {
        candidates
            .iter()
            .filter(|session| {
                !session.archived
                    && (!session.indexed
                        || !sidebar_thread_ids.contains(&session.id)
                        || session.state_needs_repair)
            })
            .filter_map(|session| session.cwd.as_deref())
            .map(normalize_codex_cwd)
            .filter(|cwd| !cwd.trim().is_empty())
            .collect::<HashSet<_>>()
    } else {
        HashSet::new()
    };

    let mut seen_ids = HashSet::new();
    let mut restore_items = Vec::new();
    let mut skipped_count = 0usize;
    for session in candidates {
        let already_visible_in_sidebar = sidebar_thread_ids.contains(&session.id);
        let belongs_to_affected_restore_all_project = session
            .cwd
            .as_deref()
            .map(normalize_codex_cwd)
            .map(|cwd| affected_restore_all_projects.contains(&cwd))
            .unwrap_or(false);
        if session.id.trim().is_empty()
            || !seen_ids.insert(session.id.clone())
            || (request.restore_all
                && (session.archived
                    || (!belongs_to_affected_restore_all_project
                        && session.indexed
                        && already_visible_in_sidebar
                        && !session.state_needs_repair)))
        {
            skipped_count += 1;
            continue;
        }
        restore_items.push(session);
    }

    restore_items.sort_by(|left, right| {
        session_index_updated_at(right)
            .cmp(&session_index_updated_at(left))
            .then_with(|| left.title.cmp(&right.title))
    });

    Ok((restore_items, skipped_count))
}

fn promote_archived_restore_sessions(sessions: &mut [ThreadSession]) -> Result<(), String> {
    let active_root = codex_sessions_path()?;

    for session in sessions.iter_mut().filter(|session| session.archived) {
        let source_path = PathBuf::from(&session.file_path);
        let file_name = source_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                format!(
                    "归档会话文件名无效，无法恢复到 sessions：{}",
                    source_path.display()
                )
            })?;
        let date = rollout_file_date(file_name).ok_or_else(|| {
            format!(
                "归档会话文件名缺少日期，无法恢复到 sessions：{}",
                source_path.display()
            )
        })?;
        let target_path = active_rollout_path(&active_root, &date, file_name);

        if target_path.exists() {
            return Err(format!(
                "恢复归档会话失败，目标文件已存在：{}",
                target_path.display()
            ));
        }

        backup_rollout_session_file(&source_path, &session.id)?;
        ensure_parent_dir(&target_path)?;
        fs::rename(&source_path, &target_path).map_err(|error| {
            format!(
                "移动归档会话到 sessions 失败：{}，源：{}，目标：{}",
                error,
                source_path.display(),
                target_path.display()
            )
        })?;
        session.file_path = target_path.display().to_string();
        session.source = "sessions".to_string();
        session.archived = false;
    }

    Ok(())
}

fn active_rollout_path(active_root: &Path, date: &str, file_name: &str) -> PathBuf {
    active_root
        .join(&date[0..4])
        .join(&date[5..7])
        .join(&date[8..10])
        .join(file_name)
}

fn rollout_file_date(file_name: &str) -> Option<String> {
    let date = file_name.strip_prefix("rollout-")?.get(0..10)?;
    if date.as_bytes().get(4) == Some(&b'-')
        && date.as_bytes().get(7) == Some(&b'-')
        && date
            .chars()
            .enumerate()
            .all(|(index, ch)| matches!(index, 4 | 7) || ch.is_ascii_digit())
    {
        Some(date.to_string())
    } else {
        None
    }
}

fn restore_codex_thread_index_blocking(
    request: RestoreCodexThreadIndexRequest,
) -> Result<RestoreCodexThreadIndexResult, String> {
    let (mut restore_items, skipped_count) = collect_restore_items_for_request(&request)?;

    if restore_items.is_empty() {
        return Ok(RestoreCodexThreadIndexResult {
            restored_count: 0,
            skipped_count,
            backup_path: None,
            message: "没有需要恢复的缺失会话索引。".to_string(),
            scan: scan_codex_threads_blocking()?,
        });
    }

    let state_backup_path = backup_codex_thread_state()?;
    let target_provider =
        read_current_codex_model_provider().unwrap_or_else(|| CODEX_PROVIDER_NAME.to_string());
    let selected_ids = restore_items
        .iter()
        .map(|session| session.id.clone())
        .collect::<HashSet<_>>();
    let restart_project_roots = restore_project_roots_for_display(&restore_items);
    let restart_codex_after_restore = if is_codex_process_running() {
        if !request.allow_codex_restart {
            let project_message = if restart_project_roots.is_empty() {
                "无项目目录".to_string()
            } else {
                restart_project_roots.join("；")
            };
            return Err(format!(
                "本次恢复需要关闭并重启 Codex 才能安全写入会话状态，请确认后重试。涉及项目：{}",
                project_message
            ));
        }
        close_codex_process_for_state_write()?;
        wait_for_codex_process_exit(Duration::from_secs(15))?;
        true
    } else {
        false
    };
    promote_archived_restore_sessions(&mut restore_items)?;
    upsert_missing_sqlite_threads_from_sessions(&restore_items, &target_provider)?;
    sync_selected_sqlite_thread_metadata(&restore_items, &target_provider, request.move_to_recent)?;
    checkpoint_codex_state_db()?;
    repair_rollout_sessions(
        &restore_items,
        &target_provider,
        true,
        request.move_to_recent,
    )?;
    if request.move_to_recent {
        touch_rollout_session_files(&restore_items)?;
    }
    let index_backup_path =
        rebuild_session_index_with_sessions(&restore_items, request.move_to_recent)?;
    let sqlite_sidebar_rows = load_restorable_sidebar_rows(&selected_ids).unwrap_or_default();
    let sidebar_rows = merge_sidebar_rows_for_restore(&restore_items, sqlite_sidebar_rows);
    rebuild_global_state_for_sidebar_rows(&sidebar_rows, false, false, request.move_to_recent)?;
    verify_thread_restore_state(&restore_items)?;
    let backup_path = state_backup_path.or(index_backup_path);
    let restored_count = restore_items.len();
    let codex_restart_message = if restart_codex_after_restore {
        let restart_result = start_codex_process_result();
        if restart_result.success {
            reinforce_thread_restore_state_after_codex_start(
                &restore_items,
                &target_provider,
                &selected_ids,
                request.move_to_recent,
            )?;
        }
        Some(restart_result.message)
    } else {
        None
    };
    let message = match codex_restart_message {
        Some(restart_message) => format!(
            "已恢复 {} 个会话到 session_index.jsonl。已关闭并重新启动 Codex：{}",
            restored_count, restart_message
        ),
        None => format!(
            "已恢复 {} 个会话到 session_index.jsonl。重启 Codex 后通常会重新出现在会话列表中。",
            restored_count
        ),
    };

    Ok(RestoreCodexThreadIndexResult {
        restored_count,
        skipped_count,
        backup_path: backup_path.map(|path| path.display().to_string()),
        message,
        scan: scan_codex_threads_blocking()?,
    })
}











fn prepare_router_startup_blocking(
    request: RouterStartupPreparationRequest,
) -> Result<RouterStartupPreparationResult, String> {
    let codex_config_path = codex_config_path()?;
    if !codex_config_path.exists() {
        return Err(format!(
            "Codex 配置文件不存在：{}",
            codex_config_path.display()
        ));
    }

    let models_cache_path = models_cache_path()?;
    if !models_cache_path.exists() {
        return Err(format!(
            "Codex 模型缓存文件不存在：{}",
            models_cache_path.display()
        ));
    }

    ensure_provider_config_file()?;
    let managed_slugs = provider_route_slugs()?;
    let models_cache_text = fs::read_to_string(&models_cache_path).map_err(|error| {
        format!(
            "读取 Codex models_cache.json 失败：{}，路径：{}",
            error,
            models_cache_path.display()
        )
    })?;
    let models_cache_root = serde_json::from_str::<serde_json::Value>(&models_cache_text).map_err(|error| {
        format!(
            "解析 Codex models_cache.json 失败：{}，路径：{}",
            error,
            models_cache_path.display()
        )
    })?;
    let base_root = clean_catalog_root(models_cache_root, &managed_slugs)
        .ok_or_else(|| "Codex models_cache.json 缺少有效 models 数据".to_string())?;
    write_catalog_root(&catalog_base_config_path()?, &base_root)?;
    let sync_catalog_result = sync_catalog_from_provider_config()?;
    let occupancy = build_port_occupancy_info();
    let killed_port_owner = false;

    Ok(RouterStartupPreparationResult {
        router_mode: if request.router_mode == 1 { 1 } else { 0 },
        codex_config_path: codex_config_path.display().to_string(),
        catalog_path: catalog_config_path()?.display().to_string(),
        provider_config_path: provider_config_path()?.display().to_string(),
        sync_catalog_result,
        port_occupancy: occupancy,
        killed_port_owner,
    })
}


fn restart_router_blocking() -> Result<RouterCommandResult, String> {
    let was_running = router_status()?.started;

    if was_running {
        drop(stop_router_runtime_blocking()?);
    } else {
        let occupancy = build_port_occupancy_info();
        if occupancy.occupied {
            return Err(format_port_occupancy_error(&occupancy));
        }
    }

    start_router_blocking()
}

fn router_state() -> &'static Mutex<RouterRuntime> {
    ROUTER_STATE.get_or_init(|| {
        Mutex::new(RouterRuntime {
            started: false,
            started_at: None,
            stop_signal: None,
            handle: None,
            pid: None,
            port: default_router_port(),
        })
    })
}

fn router_logs() -> &'static Mutex<Vec<RouterLogEntry>> {
    ROUTER_LOGS.get_or_init(|| Mutex::new(Vec::new()))
}

fn codex_oauth_state() -> &'static Mutex<Option<CodexOAuthLoginState>> {
    CODEX_OAUTH_STATE.get_or_init(|| Mutex::new(None))
}

fn codex_oauth_last_result() -> &'static Mutex<Option<CodexOAuthLoginStatus>> {
    CODEX_OAUTH_LAST_RESULT.get_or_init(|| Mutex::new(None))
}

fn set_codex_oauth_last_result(status: CodexOAuthLoginStatus) {
    if let Ok(mut last_result) = codex_oauth_last_result().lock() {
        *last_result = Some(status);
    }
}

fn format_listener_bind_error(error: std::io::Error, router_port: u16) -> String {
    if error.kind() == ErrorKind::AddrInUse
        || error.raw_os_error() == Some(ADDRESS_IN_USE_ERROR_CODE)
    {
        format!(
            "router address {}:{} is already in use",
            ROUTER_HOST, router_port
        )
    } else {
        format!(
            "failed to bind router to {}:{}: {}",
            ROUTER_HOST, router_port, error
        )
    }
}

fn build_port_occupancy_info() -> PortOccupancyInfo {
    let router_port = configured_router_port();
    match TcpListener::bind((ROUTER_HOST, router_port)) {
        Ok(listener) => {
            drop(listener);
            return PortOccupancyInfo {
                occupied: false,
                host: ROUTER_HOST.to_string(),
                port: router_port,
                pid: None,
                process_name: String::new(),
                process_path: String::new(),
            };
        }
        Err(error)
            if error.kind() != ErrorKind::AddrInUse
                && error.raw_os_error() != Some(ADDRESS_IN_USE_ERROR_CODE) =>
        {
            return PortOccupancyInfo {
                occupied: false,
                host: ROUTER_HOST.to_string(),
                port: router_port,
                pid: None,
                process_name: String::new(),
                process_path: String::new(),
            };
        }
        Err(_) => {}
    }

    let output = hidden_command("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "$connection = Get-NetTCPConnection -LocalPort {} -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1; if ($connection) {{ $process = Get-Process -Id $connection.OwningProcess -ErrorAction SilentlyContinue; Write-Output ($connection.OwningProcess.ToString() + '|' + $process.ProcessName + '|' + $process.Path) }}",
                router_port
            ),
        ])
        .output();

    if let Ok(output) = output {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() {
            let mut parts = text.splitn(3, '|');
            let pid = parts.next().and_then(|value| value.parse::<u32>().ok());
            let process_name = normalize_port_owner_field(parts.next());
            let process_path = normalize_port_owner_field(parts.next());

            return PortOccupancyInfo {
                occupied: true,
                host: ROUTER_HOST.to_string(),
                port: router_port,
                pid,
                process_name,
                process_path,
            };
        }
    }

    PortOccupancyInfo {
        occupied: true,
        host: ROUTER_HOST.to_string(),
        port: router_port,
        pid: None,
        process_name: UNKNOWN_PORT_OWNER_VALUE.to_string(),
        process_path: UNKNOWN_PORT_OWNER_VALUE.to_string(),
    }
}

fn format_port_occupancy_error(occupancy: &PortOccupancyInfo) -> String {
    format!(
        "router address {}:{} is already in use by pid={}, process={}, path={}",
        occupancy.host,
        occupancy.port,
        occupancy
            .pid
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| UNKNOWN_PORT_OWNER_VALUE.to_string()),
        occupancy.process_name,
        occupancy.process_path,
    )
}

fn normalize_port_owner_field(value: Option<&str>) -> String {
    let value = value.unwrap_or_default().trim();
    if value.is_empty() {
        UNKNOWN_PORT_OWNER_VALUE.to_string()
    } else {
        value.to_string()
    }
}

fn kill_process_by_port_occupancy(occupancy: &PortOccupancyInfo) -> Result<(), String> {
    let pid = occupancy
        .pid
        .ok_or_else(|| "端口已占用，但无法识别占用进程PID".to_string())?;

    if pid == std::process::id() {
        stop_router_blocking()?;
        return Ok(());
    }

    let status = hidden_command("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("Stop-Process -Id {} -Force -ErrorAction Stop", pid),
        ])
        .status()
        .map_err(|error| format!("执行端口占用进程结束命令失败：{}", error))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("结束端口占用进程失败：PID {}", pid))
    }
}

fn restart_codex_process() -> String {
    restart_codex_process_result().message
}

fn current_restart_target() -> String {
    load_app_settings()
        .map(|settings| normalize_restart_target(&settings.app_restart_target))
        .unwrap_or_else(|_| default_restart_target())
}

fn restart_target_display_name(target: &str) -> &'static str {
    if target.eq_ignore_ascii_case(RESTART_TARGET_CODEX) {
        "Codex"
    } else {
        "ChatGPT"
    }
}

fn restart_target_process_names(target: &str) -> &'static [&'static str] {
    if target.eq_ignore_ascii_case(RESTART_TARGET_CODEX) {
        &["Codex", "codex"]
    } else {
        &["ChatGPT", "chatgpt"]
    }
}

fn restart_target_path_patterns(target: &str) -> &'static [&'static str] {
    if target.eq_ignore_ascii_case(RESTART_TARGET_CODEX) {
        &["*\\OpenAI\\Codex\\*", "*\\OpenAI.Codex_*"]
    } else {
        &[
            "*\\OpenAI\\ChatGPT\\*",
            "*\\ChatGPT\\*",
            "*\\OpenAI.ChatGPT_*",
            // Codex Desktop is distributed as the OpenAI.Codex MSIX package,
            // even though its visible UI processes are named ChatGPT.exe.
            // Match the whole package so its codex backend cannot keep the
            // old Router connection alive and respawn the UI during restart.
            "*\\OpenAI.Codex_*",
        ]
    }
}

fn restart_target_executable_name(target: &str) -> &'static str {
    if target.eq_ignore_ascii_case(RESTART_TARGET_CODEX) {
        "Codex.exe"
    } else {
        "ChatGPT.exe"
    }
}

fn target_process_filter_script(current_pid: u32, target: &str) -> String {
    let name_conditions = restart_target_process_names(target)
        .iter()
        .map(|name| format!("$_.Name -ieq '{}'", name.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(" -or ");
    let path_conditions = restart_target_path_patterns(target)
        .iter()
        .map(|pattern| format!("$_.Path -like '{}'", pattern.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(" -or ");

    format!(
        "$currentPid = {}; Get-Process -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -ne $currentPid -and (({}) -or ($_.Path -ne $null -and ({}))) }}",
        current_pid, name_conditions, path_conditions
    )
}

fn is_target_process_running(target: &str) -> bool {
    let current_pid = std::process::id();
    let script = format!(
        "$processes = @({}); if ($processes.Count -gt 0) {{ exit 0 }} else {{ exit 1 }}",
        target_process_filter_script(current_pid, target)
    );
    hidden_command("powershell")
        .args(["-NoProfile", "-Command", &script])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn is_codex_process_running() -> bool {
    is_target_process_running(RESTART_TARGET_CODEX)
}

fn wait_for_target_process_exit(target: &str, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !is_target_process_running(target) {
            thread::sleep(Duration::from_millis(500));
            if !is_target_process_running(target) {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(250));
    }

    Err(format!(
        "关闭 {} 后仍检测到进程，请手动关闭后重试。",
        restart_target_display_name(target)
    ))
}

fn wait_for_codex_process_exit(timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !is_codex_process_running() {
            thread::sleep(Duration::from_millis(500));
            if !is_codex_process_running() {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(250));
    }

    Err(
        "关闭 Codex 后仍检测到 Codex 进程，已停止恢复写入，避免运行中的内存状态覆盖会话恢复结果。"
            .to_string(),
    )
}

fn close_target_process(target: &str) -> Result<(), String> {
    let current_pid = std::process::id();
    let process_filter = target_process_filter_script(current_pid, target);
    let executable_name = restart_target_executable_name(target);
    let display_name = restart_target_display_name(target);
    let script = format!(
        "$processes = @({process_filter}); if ($processes.Count -gt 0) {{ taskkill /IM {executable_name} 2>$null | Out-Null; $deadline = (Get-Date).AddSeconds(8); do {{ Start-Sleep -Milliseconds 250; $processes = @({process_filter}) }} while ($processes.Count -gt 0 -and (Get-Date) -lt $deadline); $processes = @({process_filter}); if ($processes.Count -gt 0) {{ $processes | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 2500 }} }}"
    );
    let output = hidden_command("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output();
    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(format!(
            "关闭 {} 失败，退出码：{}{}",
            display_name,
            output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "未知".to_string()),
            format_command_error_detail(&output.stderr),
        )),
        Err(error) => Err(format!("关闭 {} 命令执行失败：{}", display_name, error)),
    }
}

fn close_codex_process_for_state_write() -> Result<(), String> {
    close_target_process(RESTART_TARGET_CODEX).map_err(|error| {
        format!(
            "{}，无法安全写入新的 Codex 配置和会话状态。",
            error.trim_end_matches('。')
        )
    })
}

fn restart_codex_process_result() -> CodexRestartResult {
    let target = current_restart_target();
    if is_target_process_running(&target) {
        if let Err(error) = close_target_process(&target)
            .and_then(|_| wait_for_target_process_exit(&target, Duration::from_secs(15)))
        {
            return CodexRestartResult {
                success: false,
                message: error,
            };
        }
    }

    thread::sleep(Duration::from_secs(3));
    let result = start_codex_process_result();
    if result.success {
        CodexRestartResult {
            success: true,
            message: format!("{} 已重启。", restart_target_display_name(&target)),
        }
    } else {
        result
    }
}

fn start_codex_process_result() -> CodexRestartResult {
    let target = current_restart_target();
    let display_name = restart_target_display_name(&target);
    let Some(start_command) = resolve_codex_start_command() else {
        return CodexRestartResult {
            success: false,
            message: format!(
                "未检测到 {} 启动命令，请在设置里填写启动命令。",
                display_name
            ),
        };
    };

    if !is_usable_codex_start_command(&start_command) {
        return CodexRestartResult {
            success: false,
            message: format!("{} 启动命令不可用：{}", display_name, start_command),
        };
    }

    let start_script = build_app_start_script(&target, &start_command);
    let output = hidden_command("powershell")
        .args(["-NoProfile", "-Command", &start_script])
        .output();

    match output {
        Ok(output) if output.status.success() => CodexRestartResult {
            success: true,
            message: format!("{} 已启动。", display_name),
        },
        Ok(output) => CodexRestartResult {
            success: false,
            message: format!(
                "启动 {} 命令执行失败，退出码：{}{}",
                display_name,
                output
                    .status
                    .code()
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "未知".to_string()),
                format_command_error_detail(&output.stderr),
            ),
        },
        Err(error) => CodexRestartResult {
            success: false,
            message: format!("启动 {} 命令执行失败：{}", display_name, error),
        },
    }
}

fn find_app_aumid_by_name_pattern(name_pattern: &str) -> Option<String> {
    let command = format!(
        "Get-StartApps | Where-Object {{ $_.Name -match '{}' }} | Select-Object -First 1 -ExpandProperty AppID",
        name_pattern.replace('\'', "''")
    );
    let output = hidden_command("powershell")
        .args(["-NoProfile", "-Command", &command])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn build_app_start_script(target: &str, start_command: &str) -> String {
    if start_command_is_windows_apps_package(start_command) {
        let name_pattern = if target.eq_ignore_ascii_case(RESTART_TARGET_CODEX) {
            "^Codex$"
        } else {
            "^ChatGPT$"
        };
        if let Some(app_id) = find_app_aumid_by_name_pattern(name_pattern) {
            return format!(
                "Start-Process explorer.exe {}",
                powershell_single_quoted_string(&format!("shell:AppsFolder\\{}", app_id))
            );
        }
        if target.eq_ignore_ascii_case(RESTART_TARGET_CHATGPT) {
            return "Start-Process explorer.exe 'shell:AppsFolder\\OpenAI.Codex_2p2nqsd0c76g0!App'"
                .to_string();
        }
    }

    format!(
        "Start-Process -FilePath {}",
        powershell_single_quoted_string(start_command)
    )
}

fn start_command_is_windows_apps_package(start_command: &str) -> bool {
    let normalized = start_command.replace('/', "\\").to_ascii_lowercase();
    normalized.contains("\\windowsapps\\")
        && (normalized.contains("openai.codex_") || normalized.contains("openai.chatgpt"))
}

fn resolve_codex_start_command() -> Option<String> {
    let settings = read_app_settings().unwrap_or_default();
    let configured_path = settings.codex_exe_path.trim();
    let target = normalize_restart_target(&settings.app_restart_target);
    if !configured_path.is_empty() && is_usable_start_command_for_target(configured_path, &target) {
        return Some(configured_path.to_string());
    }

    detect_codex_exe_path()
}

fn powershell_single_quoted_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn format_command_error_detail(stderr: &[u8]) -> String {
    let detail = String::from_utf8_lossy(stderr).trim().to_string();
    if detail.is_empty() {
        String::new()
    } else {
        format!("，详情：{}", truncate_text(&detail, 180))
    }
}

fn build_router_result(runtime: &RouterRuntime, status: &str) -> RouterCommandResult {
    let uptime_seconds = runtime
        .started_at
        .map(|started_at| started_at.elapsed().as_secs())
        .unwrap_or_default();
    let router_port = if runtime.handle.is_some() {
        runtime.port
    } else {
        configured_router_port()
    };

    RouterCommandResult {
        status: status.to_string(),
        service: SERVICE_NAME.to_string(),
        version: SERVICE_VERSION.to_string(),
        host: ROUTER_HOST.to_string(),
        port: router_port,
        pid: runtime.pid,
        health_path: HEALTH_PATH.to_string(),
        health_url: format!("http://{}:{}{}", ROUTER_HOST, router_port, HEALTH_PATH),
        uptime_seconds,
        started: runtime.started,
        forwarding_enabled: load_provider_config().is_ok(),
        concurrency_limit: configured_router_concurrency_limit(),
    }
}

fn is_stop_requested(stop_signal: &Arc<Mutex<bool>>) -> bool {
    stop_signal
        .lock()
        .map(|requested| *requested)
        .unwrap_or(true)
}

struct ConnectionSlotPermit {
    slots: Arc<(Mutex<usize>, Condvar)>,
    acquired: bool,
}

impl Drop for ConnectionSlotPermit {
    fn drop(&mut self) {
        if !self.acquired {
            return;
        }

        let (lock, cvar) = &*self.slots;
        if let Ok(mut active) = lock.lock() {
            *active = active.saturating_sub(1);
            cvar.notify_one();
        }
    }
}

fn acquire_connection_slot(
    slots: Arc<(Mutex<usize>, Condvar)>,
    limit: usize,
    stop_signal: &Arc<Mutex<bool>>,
) -> ConnectionSlotPermit {
    let normalized_limit = normalize_router_concurrency_limit(limit);
    let (lock, cvar) = &*slots;
    let mut active = match lock.lock() {
        Ok(active) => active,
        Err(_) => {
            return ConnectionSlotPermit {
                slots: Arc::clone(&slots),
                acquired: false,
            }
        }
    };

    while *active >= normalized_limit && !is_stop_requested(stop_signal) {
        active = match cvar.wait(active) {
            Ok(active) => active,
            Err(_) => {
                return ConnectionSlotPermit {
                    slots: Arc::clone(&slots),
                    acquired: false,
                }
            }
        };
    }

    if is_stop_requested(stop_signal) {
        return ConnectionSlotPermit {
            slots: Arc::clone(&slots),
            acquired: false,
        };
    }

    *active = active.saturating_add(1);
    drop(active);
    ConnectionSlotPermit {
        slots: Arc::clone(&slots),
        acquired: true,
    }
}

fn handle_connection(
    mut stream: TcpStream,
    started_at: Instant,
    router_port: u16,
) -> std::io::Result<RouterLogEntry> {
    let request_started_at = Instant::now();
    let source_ip = stream
        .peer_addr()
        .map(|address| address.ip().to_string())
        .unwrap_or_else(|_| EMPTY_LOG_VALUE.to_string());
    let request = read_http_request(&stream)?;
    if let Some(streamed_result) =
        try_stream_custom_responses_request(&request, &mut stream, &source_ip, request_started_at)
    {
        return streamed_result;
    }
    if let Some(streamed_result) =
        try_stream_official_responses_request(&request, &mut stream, &source_ip, request_started_at)
    {
        return streamed_result;
    }

    let response = route_request(&request, started_at, router_port);
    let status_line = build_status_line(codex_sse_transport_status_code(
        response.status_code,
        &response.content_type,
    ));
    let usage = response.usage.clone().unwrap_or_default();

    if response.flush_headers_before_body {
        write_streaming_response_headers(&mut stream, &status_line, &response.content_type)?;
        stream.write_all(response.body.as_bytes())?;
        stream.flush()?;
    } else {
        write_http_response(
            &mut stream,
            &status_line,
            &response.content_type,
            response.body.as_bytes(),
        )?;
    }

    Ok(RouterLogEntry {
        time: current_log_time(),
        source_ip,
        method: request.method,
        path: request.path,
        status: response.status_code.to_string(),
        target_provider: response.target_provider,
        cost: format!("{}ms", request_started_at.elapsed().as_millis()),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cached_input_tokens: usage.cached_input_tokens,
        total_tokens: usage.total_tokens,
        usage_source: response.usage_source,
        error_detail: response.error_detail,
    })
}

fn try_stream_custom_responses_request(
    request: &ParsedRequest,
    stream: &mut TcpStream,
    source_ip: &str,
    request_started_at: Instant,
) -> Option<std::io::Result<RouterLogEntry>> {
    let clean_path = request_path_without_query(&request.path);
    if request.method != "POST" || clean_path != RESPONSES_PATH {
        return None;
    }
    if request.body.len() > MAX_REQUEST_BODY_BYTES {
        return None;
    }

    let payload = serde_json::from_slice::<serde_json::Value>(&request.body).ok()?;
    let requested_model = payload
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let config = load_provider_config().ok()?;
    let route = select_provider_route(&config, Some(requested_model))?;
    if !custom_route_can_stream(&payload, &route) {
        return None;
    }

    Some(stream_custom_responses_request(
        request,
        stream,
        source_ip,
        request_started_at,
        payload,
        route,
    ))
}

fn custom_route_can_stream(payload: &serde_json::Value, route: &ProviderRoute) -> bool {
    let protocol_type = normalize_protocol_type(&route.protocol_type);
    if request_uses_image_generation_tool(payload) {
        return false;
    }
    if protocol_type == "cpamc" {
        return true;
    }
    if protocol_type == "openai" || protocol_type == "other" {
        return true;
    }
    false
}

fn stream_custom_responses_request(
    request: &ParsedRequest,
    stream: &mut TcpStream,
    source_ip: &str,
    request_started_at: Instant,
    mut payload: serde_json::Value,
    route: ProviderRoute,
) -> std::io::Result<RouterLogEntry> {
    let active_exec_cell_ids = collect_active_exec_cell_ids(&payload);
    guard_wait_tool_for_upstream(&mut payload);
    let protocol_type = normalize_protocol_type(&route.protocol_type);
    let use_codex_chat_tool_bridge =
        protocol_type == "cpamc" && request_has_codex_custom_tools(&payload);
    let effective_protocol_type = if use_codex_chat_tool_bridge {
        "openai".to_string()
    } else {
        protocol_type.clone()
    };
    let mut effective_route = route.clone();
    if use_codex_chat_tool_bridge {
        effective_route.protocol_type = "openai".to_string();
        effective_route.endpoint_path = CHAT_COMPLETIONS_ENDPOINT_SUFFIX.to_string();
    }
    let available_tool_names = collect_available_tool_names(&payload);
    let (upstream_url, upstream_body) = build_custom_streaming_upstream_request(
        &mut payload,
        &effective_route,
        &effective_protocol_type,
    )
    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let full_debug_id = format!("custom-stream-{}", current_log_millis());
    append_router_full_debug_log(
        "custom_streaming_request",
        serde_json::json!({
            "debug_id": full_debug_id,
            "source_ip": source_ip,
            "method": request.method,
            "path": request.path,
            "target_provider": route.provider.clone(),
            "protocol_type": protocol_type.clone(),
            "effective_protocol_type": effective_protocol_type.clone(),
            "codex_chat_tool_bridge": use_codex_chat_tool_bridge,
            "upstream_url": upstream_url.clone(),
            "client_request_body": router_full_debug_body_value(&String::from_utf8_lossy(&request.body)),
            "normalized_payload": payload.clone(),
            "upstream_body": router_full_debug_body_value(&upstream_body)
        }),
    );
    append_router_debug_log(
        "custom_streaming_request",
        serde_json::json!({
            "debug_id": full_debug_id,
            "source_ip": source_ip,
            "method": request.method,
            "path": request.path,
            "target_provider": route.provider.clone(),
            "protocol_type": protocol_type.clone(),
            "effective_protocol_type": effective_protocol_type.clone(),
            "codex_chat_tool_bridge": use_codex_chat_tool_bridge,
            "upstream_url": upstream_url.clone(),
            "client_request_body": router_debug_body_value(&String::from_utf8_lossy(&request.body)),
            "normalized_payload": payload.clone(),
            "upstream_body": router_debug_body_value(&upstream_body)
        }),
    );
    let uses_image_generation_tool = request_uses_image_generation_tool(&payload);
    if uses_image_generation_tool {
        append_router_debug_log(
            "custom_streaming_image_generation_request",
            serde_json::json!({
                "requested_model": payload.get("model").cloned(),
                "target_provider": route.provider.clone(),
                "protocol_type": protocol_type.clone(),
                "upstream_url": upstream_url.clone(),
                "payload": payload.clone(),
                "upstream_body": router_debug_body_value(&upstream_body)
            }),
        );
    }
    let authorization = format!("Bearer {}", route.api_key);
    let effective_proxy_url = resolve_provider_route_proxy_url(&route);
    let upstream_result = send_custom_upstream_request_with_retries(
        &upstream_url,
        effective_proxy_url.as_deref(),
        &effective_protocol_type,
        &route.api_key,
        &authorization,
        &upstream_body,
        custom_upstream_timeout_seconds(uses_image_generation_tool),
    );

    let mut status_code;
    let mut error_detail = EMPTY_LOG_VALUE.to_string();
    let mut usage = TokenUsage::default();
    let mut usage_source = TOKEN_USAGE_SOURCE_MISSING.to_string();

    match upstream_result {
        Ok(response) => {
            status_code = response.status();
            let content_type = response
                .header(HEADER_CONTENT_TYPE)
                .unwrap_or(HEADER_JSON)
                .to_string();
            if !content_type
                .to_ascii_lowercase()
                .contains(HEADER_EVENT_STREAM)
            {
                let body = response.into_string().unwrap_or_default();
                append_router_full_debug_log(
                    "custom_streaming_upstream_response",
                    serde_json::json!({
                        "debug_id": full_debug_id,
                        "target_provider": route.provider.clone(),
                        "protocol_type": protocol_type.clone(),
                        "status_code": status_code,
                        "content_type": content_type,
                        "upstream_body": router_full_debug_body_value(&body)
                    }),
                );
                let uses_image_generation_tool = request_uses_image_generation_tool(&payload);
                let image_generation_needs_notice = uses_image_generation_tool
                    && custom_image_generation_response_needs_notice(&body, &content_type);
                let upstream_empty_text =
                    custom_upstream_response_is_empty_text(&body, &effective_protocol_type);
                append_router_debug_log(
                    "custom_streaming_upstream_response",
                    serde_json::json!({
                        "debug_id": full_debug_id,
                        "requested_model": payload.get("model").cloned(),
                        "target_provider": route.provider.clone(),
                        "protocol_type": effective_protocol_type.clone(),
                        "status_code": status_code,
                        "content_type": content_type,
                        "uses_image_generation_tool": uses_image_generation_tool,
                        "image_generation_needs_notice": image_generation_needs_notice,
                        "upstream_empty_text": upstream_empty_text,
                        "contains_image_generation_result": response_contains_image_generation_result(&body),
                        "extracted_text": extract_text_for_debug(&body, &content_type, &effective_protocol_type),
                        "raw_body": router_debug_body_value(&body)
                    }),
                );
                if image_generation_needs_notice {
                    let response = codex_sse_error_response(
                        HTTP_BAD_GATEWAY,
                        "custom_image_generation_not_supported",
                        CUSTOM_IMAGE_GENERATION_UNSUPPORTED_MESSAGE,
                        route.provider.clone(),
                    );
                    error_detail = response.error_detail.clone();
                    write_http_response(
                        stream,
                        &build_status_line(codex_sse_transport_status_code(
                            response.status_code,
                            &response.content_type,
                        )),
                        &response.content_type,
                        response.body.as_bytes(),
                    )?;
                    status_code = response.status_code;
                    return Ok(RouterLogEntry {
                        time: current_log_time(),
                        source_ip: source_ip.to_string(),
                        method: request.method.clone(),
                        path: request.path.clone(),
                        status: status_code.to_string(),
                        target_provider: route.provider,
                        cost: format!("{}ms", request_started_at.elapsed().as_millis()),
                        input_tokens: usage.input_tokens,
                        output_tokens: usage.output_tokens,
                        cached_input_tokens: usage.cached_input_tokens,
                        total_tokens: usage.total_tokens,
                        usage_source,
                        error_detail,
                    });
                }
                if upstream_empty_text {
                    let (error_code, error_message) =
                        if custom_empty_response_is_image_generation_unsupported(
                            uses_image_generation_tool,
                            &body,
                            &content_type,
                            &effective_protocol_type,
                        ) {
                            (
                                "custom_image_generation_not_supported",
                                CUSTOM_IMAGE_GENERATION_UNSUPPORTED_MESSAGE,
                            )
                        } else {
                            (
                                "upstream_empty_response",
                                CUSTOM_UPSTREAM_EMPTY_RESPONSE_MESSAGE,
                            )
                        };
                    let response = codex_sse_error_response(
                        HTTP_BAD_GATEWAY,
                        error_code,
                        error_message,
                        route.provider.clone(),
                    );
                    error_detail = response.error_detail.clone();
                    write_http_response(
                        stream,
                        &build_status_line(codex_sse_transport_status_code(
                            response.status_code,
                            &response.content_type,
                        )),
                        &response.content_type,
                        response.body.as_bytes(),
                    )?;
                    status_code = response.status_code;
                    return Ok(RouterLogEntry {
                        time: current_log_time(),
                        source_ip: source_ip.to_string(),
                        method: request.method.clone(),
                        path: request.path.clone(),
                        status: status_code.to_string(),
                        target_provider: route.provider,
                        cost: format!("{}ms", request_started_at.elapsed().as_millis()),
                        input_tokens: usage.input_tokens,
                        output_tokens: usage.output_tokens,
                        cached_input_tokens: usage.cached_input_tokens,
                        total_tokens: usage.total_tokens,
                        usage_source,
                        error_detail,
                    });
                }
                let body = if effective_protocol_type == "cpamc" {
                    ensure_responses_stream_completed(
                        normalize_repeated_tool_names_in_body_with_available(
                            &body,
                            &available_tool_names,
                        ),
                        &content_type,
                    )
                } else {
                    let wrapped = wrap_chat_response_as_responses(
                        &body,
                        &effective_route,
                        &effective_protocol_type,
                        &payload,
                    );
                    ensure_responses_stream_completed(wrapped, &content_type)
                };
                let response_usage = extract_token_usage_from_body(&body);
                usage = response_usage.clone().unwrap_or_default();
                usage_source = token_usage_source(response_usage.as_ref());
                append_router_full_debug_log(
                    "custom_streaming_router_response",
                    serde_json::json!({
                        "debug_id": full_debug_id,
                        "target_provider": route.provider.clone(),
                        "protocol_type": protocol_type.clone(),
                        "status_code": status_code,
                        "content_type": HEADER_EVENT_STREAM,
                        "router_body": router_full_debug_body_value(&body)
                    }),
                );
                write_http_response(
                    stream,
                    &build_status_line(status_code),
                    HEADER_EVENT_STREAM,
                    body.as_bytes(),
                )?;
            } else if effective_protocol_type == "cpamc" {
                append_router_full_debug_log(
                    "custom_streaming_upstream_sse_start",
                    serde_json::json!({
                        "debug_id": full_debug_id,
                        "target_provider": route.provider.clone(),
                        "protocol_type": protocol_type.clone(),
                        "status_code": status_code,
                        "content_type": content_type
                    }),
                );
                write_streaming_response_headers(
                    stream,
                    &build_status_line(status_code),
                    HEADER_EVENT_STREAM,
                )?;
                if let Err(error) = stream_raw_upstream_sse(
                    response,
                    stream,
                    Some(&full_debug_id),
                    &available_tool_names,
                    &active_exec_cell_ids,
                ) {
                    error_detail = format!("upstream stream disconnected: {}", error);
                }
            } else {
                append_router_full_debug_log(
                    "custom_streaming_upstream_sse_start",
                    serde_json::json!({
                        "debug_id": full_debug_id,
                        "target_provider": route.provider.clone(),
                        "protocol_type": protocol_type.clone(),
                        "status_code": status_code,
                        "content_type": content_type
                    }),
                );
                write_streaming_response_headers(
                    stream,
                    &build_status_line(status_code),
                    HEADER_EVENT_STREAM,
                )?;
                let uses_image_generation_tool = request_uses_image_generation_tool(&payload);
                let available_tool_names = collect_available_tool_names(&payload);
                let custom_tool_names = collect_custom_tool_names(&payload);
                let namespace_tool_mappings = collect_namespace_tool_mappings(&payload);
                if let Err(error) = stream_openai_chat_sse_as_codex(
                    response,
                    stream,
                    &effective_route,
                    uses_image_generation_tool,
                    &available_tool_names,
                    &custom_tool_names,
                    &namespace_tool_mappings,
                    Some(&full_debug_id),
                ) {
                    error_detail = format!("upstream stream disconnected: {}", error);
                }
            }
        }
        Err(ureq::Error::Status(upstream_status, response)) => {
            status_code = upstream_status;
            let body = response.into_string().unwrap_or_default();
            append_router_debug_log(
                "custom_streaming_upstream_status",
                serde_json::json!({
                    "debug_id": full_debug_id,
                    "target_provider": route.provider.clone(),
                    "protocol_type": protocol_type.clone(),
                    "effective_protocol_type": effective_protocol_type.clone(),
                    "status_code": upstream_status,
                    "upstream_request_body": router_debug_body_value(&upstream_body),
                    "upstream_response_body": router_debug_body_value(&body)
                }),
            );
            append_router_full_debug_log(
                "custom_streaming_upstream_status",
                serde_json::json!({
                    "debug_id": full_debug_id,
                    "target_provider": route.provider.clone(),
                    "protocol_type": protocol_type.clone(),
                    "status_code": upstream_status,
                    "upstream_body": router_full_debug_body_value(&body)
                }),
            );
            let upstream_message = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|value| {
                    value
                        .get("error")
                        .and_then(|error| error.get("message"))
                        .and_then(|message| message.as_str())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "no structured upstream error message".to_string());
            error_detail = format!(
                "upstream status {}: {}; tools=[{}]",
                upstream_status,
                truncate_text(&upstream_message, 500),
                responses_tool_summary_for_log(&payload)
            );
            let error_code = if upstream_status == HTTP_TOO_MANY_REQUESTS {
                "upstream_rate_limited"
            } else {
                "upstream_status"
            };
            let response = codex_sse_error_response(
                upstream_status,
                error_code,
                &format!("upstream returned status {}: {}", upstream_status, body),
                route.provider.clone(),
            );
            write_http_response(
                stream,
                &build_status_line(codex_sse_transport_status_code(
                    response.status_code,
                    &response.content_type,
                )),
                &response.content_type,
                response.body.as_bytes(),
            )?;
        }
        Err(error) => {
            status_code = HTTP_BAD_GATEWAY;
            error_detail = format_custom_upstream_error(&error);
            if upstream_error_is_retryable(&error) {
                write_streaming_response_headers(
                    stream,
                    &build_status_line(HTTP_OK),
                    HEADER_EVENT_STREAM,
                )?;
                write_codex_stream_disconnect_error(stream, &error_detail)?;
            } else {
                let response = codex_sse_error_response(
                    HTTP_BAD_GATEWAY,
                    "upstream_request_failed",
                    &error_detail,
                    route.provider.clone(),
                );
                write_http_response(
                    stream,
                    &build_status_line(codex_sse_transport_status_code(
                        response.status_code,
                        &response.content_type,
                    )),
                    &response.content_type,
                    response.body.as_bytes(),
                )?;
            }
        }
    }

    Ok(RouterLogEntry {
        time: current_log_time(),
        source_ip: source_ip.to_string(),
        method: request.method.clone(),
        path: request.path.clone(),
        status: status_code.to_string(),
        target_provider: route.provider,
        cost: format!("{}ms", request_started_at.elapsed().as_millis()),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cached_input_tokens: usage.cached_input_tokens,
        total_tokens: usage.total_tokens,
        usage_source,
        error_detail,
    })
}

fn try_stream_official_responses_request(
    request: &ParsedRequest,
    stream: &mut TcpStream,
    source_ip: &str,
    request_started_at: Instant,
) -> Option<std::io::Result<RouterLogEntry>> {
    let clean_path = request_path_without_query(&request.path);
    if request.method != "POST" || clean_path != RESPONSES_PATH {
        return None;
    }
    if request.body.len() > MAX_REQUEST_BODY_BYTES {
        return None;
    }

    let payload = serde_json::from_slice::<serde_json::Value>(&request.body).ok()?;
    let requested_model = payload
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if is_configured_custom_model(requested_model) {
        return None;
    }

    Some(stream_official_responses_request(
        request,
        stream,
        source_ip,
        request_started_at,
        payload,
    ))
}

fn stream_official_responses_request(
    request: &ParsedRequest,
    stream: &mut TcpStream,
    source_ip: &str,
    request_started_at: Instant,
    mut payload: serde_json::Value,
) -> std::io::Result<RouterLogEntry> {
    let status_code: u16;
    let mut error_detail = EMPTY_LOG_VALUE.to_string();
    let mut usage = TokenUsage::default();
    let mut usage_source = TOKEN_USAGE_SOURCE_MISSING.to_string();

    let credentials = match load_codex_auth_credentials() {
        Ok(credentials) => credentials,
        Err(error) => {
            status_code = HTTP_SERVICE_UNAVAILABLE;
            error_detail = error.clone();
            let response = json_response(
                HTTP_SERVICE_UNAVAILABLE,
                "{\"error\":\"codex_auth_missing\"}".to_string(),
                OFFICIAL_TARGET_PROVIDER,
                error,
            );
            write_http_response(
                stream,
                &build_status_line(response.status_code),
                &response.content_type,
                response.body.as_bytes(),
            )?;
            return Ok(build_router_log_entry(
                request,
                source_ip,
                request_started_at,
                status_code,
                OFFICIAL_TARGET_PROVIDER.to_string(),
                usage,
                usage_source,
                error_detail,
            ));
        }
    };

    let forward_settings = load_official_codex_forward_settings();
    normalize_official_codex_payload(&mut payload);
    let upstream_body = match serde_json::to_string(&payload) {
        Ok(body) => body,
        Err(error) => {
            status_code = HTTP_BAD_GATEWAY;
            error_detail = error.to_string();
            let response = json_response(
                HTTP_BAD_GATEWAY,
                "{\"error\":\"serialize_official_body_failed\"}".to_string(),
                OFFICIAL_TARGET_PROVIDER,
                error_detail.clone(),
            );
            write_http_response(
                stream,
                &build_status_line(response.status_code),
                &response.content_type,
                response.body.as_bytes(),
            )?;
            return Ok(build_router_log_entry(
                request,
                source_ip,
                request_started_at,
                status_code,
                OFFICIAL_TARGET_PROVIDER.to_string(),
                usage,
                usage_source,
                error_detail,
            ));
        }
    };
    let full_debug_id = format!("official-stream-{}", current_log_millis());
    append_router_full_debug_log(
        "official_request",
        serde_json::json!({
            "debug_id": full_debug_id,
            "source_ip": source_ip,
            "method": request.method,
            "path": request.path,
            "target_provider": OFFICIAL_TARGET_PROVIDER,
            "client_request_body": router_full_debug_body_value(&String::from_utf8_lossy(&request.body)),
            "upstream_body": router_full_debug_body_value(&upstream_body)
        }),
    );
    let authorization = format!("Bearer {}", credentials.access_token);
    let upstream_result = send_official_codex_request_with_retries(
        &forward_settings,
        &authorization,
        &credentials.account_id,
        &upstream_body,
    );

    match upstream_result {
        Ok(response) => {
            status_code = response.status();
            let content_type = response
                .header(HEADER_CONTENT_TYPE)
                .unwrap_or(HEADER_EVENT_STREAM)
                .to_string();
            if content_type
                .to_ascii_lowercase()
                .contains("text/event-stream")
            {
                write_streaming_response_headers(
                    stream,
                    &build_status_line(status_code),
                    HEADER_EVENT_STREAM,
                )?;
                append_router_full_debug_log(
                    "official_upstream_sse_start",
                    serde_json::json!({
                        "debug_id": full_debug_id,
                        "target_provider": OFFICIAL_TARGET_PROVIDER,
                        "status_code": status_code,
                        "content_type": content_type
                    }),
                );
                match stream_official_codex_sse(response, stream, Some(&full_debug_id)) {
                    Ok(response_usage) => {
                        usage = response_usage.clone().unwrap_or_default();
                        usage_source = token_usage_source(response_usage.as_ref());
                    }
                    Err(error) => {
                        error_detail = format!("official upstream stream disconnected: {}", error);
                    }
                }
            } else {
                let body = response.into_string().unwrap_or_default();
                append_router_full_debug_log(
                    "official_upstream_response",
                    serde_json::json!({
                        "debug_id": full_debug_id,
                        "target_provider": OFFICIAL_TARGET_PROVIDER,
                        "status_code": status_code,
                        "content_type": content_type,
                        "upstream_body": router_full_debug_body_value(&body)
                    }),
                );
                let body = ensure_official_sse_completed(body);
                append_router_full_debug_log(
                    "official_router_response",
                    serde_json::json!({
                        "debug_id": full_debug_id,
                        "target_provider": OFFICIAL_TARGET_PROVIDER,
                        "status_code": status_code,
                        "content_type": HEADER_EVENT_STREAM,
                        "router_body": router_full_debug_body_value(&body)
                    }),
                );
                let response_usage = extract_token_usage_from_body(&body);
                usage = response_usage.clone().unwrap_or_default();
                usage_source = token_usage_source(response_usage.as_ref());
                write_http_response(
                    stream,
                    &build_status_line(status_code),
                    HEADER_EVENT_STREAM,
                    body.as_bytes(),
                )?;
            }
        }
        Err(ureq::Error::Status(upstream_status, response)) => {
            status_code = upstream_status;
            error_detail = format!("official upstream status {}", upstream_status);
            let body = response.into_string().unwrap_or_default();
            append_router_full_debug_log(
                "official_upstream_status",
                serde_json::json!({
                    "debug_id": full_debug_id,
                    "target_provider": OFFICIAL_TARGET_PROVIDER,
                    "status_code": upstream_status,
                    "upstream_body": router_full_debug_body_value(&body)
                }),
            );
            let body = ensure_official_sse_completed(body);
            let response_usage = extract_token_usage_from_body(&body);
            usage = response_usage.clone().unwrap_or_default();
            usage_source = token_usage_source(response_usage.as_ref());
            write_http_response(
                stream,
                &build_status_line(upstream_status),
                HEADER_EVENT_STREAM,
                body.as_bytes(),
            )?;
        }
        Err(error) => {
            status_code = HTTP_BAD_GATEWAY;
            error_detail = format_official_upstream_error(&error, &forward_settings);
            let response = json_response(
                HTTP_BAD_GATEWAY,
                format!(
                    "{{\"error\":\"official_upstream_request_failed\",\"message\":{}}}",
                    json_string(&error_detail)
                ),
                OFFICIAL_TARGET_PROVIDER,
                error_detail.clone(),
            );
            write_http_response(
                stream,
                &build_status_line(response.status_code),
                &response.content_type,
                response.body.as_bytes(),
            )?;
        }
    }

    Ok(build_router_log_entry(
        request,
        source_ip,
        request_started_at,
        status_code,
        OFFICIAL_TARGET_PROVIDER.to_string(),
        usage,
        usage_source,
        error_detail,
    ))
}

fn build_router_log_entry(
    request: &ParsedRequest,
    source_ip: &str,
    request_started_at: Instant,
    status_code: u16,
    target_provider: String,
    usage: TokenUsage,
    usage_source: String,
    error_detail: String,
) -> RouterLogEntry {
    let error_detail = if (200..300).contains(&status_code) {
        EMPTY_LOG_VALUE.to_string()
    } else {
        error_detail
    };

    RouterLogEntry {
        time: current_log_time(),
        source_ip: source_ip.to_string(),
        method: request.method.clone(),
        path: request.path.clone(),
        status: status_code.to_string(),
        target_provider,
        cost: format!("{}ms", request_started_at.elapsed().as_millis()),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cached_input_tokens: usage.cached_input_tokens,
        total_tokens: usage.total_tokens,
        usage_source,
        error_detail,
    }
}

fn read_http_request(stream: &TcpStream) -> std::io::Result<ParsedRequest> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let (method, path) = parse_request_line(&request_line);
    let mut content_length = 0usize;
    let mut headers = HashMap::new();

    loop {
        let mut header_line = String::new();
        let bytes_read = reader.read_line(&mut header_line)?;

        if bytes_read == 0 || header_line == "\r\n" || header_line == "\n" {
            break;
        }

        if let Some((name, value)) = header_line.split_once(':') {
            let normalized_name = name.trim().to_ascii_lowercase();
            let normalized_value = value.trim().to_string();
            if normalized_name == "content-length" {
                content_length = normalized_value.parse::<usize>().unwrap_or_default();
            }
            headers.insert(normalized_name, normalized_value);
        }
    }

    if content_length > MAX_REQUEST_BODY_BYTES {
        return Ok(ParsedRequest {
            method,
            path,
            headers,
            body: Vec::new(),
        });
    }

    let mut body = vec![0; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    Ok(ParsedRequest {
        method,
        path,
        headers,
        body,
    })
}

fn route_request(request: &ParsedRequest, started_at: Instant, router_port: u16) -> RouterResponse {
    let clean_path = request_path_without_query(&request.path);

    if request.method == "GET" && clean_path == HEALTH_PATH {
        return json_response(
            HTTP_OK,
            build_health_response(started_at, router_port),
            EMPTY_LOG_VALUE,
            EMPTY_LOG_VALUE,
        );
    }

    if request.method == "OPTIONS"
        && (clean_path == RESPONSES_PATH || clean_path == CHAT_COMPLETIONS_PATH)
    {
        return json_response(
            HTTP_NO_CONTENT,
            String::new(),
            EMPTY_LOG_VALUE,
            EMPTY_LOG_VALUE,
        );
    }

    if clean_path == RESPONSES_PATH && request.method != "POST" {
        return json_response(
            HTTP_METHOD_NOT_ALLOWED,
            RESPONSE_METHOD_NOT_ALLOWED.to_string(),
            EMPTY_LOG_VALUE,
            "仅支持 POST".to_string(),
        );
    }

    if request.method == "POST" && clean_path == RESPONSES_PATH {
        if request.body.len() > MAX_REQUEST_BODY_BYTES {
            return json_response(
                HTTP_PAYLOAD_TOO_LARGE,
                "{\"error\":\"request_body_too_large\"}".to_string(),
                EMPTY_LOG_VALUE,
                "请求体超过最大限制".to_string(),
            );
        }

        return forward_responses_request(&request.body);
    }

    json_response(
        HTTP_NOT_FOUND,
        RESPONSE_NOT_FOUND.to_string(),
        EMPTY_LOG_VALUE,
        EMPTY_LOG_VALUE,
    )
}

fn handle_codex_oauth_callback(path: &str) -> RouterResponse {
    let query = path
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or_default();
    let params = parse_query_params(query);

    if let Some(error) = params.get("error") {
        return html_response(
            HTTP_BAD_REQUEST,
            format!("<h2>Codex OAuth 登录失败</h2><p>{}</p>", escape_html(error)),
            EMPTY_LOG_VALUE,
            error.clone(),
        );
    }

    let Some(code) = params
        .get("code")
        .filter(|value| !value.trim().is_empty())
        .cloned()
    else {
        return html_response(
            HTTP_BAD_REQUEST,
            "<h2>Codex OAuth 回调缺少 code</h2>".to_string(),
            EMPTY_LOG_VALUE,
            "oauth code missing",
        );
    };
    let Some(returned_state) = params
        .get("state")
        .filter(|value| !value.trim().is_empty())
        .cloned()
    else {
        return html_response(
            HTTP_BAD_REQUEST,
            "<h2>Codex OAuth 回调缺少 state</h2>".to_string(),
            EMPTY_LOG_VALUE,
            "oauth state missing",
        );
    };

    let pending = {
        let oauth_state = codex_oauth_state();
        let mut guard = match oauth_state.lock() {
            Ok(guard) => guard,
            Err(error) => {
                return html_response(
                    HTTP_SERVICE_UNAVAILABLE,
                    "<h2>OAuth 状态读取失败</h2>".to_string(),
                    EMPTY_LOG_VALUE,
                    error.to_string(),
                )
            }
        };
        guard.take()
    };

    let Some(pending) = pending else {
        return html_response(
            HTTP_BAD_REQUEST,
            "<h2>未找到待完成的 OAuth 登录</h2><p>请回到应用重新点击添加账号。</p>".to_string(),
            EMPTY_LOG_VALUE,
            "oauth pending state missing",
        );
    };

    if pending.created_at.elapsed() > Duration::from_secs(10 * 60) {
        return html_response(
            HTTP_BAD_REQUEST,
            "<h2>OAuth 登录已过期</h2><p>请回到应用重新点击添加账号。</p>".to_string(),
            EMPTY_LOG_VALUE,
            "oauth state expired",
        );
    }

    if pending.state != returned_state {
        return html_response(
            HTTP_BAD_REQUEST,
            "<h2>OAuth state 校验失败</h2>".to_string(),
            EMPTY_LOG_VALUE,
            "oauth state mismatch",
        );
    }

    match finish_codex_oauth_login(&code, &pending.code_verifier) {
        Ok(account_key) => html_response(
            HTTP_OK,
            format!("<h2>Codex 账号添加成功</h2><p>账号已保存到项目账号管理：{}</p><p>现在可以关闭这个页面。</p>", escape_html(&account_key)),
            "codex-oauth",
            EMPTY_LOG_VALUE,
        ),
        Err(error) => html_response(
            HTTP_SERVICE_UNAVAILABLE,
            format!("<h2>Codex OAuth 换取 token 失败</h2><p>{}</p>", escape_html(&error)),
            EMPTY_LOG_VALUE,
            error,
        ),
    }
}

fn ensure_codex_oauth_callback_listener() -> Result<(), String> {
    CODEX_OAUTH_CALLBACK_LISTENER
        .get_or_init(start_codex_oauth_callback_listener)
        .clone()
}

fn start_codex_oauth_callback_listener() -> Result<(), String> {
    let callback_port = configured_oauth_callback_port();
    let _ = CODEX_OAUTH_CALLBACK_LISTENER_PORT.set(callback_port);
    let listener = TcpListener::bind((ROUTER_HOST, callback_port)).map_err(|error| {
        format!(
            "启动 OAuth 回调监听失败：{}。请确认 http://{}:{} 没有被其他程序占用。",
            error, ROUTER_HOST, callback_port
        )
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("设置 OAuth 回调监听失败：{}", error))?;

    thread::spawn(move || loop {
        match listener.accept() {
            Ok((mut stream, address)) => {
                let source_ip = address.ip().to_string();
                let mut is_oauth_callback = false;
                let response = match read_http_request(&stream) {
                    Ok(request) => {
                        let clean_path = request_path_without_query(&request.path);
                        is_oauth_callback =
                            request.method == "GET" && clean_path == OAUTH_CALLBACK_PATH;
                        if is_oauth_callback {
                            expire_codex_oauth_state_if_needed();
                            handle_codex_oauth_callback(&request.path)
                        } else {
                            route_account_service_request(&request, &source_ip)
                        }
                    }
                    Err(error) => html_response(
                        HTTP_BAD_REQUEST,
                        format!(
                            "<h2>读取 OAuth 回调失败</h2><p>{}</p>",
                            escape_html(&error.to_string())
                        ),
                        EMPTY_LOG_VALUE,
                        error.to_string(),
                    ),
                };
                if is_oauth_callback && response.status_code != HTTP_OK {
                    set_codex_oauth_last_result(CodexOAuthLoginStatus {
                        status: "error".to_string(),
                        message: if response.error_detail.is_empty() {
                            "OAuth 回调失败。".to_string()
                        } else {
                            response.error_detail.clone()
                        },
                        account_key: None,
                        account_email: None,
                    });
                }
                let status_line = build_status_line(response.status_code);
                let _ = write_http_response(
                    &mut stream,
                    &status_line,
                    &response.content_type,
                    response.body.as_bytes(),
                );
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(LISTENER_IDLE_SLEEP_MS))
            }
            Err(_) => break,
        }
    });

    Ok(())
}

fn route_account_service_request(request: &ParsedRequest, source_ip: &str) -> RouterResponse {
    let started_at = Instant::now();
    let clean_path = request_path_without_query(&request.path);

    if request.method == "OPTIONS" && is_account_proxy_path(&clean_path) {
        return json_response(
            HTTP_NO_CONTENT,
            String::new(),
            EMPTY_LOG_VALUE,
            EMPTY_LOG_VALUE,
        );
    }

    if !is_account_proxy_path(&clean_path) {
        return html_response(
            HTTP_NOT_FOUND,
            "<h2>account service path invalid</h2>".to_string(),
            EMPTY_LOG_VALUE,
            "account service path invalid",
        );
    }

    let protocol = account_proxy_protocol_for_path(&clean_path);
    let model = account_proxy_request_model(request).unwrap_or_else(|| EMPTY_LOG_VALUE.to_string());
    let stream = account_proxy_request_stream(request);
    let account = load_codex_auth_credentials()
        .map(|credentials| mask_secret(&credentials.account_id))
        .unwrap_or_else(|_| EMPTY_LOG_VALUE.to_string());
    let response = route_account_service_request_inner(request, clean_path);
    let usage = response.usage.clone().unwrap_or_default();

    let _ = append_account_proxy_log_entry(&AccountProxyLogEntry {
        time: current_log_time(),
        source_ip: source_ip.to_string(),
        method: request.method.clone(),
        path: clean_path.to_string(),
        protocol: protocol.clone(),
        model: model.clone(),
        stream,
        status: response.status_code.to_string(),
        cost: format!("{}ms", started_at.elapsed().as_millis()),
        account: account.clone(),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cached_input_tokens: usage.cached_input_tokens,
        total_tokens: usage.total_tokens,
        usage_source: response.usage_source.clone(),
        error_detail: response.error_detail.clone(),
    });

    response
}

fn route_account_service_request_inner(
    request: &ParsedRequest,
    clean_path: &str,
) -> RouterResponse {
    let settings = match read_app_settings() {
        Ok(settings) => settings,
        Err(error) => {
            return json_response(
                HTTP_SERVICE_UNAVAILABLE,
                format!(
                    "{{\"error\":\"account_proxy_settings_unavailable\",\"message\":{}}}",
                    json_string(&error)
                ),
                EMPTY_LOG_VALUE,
                error,
            )
        }
    };
    let account_proxy = settings.account_proxy;

    if !account_proxy.account_proxy_enabled {
        return json_response(
            HTTP_FORBIDDEN,
            "{\"error\":\"account_proxy_disabled\"}".to_string(),
            EMPTY_LOG_VALUE,
            "account proxy disabled",
        );
    }

    if !validate_account_proxy_api_key(request, &account_proxy.api_key) {
        return json_response(
            HTTP_UNAUTHORIZED,
            "{\"error\":\"invalid_api_key\"}".to_string(),
            EMPTY_LOG_VALUE,
            "invalid account proxy api key",
        );
    }

    match (request.method.as_str(), clean_path) {
        ("GET", ACCOUNT_PROXY_MODELS_PATH) => account_proxy_models_response(),
        ("POST", ACCOUNT_PROXY_RESPONSES_PATH) => account_proxy_responses_request(request),
        ("POST", ACCOUNT_PROXY_CHAT_COMPLETIONS_PATH) => {
            account_proxy_chat_completions_request(request)
        }
        ("POST", ACCOUNT_PROXY_MESSAGES_PATH) => account_proxy_messages_request(request),
        _ => json_response(
            HTTP_METHOD_NOT_ALLOWED,
            RESPONSE_METHOD_NOT_ALLOWED.to_string(),
            EMPTY_LOG_VALUE,
            "account proxy method not allowed",
        ),
    }
}

fn account_proxy_protocol_for_path(path: &str) -> String {
    match path {
        ACCOUNT_PROXY_MODELS_PATH => "models",
        ACCOUNT_PROXY_RESPONSES_PATH => "responses",
        ACCOUNT_PROXY_CHAT_COMPLETIONS_PATH => "chat_completions",
        ACCOUNT_PROXY_MESSAGES_PATH => "messages",
        _ => EMPTY_LOG_VALUE,
    }
    .to_string()
}

fn account_proxy_request_model(request: &ParsedRequest) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(&request.body)
        .ok()
        .and_then(|root| {
            root.get("model")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

fn account_proxy_request_stream(request: &ParsedRequest) -> bool {
    serde_json::from_slice::<serde_json::Value>(&request.body)
        .ok()
        .and_then(|root| root.get("stream").and_then(|value| value.as_bool()))
        .unwrap_or(false)
}

fn is_account_proxy_path(path: &str) -> bool {
    path == ACCOUNT_PROXY_MODELS_PATH
        || path == ACCOUNT_PROXY_RESPONSES_PATH
        || path == ACCOUNT_PROXY_CHAT_COMPLETIONS_PATH
        || path == ACCOUNT_PROXY_MESSAGES_PATH
}

fn validate_account_proxy_api_key(request: &ParsedRequest, expected_api_key: &str) -> bool {
    let expected = expected_api_key.trim();
    if expected.is_empty() {
        return false;
    }

    let authorization_value = request
        .headers
        .get("authorization")
        .map(|value| value.trim())
        .unwrap_or_default();
    let bearer_value = authorization_value
        .strip_prefix("Bearer ")
        .or_else(|| authorization_value.strip_prefix("bearer "))
        .unwrap_or(authorization_value)
        .trim();
    let x_api_key = request
        .headers
        .get("x-api-key")
        .map(|value| value.trim())
        .unwrap_or_default();

    bearer_value == expected || x_api_key == expected
}

fn account_proxy_models_response() -> RouterResponse {
    let models = load_account_proxy_catalog_models().unwrap_or_default();
    let mut root = serde_json::Map::new();
    root.insert(
        "object".to_string(),
        serde_json::Value::String("list".to_string()),
    );
    root.insert("data".to_string(), serde_json::Value::Array(models));
    json_response(
        HTTP_OK,
        serde_json::Value::Object(root).to_string(),
        OFFICIAL_TARGET_PROVIDER,
        EMPTY_LOG_VALUE,
    )
}

fn load_account_proxy_catalog_models() -> Option<Vec<serde_json::Value>> {
    let model_ids = load_account_proxy_model_ids();
    let mut models = Vec::new();

    for model_id in model_ids {
        let mut model = serde_json::Map::new();
        model.insert("id".to_string(), serde_json::Value::String(model_id));
        model.insert(
            "object".to_string(),
            serde_json::Value::String("model".to_string()),
        );
        model.insert(
            "created".to_string(),
            serde_json::Value::Number(serde_json::Number::from(0)),
        );
        model.insert(
            "owned_by".to_string(),
            serde_json::Value::String("openai".to_string()),
        );
        models.push(serde_json::Value::Object(model));
    }

    Some(models)
}

fn load_account_proxy_catalog_model_ids() -> Option<Vec<String>> {
    let path = catalog_config_path().ok()?;
    let text = fs::read_to_string(path).ok()?;
    let root = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    let source_models = root.get(CATALOG_MODELS_KEY)?.as_array()?;
    let mut model_ids = Vec::new();

    for source_model in source_models {
        let Some(model_id) = source_model
            .get("id")
            .or_else(|| source_model.get("slug"))
            .or_else(|| source_model.get("name"))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };

        model_ids.push(model_id.to_string());
    }

    Some(model_ids)
}

fn load_account_proxy_model_ids() -> Vec<String> {
    let mut model_ids = load_account_proxy_catalog_model_ids().unwrap_or_default();
    for item in enabled_provider_routes().unwrap_or_default() {
        if !model_ids.iter().any(|model_id| model_id == &item.slug) {
            model_ids.push(item.slug);
        }
    }
    model_ids
}

#[derive(serde::Serialize)]
struct CatalogModelOption {
    value: String,
    label: String,
}

#[tauri::command]
fn read_catalog_model_options() -> Result<Vec<CatalogModelOption>, String> {
    let router_mode = load_router_config_command()
        .map(|config| config.runtime.router_mode)
        .unwrap_or(0);
    let path = if router_mode == 1 {
        catalog_base_config_path()?
    } else {
        catalog_config_path()?
    };
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return Ok(Vec::new()),
    };
    let root = match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(value) => value,
        Err(_) => return Ok(Vec::new()),
    };
    let Some(source_models) = root.get(CATALOG_MODELS_KEY).and_then(|value| value.as_array()) else {
        return Ok(Vec::new());
    };

    let mut options = Vec::new();
    for source_model in source_models {
        let value = source_model
            .get("id")
            .or_else(|| source_model.get("slug"))
            .or_else(|| source_model.get("name"))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or_default()
            .to_string();
        if value.is_empty() {
            continue;
        }
        let label = source_model
            .get("display_name")
            .or_else(|| source_model.get("displayName"))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&value)
            .to_string();
        options.push(CatalogModelOption { value, label });
    }

    Ok(options)
}

fn account_proxy_responses_request(request: &ParsedRequest) -> RouterResponse {
    if request.body.len() > MAX_REQUEST_BODY_BYTES {
        return json_response(
            HTTP_PAYLOAD_TOO_LARGE,
            "{\"error\":\"request_body_too_large\"}".to_string(),
            EMPTY_LOG_VALUE,
            "?????????",
        );
    }

    let payload = match serde_json::from_slice::<serde_json::Value>(&request.body) {
        Ok(payload) => payload,
        Err(error) => {
            return json_response(
                HTTP_BAD_REQUEST,
                "{\"error\":\"invalid_json_body\"}".to_string(),
                EMPTY_LOG_VALUE,
                error.to_string(),
            )
        }
    };

    if let Some(model) = payload
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !account_proxy_catalog_contains_model(model) {
            return account_proxy_model_not_found_response(model);
        }
    }

    forward_official_codex_responses_request(payload)
}

fn account_proxy_chat_completions_request(request: &ParsedRequest) -> RouterResponse {
    if request.body.len() > MAX_REQUEST_BODY_BYTES {
        return json_response(
            HTTP_PAYLOAD_TOO_LARGE,
            "{\"error\":\"request_body_too_large\"}".to_string(),
            EMPTY_LOG_VALUE,
            "?????????",
        );
    }

    let payload = match serde_json::from_slice::<serde_json::Value>(&request.body) {
        Ok(payload) => payload,
        Err(error) => {
            return json_response(
                HTTP_BAD_REQUEST,
                "{\"error\":\"invalid_json_body\"}".to_string(),
                EMPTY_LOG_VALUE,
                error.to_string(),
            )
        }
    };

    let requested_model = payload
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(default_account_proxy_upstream_model);
    if !account_proxy_catalog_contains_model(&requested_model) {
        return account_proxy_model_not_found_response(&requested_model);
    }
    let stream = payload
        .get("stream")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let responses_payload =
        build_responses_payload_from_chat_completions(&payload, &requested_model);
    let upstream_response = forward_official_codex_responses_request(responses_payload);

    if upstream_response.status_code >= 400 {
        return upstream_response;
    }

    if stream {
        RouterResponse {
            flush_headers_before_body: false,
            status_code: upstream_response.status_code,
            content_type: HEADER_EVENT_STREAM.to_string(),
            body: codex_sse_to_chat_completions_sse(&upstream_response.body, &requested_model),
            target_provider: upstream_response.target_provider,
            error_detail: upstream_response.error_detail,
            usage: upstream_response.usage,
            usage_source: upstream_response.usage_source,
        }
    } else {
        RouterResponse {
            flush_headers_before_body: false,
            status_code: upstream_response.status_code,
            content_type: HEADER_JSON.to_string(),
            body: codex_sse_to_chat_completions_json(&upstream_response.body, &requested_model),
            target_provider: upstream_response.target_provider,
            error_detail: upstream_response.error_detail,
            usage: upstream_response.usage,
            usage_source: upstream_response.usage_source,
        }
    }
}

fn account_proxy_messages_request(request: &ParsedRequest) -> RouterResponse {
    if request.body.len() > MAX_REQUEST_BODY_BYTES {
        return json_response(
            HTTP_PAYLOAD_TOO_LARGE,
            "{\"error\":\"request_body_too_large\"}".to_string(),
            EMPTY_LOG_VALUE,
            "?????????",
        );
    }

    let payload = match serde_json::from_slice::<serde_json::Value>(&request.body) {
        Ok(payload) => payload,
        Err(error) => {
            return json_response(
                HTTP_BAD_REQUEST,
                "{\"error\":\"invalid_json_body\"}".to_string(),
                EMPTY_LOG_VALUE,
                error.to_string(),
            )
        }
    };

    let requested_model = payload
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(default_account_proxy_upstream_model);
    if !account_proxy_catalog_contains_model(&requested_model) {
        return account_proxy_model_not_found_response(&requested_model);
    }
    let stream = payload
        .get("stream")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let responses_payload =
        build_responses_payload_from_anthropic_messages(&payload, &requested_model);
    let upstream_response = forward_official_codex_responses_request(responses_payload);

    if upstream_response.status_code >= 400 {
        return upstream_response;
    }

    if stream {
        RouterResponse {
            flush_headers_before_body: false,
            status_code: upstream_response.status_code,
            content_type: HEADER_EVENT_STREAM.to_string(),
            body: codex_sse_to_anthropic_messages_sse(&upstream_response.body, &requested_model),
            target_provider: upstream_response.target_provider,
            error_detail: upstream_response.error_detail,
            usage: upstream_response.usage,
            usage_source: upstream_response.usage_source,
        }
    } else {
        RouterResponse {
            flush_headers_before_body: false,
            status_code: upstream_response.status_code,
            content_type: HEADER_JSON.to_string(),
            body: codex_sse_to_anthropic_message_json(&upstream_response.body, &requested_model),
            target_provider: upstream_response.target_provider,
            error_detail: upstream_response.error_detail,
            usage: upstream_response.usage,
            usage_source: upstream_response.usage_source,
        }
    }
}

fn build_responses_payload_from_anthropic_messages(
    payload: &serde_json::Value,
    model: &str,
) -> serde_json::Value {
    let mut root = serde_json::Map::new();
    root.insert(
        "model".to_string(),
        serde_json::Value::String(model.to_string()),
    );
    root.insert(
        OFFICIAL_STREAM_KEY.to_string(),
        serde_json::Value::Bool(true),
    );

    if let Some(system) = anthropic_system_to_text(payload.get("system")) {
        root.insert(
            OFFICIAL_INSTRUCTIONS_KEY.to_string(),
            serde_json::Value::String(system),
        );
    }

    let messages = payload
        .get("messages")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .map(normalize_anthropic_message_for_responses)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    root.insert(
        OFFICIAL_INPUT_KEY.to_string(),
        serde_json::Value::Array(messages),
    );

    serde_json::Value::Object(root)
}

fn default_account_proxy_upstream_model() -> String {
    load_account_proxy_catalog_model_ids()
        .and_then(|model_ids| model_ids.into_iter().next())
        .unwrap_or_else(|| "gpt-5-codex".to_string())
}

fn account_proxy_catalog_contains_model(model: &str) -> bool {
    load_account_proxy_catalog_model_ids()
        .map(|model_ids| model_ids.iter().any(|model_id| model_id == model))
        .unwrap_or(false)
}

fn account_proxy_model_not_found_response(model: &str) -> RouterResponse {
    let mut error = serde_json::Map::new();
    error.insert(
        "message".to_string(),
        serde_json::Value::String(format!("model not found: {}", model)),
    );
    error.insert(
        "type".to_string(),
        serde_json::Value::String("invalid_request_error".to_string()),
    );
    error.insert(
        "code".to_string(),
        serde_json::Value::String("model_not_found".to_string()),
    );
    let mut root = serde_json::Map::new();
    root.insert("error".to_string(), serde_json::Value::Object(error));
    let body = serde_json::Value::Object(root).to_string();
    json_response(
        HTTP_NOT_FOUND,
        body,
        OFFICIAL_TARGET_PROVIDER,
        format!("model not found: {}", model),
    )
}

fn anthropic_system_to_text(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .map(content_to_text)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn normalize_anthropic_message_for_responses(message: &serde_json::Value) -> serde_json::Value {
    let role = message
        .get("role")
        .and_then(|value| value.as_str())
        .map(normalize_message_role)
        .unwrap_or_else(|| "user".to_string());
    json_chat_message(
        &role,
        &message
            .get("content")
            .map(content_to_text)
            .unwrap_or_default(),
    )
}

fn codex_sse_to_anthropic_message_json(body: &str, model: &str) -> String {
    let text = extract_text_from_codex_sse(body);
    let mut text_block = serde_json::Map::new();
    text_block.insert(
        "type".to_string(),
        serde_json::Value::String("text".to_string()),
    );
    text_block.insert("text".to_string(), serde_json::Value::String(text));

    let mut usage = serde_json::Map::new();
    usage.insert(
        "input_tokens".to_string(),
        serde_json::Value::Number(serde_json::Number::from(0)),
    );
    usage.insert(
        "output_tokens".to_string(),
        serde_json::Value::Number(serde_json::Number::from(0)),
    );

    let mut root = serde_json::Map::new();
    root.insert(
        "id".to_string(),
        serde_json::Value::String(format!("msg_{}", current_log_millis())),
    );
    root.insert(
        "type".to_string(),
        serde_json::Value::String("message".to_string()),
    );
    root.insert(
        "role".to_string(),
        serde_json::Value::String("assistant".to_string()),
    );
    root.insert(
        "model".to_string(),
        serde_json::Value::String(model.to_string()),
    );
    root.insert(
        "content".to_string(),
        serde_json::Value::Array(vec![serde_json::Value::Object(text_block)]),
    );
    root.insert(
        "stop_reason".to_string(),
        serde_json::Value::String("end_turn".to_string()),
    );
    root.insert("stop_sequence".to_string(), serde_json::Value::Null);
    root.insert("usage".to_string(), serde_json::Value::Object(usage));
    serde_json::Value::Object(root).to_string()
}

fn codex_sse_to_anthropic_messages_sse(body: &str, model: &str) -> String {
    let message_id = format!("msg_{}", current_log_millis());
    let mut output = String::new();

    let mut start_message = serde_json::Map::new();
    start_message.insert("id".to_string(), serde_json::Value::String(message_id));
    start_message.insert(
        "type".to_string(),
        serde_json::Value::String("message".to_string()),
    );
    start_message.insert(
        "role".to_string(),
        serde_json::Value::String("assistant".to_string()),
    );
    start_message.insert(
        "model".to_string(),
        serde_json::Value::String(model.to_string()),
    );
    start_message.insert("content".to_string(), serde_json::Value::Array(Vec::new()));
    start_message.insert("stop_reason".to_string(), serde_json::Value::Null);
    start_message.insert("stop_sequence".to_string(), serde_json::Value::Null);
    let mut usage = serde_json::Map::new();
    usage.insert(
        "input_tokens".to_string(),
        serde_json::Value::Number(serde_json::Number::from(0)),
    );
    usage.insert(
        "output_tokens".to_string(),
        serde_json::Value::Number(serde_json::Number::from(0)),
    );
    start_message.insert("usage".to_string(), serde_json::Value::Object(usage));
    push_anthropic_sse_event(
        &mut output,
        "message_start",
        serde_json::Value::Object({
            let mut event = serde_json::Map::new();
            event.insert(
                "type".to_string(),
                serde_json::Value::String("message_start".to_string()),
            );
            event.insert(
                "message".to_string(),
                serde_json::Value::Object(start_message),
            );
            event
        }),
    );

    push_anthropic_sse_event(
        &mut output,
        "content_block_start",
        serde_json::Value::Object({
            let mut block = serde_json::Map::new();
            block.insert(
                "type".to_string(),
                serde_json::Value::String("text".to_string()),
            );
            block.insert("text".to_string(), serde_json::Value::String(String::new()));
            let mut event = serde_json::Map::new();
            event.insert(
                "type".to_string(),
                serde_json::Value::String("content_block_start".to_string()),
            );
            event.insert(
                "index".to_string(),
                serde_json::Value::Number(serde_json::Number::from(0)),
            );
            event.insert(
                "content_block".to_string(),
                serde_json::Value::Object(block),
            );
            event
        }),
    );

    let deltas = extract_text_deltas_from_codex_sse(body);
    let effective_deltas = if deltas.is_empty() {
        vec![extract_text_from_codex_sse(body)]
    } else {
        deltas
    };

    for delta in effective_deltas.into_iter().filter(|text| !text.is_empty()) {
        push_anthropic_sse_event(
            &mut output,
            "content_block_delta",
            serde_json::Value::Object({
                let mut delta_map = serde_json::Map::new();
                delta_map.insert(
                    "type".to_string(),
                    serde_json::Value::String("text_delta".to_string()),
                );
                delta_map.insert("text".to_string(), serde_json::Value::String(delta));
                let mut event = serde_json::Map::new();
                event.insert(
                    "type".to_string(),
                    serde_json::Value::String("content_block_delta".to_string()),
                );
                event.insert(
                    "index".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(0)),
                );
                event.insert("delta".to_string(), serde_json::Value::Object(delta_map));
                event
            }),
        );
    }

    push_anthropic_sse_event(
        &mut output,
        "content_block_stop",
        serde_json::Value::Object({
            let mut event = serde_json::Map::new();
            event.insert(
                "type".to_string(),
                serde_json::Value::String("content_block_stop".to_string()),
            );
            event.insert(
                "index".to_string(),
                serde_json::Value::Number(serde_json::Number::from(0)),
            );
            event
        }),
    );

    push_anthropic_sse_event(
        &mut output,
        "message_delta",
        serde_json::Value::Object({
            let mut delta = serde_json::Map::new();
            delta.insert(
                "stop_reason".to_string(),
                serde_json::Value::String("end_turn".to_string()),
            );
            delta.insert("stop_sequence".to_string(), serde_json::Value::Null);
            let mut usage = serde_json::Map::new();
            usage.insert(
                "output_tokens".to_string(),
                serde_json::Value::Number(serde_json::Number::from(0)),
            );
            let mut event = serde_json::Map::new();
            event.insert(
                "type".to_string(),
                serde_json::Value::String("message_delta".to_string()),
            );
            event.insert("delta".to_string(), serde_json::Value::Object(delta));
            event.insert("usage".to_string(), serde_json::Value::Object(usage));
            event
        }),
    );

    push_anthropic_sse_event(
        &mut output,
        "message_stop",
        serde_json::Value::Object({
            let mut event = serde_json::Map::new();
            event.insert(
                "type".to_string(),
                serde_json::Value::String("message_stop".to_string()),
            );
            event
        }),
    );

    output
}

fn push_anthropic_sse_event(output: &mut String, event: &str, data: serde_json::Value) {
    output.push_str("event: ");
    output.push_str(event);
    output.push_str("\n");
    output.push_str("data: ");
    output.push_str(&data.to_string());
    output.push_str("\n\n");
}

fn build_responses_payload_from_chat_completions(
    payload: &serde_json::Value,
    model: &str,
) -> serde_json::Value {
    let mut root = serde_json::Map::new();
    root.insert(
        "model".to_string(),
        serde_json::Value::String(model.to_string()),
    );

    if let Some(instructions) = extract_chat_system_instructions(payload) {
        root.insert(
            OFFICIAL_INSTRUCTIONS_KEY.to_string(),
            serde_json::Value::String(instructions),
        );
    }

    let input_messages = extract_chat_messages(payload, true);
    root.insert(
        OFFICIAL_INPUT_KEY.to_string(),
        serde_json::Value::Array(input_messages),
    );
    root.insert(
        OFFICIAL_STREAM_KEY.to_string(),
        serde_json::Value::Bool(true),
    );

    serde_json::Value::Object(root)
}

fn extract_chat_system_instructions(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("messages")
        .and_then(|value| value.as_array())
        .map(|messages| {
            messages
                .iter()
                .filter(|message| {
                    message
                        .get("role")
                        .and_then(|role| role.as_str())
                        .map(|role| role.eq_ignore_ascii_case("system"))
                        .unwrap_or(false)
                })
                .map(|message| {
                    message
                        .get("content")
                        .map(content_to_text)
                        .unwrap_or_default()
                })
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.trim().is_empty())
}

fn codex_sse_to_chat_completions_json(body: &str, model: &str) -> String {
    let text = extract_text_from_codex_sse(body);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let mut message = serde_json::Map::new();
    message.insert(
        "role".to_string(),
        serde_json::Value::String("assistant".to_string()),
    );
    message.insert("content".to_string(), serde_json::Value::String(text));

    let mut choice = serde_json::Map::new();
    choice.insert(
        "index".to_string(),
        serde_json::Value::Number(serde_json::Number::from(0)),
    );
    choice.insert("message".to_string(), serde_json::Value::Object(message));
    choice.insert(
        "finish_reason".to_string(),
        serde_json::Value::String("stop".to_string()),
    );

    let mut root = serde_json::Map::new();
    root.insert(
        "id".to_string(),
        serde_json::Value::String(format!("chatcmpl_{}", current_log_millis())),
    );
    root.insert(
        "object".to_string(),
        serde_json::Value::String("chat.completion".to_string()),
    );
    root.insert(
        "created".to_string(),
        serde_json::Value::Number(serde_json::Number::from(now)),
    );
    root.insert(
        "model".to_string(),
        serde_json::Value::String(model.to_string()),
    );
    root.insert(
        "choices".to_string(),
        serde_json::Value::Array(vec![serde_json::Value::Object(choice)]),
    );
    serde_json::Value::Object(root).to_string()
}

fn codex_sse_to_chat_completions_sse(body: &str, model: &str) -> String {
    let id = format!("chatcmpl_{}", current_log_millis());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let mut output = String::new();
    let mut emitted_any = false;

    for delta in extract_text_deltas_from_codex_sse(body) {
        let mut delta_map = serde_json::Map::new();
        delta_map.insert("content".to_string(), serde_json::Value::String(delta));

        let mut choice = serde_json::Map::new();
        choice.insert(
            "index".to_string(),
            serde_json::Value::Number(serde_json::Number::from(0)),
        );
        choice.insert("delta".to_string(), serde_json::Value::Object(delta_map));
        choice.insert("finish_reason".to_string(), serde_json::Value::Null);

        let mut chunk = serde_json::Map::new();
        chunk.insert("id".to_string(), serde_json::Value::String(id.clone()));
        chunk.insert(
            "object".to_string(),
            serde_json::Value::String("chat.completion.chunk".to_string()),
        );
        chunk.insert(
            "created".to_string(),
            serde_json::Value::Number(serde_json::Number::from(now)),
        );
        chunk.insert(
            "model".to_string(),
            serde_json::Value::String(model.to_string()),
        );
        chunk.insert(
            "choices".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::Object(choice)]),
        );
        output.push_str("data: ");
        output.push_str(&serde_json::Value::Object(chunk).to_string());
        output.push_str("\n\n");
        emitted_any = true;
    }

    if !emitted_any {
        let text = extract_text_from_codex_sse(body);
        if !text.is_empty() {
            output.push_str(&codex_sse_to_chat_completions_sse(
                &format!(
                    "data: {{\"type\":\"response.output_text.delta\",\"delta\":{}}}\n\n",
                    json_string(&text)
                ),
                model,
            ));
        }
    }

    output.push_str("data: [DONE]\n\n");
    output
}

fn extract_text_from_codex_sse(body: &str) -> String {
    let deltas = extract_text_deltas_from_codex_sse(body).join("");
    if !deltas.trim().is_empty() {
        return deltas;
    }

    for root in iter_codex_sse_json_values(body) {
        let text = extract_responses_text(&root);
        if !text.trim().is_empty() && text != root.to_string() {
            return text;
        }
    }

    String::new()
}

fn extract_text_deltas_from_codex_sse(body: &str) -> Vec<String> {
    iter_codex_sse_json_values(body)
        .into_iter()
        .filter_map(|root| {
            let event_type = root
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if event_type == "response.output_text.delta"
                || event_type == "response.reasoning_text.delta"
            {
                return root
                    .get("delta")
                    .and_then(|value| value.as_str())
                    .map(str::to_string);
            }
            None
        })
        .collect()
}

fn iter_codex_sse_json_values(body: &str) -> Vec<serde_json::Value> {
    body.lines()
        .filter_map(|line| line.trim().strip_prefix("data:"))
        .map(str::trim)
        .filter(|data| !data.is_empty() && *data != "[DONE]")
        .filter_map(|data| serde_json::from_str::<serde_json::Value>(data).ok())
        .collect()
}

fn extract_token_usage_from_body(body: &str) -> Option<TokenUsage> {
    if let Ok(root) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(usage) = extract_token_usage_from_value(&root) {
            return Some(usage);
        }
    }

    for root in iter_codex_sse_json_values(body) {
        if let Some(usage) = extract_token_usage_from_value(&root) {
            return Some(usage);
        }
    }

    None
}

fn extract_token_usage_from_value(value: &serde_json::Value) -> Option<TokenUsage> {
    if let Some(usage) = value.get("usage").and_then(token_usage_from_usage_value) {
        return Some(usage);
    }

    if let Some(usage) = value
        .get("response")
        .and_then(|response| response.get("usage"))
        .and_then(token_usage_from_usage_value)
    {
        return Some(usage);
    }

    match value {
        serde_json::Value::Object(map) => map.values().find_map(extract_token_usage_from_value),
        serde_json::Value::Array(items) => items.iter().find_map(extract_token_usage_from_value),
        _ => None,
    }
}

fn token_usage_from_usage_value(value: &serde_json::Value) -> Option<TokenUsage> {
    let input_tokens = first_u64_by_keys(value, &["input_tokens", "prompt_tokens"]);
    let output_tokens = first_u64_by_keys(value, &["output_tokens", "completion_tokens"]);
    let cached_input_tokens = first_u64_by_keys(value, &["cached_input_tokens", "cached_tokens"])
        .or_else(|| {
            value
                .get("input_tokens_details")
                .and_then(|details| first_u64_by_keys(details, &["cached_tokens"]))
        })
        .or_else(|| {
            value
                .get("prompt_tokens_details")
                .and_then(|details| first_u64_by_keys(details, &["cached_tokens"]))
        })
        .unwrap_or_default();
    let reasoning_tokens = first_u64_by_keys(value, &["reasoning_tokens"])
        .or_else(|| {
            value
                .get("output_tokens_details")
                .and_then(|details| first_u64_by_keys(details, &["reasoning_tokens"]))
        })
        .or_else(|| {
            value
                .get("completion_tokens_details")
                .and_then(|details| first_u64_by_keys(details, &["reasoning_tokens"]))
        })
        .unwrap_or_default();

    if input_tokens.is_none()
        && output_tokens.is_none()
        && cached_input_tokens == 0
        && reasoning_tokens == 0
    {
        return None;
    }

    let input_tokens = input_tokens.unwrap_or_default();
    let output_tokens = output_tokens.unwrap_or_default();
    let total_tokens = first_u64_by_keys(value, &["total_tokens"])
        .unwrap_or_else(|| input_tokens.saturating_add(output_tokens));

    Some(TokenUsage {
        input_tokens,
        output_tokens,
        cached_input_tokens,
        reasoning_tokens,
        total_tokens,
    })
}

fn normalize_upstream_usage_value(usage: &serde_json::Value) -> Option<serde_json::Value> {
    let token_usage = token_usage_from_usage_value(usage)?;
    let mut normalized = serde_json::Map::new();
    normalized.insert(
        "input_tokens".to_string(),
        serde_json::Value::Number(serde_json::Number::from(token_usage.input_tokens)),
    );
    normalized.insert(
        "output_tokens".to_string(),
        serde_json::Value::Number(serde_json::Number::from(token_usage.output_tokens)),
    );
    normalized.insert(
        "total_tokens".to_string(),
        serde_json::Value::Number(serde_json::Number::from(token_usage.total_tokens)),
    );
    if token_usage.cached_input_tokens > 0 {
        normalized.insert(
            "input_tokens_details".to_string(),
            serde_json::json!({ "cached_tokens": token_usage.cached_input_tokens }),
        );
    }
    if token_usage.reasoning_tokens > 0 {
        normalized.insert(
            "output_tokens_details".to_string(),
            serde_json::json!({ "reasoning_tokens": token_usage.reasoning_tokens }),
        );
    }
    Some(serde_json::Value::Object(normalized))
}

fn first_u64_by_keys(value: &serde_json::Value, keys: &[&str]) -> Option<u64> {
    for key in keys {
        if let Some(item) = value.get(*key) {
            if let Some(number) = item.as_u64() {
                return Some(number);
            }
            if let Some(number) = item.as_i64().filter(|number| *number >= 0) {
                return Some(number as u64);
            }
            if let Some(number) = item
                .as_str()
                .and_then(|text| text.trim().parse::<u64>().ok())
            {
                return Some(number);
            }
        }
    }
    None
}

fn token_usage_source(usage: Option<&TokenUsage>) -> String {
    if usage.is_some() {
        TOKEN_USAGE_SOURCE_UPSTREAM.to_string()
    } else {
        TOKEN_USAGE_SOURCE_MISSING.to_string()
    }
}

fn expire_codex_oauth_state_if_needed() {
    let expired = codex_oauth_state()
        .lock()
        .ok()
        .and_then(|guard| {
            guard
                .as_ref()
                .map(|pending| pending.created_at.elapsed() > Duration::from_secs(10 * 60))
        })
        .unwrap_or(false);

    if !expired {
        return;
    }

    if let Ok(mut pending) = codex_oauth_state().lock() {
        *pending = None;
    }
    set_codex_oauth_last_result(CodexOAuthLoginStatus {
        status: "error".to_string(),
        message: "OAuth 登录等待超时，请重新点击添加账号。".to_string(),
        account_key: None,
        account_email: None,
    });
}

fn finish_codex_oauth_login(code: &str, code_verifier: &str) -> Result<String, String> {
    let token_root = exchange_codex_oauth_code(code, code_verifier)?;
    let auth_root = build_codex_auth_from_oauth_token(&token_root)?;
    let account_email = find_string_by_keys(
        &auth_root,
        &[
            "email",
            "account_email",
            "accountEmail",
            "user_email",
            "userEmail",
        ],
    );
    let mut registry = read_accounts_registry()?;
    let account_key = upsert_codex_auth_value_account(&mut registry, &auth_root, false)?;
    write_accounts_registry(&registry)?;

    let mut refreshed_registry = read_accounts_registry()?;
    if refresh_account_usage_from_backend_api(&mut refreshed_registry, &account_key, false) {
        write_accounts_registry(&refreshed_registry)?;
    }

    set_codex_oauth_last_result(CodexOAuthLoginStatus {
        status: "success".to_string(),
        message: "OAuth 回调已接收，账号已保存。".to_string(),
        account_key: Some(account_key.clone()),
        account_email,
    });

    Ok(account_key)
}

fn exchange_codex_oauth_code(code: &str, code_verifier: &str) -> Result<serde_json::Value, String> {
    let settings = load_official_codex_forward_settings();
    let response = build_oauth_token_request(&settings)
        .timeout(Duration::from_secs(MODEL_TEST_TIMEOUT_SECONDS))
        .set(HEADER_ACCEPT, HEADER_JSON)
        .set(HEADER_CONTENT_TYPE, "application/x-www-form-urlencoded")
        .send_form(&[
            ("grant_type", "authorization_code"),
            ("client_id", OAUTH_CLIENT_ID),
            ("code", code),
            ("redirect_uri", &oauth_redirect_uri()),
            ("code_verifier", code_verifier),
        ])
        .map_err(format_oauth_token_request_error)?;

    let text = response
        .into_string()
        .map_err(|error| format!("读取 OAuth token 响应失败：{}", error))?;
    serde_json::from_str::<serde_json::Value>(&text)
        .map_err(|error| format!("解析 OAuth token 响应失败：{}，响应：{}", error, text))
}

struct OAuthTokenRefreshError {
    message: String,
    permanently_failed: bool,
}

fn exchange_codex_oauth_refresh_token(
    refresh_token: &str,
) -> Result<serde_json::Value, OAuthTokenRefreshError> {
    let settings = load_official_codex_forward_settings();
    let response = build_oauth_token_request(&settings)
        .timeout(Duration::from_secs(MODEL_TEST_TIMEOUT_SECONDS))
        .set(HEADER_ACCEPT, HEADER_JSON)
        .set(HEADER_CONTENT_TYPE, "application/x-www-form-urlencoded")
        .send_form(&[
            ("grant_type", "refresh_token"),
            ("client_id", OAUTH_CLIENT_ID),
            ("refresh_token", refresh_token),
        ]);

    let response = match response {
        Ok(response) => response,
        Err(error) => return Err(format_oauth_refresh_token_error(error)),
    };
    let text = response
        .into_string()
        .map_err(|error| OAuthTokenRefreshError {
            message: format!("读取 OAuth token 刷新响应失败：{}", error),
            permanently_failed: false,
        })?;
    serde_json::from_str::<serde_json::Value>(&text).map_err(|error| OAuthTokenRefreshError {
        message: format!("解析 OAuth token 刷新响应失败：{}", error),
        permanently_failed: false,
    })
}

fn format_oauth_refresh_token_error(error: ureq::Error) -> OAuthTokenRefreshError {
    match error {
        ureq::Error::Status(status, response) => {
            let _ = response.into_string();
            let permanently_failed = status == 401 || status == 403;
            OAuthTokenRefreshError {
                message: if permanently_failed {
                    "refresh_token 已失效，请重新授权。".to_string()
                } else {
                    format!("OAuth token 刷新失败：status code {}", status)
                },
                permanently_failed,
            }
        }
        other => OAuthTokenRefreshError {
            message: format!("OAuth token 刷新请求失败：{}", other),
            permanently_failed: false,
        },
    }
}

fn fetch_chatgpt_session_with_cookie(
    session_token: &str,
) -> Result<serde_json::Value, OAuthTokenRefreshError> {
    let settings = load_official_codex_forward_settings();
    let cookie = format!("__Secure-next-auth.session-token={}", session_token);
    let response = build_upstream_get_request(
        CHATGPT_SESSION_API_URL,
        settings.proxy_url.as_deref(),
        MODEL_TEST_TIMEOUT_SECONDS,
    )
    .set(HEADER_ACCEPT, HEADER_JSON)
    .set(HEADER_COOKIE, &cookie)
    .set(HEADER_USER_AGENT, "Mozilla/5.0 codex-router-shell")
    .call();

    let response = match response {
        Ok(response) => response,
        Err(error) => return Err(format_chatgpt_session_refresh_error(error)),
    };
    let text = response
        .into_string()
        .map_err(|error| OAuthTokenRefreshError {
            message: format!("read ChatGPT session response failed: {}", error),
            permanently_failed: false,
        })?;
    serde_json::from_str::<serde_json::Value>(&text).map_err(|error| OAuthTokenRefreshError {
        message: format!("parse ChatGPT session response failed: {}", error),
        permanently_failed: false,
    })
}

fn format_chatgpt_session_refresh_error(error: ureq::Error) -> OAuthTokenRefreshError {
    match error {
        ureq::Error::Status(status, response) => {
            let _ = response.into_string();
            let permanently_failed = status == 401 || status == 403;
            OAuthTokenRefreshError {
                message: if permanently_failed {
                    "web_session cookie 已失效，请重新登录 ChatGPT 后导入。".to_string()
                } else {
                    format!("ChatGPT session refresh failed: status code {}", status)
                },
                permanently_failed,
            }
        }
        other => OAuthTokenRefreshError {
            message: format!("ChatGPT session refresh request failed: {}", other),
            permanently_failed: false,
        },
    }
}

fn ensure_oauth_refresh_token_field(token_root: &mut serde_json::Value, refresh_token: &str) {
    let Some(map) = token_root.as_object_mut() else {
        return;
    };
    let has_refresh_token = map
        .get("refresh_token")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    if !has_refresh_token {
        map.insert(
            "refresh_token".to_string(),
            serde_json::Value::String(refresh_token.to_string()),
        );
    }
}

fn ensure_chatgpt_session_token_field(auth_root: &mut serde_json::Value, session_token: &str) {
    if session_token.trim().is_empty() {
        return;
    }
    let Some(map) = auth_root.as_object_mut() else {
        return;
    };
    let tokens = map
        .entry("tokens".to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let Some(tokens_map) = tokens.as_object_mut() else {
        return;
    };
    tokens_map.insert(
        "session_token".to_string(),
        serde_json::Value::String(session_token.to_string()),
    );
}

fn ensure_synthetic_id_token_field(auth_root: &mut serde_json::Value) -> bool {
    if find_string_by_keys(auth_root, &["id_token", "idToken"]).is_some() {
        return false;
    }
    let Some(id_token) = build_synthetic_codex_id_token(auth_root) else {
        return false;
    };
    let Some(map) = auth_root.as_object_mut() else {
        return false;
    };
    let tokens = map
        .entry("tokens".to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let Some(tokens_map) = tokens.as_object_mut() else {
        return false;
    };
    tokens_map.insert("id_token".to_string(), serde_json::Value::String(id_token));
    map.insert(
        "id_token_synthetic".to_string(),
        serde_json::Value::Bool(true),
    );
    true
}

fn build_synthetic_codex_id_token(auth_root: &serde_json::Value) -> Option<String> {
    let access_claims = find_codex_access_token(auth_root)
        .and_then(|token| decode_jwt_payload(&token))
        .unwrap_or(serde_json::Value::Null);
    let auth_info = access_claims
        .get("https://api.openai.com/auth")
        .filter(|value| value.is_object())
        .unwrap_or(&serde_json::Value::Null);
    let profile = access_claims
        .get("https://api.openai.com/profile")
        .filter(|value| value.is_object())
        .unwrap_or(&serde_json::Value::Null);
    let email = find_string_by_keys(auth_root, &["email", "account_email", "accountEmail"])
        .or_else(|| find_string_by_keys(profile, &["email"]));
    let account_id =
        find_codex_account_id(auth_root).or_else(|| find_codex_account_id(auth_info))?;
    let plan_type = find_string_by_keys(
        auth_root,
        &[
            "plan_type",
            "planType",
            "chatgpt_plan_type",
            "chatgptPlanType",
            "plan",
        ],
    )
    .or_else(|| find_string_by_keys(auth_info, &["chatgpt_plan_type", "plan_type", "planType"]));
    let user_id = find_codex_user_id(auth_root)
        .or_else(|| find_string_by_keys(auth_info, &["chatgpt_user_id", "user_id", "userId"]));
    let expires = find_string_by_keys(
        auth_root,
        &["expired", "expires", "expires_at", "expiresAt"],
    )
    .and_then(|value| epoch_from_json_time_value(&value))
    .or_else(|| {
        access_claims.get("exp").and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().map(|item| item as i64))
        })
    })
    .unwrap_or_else(|| current_unix_timestamp() + 90 * 24 * 60 * 60);
    let now = current_unix_timestamp();

    let mut header = serde_json::Map::new();
    header.insert(
        "alg".to_string(),
        serde_json::Value::String("none".to_string()),
    );
    header.insert(
        "typ".to_string(),
        serde_json::Value::String("JWT".to_string()),
    );
    header.insert("cpa_synthetic".to_string(), serde_json::Value::Bool(true));

    let mut auth_claims = serde_json::Map::new();
    auth_claims.insert(
        "chatgpt_account_id".to_string(),
        serde_json::Value::String(account_id),
    );
    if let Some(plan_type) = plan_type {
        auth_claims.insert(
            "chatgpt_plan_type".to_string(),
            serde_json::Value::String(plan_type),
        );
    }
    if let Some(user_id) = user_id {
        auth_claims.insert(
            "chatgpt_user_id".to_string(),
            serde_json::Value::String(user_id.clone()),
        );
        auth_claims.insert("user_id".to_string(), serde_json::Value::String(user_id));
    }

    let mut payload = serde_json::Map::new();
    payload.insert("iat".to_string(), serde_json::Value::Number(now.into()));
    payload.insert("exp".to_string(), serde_json::Value::Number(expires.into()));
    payload.insert(
        "https://api.openai.com/auth".to_string(),
        serde_json::Value::Object(auth_claims),
    );
    if let Some(email) = email {
        payload.insert("email".to_string(), serde_json::Value::String(email));
    }

    Some(format!(
        "{}.{}.synthetic",
        base64_url_json(&serde_json::Value::Object(header))?,
        base64_url_json(&serde_json::Value::Object(payload))?
    ))
}



fn epoch_from_json_time_value(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(number) = trimmed.parse::<i64>() {
        return Some(number);
    }
    OffsetDateTime::parse(trimmed, &Rfc3339)
        .ok()
        .map(|time| time.unix_timestamp())
}

fn find_chatgpt_session_cookie_token(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            for key in [
                "session_token",
                "sessionToken",
                "next_auth_session_token",
                "nextAuthSessionToken",
                "__Secure-next-auth.session-token",
            ] {
                if let Some(found) = map
                    .get(key)
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    return Some(found.to_string());
                }
            }

            for key in ["cookie", "Cookie", "cookies", "Cookies"] {
                if let Some(found) = map
                    .get(key)
                    .and_then(|value| value.as_str())
                    .and_then(extract_chatgpt_session_token_from_cookie)
                {
                    return Some(found);
                }
            }

            for child in map.values() {
                if let Some(found) = find_chatgpt_session_cookie_token(child) {
                    return Some(found);
                }
            }

            None
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(found) = find_chatgpt_session_cookie_token(item) {
                    return Some(found);
                }
            }
            None
        }
        serde_json::Value::String(text) => extract_chatgpt_session_token_from_cookie(text),
        _ => None,
    }
}

fn extract_chatgpt_session_token_from_cookie(cookie: &str) -> Option<String> {
    cookie.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        if name.trim() == "__Secure-next-auth.session-token" {
            let token = value.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
        None
    })
}

fn preserve_refreshed_auth_metadata(
    refreshed_auth: &mut serde_json::Value,
    previous_snapshot: &serde_json::Value,
) {
    let Some(map) = refreshed_auth.as_object_mut() else {
        return;
    };
    for key in [
        "email",
        "name",
        "plan",
        "account_id",
        "accountId",
        "workspaceName",
        "accountName",
    ] {
        if map.get(key).is_some() {
            continue;
        }
        if let Some(value) = find_string_by_keys(previous_snapshot, &[key]) {
            map.insert(key.to_string(), serde_json::Value::String(value));
        }
    }
}

fn build_oauth_token_request(settings: &OfficialCodexForwardSettings) -> ureq::Request {
    match settings.proxy_url.as_deref() {
        Some(proxy_url) => match ureq::Proxy::new(proxy_url) {
            Ok(proxy) => ureq::builder().proxy(proxy).build().post(OAUTH_TOKEN_URL),
            Err(_) => ureq::post(OAUTH_TOKEN_URL),
        },
        None => ureq::post(OAUTH_TOKEN_URL),
    }
}

fn format_oauth_token_request_error(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(status, response) => {
            let body = response.into_string().unwrap_or_default();
            if body.trim().is_empty() {
                format!(
                    "璇锋眰 OAuth token endpoint 澶辫触：{}: status code {}",
                    OAUTH_TOKEN_URL, status
                )
            } else {
                format!(
                    "璇锋眰 OAuth token endpoint 澶辫触：{}: status code {}锛屽搷搴旓細{}",
                    OAUTH_TOKEN_URL, status, body
                )
            }
        }
        other => format!("璇锋眰 OAuth token endpoint 澶辫触：{}", other),
    }
}

fn build_codex_auth_from_oauth_token(
    token_root: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let access_token = json_string_field(token_root, "access_token")
        .ok_or_else(|| "OAuth token 响应缺少 access_token".to_string())?;
    let refresh_token = json_string_field(token_root, "refresh_token").unwrap_or_default();
    let id_token = json_string_field(token_root, "id_token").unwrap_or_default();
    let access_claims = decode_jwt_payload(&access_token).unwrap_or(serde_json::Value::Null);
    let id_claims = decode_jwt_payload(&id_token).unwrap_or(serde_json::Value::Null);
    let account_id = find_codex_account_id(&access_claims)
        .or_else(|| find_codex_account_id(&id_claims))
        .or_else(|| find_string_by_keys(token_root, &["account_id", "accountId"]))
        .unwrap_or_else(|| format!("token-{}", stable_text_key(&access_token)));
    let email = find_string_by_keys(&id_claims, &["email"])
        .or_else(|| find_string_by_keys(&access_claims, &["email"]));
    let name = find_string_by_keys(&id_claims, &["name"]).or_else(|| email.clone());

    let mut tokens = serde_json::Map::new();
    tokens.insert(
        "access_token".to_string(),
        serde_json::Value::String(access_token.clone()),
    );
    tokens.insert(
        "account_id".to_string(),
        serde_json::Value::String(account_id),
    );
    if !id_token.is_empty() {
        tokens.insert("id_token".to_string(), serde_json::Value::String(id_token));
    }
    if !refresh_token.is_empty() {
        tokens.insert(
            "refresh_token".to_string(),
            serde_json::Value::String(refresh_token),
        );
    }

    let mut root = serde_json::Map::new();
    root.insert("OPENAI_API_KEY".to_string(), serde_json::Value::Null);
    root.insert(
        "auth_mode".to_string(),
        serde_json::Value::String("chatgpt".to_string()),
    );
    root.insert(
        "last_refresh".to_string(),
        serde_json::Value::String(current_auth_refresh_time()),
    );
    root.insert("tokens".to_string(), serde_json::Value::Object(tokens));
    if let Some(email) = email {
        root.insert("email".to_string(), serde_json::Value::String(email));
    }
    if let Some(name) = name {
        root.insert("name".to_string(), serde_json::Value::String(name));
    }

    Ok(enrich_codex_auth_identity(serde_json::Value::Object(root)))
}

fn build_codex_auth_from_chatgpt_session(
    session_root: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let access_token = json_string_field(session_root, "accessToken")
        .or_else(|| json_string_field(session_root, "access_token"))
        .ok_or_else(|| "ChatGPT session 缺少 accessToken。".to_string())?;
    let session_token = json_string_field(session_root, "sessionToken")
        .or_else(|| json_string_field(session_root, "session_token"))
        .or_else(|| find_chatgpt_session_cookie_token(session_root));
    let id_token = json_string_field(session_root, "idToken")
        .or_else(|| json_string_field(session_root, "id_token"));
    let user = session_root.get("user").unwrap_or(&serde_json::Value::Null);
    let account = session_root
        .get("account")
        .unwrap_or(&serde_json::Value::Null);
    let account_id = json_string_field(account, "id")
        .or_else(|| find_string_by_keys(session_root, CODEX_ACCOUNT_ID_KEYS))
        .or_else(|| json_string_field(user, "id"))
        .ok_or_else(|| "ChatGPT session 缺少 account.id / user.id。".to_string())?;
    let email =
        json_string_field(user, "email").or_else(|| json_string_field(session_root, "email"));
    let name = json_string_field(user, "name").or_else(|| email.clone());
    let plan = json_string_field(account, "planType")
        .or_else(|| json_string_field(account, "plan_type"))
        .unwrap_or_else(|| "unknown".to_string());
    let expires = json_string_field(session_root, "expires");

    let mut tokens = serde_json::Map::new();
    tokens.insert(
        "access_token".to_string(),
        serde_json::Value::String(access_token.clone()),
    );
    tokens.insert(
        "account_id".to_string(),
        serde_json::Value::String(account_id.clone()),
    );
    if let Some(session_token) = session_token {
        tokens.insert(
            "session_token".to_string(),
            serde_json::Value::String(session_token),
        );
    }
    if let Some(id_token) = id_token {
        tokens.insert("id_token".to_string(), serde_json::Value::String(id_token));
    }
    if let Some(expires) = expires.as_ref() {
        tokens.insert(
            "expires_at".to_string(),
            serde_json::Value::String(expires.clone()),
        );
    }

    let mut root = serde_json::Map::new();
    root.insert("OPENAI_API_KEY".to_string(), serde_json::Value::Null);
    root.insert(
        "auth_mode".to_string(),
        serde_json::Value::String("web_session".to_string()),
    );
    root.insert(
        "last_refresh".to_string(),
        serde_json::Value::String(current_log_time()),
    );
    root.insert("tokens".to_string(), serde_json::Value::Object(tokens));
    root.insert(
        "account_id".to_string(),
        serde_json::Value::String(account_id),
    );
    root.insert("plan".to_string(), serde_json::Value::String(plan));
    if let Some(email) = email {
        root.insert("email".to_string(), serde_json::Value::String(email));
    }
    if let Some(name) = name {
        root.insert("name".to_string(), serde_json::Value::String(name));
    }
    if let Some(expires) = expires {
        root.insert("expires_at".to_string(), serde_json::Value::String(expires));
    }

    let mut auth_root = serde_json::Value::Object(root);
    ensure_synthetic_id_token_field(&mut auth_root);
    Ok(enrich_codex_auth_identity(auth_root))
}

fn build_codex_auth_file_from_snapshot(
    snapshot_root: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let access_token = find_codex_access_token(snapshot_root)
        .or_else(|| find_string_by_keys(snapshot_root, &["OPENAI_API_KEY"]))
        .or_else(|| find_string_by_keys(snapshot_root, &["accessToken"]))
        .ok_or_else(|| "账号快照缺少 access_token，无法写入 Codex auth.json。".to_string())?;
    let account = snapshot_root
        .get("account")
        .unwrap_or(&serde_json::Value::Null);
    let user = snapshot_root
        .get("user")
        .unwrap_or(&serde_json::Value::Null);
    let account_id = find_codex_account_id(snapshot_root)
        .or_else(|| json_string_field(account, "id"))
        .or_else(|| json_string_field(user, "id"))
        .ok_or_else(|| "账号快照缺少 account_id，无法写入 Codex auth.json。".to_string())?;
    let id_token = find_string_by_keys(snapshot_root, &["id_token", "idToken"]).unwrap_or_default();
    let refresh_token =
        find_string_by_keys(snapshot_root, &["refresh_token", "refreshToken"]).unwrap_or_default();

    let mut tokens = serde_json::Map::new();
    tokens.insert(
        "access_token".to_string(),
        serde_json::Value::String(access_token),
    );
    tokens.insert(
        "account_id".to_string(),
        serde_json::Value::String(account_id),
    );
    tokens.insert("id_token".to_string(), serde_json::Value::String(id_token));
    tokens.insert(
        "refresh_token".to_string(),
        serde_json::Value::String(refresh_token),
    );

    let mut root = serde_json::Map::new();
    root.insert("OPENAI_API_KEY".to_string(), serde_json::Value::Null);
    root.insert(
        "auth_mode".to_string(),
        serde_json::Value::String("chatgpt".to_string()),
    );
    root.insert(
        "last_refresh".to_string(),
        serde_json::Value::String(current_auth_refresh_time()),
    );
    root.insert("tokens".to_string(), serde_json::Value::Object(tokens));

    Ok(serde_json::Value::Object(root))
}

fn prepare_account_snapshot_for_switch(
    snapshot_path: &Path,
    snapshot_root: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let has_refresh_token = find_string_by_keys(snapshot_root, &["refresh_token", "refreshToken"])
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let session_token = find_chatgpt_session_cookie_token(snapshot_root);

    if !has_refresh_token {
        if let Some(session_token) = session_token.as_deref() {
            match fetch_chatgpt_session_with_cookie(session_token) {
                Ok(session_root) => {
                    let mut refreshed_auth = build_codex_auth_from_chatgpt_session(&session_root)?;
                    ensure_chatgpt_session_token_field(&mut refreshed_auth, session_token);
                    preserve_refreshed_auth_metadata(&mut refreshed_auth, snapshot_root);
                    let refreshed_auth = enrich_codex_auth_identity(refreshed_auth);
                    write_account_snapshot_value(snapshot_path, &refreshed_auth)?;
                    return Ok(refreshed_auth);
                }
                Err(error) => {
                    let (_, _, token_expired) = compute_token_expiry(Some(snapshot_root));
                    if token_expired {
                        return Err(format!(
                            "web_session token 已过期，且通过 session_token 刷新失败：{}。请重新导入登录结果。",
                            error.message
                        ));
                    }
                }
            }
        }
    }

    let mut completed_snapshot = snapshot_root.clone();
    if ensure_synthetic_id_token_field(&mut completed_snapshot) {
        write_account_snapshot_value(snapshot_path, &completed_snapshot)?;
    }
    Ok(completed_snapshot)
}

fn write_account_snapshot_value(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("序列化账号快照失败：{}", error))?;
    fs::write(path, text)
        .map_err(|error| format!("写入账号快照失败：{}，路径：{}", error, path.display()))
}

fn enrich_codex_auth_identity(mut auth_root: serde_json::Value) -> serde_json::Value {
    let access_token = find_codex_access_token(&auth_root)
        .or_else(|| find_string_by_keys(&auth_root, &["OPENAI_API_KEY"]));
    let id_token = find_string_by_keys(&auth_root, &["id_token", "idToken"]);
    let access_claims = access_token
        .as_deref()
        .and_then(decode_jwt_payload)
        .unwrap_or(serde_json::Value::Null);
    let id_claims = id_token
        .as_deref()
        .and_then(decode_jwt_payload)
        .unwrap_or(serde_json::Value::Null);
    let email = find_string_by_keys(
        &auth_root,
        &[
            "email",
            "account_email",
            "accountEmail",
            "user_email",
            "userEmail",
        ],
    )
    .or_else(|| find_string_by_keys(&id_claims, &["email"]))
    .or_else(|| find_string_by_keys(&access_claims, &["email"]));
    let name = find_string_by_keys(
        &auth_root,
        &[
            "name",
            "display_name",
            "displayName",
            "user_name",
            "userName",
        ],
    )
    .or_else(|| find_string_by_keys(&id_claims, &["name", "display_name", "displayName"]))
    .or_else(|| email.clone());
    let account_id = find_codex_account_id(&auth_root)
        .or_else(|| find_codex_account_id(&access_claims))
        .or_else(|| find_codex_account_id(&id_claims));
    let plan = find_string_by_keys(
        &auth_root,
        &[
            "plan",
            "plan_type",
            "planType",
            "chatgpt_plan_type",
            "chatgptPlanType",
        ],
    )
    .or_else(|| {
        find_string_by_keys(
            &id_claims,
            &[
                "plan",
                "plan_type",
                "planType",
                "chatgpt_plan_type",
                "chatgptPlanType",
            ],
        )
    })
    .or_else(|| {
        find_string_by_keys(
            &access_claims,
            &[
                "plan",
                "plan_type",
                "planType",
                "chatgpt_plan_type",
                "chatgptPlanType",
            ],
        )
    });

    if let Some(root) = auth_root.as_object_mut() {
        if let Some(email) = email {
            root.insert("email".to_string(), serde_json::Value::String(email));
        }
        if let Some(name) = name {
            root.insert("name".to_string(), serde_json::Value::String(name));
        }
        if let Some(account_id) = account_id {
            if let Some(tokens) = root
                .get_mut("tokens")
                .and_then(|value| value.as_object_mut())
            {
                tokens.insert(
                    "account_id".to_string(),
                    serde_json::Value::String(account_id),
                );
            }
        }
        if let Some(plan) = plan {
            root.insert("plan".to_string(), serde_json::Value::String(plan));
        }
    }

    auth_root
}

fn forward_responses_request(body: &[u8]) -> RouterResponse {
    let payload = match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(payload) => payload,
        Err(error) => {
            return json_response(
                HTTP_BAD_REQUEST,
                "{\"error\":\"invalid_json_body\"}".to_string(),
                EMPTY_LOG_VALUE,
                error.to_string(),
            )
        }
    };

    let requested_model = payload
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    match requested_model.as_deref() {
        Some(model) if is_configured_custom_model(model) => {
            forward_custom_responses_request(payload, model)
        }
        Some(_) => forward_official_codex_responses_request(payload),
        None => json_response(
            HTTP_BAD_REQUEST,
            "{\"error\":\"model_missing\"}".to_string(),
            EMPTY_LOG_VALUE,
            "璇锋眰浣撶己灏?model".to_string(),
        ),
    }
}

fn is_configured_custom_model(model: &str) -> bool {
    load_provider_config()
        .ok()
        .and_then(|config| select_provider_route(&config, Some(model)))
        .is_some()
}

fn forward_custom_responses_request(
    mut payload: serde_json::Value,
    requested_model: &str,
) -> RouterResponse {
    guard_wait_tool_for_upstream(&mut payload);
    let config = match load_provider_config() {
        Ok(config) => config,
        Err(error) => {
            return json_response(
                HTTP_SERVICE_UNAVAILABLE,
                RESPONSE_CONFIG_MISSING.to_string(),
                EMPTY_LOG_VALUE,
                error,
            )
        }
    };

    let route = match select_provider_route(&config, Some(requested_model)) {
        Some(route) => route,
        None => {
            return json_response(
                HTTP_SERVICE_UNAVAILABLE,
                RESPONSE_ROUTE_MISSING.to_string(),
                EMPTY_LOG_VALUE,
                format!("未找到模型{} 对应的provider", requested_model),
            )
        }
    };

    let protocol_type = normalize_protocol_type(&route.protocol_type);
    let (upstream_url, upstream_body) = match build_custom_upstream_request(&mut payload, &route) {
        Ok(request) => request,
        Err(error) => {
            return json_response(
                HTTP_BAD_GATEWAY,
                "{\"error\":\"build_forward_body_failed\"}".to_string(),
                route.provider,
                error,
            )
        }
    };
    let full_debug_id = format!("custom-{}", current_log_millis());
    append_router_full_debug_log(
        "custom_request",
        serde_json::json!({
            "debug_id": full_debug_id,
            "requested_model": requested_model,
            "target_provider": route.provider.clone(),
            "protocol_type": protocol_type.clone(),
            "upstream_url": upstream_url.clone(),
            "normalized_payload": payload.clone(),
            "upstream_body": router_full_debug_body_value(&upstream_body)
        }),
    );
    let uses_image_generation_tool = request_uses_image_generation_tool(&payload);
    if uses_image_generation_tool {
        append_router_debug_log(
            "custom_image_generation_request",
            serde_json::json!({
                "requested_model": requested_model,
                "target_provider": route.provider.clone(),
                "protocol_type": protocol_type.clone(),
                "upstream_url": upstream_url.clone(),
                "payload": payload.clone(),
                "upstream_body": router_debug_body_value(&upstream_body)
            }),
        );
    }
    let authorization = format!("Bearer {}", route.api_key);
    let effective_proxy_url = resolve_provider_route_proxy_url(&route);
    let upstream_result = send_custom_upstream_request_with_retries(
        &upstream_url,
        effective_proxy_url.as_deref(),
        &protocol_type,
        &route.api_key,
        &authorization,
        &upstream_body,
        custom_upstream_timeout_seconds(uses_image_generation_tool),
    );

    match upstream_result {
        Ok(response) => {
            let status_code = response.status();
            let content_type = response
                .header(HEADER_CONTENT_TYPE)
                .unwrap_or(HEADER_JSON)
                .to_string();
            let body = response.into_string().unwrap_or_default();
            append_router_full_debug_log(
                "custom_upstream_response",
                serde_json::json!({
                    "debug_id": full_debug_id,
                    "requested_model": requested_model,
                    "target_provider": route.provider.clone(),
                    "protocol_type": protocol_type.clone(),
                    "status_code": status_code,
                    "content_type": content_type,
                    "upstream_body": router_full_debug_body_value(&body)
                }),
            );
            let image_generation_needs_notice = uses_image_generation_tool
                && custom_image_generation_response_needs_notice(&body, &content_type);
            let upstream_empty_text = custom_upstream_response_is_empty_text(&body, &protocol_type);
            if uses_image_generation_tool || upstream_empty_text {
                append_router_debug_log(
                    "custom_upstream_response",
                    serde_json::json!({
                        "requested_model": requested_model,
                        "target_provider": route.provider.clone(),
                        "protocol_type": protocol_type.clone(),
                        "status_code": status_code,
                        "content_type": content_type,
                        "uses_image_generation_tool": uses_image_generation_tool,
                        "image_generation_needs_notice": image_generation_needs_notice,
                        "upstream_empty_text": upstream_empty_text,
                        "contains_image_generation_result": response_contains_image_generation_result(&body),
                        "extracted_text": extract_text_for_debug(&body, &content_type, &protocol_type),
                        "raw_body": router_debug_body_value(&body)
                    }),
                );
            }
            if image_generation_needs_notice {
                return codex_sse_error_response(
                    HTTP_BAD_GATEWAY,
                    "custom_image_generation_not_configured",
                    CUSTOM_IMAGE_GENERATION_UNSUPPORTED_MESSAGE,
                    route.provider,
                );
            }
            if upstream_empty_text {
                let (error_code, error_message) =
                    if custom_empty_response_is_image_generation_unsupported(
                        uses_image_generation_tool,
                        &body,
                        &content_type,
                        &protocol_type,
                    ) {
                        (
                            "custom_image_generation_not_supported",
                            CUSTOM_IMAGE_GENERATION_UNSUPPORTED_MESSAGE,
                        )
                    } else {
                        (
                            "upstream_empty_response",
                            CUSTOM_UPSTREAM_EMPTY_RESPONSE_MESSAGE,
                        )
                    };
                return codex_sse_error_response(
                    HTTP_BAD_GATEWAY,
                    error_code,
                    error_message,
                    route.provider,
                );
            }
            let body = if protocol_type == "cpamc" {
                normalize_repeated_tool_names_in_body_with_available(
                    &body,
                    &collect_available_tool_names(&payload),
                )
            } else {
                wrap_chat_response_as_responses(&body, &route, &protocol_type, &payload)
            };
            let body = ensure_responses_stream_completed(body, &content_type);
            append_router_full_debug_log(
                "custom_router_response",
                serde_json::json!({
                    "debug_id": full_debug_id,
                    "requested_model": requested_model,
                    "target_provider": route.provider.clone(),
                    "protocol_type": protocol_type.clone(),
                    "status_code": status_code,
                    "content_type": HEADER_EVENT_STREAM,
                    "router_body": router_full_debug_body_value(&body)
                }),
            );
            if uses_image_generation_tool {
                append_router_debug_log(
                    "custom_router_response",
                    serde_json::json!({
                        "requested_model": requested_model,
                        "target_provider": route.provider.clone(),
                        "protocol_type": protocol_type.clone(),
                        "status_code": status_code,
                        "content_type": HEADER_EVENT_STREAM,
                        "extracted_text": extract_text_from_codex_sse(&body),
                        "contains_image_generation_result": response_contains_image_generation_result(&body),
                        "body": router_debug_body_value(&body)
                    }),
                );
            }
            let usage = extract_token_usage_from_body(&body);
            let usage_source = token_usage_source(usage.as_ref());

            RouterResponse {
                flush_headers_before_body: false,
                status_code,
                content_type: HEADER_EVENT_STREAM.to_string(),
                body,
                target_provider: route.provider,
                error_detail: EMPTY_LOG_VALUE.to_string(),
                usage,
                usage_source,
            }
        }
        Err(ureq::Error::Status(status_code, response)) => {
            let body = response.into_string().unwrap_or_default();
            append_router_full_debug_log(
                "custom_upstream_status",
                serde_json::json!({
                    "debug_id": full_debug_id,
                    "requested_model": requested_model,
                    "target_provider": route.provider.clone(),
                    "protocol_type": protocol_type.clone(),
                    "status_code": status_code,
                    "upstream_body": router_full_debug_body_value(&body)
                }),
            );
            let error_code = if status_code == HTTP_TOO_MANY_REQUESTS {
                "upstream_rate_limited"
            } else {
                "upstream_status"
            };
            codex_sse_error_response(
                status_code,
                error_code,
                &format!("上游返回状态码 {}：{}", status_code, body),
                route.provider,
            )
        }
        Err(error) => {
            let error_message = format_custom_upstream_error(&error);
            let mut response = codex_sse_error_response(
                HTTP_BAD_GATEWAY,
                "upstream_request_failed",
                &error_message,
                route.provider,
            );
            if upstream_error_is_retryable(&error) {
                response.flush_headers_before_body = true;
            }
            response
        }
    }
}

fn build_custom_upstream_request(
    payload: &mut serde_json::Value,
    route: &ProviderRoute,
) -> Result<(String, String), String> {
    match ProviderProtocol::from_config(&route.protocol_type) {
        protocol @ (ProviderProtocol::OpenAi | ProviderProtocol::Other) => {
            let body = build_openai_chat_body(payload, route);
            serde_json::to_string(&body)
                .map(|body| {
                    (
                        build_upstream_endpoint_url(
                            &route.base_url,
                            &route.endpoint_path,
                            protocol.default_completion_endpoint(),
                        ),
                        body,
                    )
                })
                .map_err(|error| error.to_string())
        }
        ProviderProtocol::Anthropic => {
            let body = build_anthropic_messages_body(payload, route);
            serde_json::to_string(&body)
                .map(|body| {
                    (
                        build_upstream_endpoint_url(
                            &route.base_url,
                            &route.endpoint_path,
                            ProviderProtocol::Anthropic.default_completion_endpoint(),
                        ),
                        body,
                    )
                })
                .map_err(|error| error.to_string())
        }
        ProviderProtocol::CpaMc => {
            sanitize_custom_responses_payload(payload);
            if let Some(payload_object) = payload.as_object_mut() {
                payload_object.insert(
                    "model".to_string(),
                    serde_json::Value::String(route.real_model.clone()),
                );
            }

            serde_json::to_string(payload)
                .map(|body| {
                    (
                        build_upstream_endpoint_url(
                            &route.base_url,
                            &route.endpoint_path,
                            ProviderProtocol::CpaMc.default_completion_endpoint(),
                        ),
                        body,
                    )
                })
                .map_err(|error| error.to_string())
        }
    }
}

fn build_custom_streaming_upstream_request(
    payload: &mut serde_json::Value,
    route: &ProviderRoute,
    protocol_type: &str,
) -> Result<(String, String), String> {
    match ProviderProtocol::from_config(protocol_type) {
        protocol @ (ProviderProtocol::OpenAi | ProviderProtocol::Other) => {
            let mut body = build_openai_chat_body(payload, route);
            if let Some(body_object) = body.as_object_mut() {
                body_object.insert("stream".to_string(), serde_json::Value::Bool(true));
            }
            serde_json::to_string(&body)
                .map(|body| {
                    (
                        build_upstream_endpoint_url(
                            &route.base_url,
                            &route.endpoint_path,
                            protocol.default_completion_endpoint(),
                        ),
                        body,
                    )
                })
                .map_err(|error| error.to_string())
        }
        ProviderProtocol::CpaMc => {
            sanitize_custom_responses_payload(payload);
            if let Some(payload_object) = payload.as_object_mut() {
                payload_object.insert(
                    "model".to_string(),
                    serde_json::Value::String(route.real_model.clone()),
                );
                payload_object.insert(
                    OFFICIAL_STREAM_KEY.to_string(),
                    serde_json::Value::Bool(true),
                );
            }
            serde_json::to_string(payload)
                .map(|body| {
                    (
                        build_upstream_endpoint_url(
                            &route.base_url,
                            &route.endpoint_path,
                            ProviderProtocol::CpaMc.default_completion_endpoint(),
                        ),
                        body,
                    )
                })
                .map_err(|error| error.to_string())
        }
        protocol => Err(format!(
            "streaming is not supported for protocol {}",
            protocol.as_str()
        )),
    }
}

fn build_provider_model_chat_test_request(
    route: &ProviderRoute,
    protocol_type: &str,
) -> Result<(String, String), String> {
    match ProviderProtocol::from_config(protocol_type) {
        protocol @ (ProviderProtocol::OpenAi | ProviderProtocol::Other) => {
            let mut body = serde_json::Map::new();
            body.insert(
                "model".to_string(),
                serde_json::Value::String(route.real_model.clone()),
            );
            body.insert(
                "messages".to_string(),
                serde_json::Value::Array(vec![json_chat_message("user", "hello")]),
            );
            body.insert("stream".to_string(), serde_json::Value::Bool(false));
            body.insert(
                "max_tokens".to_string(),
                serde_json::Value::Number(serde_json::Number::from(64)),
            );

            serde_json::to_string(&serde_json::Value::Object(body))
                .map(|body| {
                    (
                        build_upstream_endpoint_url(
                            &route.base_url,
                            &route.endpoint_path,
                            protocol.default_completion_endpoint(),
                        ),
                        body,
                    )
                })
                .map_err(|error| error.to_string())
        }
        ProviderProtocol::Anthropic => {
            let mut body = serde_json::Map::new();
            body.insert(
                "model".to_string(),
                serde_json::Value::String(route.real_model.clone()),
            );
            body.insert(
                "messages".to_string(),
                serde_json::Value::Array(vec![json_chat_message("user", "hello")]),
            );
            body.insert(
                "max_tokens".to_string(),
                serde_json::Value::Number(serde_json::Number::from(64)),
            );

            serde_json::to_string(&serde_json::Value::Object(body))
                .map(|body| {
                    (
                        build_upstream_endpoint_url(
                            &route.base_url,
                            &route.endpoint_path,
                            ProviderProtocol::Anthropic.default_completion_endpoint(),
                        ),
                        body,
                    )
                })
                .map_err(|error| error.to_string())
        }
        ProviderProtocol::CpaMc => {
            let mut body = serde_json::Map::new();
            body.insert(
                "model".to_string(),
                serde_json::Value::String(route.real_model.clone()),
            );
            body.insert(
                "input".to_string(),
                serde_json::Value::String("hello".to_string()),
            );
            body.insert("stream".to_string(), serde_json::Value::Bool(false));
            body.insert(
                "max_output_tokens".to_string(),
                serde_json::Value::Number(serde_json::Number::from(64)),
            );

            serde_json::to_string(&serde_json::Value::Object(body))
                .map(|body| {
                    (
                        build_upstream_endpoint_url(
                            &route.base_url,
                            &route.endpoint_path,
                            ProviderProtocol::CpaMc.default_completion_endpoint(),
                        ),
                        body,
                    )
                })
                .map_err(|error| error.to_string())
        }
    }
}

fn sanitize_custom_responses_payload(payload: &mut serde_json::Value) {
    remove_responses_input_namespace(payload);
    normalize_custom_responses_tool_definitions(payload);
    remove_hosted_image_generation_tool_conflicts(payload);
}

fn normalize_custom_responses_tool_definitions(payload: &mut serde_json::Value) {
    fn normalize_containers(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(tools) = object
                    .get_mut("tools")
                    .and_then(|tools| tools.as_array_mut())
                {
                    normalize_responses_tool_array(tools);
                }
                for child in object.values_mut() {
                    normalize_containers(child);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    normalize_containers(item);
                }
            }
            _ => {}
        }
    }

    normalize_containers(payload);
}

fn normalize_responses_tool_array(tools: &mut Vec<serde_json::Value>) {
    tools.retain_mut(|tool| {
        normalize_responses_tool_definition(tool);
        responses_tool_definition_is_valid(tool)
    });
}

fn normalize_responses_tool_definition(tool: &mut serde_json::Value) {
    let Some(object) = tool.as_object_mut() else {
        return;
    };
    let tool_type = object
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();

    if tool_type == "function" && !object.contains_key("name") {
        if let Some(function) = object
            .remove("function")
            .and_then(|function| function.as_object().cloned())
        {
            for key in ["name", "description", "parameters", "strict", "namespace"] {
                if !object.contains_key(key) {
                    if let Some(value) = function.get(key).cloned() {
                        object.insert(key.to_string(), value);
                    }
                }
            }
        }
    }

    if matches!(tool_type.as_str(), "namespace" | "function" | "custom")
        && !object.contains_key("name")
    {
        let derived_name = ["namespace", "server_label", "serverLabel"]
            .into_iter()
            .find_map(|key| object.get(key).and_then(|value| value.as_str()))
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string);
        if let Some(name) = derived_name {
            object.insert("name".to_string(), serde_json::Value::String(name));
        }
    }

    if matches!(tool_type.as_str(), "namespace" | "function" | "custom") {
        let namespace = object
            .remove("namespace")
            .or_else(|| object.remove("server_label"))
            .or_else(|| object.remove("serverLabel"))
            .and_then(|value| value.as_str().map(str::to_string))
            .map(|namespace| namespace.trim().to_string())
            .filter(|namespace| !namespace.is_empty());
        if tool_type != "namespace" {
            if let (Some(namespace), Some(name)) = (
                namespace,
                object
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
            ) {
                if !name.starts_with(&format!("{}.", namespace))
                    && !name.starts_with(&format!("{}__", namespace))
                {
                    object.insert(
                        "name".to_string(),
                        serde_json::Value::String(format!("{}.{}", namespace, name)),
                    );
                }
            }
        }
    }

    if tool_type == "namespace"
        && !object
            .get("description")
            .and_then(|value| value.as_str())
            .is_some_and(|description| !description.trim().is_empty())
    {
        let name = object
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("tools");
        object.insert(
            "description".to_string(),
            serde_json::Value::String(format!("Tools in the {} namespace.", name)),
        );
    }

    if let Some(children) = object
        .get_mut("tools")
        .and_then(|children| children.as_array_mut())
    {
        normalize_responses_tool_array(children);
    }
}

fn responses_tool_definition_is_valid(tool: &serde_json::Value) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if !matches!(tool_type, "namespace" | "function" | "custom") {
        return true;
    }
    extract_tool_definition_name(tool).is_some()
}

fn remove_hosted_image_generation_tool_conflicts(payload: &mut serde_json::Value) {
    fn remove_conflicts_from_tool_array(tools: &mut Vec<serde_json::Value>) -> bool {
        if !tools.iter().any(is_hosted_image_generation_tool) {
            return false;
        }

        let mut removed_conflict = false;
        tools.retain_mut(|tool| {
            let namespace = tool_namespace_name(tool).map(str::to_string);
            if let Some(children) = tool
                .get_mut("tools")
                .and_then(|children| children.as_array_mut())
            {
                children.retain(|child| {
                    let conflicts =
                        is_local_image_generation_tool(child, namespace.as_deref());
                    removed_conflict |= conflicts;
                    !conflicts
                });
                if children.is_empty() && namespace.as_deref() == Some("image_gen") {
                    removed_conflict = true;
                    return false;
                }
                if namespace.as_deref() == Some("image_gen") {
                    return true;
                }
            }

            let conflicts = is_local_image_generation_tool(tool, None);
            removed_conflict |= conflicts;
            !conflicts
        });
        removed_conflict
    }

    fn remove_conflicts_recursively(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => {
                let removed_conflict = object
                    .get_mut("tools")
                    .and_then(|tools| tools.as_array_mut())
                    .is_some_and(remove_conflicts_from_tool_array);

                if removed_conflict
                    && object
                        .get("tool_choice")
                        .is_some_and(tool_choice_targets_local_image_generation)
                {
                    object.insert(
                        "tool_choice".to_string(),
                        serde_json::Value::String("auto".to_string()),
                    );
                }

                for child in object.values_mut() {
                    remove_conflicts_recursively(child);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    remove_conflicts_recursively(item);
                }
            }
            _ => {}
        }
    }

    remove_conflicts_recursively(payload);
}

fn is_hosted_image_generation_tool(tool: &serde_json::Value) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if matches!(tool_type, "image_generation" | "tool_search") {
        return true;
    }
    matches!(tool_type, "hosted" | "hosted_tool")
        && extract_tool_definition_name(tool) == Some("image_generation")
}

fn tool_namespace_name(tool: &serde_json::Value) -> Option<&str> {
    tool.get("namespace")
        .or_else(|| tool.get("server_label"))
        .or_else(|| tool.get("serverLabel"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            (tool.get("type").and_then(|value| value.as_str()) == Some("namespace"))
                .then(|| {
                    tool.get("name")
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                })
                .flatten()
        })
}

fn is_local_image_generation_tool(
    tool: &serde_json::Value,
    parent_namespace: Option<&str>,
) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let name = extract_tool_definition_name(tool).unwrap_or_default();
    if matches!(name, "image_gen.imagegen" | "image_gen__imagegen") {
        return true;
    }

    let namespace = tool_namespace_name(tool).or(parent_namespace);
    if tool_type == "mcp" && namespace == Some("image_gen") {
        return true;
    }
    if namespace == Some("image_gen") && matches!(name, "imagegen" | "image_gen") {
        return true;
    }

    if tool
        .get("function")
        .is_some_and(|function| is_local_image_generation_tool(function, namespace))
    {
        return true;
    }

    let serialized = tool.to_string();
    serialized.contains("\"image_gen.imagegen\"")
        || serialized.contains("\"image_gen__imagegen\"")
        || (serialized.contains("\"image_gen\"") && serialized.contains("\"imagegen\""))
}

fn tool_choice_targets_local_image_generation(tool_choice: &serde_json::Value) -> bool {
    match tool_choice {
        serde_json::Value::String(name) => {
            matches!(
                name.as_str(),
                "imagegen" | "image_gen.imagegen" | "image_gen__imagegen"
            )
        }
        serde_json::Value::Object(object) => {
            is_local_image_generation_tool(tool_choice, None)
                || extract_tool_definition_name(tool_choice) == Some("imagegen")
                || object
                    .values()
                    .any(tool_choice_targets_local_image_generation)
        }
        serde_json::Value::Array(items) => {
            items.iter().any(tool_choice_targets_local_image_generation)
        }
        _ => false,
    }
}

fn remove_responses_input_namespace(payload: &mut serde_json::Value) {
    let Some(items) = payload
        .get_mut(OFFICIAL_INPUT_KEY)
        .and_then(|value| value.as_array_mut())
    else {
        return;
    };

    for item in items {
        remove_json_key_recursive(item, RESPONSES_INPUT_NAMESPACE_KEY);
    }
}

fn remove_json_key_recursive(value: &mut serde_json::Value, key: &str) {
    match value {
        serde_json::Value::Object(object) => {
            object.remove(key);
            for child in object.values_mut() {
                remove_json_key_recursive(child, key);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                remove_json_key_recursive(item, key);
            }
        }
        _ => {}
    }
}

fn build_openai_chat_body(payload: &serde_json::Value, route: &ProviderRoute) -> serde_json::Value {
    let mut body = codex_protocol::responses_to_openai(payload, &route.real_model);
    let active_exec_cell_ids = collect_active_exec_cell_ids(payload);
    let uses_codex_custom_tool_bridge = request_has_codex_custom_tools(payload);
    let object = body.as_object_mut().expect("protocol bridge returns an object");
    object.insert("stream".to_string(), serde_json::Value::Bool(false));

    if let Some(mut tools) = object.get("tools").and_then(|value| value.as_array()).cloned() {
        tools.retain(|tool| {
            chat_function_tool_name(tool) != Some("wait") || !active_exec_cell_ids.is_empty()
        });
        for tool in &mut tools {
            if chat_function_tool_name(tool) == Some("wait") {
                constrain_chat_wait_tool(tool, &active_exec_cell_ids);
            }
        }
        object.insert("tools".to_string(), serde_json::Value::Array(tools));
    }

    if uses_codex_custom_tool_bridge {
        object.insert(
            "parallel_tool_calls".to_string(),
            serde_json::Value::Bool(false),
        );
    } else if let Some(parallel_tool_calls) = payload.get("parallel_tool_calls").cloned() {
        object.insert("parallel_tool_calls".to_string(), parallel_tool_calls);
    }

    if let Some(temperature) = payload.get(OFFICIAL_TEMPERATURE_KEY).cloned() {
        object.insert("temperature".to_string(), temperature);
    }

    if let Some(max_tokens) = payload.get(OFFICIAL_MAX_OUTPUT_TOKENS_KEY).cloned() {
        object.insert("max_tokens".to_string(), max_tokens);
    }

    body
}

fn chat_function_tool_name(tool: &serde_json::Value) -> Option<&str> {
    tool.get("function")
        .and_then(|function| function.get("name"))
        .and_then(|name| name.as_str())
}

fn constrain_chat_wait_tool(tool: &mut serde_json::Value, active_cell_ids: &[String]) {
    let Some(function) = tool
        .get_mut("function")
        .and_then(|function| function.as_object_mut())
    else {
        return;
    };

    function.insert(
        "description".to_string(),
        serde_json::Value::String(format!(
            "Wait only for an exec cell that is currently running. Valid cell_id values: {}. Never invent a cell ID and never use noop.",
            active_cell_ids.join(", ")
        )),
    );
    let parameters = function
        .entry("parameters".to_string())
        .or_insert_with(|| serde_json::json!({ "type": "object", "properties": {} }));
    if !parameters.is_object() {
        *parameters = serde_json::json!({ "type": "object", "properties": {} });
    }
    let parameters = parameters.as_object_mut().expect("wait parameters object");
    parameters.insert(
        "type".to_string(),
        serde_json::Value::String("object".to_string()),
    );
    let properties = parameters
        .entry("properties".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !properties.is_object() {
        *properties = serde_json::json!({});
    }
    let properties = properties.as_object_mut().expect("wait properties object");
    let cell_id = properties
        .entry("cell_id".to_string())
        .or_insert_with(|| serde_json::json!({ "type": "string" }));
    if !cell_id.is_object() {
        *cell_id = serde_json::json!({ "type": "string" });
    }
    let cell_id = cell_id.as_object_mut().expect("wait cell_id object");
    cell_id.insert(
        "type".to_string(),
        serde_json::Value::String("string".to_string()),
    );
    cell_id.insert(
        "enum".to_string(),
        serde_json::Value::Array(
            active_cell_ids
                .iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect(),
        ),
    );
}

fn collect_active_exec_cell_ids(payload: &serde_json::Value) -> Vec<String> {
    let Some(items) = payload
        .get(OFFICIAL_INPUT_KEY)
        .and_then(|input| input.as_array())
    else {
        return Vec::new();
    };
    let mut active = HashSet::<String>::new();
    let mut calls = HashMap::<String, (String, Option<String>)>::new();

    for item in items {
        let item_type = item
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        match item_type {
            "function_call" | "custom_tool_call" => {
                let name = item
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                if name != "wait" {
                    active.clear();
                }
                let Some(call_id) = item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(|value| value.as_str())
                else {
                    continue;
                };
                let cell_id = item
                    .get("arguments")
                    .and_then(|arguments| arguments.as_str())
                    .and_then(wait_cell_id_from_arguments);
                calls.insert(call_id.to_string(), (name, cell_id));
            }
            "function_call_output" | "custom_tool_call_output" => {
                let output = item.get("output").map(content_to_text).unwrap_or_default();
                if let Some(call_id) = item.get("call_id").and_then(|value| value.as_str()) {
                    if let Some((name, Some(cell_id))) = calls.get(call_id) {
                        if name == "wait" && extract_running_exec_cell_ids(&output).is_empty() {
                            active.remove(cell_id);
                        }
                    }
                }
                active.extend(extract_running_exec_cell_ids(&output));
            }
            "message" => active.clear(),
            _ => {}
        }
    }

    let mut active = active.into_iter().collect::<Vec<_>>();
    active.sort();
    active
}

fn wait_cell_id_from_arguments(arguments: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .and_then(|arguments| arguments.get("cell_id").cloned())
        .and_then(|cell_id| cell_id.as_str().map(str::to_string))
        .filter(|cell_id| !cell_id.trim().is_empty())
}

fn extract_running_exec_cell_ids(output: &str) -> Vec<String> {
    const MARKER: &str = "Script running with cell ID ";
    let mut ids = Vec::new();
    let mut remaining = output;
    while let Some(index) = remaining.find(MARKER) {
        let after_marker = &remaining[index + MARKER.len()..];
        let id = after_marker
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_matches(|character: char| {
                matches!(
                    character,
                    '`' | '\'' | '"' | ',' | '.' | ';' | ':' | ')' | ']'
                )
            });
        if !id.is_empty() && id != "noop" {
            ids.push(id.to_string());
        }
        remaining = after_marker;
    }
    ids
}

fn guard_wait_tool_for_upstream(payload: &mut serde_json::Value) {
    let active_cell_ids = collect_active_exec_cell_ids(payload);
    let mut saw_wait_tool = false;
    if let Some(tools) = payload
        .get_mut("tools")
        .and_then(|tools| tools.as_array_mut())
    {
        tools.retain_mut(|tool| {
            let tool_type = tool
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if tool_type == "namespace" {
                let namespace = tool
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                if let Some(children) = tool.get_mut("tools").and_then(|tools| tools.as_array_mut())
                {
                    children.retain_mut(|child| {
                        let is_wait = child.get("name").and_then(|value| value.as_str())
                            == Some("wait")
                            || (namespace == "wait"
                                && child.get("type").and_then(|value| value.as_str())
                                    == Some("function"));
                        if !is_wait {
                            return true;
                        }
                        saw_wait_tool = true;
                        if active_cell_ids.is_empty() {
                            return false;
                        }
                        constrain_responses_wait_tool(child, &active_cell_ids);
                        true
                    });
                    return !children.is_empty();
                }
                return true;
            }

            let is_wait = tool.get("name").and_then(|value| value.as_str()) == Some("wait");
            if !is_wait {
                return true;
            }
            saw_wait_tool = true;
            if active_cell_ids.is_empty() {
                return false;
            }
            constrain_responses_wait_tool(tool, &active_cell_ids);
            true
        });
    }

    if saw_wait_tool {
        if let Some(object) = payload.as_object_mut() {
            object.insert(
                "parallel_tool_calls".to_string(),
                serde_json::Value::Bool(false),
            );
            if active_cell_ids.is_empty()
                && object
                    .get("tool_choice")
                    .is_some_and(tool_choice_targets_wait)
            {
                object.insert(
                    "tool_choice".to_string(),
                    serde_json::Value::String("auto".to_string()),
                );
            }
        }
    }
}

fn tool_choice_targets_wait(tool_choice: &serde_json::Value) -> bool {
    match tool_choice {
        serde_json::Value::Object(object) => {
            object.get("name").and_then(|value| value.as_str()) == Some("wait")
                || object.get("namespace").and_then(|value| value.as_str()) == Some("wait")
                || object.values().any(tool_choice_targets_wait)
        }
        serde_json::Value::Array(items) => items.iter().any(tool_choice_targets_wait),
        _ => false,
    }
}

fn constrain_responses_wait_tool(tool: &mut serde_json::Value, active_cell_ids: &[String]) {
    let Some(object) = tool.as_object_mut() else {
        return;
    };
    object.insert(
        "description".to_string(),
        serde_json::Value::String(format!(
            "Wait only for a currently running exec cell. Valid cell_id values: {}. Never invent a cell ID and never use noop.",
            active_cell_ids.join(", ")
        )),
    );
    let parameters = object
        .entry("parameters".to_string())
        .or_insert_with(|| serde_json::json!({ "type": "object", "properties": {} }));
    if !parameters.is_object() {
        *parameters = serde_json::json!({ "type": "object", "properties": {} });
    }
    let parameters = parameters.as_object_mut().expect("wait parameters object");
    parameters.insert(
        "type".to_string(),
        serde_json::Value::String("object".to_string()),
    );
    let properties = parameters
        .entry("properties".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !properties.is_object() {
        *properties = serde_json::json!({});
    }
    let properties = properties.as_object_mut().expect("wait properties object");
    let cell_id = properties
        .entry("cell_id".to_string())
        .or_insert_with(|| serde_json::json!({ "type": "string" }));
    if !cell_id.is_object() {
        *cell_id = serde_json::json!({ "type": "string" });
    }
    let cell_id = cell_id.as_object_mut().expect("wait cell_id object");
    cell_id.insert(
        "type".to_string(),
        serde_json::Value::String("string".to_string()),
    );
    cell_id.insert(
        "enum".to_string(),
        serde_json::Value::Array(
            active_cell_ids
                .iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect(),
        ),
    );
}

fn build_anthropic_messages_body(
    payload: &serde_json::Value,
    route: &ProviderRoute,
) -> serde_json::Value {
    let mut body = serde_json::Map::new();
    body.insert(
        "model".to_string(),
        serde_json::Value::String(route.real_model.clone()),
    );
    body.insert(
        "messages".to_string(),
        serde_json::Value::Array(extract_chat_messages(payload, true)),
    );
    body.insert(
        "max_tokens".to_string(),
        payload
            .get(OFFICIAL_MAX_OUTPUT_TOKENS_KEY)
            .cloned()
            .unwrap_or_else(|| {
                serde_json::Value::Number(serde_json::Number::from(DEFAULT_MAX_OUTPUT_TOKENS))
            }),
    );

    if let Some(system) = extract_system_prompt(payload) {
        body.insert("system".to_string(), serde_json::Value::String(system));
    }

    if let Some(temperature) = payload.get(OFFICIAL_TEMPERATURE_KEY).cloned() {
        body.insert("temperature".to_string(), temperature);
    }

    serde_json::Value::Object(body)
}

fn extract_chat_messages(payload: &serde_json::Value, omit_system: bool) -> Vec<serde_json::Value> {
    if let Some(messages) = payload.get("messages").and_then(|value| value.as_array()) {
        return messages
            .iter()
            .filter_map(|message| normalize_chat_message(message, omit_system))
            .collect::<Vec<_>>();
    }

    let mut messages = Vec::new();
    if let Some(instructions) = payload
        .get(OFFICIAL_INSTRUCTIONS_KEY)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !omit_system {
            messages.push(json_chat_message("system", instructions));
        }
    }

    match payload.get(OFFICIAL_INPUT_KEY) {
        Some(serde_json::Value::Array(items)) => {
            let mut pending_function_calls: Vec<serde_json::Value> = Vec::new();
            let mut seen_tool_call_ids = HashSet::<String>::new();

            for item in items {
                let item_type = item
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();

                if item_type == "function_call" || item_type == "custom_tool_call" {
                    if let Some(call_id) = item
                        .get("call_id")
                        .or_else(|| item.get("id"))
                        .and_then(|value| value.as_str())
                    {
                        seen_tool_call_ids.insert(call_id.to_string());
                    }
                    pending_function_calls.push(item.clone());
                } else {
                    if !pending_function_calls.is_empty() {
                        if let Some(merged) =
                            merge_function_calls_into_assistant(&pending_function_calls)
                        {
                            messages.push(merged);
                        }
                        pending_function_calls.clear();
                    }
                    if matches!(
                        item_type,
                        "function_call_output" | "custom_tool_call_output"
                    ) {
                        let call_id = item
                            .get("call_id")
                            .and_then(|value| value.as_str())
                            .unwrap_or_default();
                        if !call_id.is_empty() && !seen_tool_call_ids.contains(call_id) {
                            let output =
                                item.get("output").map(content_to_text).unwrap_or_default();
                            messages.push(json_chat_message(
                                "user",
                                &format!(
                                    "Tool output for unavailable call {}: {}",
                                    call_id, output
                                ),
                            ));
                            continue;
                        }
                    }
                    if let Some(message) = normalize_chat_message(item, omit_system) {
                        messages.push(message);
                    }
                }
            }

            if !pending_function_calls.is_empty() {
                if let Some(merged) = merge_function_calls_into_assistant(&pending_function_calls) {
                    messages.push(merged);
                }
            }
        }
        Some(value) => messages.push(json_chat_message("user", &content_to_text(value))),
        None => messages.push(json_chat_message("user", "")),
    }

    if messages.is_empty() {
        messages.push(json_chat_message("user", ""));
    }

    messages
}

fn merge_function_calls_into_assistant(
    function_calls: &[serde_json::Value],
) -> Option<serde_json::Value> {
    if function_calls.is_empty() {
        return None;
    }

    let tool_calls: Vec<serde_json::Value> = function_calls
        .iter()
        .filter_map(|fc| {
            let name = fc.get("name")?.as_str()?.trim();
            if name.is_empty() {
                return None;
            }
            let call_id = fc
                .get("call_id")
                .or_else(|| fc.get("id"))
                .and_then(|id| id.as_str())
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .unwrap_or("call_router");
            let arguments =
                if fc.get("type").and_then(|value| value.as_str()) == Some("custom_tool_call") {
                    serde_json::json!({
                        "input": fc.get("input").cloned().unwrap_or_default()
                    })
                    .to_string()
                } else {
                    fc.get("arguments")
                        .and_then(|a| a.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| {
                            fc.get("arguments")
                                .map(|a| a.to_string())
                                .unwrap_or_else(|| "{}".to_string())
                        })
                };

            Some(serde_json::json!({
                "id": call_id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": arguments
                }
            }))
        })
        .collect();

    if tool_calls.is_empty() {
        return None;
    }

    Some(serde_json::json!({
        "role": "assistant",
        "content": null,
        "tool_calls": tool_calls
    }))
}

#[allow(dead_code)]
fn convert_responses_tools_to_chat_tools(
    value: &serde_json::Value,
) -> Option<Vec<serde_json::Value>> {
    let tools = value.as_array()?;
    let converted = tools
        .iter()
        .flat_map(|tool| {
            if tool.get("type").and_then(|value| value.as_str()) == Some("namespace") {
                convert_responses_namespace_tool_to_chat_tools(tool)
            } else {
                convert_responses_tool_to_chat_tool(tool)
                    .into_iter()
                    .collect()
            }
        })
        .collect::<Vec<_>>();
    if converted.is_empty() {
        None
    } else {
        Some(converted)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NamespaceToolMapping {
    flattened_name: String,
    namespace: String,
    name: String,
}

fn flatten_namespace_tool_name(namespace: &str, name: &str) -> String {
    if namespace.is_empty() {
        return name.to_string();
    }
    if name.is_empty() {
        return namespace.to_string();
    }
    if namespace.ends_with("__") || name.starts_with("__") {
        format!("{}{}", namespace, name)
    } else {
        format!("{}__{}", namespace, name)
    }
}

fn collect_namespace_tool_mappings(payload: &serde_json::Value) -> Vec<NamespaceToolMapping> {
    let Some(tools) = payload.get("tools").and_then(|tools| tools.as_array()) else {
        return Vec::new();
    };
    let mut mappings = Vec::new();
    for tool in tools {
        if tool.get("type").and_then(|value| value.as_str()) != Some("namespace") {
            continue;
        }
        let namespace = tool
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let Some(children) = tool.get("tools").and_then(|tools| tools.as_array()) else {
            continue;
        };
        for child in children {
            let Some(name) = child.get("name").and_then(|value| value.as_str()) else {
                continue;
            };
            mappings.push(NamespaceToolMapping {
                flattened_name: flatten_namespace_tool_name(namespace, name),
                namespace: namespace.to_string(),
                name: name.to_string(),
            });
        }
    }
    mappings
}

#[allow(dead_code)]
fn convert_responses_namespace_tool_to_chat_tools(
    tool: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let namespace = tool
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let namespace_description = tool
        .get("description")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let Some(children) = tool.get("tools").and_then(|tools| tools.as_array()) else {
        return Vec::new();
    };
    children
        .iter()
        .filter(|child| child.get("type").and_then(|value| value.as_str()) == Some("function"))
        .filter_map(|child| {
            let name = child.get("name")?.as_str()?;
            let child_description = child
                .get("description")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let description = [namespace_description, child_description]
                .into_iter()
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            let mut function = serde_json::Map::new();
            function.insert(
                "name".to_string(),
                serde_json::Value::String(flatten_namespace_tool_name(namespace, name)),
            );
            if !description.is_empty() {
                function.insert(
                    "description".to_string(),
                    serde_json::Value::String(description),
                );
            }
            function.insert(
                "parameters".to_string(),
                child
                    .get("parameters")
                    .cloned()
                    .filter(|parameters| parameters.is_object())
                    .unwrap_or_else(|| serde_json::json!({ "type": "object", "properties": {} })),
            );
            Some(serde_json::json!({
                "type": "function",
                "function": serde_json::Value::Object(function)
            }))
        })
        .collect()
}

#[allow(dead_code)]
fn convert_responses_tool_to_chat_tool(tool: &serde_json::Value) -> Option<serde_json::Value> {
    let tool_type = tool
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if tool_type == "custom" {
        let name = tool.get("name")?.as_str()?.trim();
        if name.is_empty() {
            return None;
        }
        return Some(serde_json::json!({
            "type": "function",
            "function": {
                "name": name,
                "description": tool.get("description").cloned().unwrap_or_else(|| {
                    serde_json::Value::String(
                        "Raw string input for the original Codex custom tool".to_string(),
                    )
                }),
                "parameters": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string" }
                    },
                    "required": ["input"]
                }
            }
        }));
    }
    if tool_type != "function" {
        return None;
    }

    if tool.get("function").is_some() {
        return Some(tool.clone());
    }

    let name = tool.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }

    let mut function = serde_json::Map::new();
    function.insert(
        "name".to_string(),
        serde_json::Value::String(name.to_string()),
    );
    if let Some(description) = tool.get("description").cloned() {
        function.insert("description".to_string(), description);
    }
    if let Some(parameters) = tool.get("parameters").cloned() {
        function.insert("parameters".to_string(), parameters);
    }

    Some(serde_json::json!({
        "type": "function",
        "function": serde_json::Value::Object(function)
    }))
}

#[allow(dead_code)]
fn convert_responses_tool_choice_to_chat(tool_choice: &serde_json::Value) -> serde_json::Value {
    let choice_type = tool_choice.get("type").and_then(|value| value.as_str());
    if choice_type == Some("custom") || choice_type == Some("function") {
        if let Some(name) = tool_choice.get("name").and_then(|value| value.as_str()) {
            return serde_json::json!({
                "type": "function",
                "function": { "name": name }
            });
        }
    }
    tool_choice.clone()
}

fn normalize_chat_message(
    value: &serde_json::Value,
    omit_system: bool,
) -> Option<serde_json::Value> {
    let item_type = value
        .get("type")
        .and_then(|item_type| item_type.as_str())
        .unwrap_or_default();
    if item_type == "function_call" || item_type == "custom_tool_call" {
        return normalize_responses_function_call(value);
    }
    if item_type == "function_call_output" || item_type == "custom_tool_call_output" {
        return normalize_responses_function_call_output(value);
    }

    let role = value
        .get("role")
        .and_then(|role| role.as_str())
        .map(normalize_message_role)
        .unwrap_or_else(|| "user".to_string());

    if omit_system && role == "system" {
        return None;
    }

    let mut message = serde_json::Map::new();
    message.insert("role".to_string(), serde_json::Value::String(role.clone()));

    let content = value
        .get("content")
        .map(content_to_text)
        .unwrap_or_else(|| content_to_text(value));
    message.insert("content".to_string(), serde_json::Value::String(content));

    if role == "assistant" {
        if let Some(tool_calls) = value.get("tool_calls").cloned() {
            message.insert("tool_calls".to_string(), tool_calls);
        }
    }

    if role == "tool" {
        if let Some(tool_call_id) = value.get("tool_call_id").cloned() {
            message.insert("tool_call_id".to_string(), tool_call_id);
        }
    }

    if let Some(name) = value.get("name").and_then(|v| v.as_str()) {
        message.insert(
            "name".to_string(),
            serde_json::Value::String(name.to_string()),
        );
    }

    Some(serde_json::Value::Object(message))
}

fn normalize_responses_function_call(value: &serde_json::Value) -> Option<serde_json::Value> {
    let name = value.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    let call_id = value
        .get("call_id")
        .or_else(|| value.get("id"))
        .and_then(|call_id| call_id.as_str())
        .map(str::trim)
        .filter(|call_id| !call_id.is_empty())
        .unwrap_or("call_router");
    let arguments =
        if value.get("type").and_then(|item_type| item_type.as_str()) == Some("custom_tool_call") {
            serde_json::json!({
                "input": value.get("input").cloned().unwrap_or_default()
            })
            .to_string()
        } else {
            value
                .get("arguments")
                .and_then(|arguments| arguments.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    value
                        .get("arguments")
                        .map(|arguments| arguments.to_string())
                        .unwrap_or_else(|| "{}".to_string())
                })
        };

    Some(serde_json::json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [{
            "id": call_id,
            "type": "function",
            "function": {
                "name": name,
                "arguments": arguments
            }
        }]
    }))
}

fn normalize_responses_function_call_output(
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    let call_id = value
        .get("call_id")
        .or_else(|| value.get("id"))
        .and_then(|call_id| call_id.as_str())
        .map(str::trim)
        .filter(|call_id| !call_id.is_empty())?;
    let content = value
        .get("output")
        .or_else(|| value.get("content"))
        .map(content_to_text)
        .unwrap_or_default();

    Some(serde_json::json!({
        "role": "tool",
        "tool_call_id": call_id,
        "content": content
    }))
}

fn normalize_message_role(role: &str) -> String {
    match role.trim().to_ascii_lowercase().as_str() {
        "assistant" => "assistant".to_string(),
        "system" => "system".to_string(),
        "tool" => "tool".to_string(),
        _ => "user".to_string(),
    }
}

fn json_chat_message(role: &str, content: &str) -> serde_json::Value {
    let mut message = serde_json::Map::new();
    message.insert(
        "role".to_string(),
        serde_json::Value::String(role.to_string()),
    );
    message.insert(
        "content".to_string(),
        serde_json::Value::String(content.to_string()),
    );
    serde_json::Value::Object(message)
}

fn content_to_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => decode_protocol_text_entities(text),
        serde_json::Value::Array(items) => items
            .iter()
            .map(content_to_text)
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        serde_json::Value::Object(map) => {
            for key in ["text", "input_text", "output_text", "content", "part", "clip"] {
                if let Some(value) = map.get(key) {
                    let text = content_to_text(value);
                    if !text.trim().is_empty() {
                        return text;
                    }
                }
            }
            String::new()
        }
        serde_json::Value::Null => String::new(),
        value => value.to_string(),
    }
}

fn extract_system_prompt(payload: &serde_json::Value) -> Option<String> {
    payload
        .get(OFFICIAL_INSTRUCTIONS_KEY)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn request_uses_image_generation_tool(payload: &serde_json::Value) -> bool {
    if !request_has_image_generation_tool(payload) {
        return false;
    }

    if payload
        .get("tool_choice")
        .map(tool_choice_selects_image_generation)
        .unwrap_or(false)
    {
        return true;
    }

    latest_user_prompt_text(payload)
        .map(|text| text_asks_for_image_generation(&text))
        .unwrap_or(false)
}

fn request_has_image_generation_tool(payload: &serde_json::Value) -> bool {
    payload
        .get("tools")
        .and_then(|value| value.as_array())
        .map(|tools| {
            tools.iter().any(|tool| {
                tool.get("type")
                    .and_then(|value| value.as_str())
                    .map(|tool_type| tool_type == "image_generation")
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn tool_choice_selects_image_generation(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(choice) => choice.eq_ignore_ascii_case("image_generation"),
        serde_json::Value::Object(map) => map
            .get("type")
            .and_then(|value| value.as_str())
            .map(|tool_type| tool_type == "image_generation")
            .unwrap_or(false),
        _ => false,
    }
}

fn latest_user_prompt_text(payload: &serde_json::Value) -> Option<String> {
    if let Some(messages) = payload.get("messages").and_then(|value| value.as_array()) {
        return messages
            .iter()
            .rev()
            .find(|message| {
                message
                    .get("role")
                    .and_then(|role| role.as_str())
                    .map(|role| role.eq_ignore_ascii_case("user"))
                    .unwrap_or(false)
            })
            .map(message_text);
    }

    match payload.get(OFFICIAL_INPUT_KEY) {
        Some(serde_json::Value::String(text)) => Some(text.clone()),
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .rev()
            .find(|item| response_input_item_is_user_message(item))
            .map(message_text),
        Some(value) => Some(content_to_text(value)),
        None => None,
    }
    .map(|text| text.trim().to_string())
    .filter(|text| !text.is_empty())
}

fn response_input_item_is_user_message(item: &serde_json::Value) -> bool {
    let item_type = item
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if item_type == "function_call"
        || item_type == "function_call_output"
        || item_type == "image_generation_call"
        || item_type == "output_image"
    {
        return false;
    }

    item.get("role")
        .and_then(|role| role.as_str())
        .map(|role| role.eq_ignore_ascii_case("user"))
        .unwrap_or(item.get("content").is_some() || item.get("text").is_some())
}

fn message_text(value: &serde_json::Value) -> String {
    value
        .get("content")
        .map(content_to_text)
        .or_else(|| value.get("text").map(content_to_text))
        .unwrap_or_else(|| content_to_text(value))
}

fn text_asks_for_image_generation(text: &str) -> bool {
    let trimmed = text.trim();
    let normalized = trimmed.to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    let english_image_terms = [
        "image",
        "picture",
        "photo",
        "illustration",
        "drawing",
        "wallpaper",
        "avatar",
    ];
    let english_generate_terms = [
        "generate ",
        "create ",
        "draw ",
        "make ",
        "render ",
        "paint ",
        "edit ",
    ];
    if english_image_terms
        .iter()
        .any(|term| normalized.contains(term))
        && english_generate_terms
            .iter()
            .any(|term| normalized.contains(term))
    {
        return true;
    }

    let chinese_image_terms = [
        "图片",
        "图像",
        "照片",
        "插画",
        "头像",
        "壁纸",
        "海报",
        "封面",
        "小猫图",
        "猫图",
    ];
    let chinese_generate_terms = [
        "生成",
        "画",
        "绘制",
        "做",
        "来一张",
        "出一张",
        "帮我画",
        "帮我生成",
    ];
    if chinese_image_terms
        .iter()
        .any(|term| trimmed.contains(term))
        && chinese_generate_terms
            .iter()
            .any(|term| trimmed.contains(term))
    {
        return true;
    }

    let chinese_triggers = [
        "生图",
        "生成图片",
        "生成一张图",
        "生成一幅图",
        "画一张",
        "画个",
        "画幅",
        "绘制",
        "做一张图",
        "做个图",
        "出一张图",
        "帮我画",
        "帮我生成图",
        "帮我生成一张",
        "生成照片",
        "生成头像",
        "生成插画",
        "生成海报",
    ];
    chinese_triggers
        .iter()
        .any(|trigger| text.contains(trigger))
}

fn custom_image_generation_response_needs_notice(body: &str, content_type: &str) -> bool {
    if response_contains_image_generation_result(body) {
        return false;
    }
    if response_contains_tool_call_request(body) {
        return false;
    }

    if content_type
        .to_ascii_lowercase()
        .contains("text/event-stream")
        || body.contains("event:")
        || body.contains("data:")
    {
        return true;
    }

    true
}

fn custom_empty_response_is_image_generation_unsupported(
    uses_image_generation_tool: bool,
    body: &str,
    content_type: &str,
    protocol_type: &str,
) -> bool {
    uses_image_generation_tool
        && custom_image_generation_response_needs_notice(body, content_type)
        && custom_upstream_response_is_empty_text(body, protocol_type)
}

fn extract_text_for_debug(body: &str, content_type: &str, protocol_type: &str) -> String {
    if content_type
        .to_ascii_lowercase()
        .contains("text/event-stream")
        || body.contains("event:")
        || body.contains("data:")
    {
        return extract_text_from_codex_sse(body);
    }

    let Ok(root) = serde_json::from_str::<serde_json::Value>(body.trim()) else {
        return body.trim().to_string();
    };

    if protocol_type == "anthropic" {
        return extract_anthropic_text(&root);
    }
    if protocol_type == "openai" || protocol_type == "other" {
        return extract_openai_chat_text(&root);
    }
    extract_responses_text(&root)
}

fn response_contains_image_generation_result(body: &str) -> bool {
    if let Ok(root) = serde_json::from_str::<serde_json::Value>(body.trim()) {
        return value_contains_image_generation_result(&root);
    }

    iter_codex_sse_json_values(body)
        .iter()
        .any(value_contains_image_generation_result)
}

fn response_contains_tool_call_request(body: &str) -> bool {
    if let Ok(root) = serde_json::from_str::<serde_json::Value>(body.trim()) {
        return value_contains_tool_call_request(&root);
    }

    iter_codex_sse_json_values(body)
        .iter()
        .any(value_contains_tool_call_request)
}

fn value_contains_tool_call_request(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Array(items) => items.iter().any(value_contains_tool_call_request),
        serde_json::Value::Object(map) => {
            if map
                .get("type")
                .and_then(|value| value.as_str())
                .map(|item_type| item_type == "function_call")
                .unwrap_or(false)
            {
                return true;
            }
            if map
                .get("tool_calls")
                .and_then(|value| value.as_array())
                .map(|tool_calls| !tool_calls.is_empty())
                .unwrap_or(false)
            {
                return true;
            }
            map.values().any(value_contains_tool_call_request)
        }
        _ => false,
    }
}

fn value_contains_image_generation_result(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Array(items) => items.iter().any(value_contains_image_generation_result),
        serde_json::Value::Object(map) => {
            if map
                .get("type")
                .and_then(|value| value.as_str())
                .map(|item_type| {
                    matches!(
                        item_type,
                        "image_generation_call" | "output_image" | "image_url" | "image"
                    )
                })
                .unwrap_or(false)
            {
                return true;
            }
            if map.contains_key("b64_json") || map.contains_key("image_url") {
                return true;
            }
            map.values().any(value_contains_image_generation_result)
        }
        _ => false,
    }
}

fn custom_upstream_response_is_empty_text(body: &str, protocol_type: &str) -> bool {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return true;
    }

    let Ok(root) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return false;
    };

    match &root {
        serde_json::Value::Null => return true,
        serde_json::Value::String(text) => return text.trim().is_empty(),
        serde_json::Value::Array(items) => return items.is_empty(),
        _ => {}
    }

    if protocol_type == "anthropic" {
        return anthropic_response_has_empty_text(&root);
    }

    if protocol_type == "openai" || protocol_type == "other" {
        return openai_chat_response_has_empty_text(&root);
    }

    responses_response_has_empty_text(&root)
}

fn openai_chat_response_has_empty_text(root: &serde_json::Value) -> bool {
    let Some(choice) = root
        .get("choices")
        .and_then(|value| value.as_array())
        .and_then(|choices| choices.first())
    else {
        return false;
    };
    let message = choice.get("message").or_else(|| choice.get("delta"));
    let has_tool_calls = message
        .and_then(|message| message.get("tool_calls"))
        .and_then(|tool_calls| tool_calls.as_array())
        .map(|tool_calls| !tool_calls.is_empty())
        .unwrap_or(false);
    if has_tool_calls {
        return false;
    }
    let text = message
        .and_then(|message| {
            message
                .get("content")
                .or_else(|| message.get("reasoning_content"))
        })
        .map(content_to_text)
        .unwrap_or_default();
    text.trim().is_empty()
}

fn anthropic_response_has_empty_text(root: &serde_json::Value) -> bool {
    root.get("content")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .map(content_to_text)
                .all(|text| text.trim().is_empty())
        })
        .unwrap_or(false)
}

fn responses_response_has_empty_text(root: &serde_json::Value) -> bool {
    if let Some(output_text) = root.get("output_text").and_then(|value| value.as_str()) {
        return output_text.trim().is_empty();
    }

    root.get("output")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .map(extract_responses_output_item_text)
                .all(|text| text.trim().is_empty())
        })
        .unwrap_or(false)
}

fn wrap_chat_response_as_responses(
    body: &str,
    route: &ProviderRoute,
    protocol_type: &str,
    original_payload: &serde_json::Value,
) -> String {
    let available_tool_names = collect_available_tool_names(original_payload);
    let custom_tool_names = collect_custom_tool_names(original_payload);
    let namespace_tool_mappings = collect_namespace_tool_mappings(original_payload);
    if protocol_type != "anthropic" {
        if let Some(sse_body) = openai_chat_sse_body_to_codex_sse(
            body,
            route,
            &available_tool_names,
            &custom_tool_names,
            &namespace_tool_mappings,
        ) {
            return sse_body;
        }
    }

    let root = match serde_json::from_str::<serde_json::Value>(body) {
        Ok(root) => root,
        Err(_) => return body.to_string(),
    };
    if protocol_type != "anthropic" {
        if let Some(tool_sse) = openai_chat_tool_calls_to_codex_sse(
            &root,
            root.get("usage"),
            &available_tool_names,
            &custom_tool_names,
            &namespace_tool_mappings,
        ) {
            return tool_sse;
        }
    }
    let output_text = if protocol_type == "anthropic" {
        extract_anthropic_text(&root)
    } else {
        extract_openai_chat_text(&root)
    };
    let mut response = serde_json::Map::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();

    response.insert(
        "id".to_string(),
        serde_json::Value::String(format!("resp_{}", current_log_millis())),
    );
    response.insert(
        "object".to_string(),
        serde_json::Value::String("response".to_string()),
    );
    response.insert(
        "created_at".to_string(),
        serde_json::Value::Number(serde_json::Number::from(now)),
    );
    response.insert(
        "status".to_string(),
        serde_json::Value::String("completed".to_string()),
    );
    response.insert(
        "model".to_string(),
        serde_json::Value::String(route.real_model.clone()),
    );
    response.insert(
        "output_text".to_string(),
        serde_json::Value::String(output_text.clone()),
    );
    response.insert(
        "output".to_string(),
        serde_json::Value::Array(vec![build_response_output_message(&output_text)]),
    );
    if let Some(usage) = root.get("usage").and_then(normalize_upstream_usage_value) {
        response.insert("usage".to_string(), usage);
    }

    serde_json::Value::Object(response).to_string()
}

fn openai_chat_sse_body_to_codex_sse(
    body: &str,
    route: &ProviderRoute,
    available_tool_names: &[String],
    custom_tool_names: &[String],
    namespace_tool_mappings: &[NamespaceToolMapping],
) -> Option<String> {
    if !body
        .lines()
        .any(|line| line.trim_start().starts_with("data:"))
    {
        return None;
    }

    let mut text = String::new();
    let mut tool_calls: Vec<OpenAiStreamToolCall> = Vec::new();
    let mut usage = None;
    let mut saw_openai_chunk = false;

    for line in body.lines() {
        let Some(data) = line.trim_start().strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let Ok(root) = serde_json::from_str::<serde_json::Value>(data) else {
            continue;
        };
        if root.get("choices").is_some() {
            saw_openai_chunk = true;
        }
        merge_openai_stream_tool_calls(&root, &mut tool_calls);
        if let Some(delta) = extract_openai_chat_stream_text_delta(&root) {
            text.push_str(&delta);
        }
        if usage.is_none() {
            usage = root.get("usage").and_then(normalize_upstream_usage_value);
        }
    }

    if !saw_openai_chunk {
        return None;
    }

    let tool_items = accumulated_openai_tool_calls_to_responses_items(
        &tool_calls,
        available_tool_names,
        custom_tool_names,
        namespace_tool_mappings,
    );
    if text.trim().is_empty() && !tool_items.is_empty() {
        return accumulated_openai_tool_calls_to_codex_sse(
            &tool_calls,
            available_tool_names,
            custom_tool_names,
            namespace_tool_mappings,
        );
    }

    let raw_response = serde_json::json!({
        "model": route.real_model,
        "usage": usage
    });
    Some(codex_completed_sse_from_text(&text, Some(raw_response)))
}

fn build_response_output_message(text: &str) -> serde_json::Value {
    let mut content = serde_json::Map::new();
    content.insert(
        "type".to_string(),
        serde_json::Value::String("output_text".to_string()),
    );
    content.insert(
        "text".to_string(),
        serde_json::Value::String(text.to_string()),
    );
    content.insert(
        "annotations".to_string(),
        serde_json::Value::Array(Vec::new()),
    );

    let mut message = serde_json::Map::new();
    message.insert(
        "id".to_string(),
        serde_json::Value::String(format!("msg_{}", current_log_millis())),
    );
    message.insert(
        "type".to_string(),
        serde_json::Value::String("message".to_string()),
    );
    message.insert(
        "status".to_string(),
        serde_json::Value::String("completed".to_string()),
    );
    message.insert(
        "role".to_string(),
        serde_json::Value::String("assistant".to_string()),
    );
    message.insert(
        "content".to_string(),
        serde_json::Value::Array(vec![serde_json::Value::Object(content)]),
    );
    serde_json::Value::Object(message)
}

fn extract_openai_chat_text(root: &serde_json::Value) -> String {
    root.get("choices")
        .and_then(|value| value.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message").or_else(|| choice.get("delta")))
        .and_then(|message| {
            message
                .get("content")
                .or_else(|| message.get("reasoning_content"))
        })
        .map(content_to_text)
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| root.to_string())
}

fn openai_chat_tool_calls_to_codex_sse(
    root: &serde_json::Value,
    upstream_usage: Option<&serde_json::Value>,
    available_tool_names: &[String],
    custom_tool_names: &[String],
    namespace_tool_mappings: &[NamespaceToolMapping],
) -> Option<String> {
    let tool_calls = root
        .get("choices")
        .and_then(|value| value.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message").or_else(|| choice.get("delta")))
        .and_then(|message| message.get("tool_calls"))
        .and_then(|tool_calls| tool_calls.as_array())?;
    if tool_calls.is_empty() {
        return None;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let response_id = format!("resp_{}", current_log_millis());
    let output_items = tool_calls
        .iter()
        .filter_map(|tool_call| {
            openai_chat_tool_call_to_responses_item(
                tool_call,
                available_tool_names,
                custom_tool_names,
                namespace_tool_mappings,
            )
        })
        .collect::<Vec<_>>();
    let mut seen_tool_calls = HashSet::new();
    let output_items = output_items
        .into_iter()
        .filter(|item| seen_tool_calls.insert(codex_tool_call_fingerprint(item)))
        .collect::<Vec<_>>();
    if output_items.is_empty() {
        return None;
    }

    let mut response_obj = serde_json::Map::new();
    response_obj.insert(
        "id".to_string(),
        serde_json::Value::String(response_id.clone()),
    );
    response_obj.insert(
        "object".to_string(),
        serde_json::Value::String("response".to_string()),
    );
    response_obj.insert(
        "created_at".to_string(),
        serde_json::Value::Number(serde_json::Number::from(now)),
    );
    response_obj.insert(
        "status".to_string(),
        serde_json::Value::String("completed".to_string()),
    );
    response_obj.insert(
        "output".to_string(),
        serde_json::Value::Array(output_items.clone()),
    );
    if let Some(usage) = upstream_usage.and_then(normalize_upstream_usage_value) {
        response_obj.insert("usage".to_string(), usage);
    }
    let response = serde_json::Value::Object(response_obj);

    let mut body = String::new();
    body.push_str(&sse_event(
        "response.created",
        serde_json::json!({
            "type": "response.created",
            "response": {
                "id": response["id"].clone(),
                "object": "response",
                "created_at": now,
                "status": "in_progress",
                "output": []
            }
        }),
    ));
    for (index, item) in output_items.iter().enumerate() {
        append_codex_output_item_sse_events(&mut body, index, item);
    }
    body.push_str(&sse_event(
        "response.completed",
        serde_json::json!({
            "type": "response.completed",
            "response": response
        }),
    ));
    Some(body)
}

fn codex_tool_call_fingerprint(item: &serde_json::Value) -> String {
    serde_json::json!({
        "type": item.get("type").cloned().unwrap_or_default(),
        "namespace": item.get("namespace").cloned().unwrap_or_default(),
        "name": item.get("name").cloned().unwrap_or_default(),
        "arguments": item.get("arguments").cloned().unwrap_or_default(),
        "input": item.get("input").cloned().unwrap_or_default()
    })
    .to_string()
}

fn append_codex_output_item_sse_events(
    body: &mut String,
    output_index: usize,
    completed_item: &serde_json::Value,
) {
    for (event_name, event_data) in codex_output_item_sse_events(output_index, completed_item) {
        body.push_str(&sse_event(event_name, event_data));
    }
}

fn codex_output_item_sse_events(
    output_index: usize,
    completed_item: &serde_json::Value,
) -> Vec<(&'static str, serde_json::Value)> {
    let item_type = completed_item
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let item_id = completed_item
        .get("id")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let mut added_item = completed_item.clone();
    if let Some(object) = added_item.as_object_mut() {
        object.insert(
            "status".to_string(),
            serde_json::Value::String("in_progress".to_string()),
        );
        match item_type {
            "custom_tool_call" => {
                object.insert(
                    "input".to_string(),
                    serde_json::Value::String(String::new()),
                );
            }
            "function_call" => {
                object.insert(
                    "arguments".to_string(),
                    serde_json::Value::String(String::new()),
                );
            }
            _ => {}
        }
    }

    let mut events = vec![(
        "response.output_item.added",
        serde_json::json!({
            "type": "response.output_item.added",
            "output_index": output_index,
            "item": added_item
        }),
    )];

    match item_type {
        "custom_tool_call" => {
            let input = completed_item
                .get("input")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if !input.is_empty() {
                events.push((
                    "response.custom_tool_call_input.delta",
                    serde_json::json!({
                        "type": "response.custom_tool_call_input.delta",
                        "item_id": item_id,
                        "output_index": output_index,
                        "delta": input
                    }),
                ));
            }
            events.push((
                "response.custom_tool_call_input.done",
                serde_json::json!({
                    "type": "response.custom_tool_call_input.done",
                    "item_id": item_id,
                    "output_index": output_index,
                    "input": input
                }),
            ));
        }
        "function_call" => {
            let arguments = completed_item
                .get("arguments")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if !arguments.is_empty() {
                events.push((
                    "response.function_call_arguments.delta",
                    serde_json::json!({
                        "type": "response.function_call_arguments.delta",
                        "item_id": item_id,
                        "output_index": output_index,
                        "delta": arguments
                    }),
                ));
            }
            events.push((
                "response.function_call_arguments.done",
                serde_json::json!({
                    "type": "response.function_call_arguments.done",
                    "item_id": item_id,
                    "output_index": output_index,
                    "arguments": arguments
                }),
            ));
        }
        _ => {}
    }

    events.push((
        "response.output_item.done",
        serde_json::json!({
            "type": "response.output_item.done",
            "output_index": output_index,
            "item": completed_item
        }),
    ));
    events
}

fn openai_chat_tool_call_to_responses_item(
    tool_call: &serde_json::Value,
    available_tool_names: &[String],
    custom_tool_names: &[String],
    namespace_tool_mappings: &[NamespaceToolMapping],
) -> Option<serde_json::Value> {
    let function = tool_call.get("function")?;
    let raw_name = function.get("name")?.as_str()?.trim();
    let namespace_mapping = namespace_tool_mappings
        .iter()
        .find(|mapping| mapping.flattened_name == raw_name);
    let name = namespace_mapping
        .map(|mapping| mapping.name.clone())
        .unwrap_or_else(|| normalize_upstream_tool_name(raw_name, available_tool_names));
    if name.is_empty() {
        return None;
    }
    let call_id = tool_call
        .get("id")
        .and_then(|id| id.as_str())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .unwrap_or("call_router");
    let arguments = function
        .get("arguments")
        .and_then(|arguments| arguments.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            function
                .get("arguments")
                .map(|arguments| arguments.to_string())
                .unwrap_or_else(|| "{}".to_string())
        });

    if custom_tool_names.iter().any(|candidate| candidate == &name) {
        let input = extract_custom_tool_input_from_chat_arguments(&arguments);
        Some(serde_json::json!({
            "id": format!("ctc_{}", call_id),
            "type": "custom_tool_call",
            "status": "completed",
            "call_id": call_id,
            "name": name,
            "input": input
        }))
    } else {
        let mut item = serde_json::json!({
            "id": format!("fc_{}", current_log_millis()),
            "type": "function_call",
            "status": "completed",
            "call_id": call_id,
            "name": name,
            "arguments": arguments
        });
        if let Some(mapping) = namespace_mapping {
            item["namespace"] = serde_json::Value::String(mapping.namespace.clone());
        }
        Some(item)
    }
}

fn collect_available_tool_names(payload: &serde_json::Value) -> Vec<String> {
    payload
        .get("tools")
        .and_then(|tools| tools.as_array())
        .map(|tools| {
            tools
                .iter()
                .filter_map(extract_tool_definition_name)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn collect_custom_tool_names(payload: &serde_json::Value) -> Vec<String> {
    payload
        .get("tools")
        .and_then(|tools| tools.as_array())
        .map(|tools| {
            tools
                .iter()
                .filter(|tool| tool.get("type").and_then(|value| value.as_str()) == Some("custom"))
                .filter_map(extract_tool_definition_name)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn request_has_codex_custom_tools(payload: &serde_json::Value) -> bool {
    payload
        .get("tools")
        .and_then(|tools| tools.as_array())
        .map(|tools| {
            tools.iter().any(|tool| match tool {
                serde_json::Value::String(name) => !name.trim().is_empty(),
                serde_json::Value::Object(_) => {
                    tool.get("type").and_then(|value| value.as_str()) == Some("custom")
                }
                _ => false,
            })
        })
        .unwrap_or(false)
}

fn extract_custom_tool_input_from_chat_arguments(arguments: &str) -> String {
    serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .and_then(|value| value.get("input").cloned())
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| arguments.to_string())
}

fn extract_tool_definition_name(tool: &serde_json::Value) -> Option<&str> {
    tool.get("name")
        .and_then(|value| value.as_str())
        .or_else(|| {
            tool.get("function")
                .and_then(|function| function.get("name"))
                .and_then(|value| value.as_str())
        })
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

fn normalize_upstream_tool_name(raw_name: &str, available_tool_names: &[String]) -> String {
    let name = raw_name.trim();
    if name.is_empty() {
        return String::new();
    }

    if available_tool_names
        .iter()
        .any(|candidate| candidate == name)
    {
        return name.to_string();
    }

    if let Some(candidate) = available_tool_names
        .iter()
        .find(|candidate| candidate.trim().replace('.', "__") == name)
    {
        return candidate.trim().to_string();
    }

    for candidate in available_tool_names {
        let candidate = candidate.trim();
        if candidate.is_empty() || candidate == name || name.len() % candidate.len() != 0 {
            continue;
        }
        if name
            .as_bytes()
            .chunks(candidate.len())
            .all(|chunk| chunk == candidate.as_bytes())
        {
            return candidate.to_string();
        }
    }

    if name.len() % 2 == 0 {
        let midpoint = name.len() / 2;
        let (left, right) = name.as_bytes().split_at(midpoint);
        if left == right {
            if let Ok(left) = std::str::from_utf8(left) {
                if !left.trim().is_empty() {
                    return left.to_string();
                }
            }
        }
    }

    name.to_string()
}

fn extract_anthropic_text(root: &serde_json::Value) -> String {
    root.get("content")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .map(content_to_text)
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| root.to_string())
}

fn extract_model_chat_test_text(body: &str, protocol_type: &str) -> String {
    let root = match serde_json::from_str::<serde_json::Value>(body) {
        Ok(root) => root,
        Err(_) => return body.trim().to_string(),
    };

    if protocol_type == "anthropic" {
        return extract_anthropic_text(&root);
    }

    if protocol_type == "openai" || protocol_type == "other" {
        return extract_openai_chat_text(&root);
    }

    extract_responses_text(&root)
}

fn extract_responses_text(root: &serde_json::Value) -> String {
    if let Some(output_text) = root
        .get("output_text")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return output_text.to_string();
    }

    root.get("output")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .map(extract_responses_output_item_text)
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| root.to_string())
}

fn extract_responses_output_item_text(item: &serde_json::Value) -> String {
    item.get("content")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .map(content_to_text)
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| content_to_text(item))
}

fn forward_official_codex_responses_request(mut payload: serde_json::Value) -> RouterResponse {
    let credentials = match load_codex_auth_credentials() {
        Ok(credentials) => credentials,
        Err(error) => {
            return json_response(
                HTTP_SERVICE_UNAVAILABLE,
                "{\"error\":\"codex_auth_missing\"}".to_string(),
                OFFICIAL_TARGET_PROVIDER,
                error,
            )
        }
    };
    let forward_settings = load_official_codex_forward_settings();

    normalize_official_codex_payload(&mut payload);
    let upstream_body = match serde_json::to_string(&payload) {
        Ok(body) => body,
        Err(error) => {
            return json_response(
                HTTP_BAD_GATEWAY,
                "{\"error\":\"serialize_official_body_failed\"}".to_string(),
                OFFICIAL_TARGET_PROVIDER,
                error.to_string(),
            )
        }
    };
    let authorization = format!("Bearer {}", credentials.access_token);
    let upstream_result = send_official_codex_request_with_retries(
        &forward_settings,
        &authorization,
        &credentials.account_id,
        &upstream_body,
    );

    match upstream_result {
        Ok(response) => {
            let status_code = response.status();
            let body = response.into_string().unwrap_or_default();
            let body = ensure_official_sse_completed(body);
            let usage = extract_token_usage_from_body(&body);
            let usage_source = token_usage_source(usage.as_ref());
            RouterResponse {
                flush_headers_before_body: false,
                status_code,
                content_type: HEADER_EVENT_STREAM.to_string(),
                body,
                target_provider: OFFICIAL_TARGET_PROVIDER.to_string(),
                error_detail: EMPTY_LOG_VALUE.to_string(),
                usage,
                usage_source,
            }
        }
        Err(ureq::Error::Status(status_code, response)) => {
            let body = response.into_string().unwrap_or_default();
            let body = ensure_official_sse_completed(body);
            let usage = extract_token_usage_from_body(&body);
            let usage_source = token_usage_source(usage.as_ref());
            RouterResponse {
                flush_headers_before_body: false,
                status_code,
                content_type: HEADER_EVENT_STREAM.to_string(),
                body,
                target_provider: OFFICIAL_TARGET_PROVIDER.to_string(),
                error_detail: format!("official upstream status {}", status_code),
                usage,
                usage_source,
            }
        }
        Err(error) => {
            let error_message = format_official_upstream_error(&error, &forward_settings);
            json_response(
                HTTP_BAD_GATEWAY,
                format!(
                    "{{\"error\":\"official_upstream_request_failed\",\"message\":{}}}",
                    json_string(&error_message)
                ),
                OFFICIAL_TARGET_PROVIDER,
                error_message,
            )
        }
    }
}

fn build_official_codex_request(
    settings: &OfficialCodexForwardSettings,
    authorization: &str,
    account_id: &str,
) -> ureq::Request {
    build_upstream_post_request(
        OFFICIAL_CODEX_RESPONSES_URL,
        settings.proxy_url.as_deref(),
        OFFICIAL_FORWARD_TIMEOUT_SECONDS,
    )
    .set(HEADER_CONTENT_TYPE, HEADER_JSON_UTF8)
    .set(HEADER_ACCEPT, HEADER_EVENT_STREAM)
    .set(HEADER_AUTHORIZATION, authorization)
    .set(HEADER_CHATGPT_ACCOUNT_ID, account_id)
    .set(HEADER_OPENAI_BETA, OFFICIAL_CODEX_BETA_HEADER_VALUE)
    .set(HEADER_ORIGINATOR, OFFICIAL_CODEX_ORIGINATOR)
}

fn send_official_codex_request_with_retries(
    settings: &OfficialCodexForwardSettings,
    authorization: &str,
    account_id: &str,
    body: &str,
) -> Result<ureq::Response, ureq::Error> {
    let mut last_error = None;
    for attempt in 0..UPSTREAM_NETWORK_RETRY_ATTEMPTS {
        match build_official_codex_request(settings, authorization, account_id).send_string(body) {
            Ok(response) => return Ok(response),
            Err(error)
                if attempt + 1 < UPSTREAM_NETWORK_RETRY_ATTEMPTS
                    && upstream_error_is_retryable(&error) =>
            {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(UPSTREAM_NETWORK_RETRY_DELAY_MS));
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.expect("official retry loop should store the last upstream error"))
}

fn load_official_codex_forward_settings() -> OfficialCodexForwardSettings {
    let settings_proxy_url = load_app_settings()
        .ok()
        .map(|settings| settings.official_proxy_url)
        .unwrap_or_default();
    let proxy_url = first_non_empty_value(&[
        settings_proxy_url,
        env::var(HTTPS_PROXY_ENV).unwrap_or_default(),
        env::var(HTTP_PROXY_ENV).unwrap_or_default(),
        env::var(ALL_PROXY_ENV).unwrap_or_default(),
    ]);

    OfficialCodexForwardSettings { proxy_url }
}



fn format_official_upstream_error(
    error: &ureq::Error,
    settings: &OfficialCodexForwardSettings,
) -> String {
    let proxy_message = match settings.proxy_url.as_deref() {
        Some(proxy_url) => format!("已使用代理：{}。", proxy_url),
        None => "未配置代理。".to_string(),
    };

    format!(
        "{} {} 原始错误：{}",
        OFFICIAL_CONNECT_TIMEOUT_HINT, proxy_message, error
    )
}

fn load_codex_auth_credentials() -> Result<CodexAuthCredentials, String> {
    let path = codex_auth_path()?;
    let text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "璇诲彇 Codex OAuth 缂撳瓨澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })?;
    let root = serde_json::from_str::<serde_json::Value>(&text).map_err(|error| {
        format!(
            "瑙ｆ瀽 Codex OAuth 缂撳瓨澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })?;
    let access_token = find_string_by_keys(&root, CODEX_ACCESS_TOKEN_KEYS).ok_or_else(|| {
        format!(
            "Codex OAuth 缓存缺少 access_token，路径：{}",
            path.display()
        )
    })?;
    let account_id = find_codex_account_id(&root).ok_or_else(|| {
        format!(
            "Codex OAuth 缓存缺少 chatgpt account id，路径：{}",
            path.display()
        )
    })?;

    Ok(CodexAuthCredentials {
        access_token,
        account_id,
    })
}

fn normalize_official_codex_payload(payload: &mut serde_json::Value) {
    if !payload.is_object() {
        *payload = serde_json::Value::Object(serde_json::Map::new());
    }

    if let Some(payload_object) = payload.as_object_mut() {
        let instructions = payload_object
            .get(OFFICIAL_INSTRUCTIONS_KEY)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_CODEX_INSTRUCTIONS)
            .to_string();
        let input = payload_object
            .remove(OFFICIAL_INPUT_KEY)
            .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
        let normalized_input = match input {
            serde_json::Value::Array(_) => input,
            serde_json::Value::Null => serde_json::Value::Array(Vec::new()),
            value => serde_json::Value::Array(vec![value]),
        };

        payload_object.insert(
            OFFICIAL_INSTRUCTIONS_KEY.to_string(),
            serde_json::Value::String(instructions),
        );
        payload_object.insert(
            OFFICIAL_STORE_KEY.to_string(),
            serde_json::Value::Bool(false),
        );
        payload_object.insert(
            OFFICIAL_STREAM_KEY.to_string(),
            serde_json::Value::Bool(true),
        );
        payload_object.insert(OFFICIAL_INPUT_KEY.to_string(), normalized_input);
        payload_object.remove(OFFICIAL_TEMPERATURE_KEY);
        payload_object.remove(OFFICIAL_MAX_OUTPUT_TOKENS_KEY);
    }
    sanitize_custom_responses_payload(payload);
}

fn ensure_official_sse_completed(mut body: String) -> String {
    if !body.contains(OFFICIAL_CODEX_COMPLETED_EVENT) {
        if !body.ends_with(SSE_LINE_ENDING) {
            body.push_str(SSE_LINE_ENDING);
        }

        if !body.ends_with("\n\n") {
            body.push_str(SSE_LINE_ENDING);
        }

        body.push_str(OFFICIAL_CODEX_COMPLETED_EVENT);
        body.push_str(SSE_LINE_ENDING);
        body.push_str(OFFICIAL_CODEX_COMPLETED_DATA);
        body.push_str("\n\n");
    }

    ensure_sse_done_frame(body)
}

fn remove_sse_done_frames(body: &str) -> String {
    body.lines()
        .filter(|line| line.trim() != "data: [DONE]")
        .collect::<Vec<_>>()
        .join("\n")
}

fn ensure_sse_done_frame(mut body: String) -> String {
    body = remove_sse_done_frames(&body);

    if !body.ends_with(SSE_LINE_ENDING) {
        body.push_str(SSE_LINE_ENDING);
    }

    if !body.ends_with("\n\n") {
        body.push_str(SSE_LINE_ENDING);
    }

    body.push_str("data: [DONE]\n\n");
    body
}

#[cfg(test)]
fn normalize_repeated_tool_names_in_body(body: &str) -> String {
    normalize_repeated_tool_names_in_body_with_available(body, &[])
}

fn normalize_repeated_tool_names_in_body_with_available(
    body: &str,
    available_tool_names: &[String],
) -> String {
    if body.trim().is_empty() || (!body.contains("name") && !body.contains("custom_tool_call")) {
        return body.to_string();
    }

    match serde_json::from_str::<serde_json::Value>(body) {
        Ok(mut value) => {
            normalize_responses_tool_calls_in_json(&mut value, available_tool_names);
            serde_json::to_string(&value).unwrap_or_else(|_| body.to_string())
        }
        Err(_)
            if body.lines().any(|line| {
                line.trim_start()
                    .strip_prefix("data:")
                    .map(|data| !data.trim().is_empty())
                    .unwrap_or(false)
            }) =>
        {
            body.split_inclusive('\n')
                .map(|line| {
                    normalize_repeated_tool_names_in_sse_line_with_available(
                        line,
                        available_tool_names,
                    )
                })
                .collect()
        }
        Err(_) => body.to_string(),
    }
}

#[cfg(test)]
fn normalize_repeated_tool_names_in_sse_line(line: &str) -> String {
    normalize_repeated_tool_names_in_sse_line_with_available(line, &[])
}

fn normalize_repeated_tool_names_in_sse_line_with_available(
    line: &str,
    available_tool_names: &[String],
) -> String {
    let leading_len = line.len() - line.trim_start().len();
    let leading = &line[..leading_len];
    let trimmed = &line[leading_len..];
    let Some(data) = trimmed.strip_prefix("data:") else {
        return line.to_string();
    };
    let line_ending = if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    };
    let data = data.trim();
    if data == "[DONE]" || (!data.contains("name") && !data.contains("custom_tool_call")) {
        return line.to_string();
    }

    match serde_json::from_str::<serde_json::Value>(data) {
        Ok(mut value) => {
            normalize_responses_tool_calls_in_json(&mut value, available_tool_names);
            match serde_json::to_string(&value) {
                Ok(normalized) => format!("{}data: {}{}", leading, normalized, line_ending),
                Err(_) => line.to_string(),
            }
        }
        Err(_) => line.to_string(),
    }
}

fn normalize_responses_tool_calls_in_json(
    value: &mut serde_json::Value,
    available_tool_names: &[String],
) {
    match value {
        serde_json::Value::Object(object) => {
            let is_tool_call = matches!(
                object.get("type").and_then(|item_type| item_type.as_str()),
                Some("custom_tool_call" | "function_call")
            );
            if is_tool_call {
                object.remove("namespace");
            }
            if let Some(name_value) = object.get_mut("name") {
                if let Some(name) = name_value.as_str() {
                    let normalized = normalize_upstream_tool_name(name, available_tool_names);
                    if normalized != name {
                        *name_value = serde_json::Value::String(normalized);
                    }
                }
            }
            for child in object.values_mut() {
                normalize_responses_tool_calls_in_json(child, available_tool_names);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                normalize_responses_tool_calls_in_json(item, available_tool_names);
            }
        }
        _ => {}
    }
}

fn ensure_responses_stream_completed(body: String, content_type: &str) -> String {
    if content_type
        .to_ascii_lowercase()
        .contains(HEADER_EVENT_STREAM)
        || body.contains("event:")
        || body.contains("data:")
    {
        return ensure_official_sse_completed(body);
    }

    response_json_to_completed_sse(&body)
}

fn response_json_to_completed_sse(body: &str) -> String {
    let root = serde_json::from_str::<serde_json::Value>(body)
        .unwrap_or_else(|_| serde_json::Value::String(body.to_string()));
    let output_text = extract_responses_text(&root);
    codex_completed_sse_from_text(&output_text, Some(root))
}

fn codex_sse_error_response(
    status_code: u16,
    code: &str,
    message: &str,
    target_provider: String,
) -> RouterResponse {
    let normalized_message = normalize_codex_sse_error_message(code, message);
    let body = codex_sse_error_body_for_client(code, &normalized_message);

    RouterResponse {
        flush_headers_before_body: false,
        status_code,
        content_type: HEADER_EVENT_STREAM.to_string(),
        body,
        target_provider,
        error_detail: format!("{}: {}", code, normalized_message),
        usage: None,
        usage_source: TOKEN_USAGE_SOURCE_MISSING.to_string(),
    }
}

fn normalize_codex_sse_error_message(code: &str, message: &str) -> String {
    match code {
        "custom_image_generation_not_configured" | "custom_image_generation_not_supported" => {
            CUSTOM_IMAGE_GENERATION_UNSUPPORTED_MESSAGE.to_string()
        }
        "upstream_empty_response" => CUSTOM_UPSTREAM_EMPTY_RESPONSE_MESSAGE.to_string(),
        _ => message.to_string(),
    }
}

fn codex_sse_error_body_for_client(code: &str, message: &str) -> String {
    if codex_sse_error_should_render_text(code) {
        return codex_completed_sse_from_text(
            message,
            Some(serde_json::json!({
                "router_error": {
                    "code": code,
                    "message": message
                }
            })),
        );
    }

    codex_sse_error_body(code, message)
}

fn codex_sse_error_should_render_text(code: &str) -> bool {
    matches!(
        code,
        "custom_image_generation_not_configured"
            | "custom_image_generation_not_supported"
            | "upstream_rate_limited"
    )
}

fn codex_sse_error_body(code: &str, message: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let response_id = format!("resp_{}", current_log_millis());
    let error = serde_json::json!({
        "type": code,
        "code": code,
        "message": message
    });
    let response = serde_json::json!({
        "id": response_id,
        "object": "response",
        "created_at": now,
        "status": "failed",
        "output": [],
        "error": error
    });

    let mut body = String::new();
    body.push_str(&sse_event(
        "response.failed",
        serde_json::json!({
            "type": "response.failed",
            "response": response
        }),
    ));
    body.push_str("data: [DONE]\n\n");
    body
}

fn codex_completed_sse_from_text(
    output_text: &str,
    raw_response: Option<serde_json::Value>,
) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let response_id = format!("resp_{}", current_log_millis());
    let message_id = format!("msg_{}", current_log_millis());
    let part_id = format!("part_{}", current_log_millis());
    let normalized_text = if output_text.trim().is_empty() {
        raw_response
            .as_ref()
            .map(|value| value.to_string())
            .unwrap_or_default()
    } else {
        output_text.to_string()
    };

    let content_part = serde_json::json!({
        "id": part_id,
        "type": "output_text",
        "text": normalized_text.clone(),
        "annotations": []
    });
    let completed_message = serde_json::json!({
        "id": message_id,
        "type": "message",
        "status": "completed",
        "role": "assistant",
        "content": [content_part.clone()]
    });

    let mut response = serde_json::Map::new();
    response.insert(
        "id".to_string(),
        serde_json::Value::String(response_id.clone()),
    );
    response.insert(
        "object".to_string(),
        serde_json::Value::String("response".to_string()),
    );
    response.insert(
        "created_at".to_string(),
        serde_json::Value::Number(serde_json::Number::from(now)),
    );
    response.insert(
        "status".to_string(),
        serde_json::Value::String("completed".to_string()),
    );
    response.insert(
        "output_text".to_string(),
        serde_json::Value::String(normalized_text.clone()),
    );
    response.insert(
        "output".to_string(),
        serde_json::Value::Array(vec![completed_message.clone()]),
    );
    if let Some(raw_response) = &raw_response {
        response.insert(
            "metadata".to_string(),
            serde_json::json!({ "router_raw_response": raw_response }),
        );
        if let Some(usage) = raw_response
            .get("usage")
            .and_then(normalize_upstream_usage_value)
        {
            response.insert("usage".to_string(), usage);
        }
    }
    let response_value = serde_json::Value::Object(response.clone());

    let mut message_item = serde_json::Map::new();
    message_item.insert(
        "id".to_string(),
        serde_json::Value::String(message_id.clone()),
    );
    message_item.insert(
        "type".to_string(),
        serde_json::Value::String("message".to_string()),
    );
    message_item.insert(
        "status".to_string(),
        serde_json::Value::String("in_progress".to_string()),
    );
    message_item.insert(
        "role".to_string(),
        serde_json::Value::String("assistant".to_string()),
    );
    message_item.insert("content".to_string(), serde_json::Value::Array(Vec::new()));
    let message_value = serde_json::Value::Object(message_item.clone());

    let mut body = String::new();
    body.push_str(&sse_event(
        "response.created",
        serde_json::json!({ "type": "response.created", "response": response_value }),
    ));
    body.push_str(&sse_event("response.output_item.added", serde_json::json!({ "type": "response.output_item.added", "output_index": 0, "item": message_value })));
    body.push_str(&sse_event("response.content_part.added", serde_json::json!({ "type": "response.content_part.added", "item_id": message_id, "output_index": 0, "content_index": 0, "part": content_part })));
    body.push_str(&sse_event("response.output_text.delta", serde_json::json!({ "type": "response.output_text.delta", "item_id": message_id, "output_index": 0, "content_index": 0, "delta": normalized_text.clone() })));
    body.push_str(&sse_event("response.output_text.done", serde_json::json!({ "type": "response.output_text.done", "item_id": message_id, "output_index": 0, "content_index": 0, "text": normalized_text.clone() })));
    body.push_str(&sse_event("response.content_part.done", serde_json::json!({ "type": "response.content_part.done", "item_id": message_id, "output_index": 0, "content_index": 0 })));
    body.push_str(&sse_event("response.output_item.done", serde_json::json!({ "type": "response.output_item.done", "output_index": 0, "item": completed_message })));
    body.push_str(&sse_event("response.completed", serde_json::json!({ "type": "response.completed", "response": serde_json::Value::Object(response) })));
    body.push_str("data: [DONE]\n\n");
    body
}

fn sse_event(event_name: &str, data: serde_json::Value) -> String {
    format!("event: {}\ndata: {}\n\n", event_name, data)
}

struct CodexTextStreamState {
    response_id: String,
    message_id: String,
    part_id: String,
    created_at: u64,
    model: String,
    text: String,
}

#[derive(Clone, Default)]
struct OpenAiStreamToolCall {
    id: Option<String>,
    call_type: Option<String>,
    function_name: Option<String>,
    arguments: String,
}

fn new_codex_text_stream_state(model: &str) -> CodexTextStreamState {
    CodexTextStreamState {
        response_id: format!("resp_{}", current_log_millis()),
        message_id: format!("msg_{}", current_log_millis()),
        part_id: format!("part_{}", current_log_millis()),
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default(),
        model: model.to_string(),
        text: String::new(),
    }
}

fn write_codex_text_stream_start(
    stream: &mut TcpStream,
    state: &CodexTextStreamState,
) -> std::io::Result<()> {
    let response = serde_json::json!({
        "id": state.response_id,
        "object": "response",
        "created_at": state.created_at,
        "status": "in_progress",
        "model": state.model,
        "output": []
    });
    let message = serde_json::json!({
        "id": state.message_id,
        "type": "message",
        "status": "in_progress",
        "role": "assistant",
        "content": []
    });
    let part = serde_json::json!({
        "id": state.part_id,
        "type": "output_text",
        "text": "",
        "annotations": []
    });

    write_sse_event_to_stream(
        stream,
        "response.created",
        serde_json::json!({ "type": "response.created", "response": response }),
    )?;
    write_sse_event_to_stream(
        stream,
        "response.output_item.added",
        serde_json::json!({
            "type": "response.output_item.added",
            "output_index": 0,
            "item": message
        }),
    )?;
    write_sse_event_to_stream(
        stream,
        "response.content_part.added",
        serde_json::json!({
            "type": "response.content_part.added",
            "item_id": state.message_id,
            "output_index": 0,
            "content_index": 0,
            "part": part
        }),
    )
}

fn write_codex_text_stream_delta(
    stream: &mut TcpStream,
    state: &mut CodexTextStreamState,
    delta: &str,
) -> std::io::Result<()> {
    state.text.push_str(delta);
    write_sse_event_to_stream(
        stream,
        "response.output_text.delta",
        serde_json::json!({
            "type": "response.output_text.delta",
            "item_id": state.message_id,
            "output_index": 0,
            "content_index": 0,
            "delta": delta
        }),
    )
}

fn write_codex_stream_disconnect_error(
    stream: &mut TcpStream,
    reason: &str,
) -> std::io::Result<()> {
    stream.write_all(b"\n\n")?;
    write_codex_stream_error(
        stream,
        "upstream_stream_disconnected",
        &stream_disconnect_user_message(reason),
    )
}

fn write_codex_stream_error(
    stream: &mut TcpStream,
    code: &str,
    message: &str,
) -> std::io::Result<()> {
    stream.write_all(
        codex_sse_error_body_for_client(code, &normalize_codex_sse_error_message(code, message))
            .as_bytes(),
    )?;
    stream.flush()
}

fn write_codex_text_stream_failed(
    stream: &mut TcpStream,
    state: &CodexTextStreamState,
    code: &str,
    message: &str,
) -> std::io::Result<()> {
    let normalized_message = normalize_codex_sse_error_message(code, message);
    let response = serde_json::json!({
        "id": state.response_id,
        "object": "response",
        "created_at": state.created_at,
        "status": "failed",
        "model": state.model,
        "output_text": state.text,
        "output": [],
        "error": {
            "code": code,
            "message": normalized_message
        }
    });

    write_sse_event_to_stream(
        stream,
        "response.failed",
        serde_json::json!({
            "type": "response.failed",
            "response": response
        }),
    )?;
    stream.write_all(b"data: [DONE]\n\n")?;
    stream.flush()
}

fn stream_disconnect_user_message(reason: &str) -> String {
    format!(
        "上游流式连接中断，未收到完成事件。\n\n错误详情：{}\n\n建议检查本机网络、代理配置、Provider 服务状态，以及 API 地址和密钥是否有效。系统会尝试按 Codex 的重连机制继续请求。",
        reason.trim()
    )
}

fn write_codex_text_stream_done(
    stream: &mut TcpStream,
    state: &CodexTextStreamState,
) -> std::io::Result<()> {
    let content_part = serde_json::json!({
        "id": state.part_id,
        "type": "output_text",
        "text": state.text,
        "annotations": []
    });
    let completed_message = serde_json::json!({
        "id": state.message_id,
        "type": "message",
        "status": "completed",
        "role": "assistant",
        "content": [content_part]
    });
    let response = serde_json::json!({
        "id": state.response_id,
        "object": "response",
        "created_at": state.created_at,
        "status": "completed",
        "model": state.model,
        "output_text": state.text,
        "output": [completed_message.clone()]
    });

    write_sse_event_to_stream(
        stream,
        "response.output_text.done",
        serde_json::json!({
            "type": "response.output_text.done",
            "item_id": state.message_id,
            "output_index": 0,
            "content_index": 0,
            "text": state.text
        }),
    )?;
    write_sse_event_to_stream(
        stream,
        "response.content_part.done",
        serde_json::json!({
            "type": "response.content_part.done",
            "item_id": state.message_id,
            "output_index": 0,
            "content_index": 0
        }),
    )?;
    write_sse_event_to_stream(
        stream,
        "response.output_item.done",
        serde_json::json!({
            "type": "response.output_item.done",
            "output_index": 0,
            "item": completed_message
        }),
    )?;
    write_sse_event_to_stream(
        stream,
        "response.completed",
        serde_json::json!({ "type": "response.completed", "response": response }),
    )?;
    stream.write_all(b"data: [DONE]\n\n")?;
    stream.flush()
}

fn write_codex_text_stream_done_with_items(
    stream: &mut TcpStream,
    state: &CodexTextStreamState,
    extra_items: Vec<serde_json::Value>,
) -> std::io::Result<()> {
    let content_part = serde_json::json!({
        "id": state.part_id,
        "type": "output_text",
        "text": state.text,
        "annotations": []
    });
    let completed_message = serde_json::json!({
        "id": state.message_id,
        "type": "message",
        "status": "completed",
        "role": "assistant",
        "content": [content_part]
    });
    let mut output_items = vec![completed_message.clone()];
    output_items.extend(extra_items.clone());
    let response = serde_json::json!({
        "id": state.response_id,
        "object": "response",
        "created_at": state.created_at,
        "status": "completed",
        "model": state.model,
        "output_text": state.text,
        "output": output_items
    });

    write_sse_event_to_stream(
        stream,
        "response.output_text.done",
        serde_json::json!({
            "type": "response.output_text.done",
            "item_id": state.message_id,
            "output_index": 0,
            "content_index": 0,
            "text": state.text
        }),
    )?;
    write_sse_event_to_stream(
        stream,
        "response.content_part.done",
        serde_json::json!({
            "type": "response.content_part.done",
            "item_id": state.message_id,
            "output_index": 0,
            "content_index": 0
        }),
    )?;
    write_sse_event_to_stream(
        stream,
        "response.output_item.done",
        serde_json::json!({
            "type": "response.output_item.done",
            "output_index": 0,
            "item": completed_message
        }),
    )?;
    for (index, item) in extra_items.iter().enumerate() {
        let output_index = index + 1;
        for (event_name, event_data) in codex_output_item_sse_events(output_index, item) {
            write_sse_event_to_stream(stream, event_name, event_data)?;
        }
    }
    write_sse_event_to_stream(
        stream,
        "response.completed",
        serde_json::json!({ "type": "response.completed", "response": response }),
    )?;
    stream.write_all(b"data: [DONE]\n\n")?;
    stream.flush()
}

fn sanitize_inactive_wait_sse_line(
    line: &str,
    active_exec_cell_ids: &[String],
    suppressed_item_ids: &mut HashSet<String>,
) -> Option<String> {
    let leading_len = line.len() - line.trim_start().len();
    let leading = &line[..leading_len];
    let trimmed = &line[leading_len..];
    let Some(data) = trimmed.strip_prefix("data:") else {
        return Some(line.to_string());
    };
    let data = data.trim();
    if data.is_empty() || data == "[DONE]" {
        return Some(line.to_string());
    }
    let Ok(mut event) = serde_json::from_str::<serde_json::Value>(data) else {
        return Some(line.to_string());
    };
    let event_type = event
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or_default();

    if matches!(
        event_type,
        "response.output_item.added" | "response.output_item.done"
    ) {
        if let Some(item) = event.get("item") {
            let item_id = item
                .get("id")
                .or_else(|| item.get("call_id"))
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let is_suppressed = suppressed_item_ids.contains(item_id);
            if (is_wait_tool_call_item(item)
                && !wait_tool_call_is_valid(item, active_exec_cell_ids))
                || is_suppressed
            {
                if !item_id.is_empty() {
                    suppressed_item_ids.insert(item_id.to_string());
                }
                return None;
            }
        }
    }

    if matches!(
        event_type,
        "response.function_call_arguments.delta"
            | "response.function_call_arguments.done"
            | "response.custom_tool_call_input.delta"
            | "response.custom_tool_call_input.done"
    ) {
        let item_id = event
            .get("item_id")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if suppressed_item_ids.contains(item_id) {
            return None;
        }
    }

    if event_type == "response.completed" {
        if let Some(output) = event
            .get_mut("response")
            .and_then(|response| response.get_mut("output"))
            .and_then(|output| output.as_array_mut())
        {
            output.retain(|item| {
                !is_wait_tool_call_item(item) || wait_tool_call_is_valid(item, active_exec_cell_ids)
            });
        }
    }

    let line_ending = if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    };
    serde_json::to_string(&event)
        .ok()
        .map(|event| format!("{}data: {}{}", leading, event, line_ending))
}

fn wait_tool_call_cell_id(item: &serde_json::Value) -> Option<String> {
    let arguments = item.get("arguments").or_else(|| item.get("input"))?;
    let arguments = match arguments {
        serde_json::Value::String(arguments) => {
            serde_json::from_str::<serde_json::Value>(arguments).ok()?
        }
        serde_json::Value::Object(_) => arguments.clone(),
        _ => return None,
    };
    arguments
        .get("cell_id")
        .and_then(|cell_id| cell_id.as_str())
        .map(str::trim)
        .filter(|cell_id| !cell_id.is_empty())
        .map(str::to_string)
}

fn wait_tool_call_is_valid(item: &serde_json::Value, active_exec_cell_ids: &[String]) -> bool {
    wait_tool_call_cell_id(item)
        .is_some_and(|cell_id| active_exec_cell_ids.iter().any(|active| active == &cell_id))
}

fn sse_data_value(line: &str) -> Option<serde_json::Value> {
    let data = line.trim_start().strip_prefix("data:")?.trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    serde_json::from_str(data).ok()
}

fn is_wait_tool_call_item(item: &serde_json::Value) -> bool {
    matches!(
        item.get("type").and_then(|value| value.as_str()),
        Some("function_call" | "custom_tool_call")
    ) && (item.get("name").and_then(|value| value.as_str()) == Some("wait")
        || item.get("namespace").and_then(|value| value.as_str()) == Some("wait"))
}

fn write_sse_event_to_stream(
    stream: &mut TcpStream,
    event_name: &str,
    data: serde_json::Value,
) -> std::io::Result<()> {
    stream.write_all(sse_event(event_name, data).as_bytes())?;
    stream.flush()
}

fn sse_line_is_done(line: &str) -> bool {
    line.trim() == "data: [DONE]"
}

fn sse_text_has_response_completed(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed == "event: response.completed" {
        return true;
    }
    let Some(data) = trimmed.strip_prefix("data:") else {
        return false;
    };
    let data = data.trim();
    if data == "[DONE]" {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(data)
        .ok()
        .and_then(|value| json_string_field(&value, "type"))
        .as_deref()
        == Some("response.completed")
}

fn stream_raw_upstream_sse(
    response: ureq::Response,
    stream: &mut TcpStream,
    full_debug_id: Option<&str>,
    available_tool_names: &[String],
    active_exec_cell_ids: &[String],
) -> std::io::Result<()> {
    let mut reader = BufReader::new(response.into_reader());
    let mut saw_completed = false;
    let mut saw_done = false;
    let mut pending_event_line: Option<String> = None;
    let mut suppressed_wait_item_ids = HashSet::<String>::new();
    let mut pending_wait_item_id: Option<String> = None;
    let mut pending_wait_sse = String::new();
    loop {
        let mut line = String::new();
        let bytes_read = match reader.read_line(&mut line) {
            Ok(bytes_read) => bytes_read,
            Err(error) => {
                if saw_completed {
                    stream.write_all(b"\n\ndata: [DONE]\n\n")?;
                    stream.flush()?;
                } else {
                    write_codex_stream_disconnect_error(stream, &error.to_string())?;
                }
                return Err(error);
            }
        };
        if bytes_read == 0 {
            break;
        }
        if line.trim_start().starts_with("event:") {
            pending_event_line = Some(line);
            continue;
        }
        if sse_text_has_response_completed(&line) {
            saw_completed = true;
        }
        if sse_line_is_done(&line) {
            saw_done = true;
            append_router_full_debug_log(
                "upstream_sse_line",
                serde_json::json!({
                    "debug_id": full_debug_id,
                    "direction": "upstream",
                    "held_until_completion": true,
                    "line": router_full_debug_sse_line_value(&line)
                }),
            );
            continue;
        }
        let line =
            normalize_repeated_tool_names_in_sse_line_with_available(&line, available_tool_names);
        let event = sse_data_value(&line);
        let event_line = pending_event_line.take();

        if let Some(wait_item_id) = pending_wait_item_id.clone() {
            let current_event_line = event_line.clone();
            if let Some(event_line) = event_line {
                pending_wait_sse.push_str(&event_line);
            }
            pending_wait_sse.push_str(&line);

            let wait_resolution =
                event.as_ref().and_then(|event| {
                    let event_type = event.get("type").and_then(|value| value.as_str())?;
                    if event_type == "response.output_item.done" {
                        let item = event.get("item")?;
                        let item_id = item
                            .get("id")
                            .or_else(|| item.get("call_id"))
                            .and_then(|value| value.as_str())?;
                        (item_id == wait_item_id)
                            .then(|| wait_tool_call_is_valid(item, active_exec_cell_ids))
                    } else if event_type == "response.completed" {
                        let item = event
                            .get("response")
                            .and_then(|response| response.get("output"))
                            .and_then(|output| output.as_array())
                            .and_then(|output| {
                                output.iter().find(|item| {
                                    item.get("id")
                                        .or_else(|| item.get("call_id"))
                                        .and_then(|value| value.as_str())
                                        == Some(wait_item_id.as_str())
                                })
                            });
                        Some(item.is_some_and(|item| {
                            wait_tool_call_is_valid(item, active_exec_cell_ids)
                        }))
                    } else {
                        None
                    }
                });

            if let Some(is_valid) = wait_resolution {
                if is_valid {
                    stream.write_all(pending_wait_sse.as_bytes())?;
                    stream.flush()?;
                } else {
                    suppressed_wait_item_ids.insert(wait_item_id);
                    append_router_debug_log(
                        "inactive_wait_tool_call_suppressed",
                        serde_json::json!({
                            "active_exec_cell_ids": active_exec_cell_ids,
                            "suppressed_item_ids": suppressed_wait_item_ids
                        }),
                    );
                    if event.as_ref().and_then(|event| event.get("type"))
                        == Some(&serde_json::Value::String("response.completed".to_string()))
                    {
                        if let Some(completed) = sanitize_inactive_wait_sse_line(
                            &line,
                            active_exec_cell_ids,
                            &mut suppressed_wait_item_ids,
                        ) {
                            if let Some(event_line) = current_event_line {
                                stream.write_all(event_line.as_bytes())?;
                            }
                            stream.write_all(completed.as_bytes())?;
                            stream.flush()?;
                        }
                    }
                }
                pending_wait_item_id = None;
                pending_wait_sse.clear();
            }
            continue;
        }

        let added_wait_item_id = event.as_ref().and_then(|event| {
            (event.get("type").and_then(|value| value.as_str())
                == Some("response.output_item.added"))
            .then_some(())?;
            let item = event.get("item")?;
            is_wait_tool_call_item(item).then_some(())?;
            item.get("id")
                .or_else(|| item.get("call_id"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });
        if let Some(item_id) = added_wait_item_id {
            if let Some(event_line) = event_line {
                pending_wait_sse.push_str(&event_line);
            }
            pending_wait_sse.push_str(&line);
            pending_wait_item_id = Some(item_id);
            continue;
        }

        let Some(line) = sanitize_inactive_wait_sse_line(
            &line,
            active_exec_cell_ids,
            &mut suppressed_wait_item_ids,
        ) else {
            append_router_debug_log(
                "inactive_wait_tool_call_suppressed",
                serde_json::json!({
                    "active_exec_cell_ids": active_exec_cell_ids,
                    "suppressed_item_ids": suppressed_wait_item_ids
                }),
            );
            continue;
        };
        append_router_full_debug_log(
            "upstream_sse_line",
            serde_json::json!({
                "debug_id": full_debug_id,
                "direction": "upstream",
                "line": router_full_debug_sse_line_value(&line)
            }),
        );
        if let Some(event_line) = event_line {
            stream.write_all(event_line.as_bytes())?;
        }
        stream.write_all(line.as_bytes())?;
        stream.flush()?;
    }
    if !saw_completed {
        let reason = "upstream stream closed before response.completed";
        append_router_full_debug_log(
            "router_sse_incomplete",
            serde_json::json!({
                "debug_id": full_debug_id,
                "saw_completed": saw_completed,
                "saw_done": saw_done,
                "reason": reason
            }),
        );
        write_codex_stream_disconnect_error(stream, reason)?;
        return Err(std::io::Error::new(ErrorKind::UnexpectedEof, reason));
    } else {
        append_router_full_debug_log(
            "router_sse_done_fallback",
            serde_json::json!({
                "debug_id": full_debug_id,
                "saw_completed": saw_completed,
                "saw_done": saw_done,
                "line": "data: [DONE]"
            }),
        );
        stream.write_all(b"\n\ndata: [DONE]\n\n")?;
    }
    stream.flush()?;
    Ok(())
}

fn stream_official_codex_sse(
    response: ureq::Response,
    stream: &mut TcpStream,
    full_debug_id: Option<&str>,
) -> std::io::Result<Option<TokenUsage>> {
    let mut reader = BufReader::new(response.into_reader());
    let mut saw_completed = false;
    let mut saw_done = false;
    let mut usage_buffer = String::new();
    let mut usage = None;

    loop {
        let mut line = String::new();
        let bytes_read = match reader.read_line(&mut line) {
            Ok(bytes_read) => bytes_read,
            Err(error) => {
                if saw_completed {
                    stream.write_all(b"\n\ndata: [DONE]\n\n")?;
                    stream.flush()?;
                } else {
                    write_codex_stream_disconnect_error(stream, &error.to_string())?;
                }
                return Err(error);
            }
        };
        if bytes_read == 0 {
            break;
        }
        usage_buffer.push_str(&line);
        if sse_text_has_response_completed(&line) {
            saw_completed = true;
        }
        if sse_line_is_done(&line) {
            saw_done = true;
            append_router_full_debug_log(
                "official_sse_line",
                serde_json::json!({
                    "debug_id": full_debug_id,
                    "direction": "upstream",
                    "held_until_completion": true,
                    "line": router_full_debug_sse_line_value(&line)
                }),
            );
            continue;
        }
        if usage.is_none() {
            usage = extract_token_usage_from_body(&usage_buffer);
        }
        if usage_buffer.len() > 128 * 1024 {
            usage_buffer = usage_buffer
                .chars()
                .rev()
                .take(64 * 1024)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
        }
        append_router_full_debug_log(
            "official_sse_line",
            serde_json::json!({
                "debug_id": full_debug_id,
                "direction": "upstream",
                "line": router_full_debug_sse_line_value(&line)
            }),
        );
        stream.write_all(line.as_bytes())?;
        stream.flush()?;
    }

    if !saw_completed {
        let reason = "official upstream stream closed before response.completed";
        append_router_full_debug_log(
            "official_sse_incomplete",
            serde_json::json!({
                "debug_id": full_debug_id,
                "saw_completed": saw_completed,
                "saw_done": saw_done,
                "reason": reason
            }),
        );
        write_codex_stream_disconnect_error(stream, reason)?;
        return Err(std::io::Error::new(ErrorKind::UnexpectedEof, reason));
    } else {
        append_router_full_debug_log(
            "official_sse_done_fallback",
            serde_json::json!({
                "debug_id": full_debug_id,
                "saw_completed": saw_completed,
                "saw_done": saw_done,
                "line": "data: [DONE]"
            }),
        );
        stream.write_all(b"\n\ndata: [DONE]\n\n")?;
    }
    stream.flush()?;

    Ok(usage.or_else(|| extract_token_usage_from_body(&usage_buffer)))
}

fn merge_openai_stream_tool_calls(
    root: &serde_json::Value,
    tool_calls: &mut Vec<OpenAiStreamToolCall>,
) -> bool {
    let Some(chunks) = root
        .get("choices")
        .and_then(|value| value.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("delta"))
        .and_then(|delta| delta.get("tool_calls"))
        .and_then(|value| value.as_array())
    else {
        return false;
    };

    let mut merged = false;
    for chunk in chunks {
        let index = chunk
            .get("index")
            .and_then(|value| value.as_u64())
            .unwrap_or(tool_calls.len() as u64) as usize;
        while tool_calls.len() <= index {
            tool_calls.push(OpenAiStreamToolCall::default());
        }
        let target = &mut tool_calls[index];

        if let Some(id) = chunk
            .get("id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            target.id = Some(id.to_string());
            merged = true;
        }
        if let Some(call_type) = chunk
            .get("type")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            target.call_type = Some(call_type.to_string());
            merged = true;
        }
        if let Some(function) = chunk.get("function") {
            if let Some(name) = function
                .get("name")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                target.function_name = Some(name.to_string());
                merged = true;
            }
            if let Some(arguments) = function.get("arguments").and_then(|value| value.as_str()) {
                target.arguments.push_str(arguments);
                merged = true;
            }
        }
    }

    merged
}

fn accumulated_openai_tool_calls_to_values(
    tool_calls: &[OpenAiStreamToolCall],
) -> Vec<serde_json::Value> {
    tool_calls
        .iter()
        .enumerate()
        .filter_map(|(index, tool_call)| {
            let name = tool_call.function_name.as_deref()?.trim();
            if name.is_empty() {
                return None;
            }
            Some(serde_json::json!({
                "id": tool_call
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("call_{}", index)),
                "type": tool_call
                    .call_type
                    .clone()
                    .unwrap_or_else(|| "function".to_string()),
                "function": {
                    "name": name,
                    "arguments": if tool_call.arguments.trim().is_empty() {
                        "{}"
                    } else {
                        tool_call.arguments.as_str()
                    }
                }
            }))
        })
        .collect()
}

fn accumulated_openai_tool_calls_to_responses_items(
    tool_calls: &[OpenAiStreamToolCall],
    available_tool_names: &[String],
    custom_tool_names: &[String],
    namespace_tool_mappings: &[NamespaceToolMapping],
) -> Vec<serde_json::Value> {
    accumulated_openai_tool_calls_to_values(tool_calls)
        .iter()
        .filter_map(|tool_call| {
            openai_chat_tool_call_to_responses_item(
                tool_call,
                available_tool_names,
                custom_tool_names,
                namespace_tool_mappings,
            )
        })
        .collect()
}

fn accumulated_openai_tool_calls_to_codex_sse(
    tool_calls: &[OpenAiStreamToolCall],
    available_tool_names: &[String],
    custom_tool_names: &[String],
    namespace_tool_mappings: &[NamespaceToolMapping],
) -> Option<String> {
    let tool_call_values = accumulated_openai_tool_calls_to_values(tool_calls);
    if tool_call_values.is_empty() {
        return None;
    }
    let root = serde_json::json!({
        "choices": [{
            "message": {
                "tool_calls": tool_call_values
            }
        }]
    });
    openai_chat_tool_calls_to_codex_sse(
        &root,
        None,
        available_tool_names,
        custom_tool_names,
        namespace_tool_mappings,
    )
    .map(|body| ensure_responses_stream_completed(body, HEADER_EVENT_STREAM))
}

fn stream_openai_chat_sse_as_codex(
    response: ureq::Response,
    stream: &mut TcpStream,
    route: &ProviderRoute,
    uses_image_generation_tool: bool,
    available_tool_names: &[String],
    custom_tool_names: &[String],
    namespace_tool_mappings: &[NamespaceToolMapping],
    full_debug_id: Option<&str>,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(response.into_reader());
    let mut state = new_codex_text_stream_state(&route.real_model);
    let mut saw_done = false;
    let mut event_samples = Vec::new();
    let mut stream_started = false;
    let mut buffered_text = String::new();
    let mut tool_calls: Vec<OpenAiStreamToolCall> = Vec::new();

    loop {
        let mut line = String::new();
        let bytes_read = match reader.read_line(&mut line) {
            Ok(bytes_read) => bytes_read,
            Err(error) => {
                let reason = format!("stream read failed: {}", error);
                let message = stream_disconnect_user_message(&reason);
                if stream_started {
                    let _ = write_codex_text_stream_failed(
                        stream,
                        &state,
                        "upstream_stream_disconnected",
                        &message,
                    );
                } else {
                    let _ =
                        write_codex_stream_error(stream, "upstream_stream_disconnected", &message);
                }
                return Err(std::io::Error::new(error.kind(), reason));
            }
        };
        if bytes_read == 0 {
            break;
        }
        let line = line.trim_end_matches(&['\r', '\n'][..]).trim_start();
        let Some(data) = line.strip_prefix("data:") else {
            append_router_full_debug_log(
                "openai_chat_sse_line",
                serde_json::json!({
                    "debug_id": full_debug_id,
                    "direction": "upstream",
                    "ignored": true,
                    "line": router_full_debug_sse_line_value(line)
                }),
            );
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            saw_done = true;
            append_router_full_debug_log(
                "openai_chat_sse_line",
                serde_json::json!({
                    "debug_id": full_debug_id,
                    "direction": "upstream",
                    "held_until_conversion_completed": true,
                    "line": "data: [DONE]"
                }),
            );
            break;
        }
        append_router_full_debug_log(
            "openai_chat_sse_line",
            serde_json::json!({
                "debug_id": full_debug_id,
                "direction": "upstream",
                "line": router_full_debug_sse_line_value(line)
            }),
        );
        if event_samples.len() < 20 {
            event_samples.push(router_debug_body_value(data));
        }
        let Ok(root) = serde_json::from_str::<serde_json::Value>(data) else {
            continue;
        };
        merge_openai_stream_tool_calls(&root, &mut tool_calls);
        if let Some(delta) = extract_openai_chat_stream_text_delta(&root) {
            if !delta.is_empty() {
                if stream_started {
                    write_codex_text_stream_delta(stream, &mut state, &delta)?;
                } else {
                    buffered_text.push_str(&delta);
                    if !buffered_text.trim().is_empty() {
                        write_codex_text_stream_start(stream, &state)?;
                        stream_started = true;
                        let text = std::mem::take(&mut buffered_text);
                        write_codex_text_stream_delta(stream, &mut state, &text)?;
                    }
                }
            }
        }
    }

    if !saw_done {
        let reason = "stream closed before upstream sent [DONE]";
        append_router_debug_log(
            "custom_openai_stream_incomplete",
            serde_json::json!({
                "target_provider": route.provider.clone(),
                "model": route.real_model.clone(),
                "saw_done": saw_done,
                "text_len": state.text.len(),
                "buffered_text_len": buffered_text.len(),
                "stream_started": stream_started,
                "event_samples": event_samples
            }),
        );
        let message = stream_disconnect_user_message(reason);
        if stream_started {
            write_codex_text_stream_failed(
                stream,
                &state,
                "upstream_stream_disconnected",
                &message,
            )?;
        } else {
            write_codex_stream_error(stream, "upstream_stream_disconnected", &message)?;
        }
        return Err(std::io::Error::new(ErrorKind::UnexpectedEof, reason));
    }

    if !stream_started && !buffered_text.trim().is_empty() {
        write_codex_text_stream_start(stream, &state)?;
        stream_started = true;
        let text = std::mem::take(&mut buffered_text);
        write_codex_text_stream_delta(stream, &mut state, &text)?;
    }

    let tool_items = accumulated_openai_tool_calls_to_responses_items(
        &tool_calls,
        available_tool_names,
        custom_tool_names,
        namespace_tool_mappings,
    );
    if !tool_items.is_empty() && !stream_started {
        if let Some(tool_sse) = accumulated_openai_tool_calls_to_codex_sse(
            &tool_calls,
            available_tool_names,
            custom_tool_names,
            namespace_tool_mappings,
        ) {
            stream.write_all(tool_sse.as_bytes())?;
            return stream.flush();
        }
    }

    if state.text.trim().is_empty() {
        let (code, message) = if uses_image_generation_tool {
            (
                "custom_image_generation_not_supported",
                CUSTOM_IMAGE_GENERATION_UNSUPPORTED_MESSAGE,
            )
        } else {
            (
                "upstream_empty_response",
                CUSTOM_UPSTREAM_EMPTY_RESPONSE_MESSAGE,
            )
        };
        append_router_debug_log(
            "custom_openai_stream_empty_text",
            serde_json::json!({
                "target_provider": route.provider.clone(),
                "model": route.real_model.clone(),
                "uses_image_generation_tool": uses_image_generation_tool,
                "error_code": code,
                "saw_done": saw_done,
                "text_len": state.text.len(),
                "buffered_text_len": buffered_text.len(),
                "tool_call_count": tool_items.len(),
                "stream_started": stream_started,
                "event_samples": event_samples
            }),
        );
        if stream_started {
            write_codex_text_stream_failed(stream, &state, code, message)?;
        } else {
            write_codex_stream_error(stream, code, message)?;
        }
        return Err(std::io::Error::new(ErrorKind::InvalidData, message));
    }

    if uses_image_generation_tool && tool_items.is_empty() {
        append_router_debug_log(
            "custom_openai_stream_image_generation_not_triggered",
            serde_json::json!({
                "target_provider": route.provider.clone(),
                "model": route.real_model.clone(),
                "text_len": state.text.len(),
                "stream_started": stream_started,
                "event_samples": event_samples
            }),
        );
        write_codex_text_stream_failed(
            stream,
            &state,
            "custom_image_generation_not_supported",
            CUSTOM_IMAGE_GENERATION_UNSUPPORTED_MESSAGE,
        )?;
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            CUSTOM_IMAGE_GENERATION_UNSUPPORTED_MESSAGE,
        ));
    }

    if tool_items.is_empty() {
        append_router_full_debug_log(
            "openai_chat_router_response_completed",
            serde_json::json!({
                "debug_id": full_debug_id,
                "target_provider": route.provider.clone(),
                "model": route.real_model.clone(),
                "text_len": state.text.len(),
                "tool_call_count": 0
            }),
        );
        write_codex_text_stream_done(stream, &state)
    } else {
        append_router_full_debug_log(
            "openai_chat_router_response_completed",
            serde_json::json!({
                "debug_id": full_debug_id,
                "target_provider": route.provider.clone(),
                "model": route.real_model.clone(),
                "text_len": state.text.len(),
                "tool_call_count": tool_items.len()
            }),
        );
        write_codex_text_stream_done_with_items(stream, &state, tool_items)
    }
}

fn extract_openai_chat_stream_text_delta(root: &serde_json::Value) -> Option<String> {
    root.get("choices")
        .and_then(|value| value.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("delta"))
        .and_then(|delta| {
            delta
                .get("content")
                .or_else(|| delta.get("reasoning_content"))
                .or_else(|| delta.get("reasoning"))
        })
        .map(content_to_text)
}

fn find_codex_account_id(root: &serde_json::Value) -> Option<String> {
    find_string_by_keys(root, CODEX_ACCOUNT_ID_KEYS).or_else(|| find_first_account_id(root))
}

fn find_codex_user_id(root: &serde_json::Value) -> Option<String> {
    find_string_by_keys(root, &["user_id", "userId", "sub", "subject"])
}

fn build_codex_account_key(root: &serde_json::Value) -> Option<String> {
    let access_claims = find_codex_access_token(root).and_then(|token| decode_jwt_payload(&token));
    let id_claims = find_string_by_keys(root, &["id_token", "idToken"])
        .and_then(|token| decode_jwt_payload(&token));
    let user_id = find_codex_user_id(root)
        .or_else(|| access_claims.as_ref().and_then(find_codex_user_id))
        .or_else(|| id_claims.as_ref().and_then(find_codex_user_id))?;
    let account_id = find_codex_account_id(root)
        .or_else(|| access_claims.as_ref().and_then(find_codex_account_id))
        .or_else(|| id_claims.as_ref().and_then(find_codex_account_id))?;

    Some(format!(
        "{}{}{}",
        user_id, ACCOUNT_KEY_SEPARATOR, account_id
    ))
}

fn build_registry_snapshot_key(
    item: &serde_json::Value,
    snapshot_root: Option<&serde_json::Value>,
    account_key: &str,
) -> String {
    let snapshot_access_claims = snapshot_root
        .and_then(find_codex_access_token)
        .and_then(|token| decode_jwt_payload(&token));
    let snapshot_id_claims = snapshot_root
        .and_then(|root| find_string_by_keys(root, &["id_token", "idToken"]))
        .and_then(|token| decode_jwt_payload(&token));
    let user_id = find_string_by_keys(item, &["userId", "user_id"])
        .or_else(|| snapshot_root.and_then(find_codex_user_id))
        .or_else(|| snapshot_access_claims.as_ref().and_then(find_codex_user_id))
        .or_else(|| snapshot_id_claims.as_ref().and_then(find_codex_user_id))
        .unwrap_or_else(|| "-".to_string());
    let account_id = find_string_by_keys(item, &["usageAccountId", "accountId", "account_id"])
        .or_else(|| snapshot_root.and_then(find_codex_account_id))
        .or_else(|| {
            snapshot_access_claims
                .as_ref()
                .and_then(find_codex_account_id)
        })
        .or_else(|| snapshot_id_claims.as_ref().and_then(find_codex_account_id))
        .unwrap_or_else(|| account_key.to_string());

    format!("{}{}{}", user_id, ACCOUNT_KEY_SEPARATOR, account_id)
}

fn find_first_account_id(root: &serde_json::Value) -> Option<String> {
    let accounts = root.get(CODEX_AUTH_ACCOUNTS_KEY)?.as_object()?;

    for (key, value) in accounts {
        if !key.trim().is_empty() {
            return Some(key.trim().to_string());
        }

        if let Some(account_id) = find_string_by_keys(value, CODEX_ACCOUNT_ID_KEYS) {
            return Some(account_id);
        }
    }

    None
}



fn load_provider_config() -> Result<RouterProviderConfig, String> {
    ensure_provider_config_file()?;
    let config_path = provider_config_path()?;
    let config_text = fs::read_to_string(&config_path).map_err(|error| {
        format!(
            "读取配置文件失败：{}，路径：{}",
            error,
            config_path.display()
        )
    })?;
    serde_json::from_str::<RouterProviderConfig>(&config_text).map_err(|error| {
        format!(
            "解析配置文件失败：{}，路径：{}",
            error,
            config_path.display()
        )
    })
}

fn enabled_provider_routes() -> Result<Vec<EnabledProviderRoute>, String> {
    let provider_config = load_provider_config()?;
    let mut routes = Vec::new();

    for (slug, value) in provider_config.0 {
        let route = serde_json::from_value::<ProviderRouteFileItem>(value)
            .map_err(|error| format!("解析 provider 配置失败：{}，slug：{}", error, slug))?;

        if route.enabled {
            routes.push(EnabledProviderRoute { slug, route });
        }
    }

    Ok(routes)
}

fn provider_route_slugs() -> Result<HashSet<String>, String> {
    Ok(load_provider_config()?.0.keys().cloned().collect())
}

fn ensure_provider_config_file() -> Result<(), String> {
    let path = provider_config_path()?;
    ensure_parent_dir(&path)?;
    if !path.exists() {
        fs::write(&path, "[]").map_err(|error| {
            format!(
                "创建 provider 配置文件失败：{}，路径：{}",
                error,
                path.display()
            )
        })?;
    }
    Ok(())
}

fn ensure_app_settings_file() -> Result<(), String> {
    ensure_json_file(&app_settings_path()?)
}

fn ensure_catalog_config_file() -> Result<(), String> {
    let path = catalog_config_path()?;
    ensure_parent_dir(&path)?;

    if !path.exists() {
        repair_catalog_config_file()?;
        return Ok(());
    }

    let text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "读取 Catalog 配置文件失败：{}，路径：{}",
            error,
            path.display()
        )
    })?;

    if !is_valid_catalog_config_text(&text) {
        repair_catalog_config_file()?;
    }

    Ok(())
}

fn is_valid_catalog_config_text(text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }

    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|root| {
            root.get(CATALOG_MODELS_KEY)
                .and_then(|value| value.as_array())
                .map(|models| {
                    models
                        .first()
                        .and_then(|model| model.get("shell_type"))
                        .and_then(|value| value.as_str())
                        .is_some()
                })
        })
        .unwrap_or(false)
}

fn repair_catalog_config_file() -> Result<(), String> {
    sync_catalog_from_provider_config().map(|_| ())
}

#[allow(unreachable_code)]
fn ensure_catalog_base_config_file() -> Result<(), String> {
    let base_path = catalog_base_config_path()?;
    let managed_slugs = provider_route_slugs()?;
    let (_, base_root) = load_catalog_base_root(&managed_slugs)?;
    return write_catalog_root(&base_path, &base_root);
    ensure_parent_dir(&base_path)?;

    if base_path.exists() {
        let base_text = fs::read_to_string(&base_path).map_err(|error| {
            format!(
                "read base catalog failed: {}, path: {}",
                error,
                base_path.display()
            )
        })?;
        if is_valid_catalog_config_text(&base_text) {
            return Ok(());
        }
    }

    let source_path = models_cache_path()?;
    if !source_path.exists() {
        let mut base_root = serde_json::Map::new();
        base_root.insert(
            "models".to_string(),
            serde_json::Value::Array(build_fallback_catalog_base_models()),
        );
        let text = serde_json::to_string_pretty(&serde_json::Value::Object(base_root))
            .map_err(|error| format!("序列化基础 Catalog 配置失败：{}", error))?;
        return fs::write(&base_path, text).map_err(|error| {
            format!(
                "写入基础 Catalog 配置失败：{}，路径：{}",
                error,
                base_path.display()
            )
        });
    }
    let source_text = fs::read_to_string(&source_path).map_err(|error| {
        format!(
            "读取 Codex models_cache.json 失败：{}，路径：{}",
            error,
            source_path.display()
        )
    })?;
    let source_root = serde_json::from_str::<serde_json::Value>(&source_text).map_err(|error| {
        format!(
            "解析 Codex models_cache.json 失败：{}，路径：{}",
            error,
            source_path.display()
        )
    })?;
    let source_models = source_root
        .get("models")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "原配置缺少 models 或 models 为空".to_string())?;

    if source_models.is_empty() {
        return Err("原配置缺少 models 或 models 为空".to_string());
    }

    let mut base_root = serde_json::Map::new();
    base_root.insert(
        "models".to_string(),
        serde_json::Value::Array(source_models.clone()),
    );
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(base_root))
        .map_err(|error| format!("序列化基础 Catalog 配置失败：{}", error))?;
    fs::write(&base_path, text).map_err(|error| {
        format!(
            "写入基础 Catalog 配置失败：{}，路径：{}",
            error,
            base_path.display()
        )
    })
}

fn build_fallback_catalog_base_models() -> Vec<serde_json::Value> {
    let mut model = serde_json::Map::new();
    model.insert(
        "slug".to_string(),
        serde_json::Value::String("codex-router-fallback".to_string()),
    );
    model.insert(
        "display_name".to_string(),
        serde_json::Value::String("Codex Router Fallback".to_string()),
    );
    model.insert(
        "description".to_string(),
        serde_json::Value::String(
            "Fallback catalog entry generated when Codex models_cache.json is unavailable."
                .to_string(),
        ),
    );
    model.insert(
        "base_instructions".to_string(),
        serde_json::Value::String(CODEX_ROUTER_IDENTITY_PREFIX.to_string()),
    );
    model.insert(
        "model_messages".to_string(),
        serde_json::json!({
            "instructions_template": CODEX_ROUTER_IDENTITY_PREFIX,
        }),
    );
    model.insert(
        "shell_type".to_string(),
        serde_json::Value::String("default".to_string()),
    );
    model.insert(
        "priority".to_string(),
        serde_json::Value::Number(serde_json::Number::from(0)),
    );
    model.insert("availability_nux".to_string(), serde_json::Value::Null);
    model.insert(
        "visibility".to_string(),
        serde_json::Value::String("list".to_string()),
    );
    model.insert(
        "supported_in_api".to_string(),
        serde_json::Value::Bool(true),
    );

    vec![serde_json::Value::Object(model)]
}

fn ensure_codex_config_backup() -> Result<(), String> {
    let path = codex_config_path()?;
    let backup_path = codex_config_backup_path()?;
    ensure_parent_dir(&backup_path)?;

    if !backup_path.exists() {
        fs::copy(&path, &backup_path).map_err(|error| {
            format!(
                "澶囦唤 Codex config.toml 澶辫触：{}锛岃矾寰勶細{}",
                error,
                backup_path.display()
            )
        })?;
    }

    Ok(())
}

fn upsert_codex_router_config() -> Result<(), String> {
    let settings = load_app_settings()?;
    let path = codex_config_path()?;
    let current_text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "璇诲彇 Codex config.toml 澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })?;
    let cleaned_text = remove_managed_codex_router_block(&current_text);
    let cleaned_text = remove_codex_router_top_level_keys(&cleaned_text);
    let managed_block = build_codex_router_config_block(&settings)?;
    let next_text = if cleaned_text.trim().is_empty() {
        managed_block
    } else {
        format!("{}\n\n{}", managed_block, cleaned_text.trim_start())
    };

    fs::write(&path, next_text).map_err(|error| {
        format!(
            "鍐欏叆 Codex config.toml 澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })
}

fn remove_codex_router_config() -> Result<(), String> {
    let path = codex_config_path()?;

    if !path.exists() {
        return Ok(());
    }

    let current_text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "璇诲彇 Codex config.toml 澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })?;
    let next_text = remove_managed_codex_router_block(&current_text);
    fs::write(&path, next_text).map_err(|error| {
        format!(
            "娓呯悊 Codex config.toml 澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })
}

fn is_codex_router_config_present() -> bool {
    let Ok(path) = codex_config_path() else {
        return false;
    };
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };

    text.contains(CODEX_ROUTER_TOP_MANAGED_START_MARKER)
        || text.contains(CODEX_ROUTER_MANAGED_START_MARKER)
        || text.contains("# <<< codex-router managed start")
}

fn recover_router_state_on_startup() -> Result<(), String> {
    if !is_codex_router_config_present() || router_status()?.started {
        return Ok(());
    }

    start_router_blocking().map(|_| ())
}

fn remove_managed_codex_router_block(text: &str) -> String {
    let mut result = String::new();
    let mut in_managed_block = false;

    for line_with_ending in text.split_inclusive('\n') {
        let line = line_with_ending.trim_end_matches(['\r', '\n']);
        let trimmed_line = line.trim();

        if trimmed_line == CODEX_ROUTER_TOP_MANAGED_START_MARKER
            || trimmed_line == CODEX_ROUTER_MANAGED_START_MARKER
            || trimmed_line == "# <<< codex-router managed start"
        {
            in_managed_block = true;
            continue;
        }

        if trimmed_line == CODEX_ROUTER_TOP_MANAGED_END_MARKER
            || trimmed_line == CODEX_ROUTER_MANAGED_END_MARKER
            || trimmed_line == "# <<< codex-router managed end"
        {
            in_managed_block = false;
            continue;
        }

        if !in_managed_block {
            result.push_str(line_with_ending);
        }
    }

    result
}

fn remove_codex_router_top_level_keys(text: &str) -> String {
    let mut result = String::new();
    let mut in_table = false;

    for line_with_ending in text.split_inclusive('\n') {
        let line = line_with_ending.trim_end_matches(['\r', '\n']);
        let trimmed_line = line.trim();

        if trimmed_line.starts_with('[') {
            in_table = true;
            result.push_str(line_with_ending);
            continue;
        }

        if !in_table && is_codex_router_top_level_key(trimmed_line) {
            continue;
        }

        result.push_str(line_with_ending);
    }

    result
}

fn is_codex_router_top_level_key(trimmed_line: &str) -> bool {
    let Some((left, _)) = trimmed_line.split_once('=') else {
        return false;
    };
    matches!(
        left.trim(),
        "model_provider" | "model" | "model_catalog_json"
    ) || left.trim().starts_with("model_providers.")
}

fn read_current_codex_model_provider() -> Option<String> {
    let path = codex_config_path().ok()?;
    let text = fs::read_to_string(path).ok()?;
    read_toml_string_key(&text, "model_provider").filter(|provider| !provider.trim().is_empty())
}

fn read_toml_string_key(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') || trimmed.starts_with('#') {
            continue;
        }
        if let Some(value) = parse_toml_key_value(trimmed, key) {
            return Some(value);
        }
    }
    None
}

fn parse_toml_key_value(line: &str, key: &str) -> Option<String> {
    let (left, right) = line.split_once('=')?;
    if left.trim() != key {
        return None;
    }
    let value = right.split('#').next().unwrap_or(right).trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        return Some(value[1..value.len() - 1].to_string());
    }
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return Some(value[1..value.len() - 1].to_string());
    }
    Some(value.to_string())
}

fn build_codex_router_config_block(settings: &AppSettings) -> Result<String, String> {
    let default_model = if settings.router_default_model.trim().is_empty() {
        read_default_router_profile_model()?
    } else {
        settings.router_default_model.trim().to_string()
    };
    let catalog_path = if settings.router_model_catalog_json.trim().is_empty() {
        catalog_config_path()?.display().to_string()
    } else {
        settings.router_model_catalog_json.trim().to_string()
    };
    let base_url = if settings.router_mode == "system" {
        format!("http://{}:{}/v1", ROUTER_HOST, configured_router_port())
    } else if settings.router_base_url.trim().is_empty() {
        format!("http://{}:{}/v1", ROUTER_HOST, configured_router_port())
    } else {
        settings.router_base_url.trim().to_string()
    };
    let provider = CODEX_PROVIDER_NAME;
    let provider_display_name = if settings.router_name.trim().is_empty() {
        CODEX_MODEL_PROVIDER_NAME.to_string()
    } else {
        settings.router_name.trim().to_string()
    };

    let auth_method = settings.router_auth_method.trim();
    let external_token = settings.router_auth_external_token.trim();
    let env_key = settings.router_auth_env_key.trim();

    let auth_block = match auth_method {
        "external" => format!(
            "#直接指定外部key形式\nexperimental_bearer_token = \"{}\"",
            toml_basic_string(external_token)
        ),
        "env" => format!(
            "#配置自定义模型apikey形式\nenv_key = \"{}\"",
            toml_basic_string(env_key)
        ),
        _ => "#官方常规登录形式\nrequires_openai_auth = true".to_string(),
    };

    Ok(format!(
        "{start}\nmodel_provider = \"{provider}\"\nmodel = \"{model}\"\nmodel_catalog_json = \"{catalog_path}\"\n\n[model_providers.{provider}]\nname = \"{name}\"\nbase_url = \"{base_url}\"\nwire_api = \"{wire_api}\"\n{auth_block}\n{end}",
        start = CODEX_ROUTER_TOP_MANAGED_START_MARKER,
        provider = provider,
        model = toml_basic_string(&default_model),
        name = toml_basic_string(&provider_display_name),
        base_url = toml_basic_string(&base_url),
        wire_api = CODEX_WIRE_API,
        auth_block = auth_block,
        catalog_path = toml_basic_string(&catalog_path),
        end = CODEX_ROUTER_TOP_MANAGED_END_MARKER,
    ))
}

fn toml_basic_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn read_default_router_profile_model() -> Result<String, String> {
    let routes = enabled_provider_routes()?;
    if let Some(item) = routes.iter().find(|item| item.route.active) {
        return Ok(item.slug.clone());
    }
    if let Some(item) = routes.into_iter().next() {
        return Ok(item.slug);
    }

    read_first_official_model_display_name()
}
fn read_first_official_model_display_name() -> Result<String, String> {
    ensure_catalog_config_file()?;
    let source_path = catalog_config_path()?;
    let source_text = fs::read_to_string(&source_path).map_err(|error| {
        format!(
            "璇诲彇 Codex models_cache.json 澶辫触：{}锛岃矾寰勶細{}",
            error,
            source_path.display()
        )
    })?;
    let source_root = serde_json::from_str::<serde_json::Value>(&source_text).map_err(|error| {
        format!(
            "瑙ｆ瀽 Codex models_cache.json 澶辫触：{}锛岃矾寰勶細{}",
            error,
            source_path.display()
        )
    })?;
    let first_model = source_root
        .get("models")
        .and_then(|value| value.as_array())
        .and_then(|models| models.first())
        .ok_or_else(|| "原配置缺少models 或models 为空".to_string())?;

    first_model
        .get("display_name")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            first_model
                .get("slug")
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
        })
        .map(|value| value.trim().to_string())
        .ok_or_else(|| {
            "官方模型第一项缺少display_name 和slug，无法生成默认profile model".to_string()
        })
}









































#[allow(dead_code)]


#[allow(dead_code)]




























fn clean_invalid_account_snapshots() -> Result<CleanCount, String> {
    let snapshots_dir = codex_accounts_snapshots_path()?;
    assert_safe_cleanup_root(&snapshots_dir)?;

    if !snapshots_dir.exists() {
        return Ok(CleanCount::default());
    }

    let referenced_paths = read_json_file_optional(&codex_accounts_registry_path()?)
        .and_then(|registry| {
            registry
                .get("items")
                .and_then(|value| value.as_array())
                .cloned()
        })
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| json_string_field(&item, "snapshotPath"))
        .map(PathBuf::from)
        .filter_map(|path| path.canonicalize().ok())
        .collect::<HashSet<_>>();

    let mut result = CleanCount::default();
    let entries = fs::read_dir(&snapshots_dir).map_err(|error| {
        format!(
            "读取账号快照目录失败：{}，路径：{}",
            error,
            snapshots_dir.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("读取账号快照条目失败：{}", error))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let is_json = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        let is_referenced = path
            .canonicalize()
            .ok()
            .map(|canonical| referenced_paths.contains(&canonical))
            .unwrap_or(false);
        let is_valid_snapshot = is_json && read_json_file_optional(&path).is_some();

        if is_referenced && is_valid_snapshot {
            continue;
        }

        let deleted = remove_file_counted(&path, "失效账号快照")?;
        result.count += deleted.count;
        result.bytes += deleted.bytes;
    }

    Ok(result)
}











fn read_mcp_servers_from_config(config_path: &Path) -> Result<Vec<McpServerSummary>, String> {
    if !config_path.exists() {
        return Ok(Vec::new());
    }

    let text = fs::read_to_string(config_path).map_err(|error| {
        format!(
            "读取 Codex config.toml 失败：{}，路径：{}",
            error,
            config_path.display()
        )
    })?;
    let source_path = config_path.display().to_string();
    let mut servers: HashMap<String, McpServerSummary> = HashMap::new();
    let mut current_name: Option<String> = None;
    let mut current_subsection: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = trimmed.trim_start_matches('[').trim_end_matches(']');
            if let Some(rest) = section.strip_prefix("mcp_servers.") {
                let parts = split_toml_section(rest);
                if let Some(name) = parts.first() {
                    let name = unquote_toml(name);
                    current_name = Some(name.clone());
                    current_subsection = parts.get(1).map(|value| unquote_toml(value));
                    servers
                        .entry(name.clone())
                        .or_insert_with(|| McpServerSummary {
                            name,
                            transport: "stdio".to_string(),
                            enabled: true,
                            source_path: source_path.clone(),
                            command: None,
                            args: Vec::new(),
                            url: None,
                            headers: HashMap::new(),
                            environment: HashMap::new(),
                        });
                } else {
                    current_name = None;
                    current_subsection = None;
                }
            } else {
                current_name = None;
                current_subsection = None;
            }
            continue;
        }

        let Some(name) = current_name.as_ref() else {
            continue;
        };
        let Some((key, value)) = parse_toml_assignment(trimmed) else {
            continue;
        };
        let Some(server) = servers.get_mut(name) else {
            continue;
        };

        match current_subsection.as_deref() {
            Some("env") => {
                server
                    .environment
                    .insert(key.to_string(), parse_toml_string(value));
            }
            Some("headers") => {
                server
                    .headers
                    .insert(key.to_string(), parse_toml_string(value));
            }
            _ => match key {
                "enabled" => server.enabled = value.eq_ignore_ascii_case("true"),
                "transport" => server.transport = parse_toml_string(value),
                "command" => server.command = Some(parse_toml_string(value)),
                "url" => server.url = Some(parse_toml_string(value)),
                "args" => server.args = parse_toml_array(value),
                _ => {}
            },
        }
    }

    let mut items: Vec<McpServerSummary> = servers.into_values().collect();
    items.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(items)
}

fn validate_mcp_server(server: &McpServerSummary) -> Result<(), String> {
    if server.name.trim().is_empty() {
        return Err("MCP 服务名称不能为空。".to_string());
    }
    if !matches!(server.transport.as_str(), "stdio" | "http" | "sse") {
        return Err("MCP transport 只能是 stdio、http 或 sse。".to_string());
    }
    if server.transport == "stdio" && server.command.as_deref().unwrap_or("").trim().is_empty() {
        return Err("stdio 类型 MCP 必须填写 command。".to_string());
    }
    if matches!(server.transport.as_str(), "http" | "sse")
        && server.url.as_deref().unwrap_or("").trim().is_empty()
    {
        return Err("http/sse 类型 MCP 必须填写 url。".to_string());
    }
    Ok(())
}

fn write_mcp_server_to_config(config_path: &Path, server: &McpServerSummary) -> Result<(), String> {
    let original = if config_path.exists() {
        fs::read_to_string(config_path).map_err(|error| {
            format!(
                "读取 Codex config.toml 失败：{}，路径：{}",
                error,
                config_path.display()
            )
        })?
    } else {
        String::new()
    };
    let rendered = render_mcp_server_block(server);
    let updated = replace_or_append_mcp_block(&original, &server.name, &rendered);

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "创建 Codex 配置目录失败：{}，路径：{}",
                error,
                parent.display()
            )
        })?;
    }
    fs::write(config_path, updated).map_err(|error| {
        format!(
            "写入 Codex config.toml 失败：{}，路径：{}",
            error,
            config_path.display()
        )
    })
}

fn remove_mcp_server_from_config(config_path: &Path, name: &str) -> Result<(), String> {
    if !config_path.exists() {
        return Err(format!("MCP 服务不存在：{}", name));
    }
    let original = fs::read_to_string(config_path).map_err(|error| {
        format!(
            "读取 Codex config.toml 失败：{}，路径：{}",
            error,
            config_path.display()
        )
    })?;
    let lines: Vec<&str> = original.lines().collect();
    let Some((start, end)) = find_mcp_block_range(&lines, name) else {
        return Err(format!("MCP 服务不存在：{}", name));
    };
    let mut next = Vec::new();
    next.extend_from_slice(&lines[..start]);
    next.extend_from_slice(&lines[end..]);
    fs::write(config_path, next.join("\n")).map_err(|error| {
        format!(
            "写入 Codex config.toml 失败：{}，路径：{}",
            error,
            config_path.display()
        )
    })
}

fn replace_or_append_mcp_block(original: &str, name: &str, rendered: &str) -> String {
    let lines: Vec<&str> = original.lines().collect();
    if let Some((start, end)) = find_mcp_block_range(&lines, name) {
        let mut next = Vec::new();
        next.extend_from_slice(&lines[..start]);
        next.extend(rendered.lines());
        next.extend_from_slice(&lines[end..]);
        return format!("{}\n", next.join("\n").trim_end());
    }

    let mut next = original.trim_end().to_string();
    if !next.is_empty() {
        next.push_str("\n\n");
    }
    next.push_str(rendered.trim_end());
    next.push('\n');
    next
}

fn find_mcp_block_range(lines: &[&str], name: &str) -> Option<(usize, usize)> {
    let mut start = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = trimmed.trim_start_matches('[').trim_end_matches(']');
            if let Some(rest) = section.strip_prefix("mcp_servers.") {
                let parts = split_toml_section(rest);
                if parts
                    .first()
                    .map(|value| unquote_toml(value) == name)
                    .unwrap_or(false)
                {
                    if start.is_none() {
                        start = Some(index);
                    }
                    continue;
                }
                if let Some(start_index) = start {
                    return Some((start_index, index));
                }
            } else if let Some(start_index) = start {
                return Some((start_index, index));
            }
        }
    }
    start.map(|start_index| (start_index, lines.len()))
}

fn render_mcp_server_block(server: &McpServerSummary) -> String {
    let header = quote_toml_string(&server.name);
    let mut lines = vec![
        format!("[mcp_servers.{header}]"),
        format!(
            "enabled = {}",
            if server.enabled { "true" } else { "false" }
        ),
        format!("transport = {}", quote_toml_string(&server.transport)),
    ];

    if server.transport == "stdio" {
        if let Some(command) = server.command.as_ref() {
            lines.push(format!("command = {}", quote_toml_string(command)));
        }
        if !server.args.is_empty() {
            let args = server
                .args
                .iter()
                .map(|arg| quote_toml_string(arg))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("args = [{args}]"));
        }
    } else if let Some(url) = server.url.as_ref() {
        lines.push(format!("url = {}", quote_toml_string(url)));
    }

    if !server.environment.is_empty() {
        lines.push(String::new());
        lines.push(format!("[mcp_servers.{header}.env]"));
        let mut keys: Vec<&String> = server.environment.keys().collect();
        keys.sort();
        for key in keys {
            let value = server
                .environment
                .get(key)
                .map(String::as_str)
                .unwrap_or("");
            lines.push(format!("{} = {}", key, quote_toml_string(value)));
        }
    }

    if !server.headers.is_empty() {
        lines.push(String::new());
        lines.push(format!("[mcp_servers.{header}.headers]"));
        let mut keys: Vec<&String> = server.headers.keys().collect();
        keys.sort();
        for key in keys {
            let value = server.headers.get(key).map(String::as_str).unwrap_or("");
            lines.push(format!("{} = {}", key, quote_toml_string(value)));
        }
    }

    lines.join("\n")
}

fn split_toml_section(section: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut escape = false;
    for ch in section.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }
        if ch == '\\' && in_quote {
            current.push(ch);
            escape = true;
            continue;
        }
        if ch == '"' {
            in_quote = !in_quote;
            current.push(ch);
            continue;
        }
        if ch == '.' && !in_quote {
            parts.push(current.trim().to_string());
            current.clear();
            continue;
        }
        current.push(ch);
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

fn parse_toml_assignment(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.split('#').next()?.trim();
    let (key, value) = trimmed.split_once('=')?;
    Some((key.trim(), value.trim()))
}

fn parse_toml_string(value: &str) -> String {
    unquote_toml(value.trim().trim_end_matches(',').trim())
}

fn parse_toml_array(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    let Some(inner) = trimmed
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    else {
        return Vec::new();
    };
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut escape = false;
    for ch in inner.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }
        if ch == '\\' && in_quote {
            current.push(ch);
            escape = true;
            continue;
        }
        if ch == '"' {
            in_quote = !in_quote;
            current.push(ch);
            continue;
        }
        if ch == ',' && !in_quote {
            let item = parse_toml_string(&current);
            if !item.is_empty() {
                values.push(item);
            }
            current.clear();
            continue;
        }
        current.push(ch);
    }
    let item = parse_toml_string(&current);
    if !item.is_empty() {
        values.push(item);
    }
    values
}

fn quote_toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn unquote_toml(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    } else {
        trimmed.to_string()
    }
}

fn load_codex_plugins_result() -> Result<PluginListResult, String> {
    let root = codex_plugins_cache_path()?;
    let state = read_codex_plugin_state()?;
    let mut items = scan_codex_plugins(&root, &state)?;
    items.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| {
                left.display_name
                    .to_lowercase()
                    .cmp(&right.display_name.to_lowercase())
            })
            .then_with(|| left.version.cmp(&right.version))
    });

    Ok(PluginListResult {
        total: items.len(),
        root_path: root.display().to_string(),
        items,
    })
}

fn scan_codex_plugins(
    root: &Path,
    state: &CodexPluginState,
) -> Result<Vec<CodexPluginSummary>, String> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut items = Vec::new();
    for source_entry in fs::read_dir(root)
        .map_err(|error| format!("读取插件缓存目录失败：{}，路径：{}", error, root.display()))?
    {
        let source_entry = source_entry.map_err(|error| format!("读取插件来源失败：{}", error))?;
        let source_path = source_entry.path();
        if !source_path.is_dir() || source_entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let source = source_entry.file_name().to_string_lossy().to_string();
        for plugin_entry in fs::read_dir(&source_path).map_err(|error| {
            format!(
                "读取插件来源目录失败：{}，路径：{}",
                error,
                source_path.display()
            )
        })? {
            let plugin_entry =
                plugin_entry.map_err(|error| format!("读取插件目录失败：{}", error))?;
            let plugin_path = plugin_entry.path();
            let plugin_name = plugin_entry.file_name().to_string_lossy().to_string();
            if !plugin_path.is_dir()
                || plugin_name.starts_with('.')
                || plugin_name.starts_with("plugin-install-")
                || plugin_name.starts_with("plugin-backup-")
            {
                continue;
            }
            for version_entry in fs::read_dir(&plugin_path).map_err(|error| {
                format!(
                    "读取插件版本目录失败：{}，路径：{}",
                    error,
                    plugin_path.display()
                )
            })? {
                let version_entry =
                    version_entry.map_err(|error| format!("读取插件版本失败：{}", error))?;
                let version_path = version_entry.path();
                if !version_path.is_dir() {
                    continue;
                }
                if let Some(plugin) =
                    load_codex_plugin_summary(&source, &plugin_name, &version_path, state)
                {
                    items.push(plugin);
                }
            }
        }
    }
    Ok(items)
}

fn load_codex_plugin_summary(
    source: &str,
    fallback_name: &str,
    dir: &Path,
    state: &CodexPluginState,
) -> Option<CodexPluginSummary> {
    let manifest_path = dir.join(".codex-plugin").join("plugin.json");
    let manifest_text = fs::read_to_string(&manifest_path).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text).ok()?;
    let name = json_string_field(&manifest, "name").unwrap_or_else(|| fallback_name.to_string());
    let version = json_string_field(&manifest, "version")
        .or_else(|| {
            dir.file_name()
                .map(|value| value.to_string_lossy().to_string())
        })
        .unwrap_or_default();
    let id = format!("{}/{}/{}", source, name, version);
    let interface = manifest
        .get("interface")
        .and_then(|value| value.as_object());
    let display_name = interface
        .and_then(|item| item.get("displayName"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| name.clone());
    let description = json_string_field(&manifest, "description");
    let short_description = interface
        .and_then(|item| item.get("shortDescription"))
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let developer_name = interface
        .and_then(|item| item.get("developerName"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .or_else(|| {
            manifest
                .get("author")
                .and_then(|author| author.get("name"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });
    let category = interface
        .and_then(|item| item.get("category"))
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let skills_root = manifest
        .get("skills")
        .and_then(|value| value.as_str())
        .map(|value| dir.join(value.trim_start_matches("./")))
        .unwrap_or_else(|| dir.join("skills"));
    let plugin_enabled = !state.disabled_plugins.contains(&id);
    let skills = scan_plugin_skills(&name, &skills_root, state, plugin_enabled).unwrap_or_default();

    Some(CodexPluginSummary {
        id,
        name,
        display_name,
        source: source.to_string(),
        version,
        description,
        short_description,
        developer_name,
        category,
        enabled: plugin_enabled,
        directory_path: dir.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
        skill_count: skills.len(),
        skills,
    })
}

fn scan_plugin_skills(
    plugin_name: &str,
    root: &Path,
    state: &CodexPluginState,
    plugin_enabled: bool,
) -> Result<Vec<PluginSkillSummary>, String> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut installed = Vec::new();
    scan_skills_recursive(root, root, &mut installed)?;
    let mut items = installed
        .into_iter()
        .map(|skill| {
            let full_name = format!("{}:{}", plugin_name, skill.name);
            PluginSkillSummary {
                id: skill.id,
                name: skill.name,
                enabled: plugin_enabled && !state.disabled_skills.contains(&full_name),
                full_name,
                title: skill.title,
                summary: skill.summary,
                relative_path: skill.relative_path,
                directory_path: skill.directory_path,
                skill_file_path: skill.skill_file_path,
                updated_at: skill.updated_at,
            }
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(items)
}

fn read_codex_plugin_state() -> Result<CodexPluginState, String> {
    let path = codex_plugin_state_path()?;
    if !path.exists() {
        return Ok(CodexPluginState::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("读取插件状态失败：{}，路径：{}", error, path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("解析插件状态失败：{}，路径：{}", error, path.display()))
}

fn write_codex_plugin_state(state: &CodexPluginState) -> Result<(), String> {
    let path = codex_plugin_state_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "创建插件状态目录失败：{}，路径：{}",
                error,
                parent.display()
            )
        })?;
    }
    let text = serde_json::to_string_pretty(state)
        .map_err(|error| format!("序列化插件状态失败：{}", error))?;
    fs::write(&path, text)
        .map_err(|error| format!("写入插件状态失败：{}，路径：{}", error, path.display()))
}

fn scan_installed_skills(skills_dir: &Path) -> Result<Vec<InstalledSkillSummary>, String> {
    if !skills_dir.exists() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    scan_skills_recursive(skills_dir, skills_dir, &mut items)?;
    items.sort_by(|left, right| {
        right
            .updated_at
            .unwrap_or(0)
            .cmp(&left.updated_at.unwrap_or(0))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(items)
}

fn quick_skill_count(skills_dir: &Path) -> Result<usize, String> {
    if !skills_dir.exists() {
        return Ok(0);
    }

    let mut count = 0usize;
    count_skills_recursive(skills_dir, &mut count)?;
    Ok(count)
}

fn count_skills_recursive(dir: &Path, count: &mut usize) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|error| {
        format!(
            "read Skills directory failed: {}, path: {}",
            error,
            dir.display()
        )
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        if path.join("SKILL.md").exists() {
            *count += 1;
        } else {
            count_skills_recursive(&path, count)?;
        }
    }
    Ok(())
}

fn scan_skills_recursive(
    dir: &Path,
    root: &Path,
    items: &mut Vec<InstalledSkillSummary>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|error| format!("读取 Skills 目录失败：{}，路径：{}", error, dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.exists() {
                if let Some(summary) = load_skill_summary(&skill_file, root) {
                    items.push(summary);
                }
            } else {
                scan_skills_recursive(&path, root, items)?;
            }
        }
    }
    Ok(())
}

fn load_skill_summary(skill_file: &Path, root: &Path) -> Option<InstalledSkillSummary> {
    let text = fs::read_to_string(skill_file).ok()?;
    let dir = skill_file.parent()?;
    let relative = dir.strip_prefix(root).ok().unwrap_or(dir);
    let name = dir.file_name()?.to_string_lossy().to_string();
    let title = first_markdown_heading(&text);
    let summary = first_skill_summary_line(&text);
    let updated_at = fs::metadata(skill_file)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64);
    let relative_path = relative.display().to_string();

    Some(InstalledSkillSummary {
        id: relative_path.clone(),
        name,
        title,
        summary,
        relative_path,
        directory_path: dir.display().to_string(),
        skill_file_path: skill_file.display().to_string(),
        updated_at,
    })
}

fn first_markdown_heading(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let heading: String = trimmed
                .chars()
                .skip_while(|ch| *ch == '#' || *ch == ' ')
                .collect();
            if !heading.is_empty() {
                return Some(heading);
            }
        }
    }
    None
}

fn first_skill_summary_line(text: &str) -> Option<String> {
    let mut in_frontmatter = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "---" {
            in_frontmatter = !in_frontmatter;
            continue;
        }
        if in_frontmatter {
            continue;
        }
        if trimmed.starts_with('#')
            || trimmed.starts_with("```")
            || trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
        {
            continue;
        }
        return Some(trimmed.to_string());
    }
    None
}

fn resolve_skill_source(path: &Path) -> Result<PathBuf, String> {
    let source = if path.is_file()
        && path
            .file_name()
            .map(|value| value == "SKILL.md")
            .unwrap_or(false)
    {
        path.parent()
            .ok_or_else(|| "无法识别 SKILL.md 所在目录。".to_string())?
            .to_path_buf()
    } else {
        path.to_path_buf()
    };
    if !source.is_dir() {
        return Err(format!("Skill 来源必须是目录：{}", source.display()));
    }
    if !source.join("SKILL.md").exists() {
        return Err(format!("Skill 目录缺少 SKILL.md：{}", source.display()));
    }
    Ok(source)
}

fn scan_skill_backups(backup_dir: &Path) -> Result<Vec<SkillBackupSummary>, String> {
    if !backup_dir.exists() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    let entries = fs::read_dir(backup_dir).map_err(|error| {
        format!(
            "读取 Skill 备份目录失败：{}，路径：{}",
            error,
            backup_dir.display()
        )
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let metadata_path = path.join("metadata.json");
        let Ok(metadata_text) = fs::read_to_string(&metadata_path) else {
            continue;
        };
        let Ok(metadata) = serde_json::from_str::<SkillBackupMetadata>(&metadata_text) else {
            continue;
        };
        items.push(SkillBackupSummary {
            id: metadata.backup_id,
            skill_id: metadata.skill_id,
            name: metadata.name,
            title: metadata.title,
            relative_path: metadata.relative_path,
            backup_path: path.join("skill").display().to_string(),
            created_at: metadata.created_at,
        });
    }
    items.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(items)
}

fn backup_skill_directory(
    dir: &Path,
    skills_root: &Path,
    backup_root: &Path,
    reason: &str,
) -> Result<SkillBackupSummary, String> {
    fs::create_dir_all(backup_root).map_err(|error| {
        format!(
            "创建 Skill 备份目录失败：{}，路径：{}",
            error,
            backup_root.display()
        )
    })?;
    let skill = load_skill_summary(&dir.join("SKILL.md"), skills_root)
        .ok_or_else(|| format!("Skill 无效，无法备份：{}", dir.display()))?;
    let timestamp = current_unix_timestamp();
    let safe_name = sanitize_path_segment(&skill.name);
    let backup_id = format!("{}-{}-{}", timestamp, reason, safe_name);
    let backup_dir = backup_root.join(&backup_id);
    let staged = backup_dir.join("skill");
    fs::create_dir_all(&backup_dir).map_err(|error| {
        format!(
            "创建 Skill 备份目录失败：{}，路径：{}",
            error,
            backup_dir.display()
        )
    })?;
    copy_dir_all(dir, &staged)?;

    let metadata = SkillBackupMetadata {
        backup_id: backup_id.clone(),
        skill_id: skill.id.clone(),
        name: skill.name.clone(),
        title: skill.title.clone(),
        relative_path: skill.relative_path.clone(),
        created_at: timestamp,
    };
    let metadata_text = serde_json::to_string_pretty(&metadata)
        .map_err(|error| format!("序列化 Skill 备份元数据失败：{}", error))?;
    fs::write(backup_dir.join("metadata.json"), metadata_text)
        .map_err(|error| format!("写入 Skill 备份元数据失败：{}", error))?;

    Ok(SkillBackupSummary {
        id: backup_id,
        skill_id: skill.id,
        name: skill.name,
        title: skill.title,
        relative_path: skill.relative_path,
        backup_path: staged.display().to_string(),
        created_at: timestamp,
    })
}



fn sanitize_path_segment(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect();
    sanitized
        .trim_matches('-')
        .chars()
        .take(48)
        .collect::<String>()
}







fn hidden_command(program: &str) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW_FLAG);
    command
}

fn scan_thread_root(
    root: &Path,
    source: &str,
    archived: bool,
    index_info: &SessionIndexInfo,
    sessions: &mut Vec<ThreadSession>,
) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }

    scan_thread_dir(root, source, archived, index_info, sessions)
}

fn scan_thread_dir(
    dir: &Path,
    source: &str,
    archived: bool,
    index_info: &SessionIndexInfo,
    sessions: &mut Vec<ThreadSession>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|error| format!("读取会话目录失败：{}，路径：{}", error, dir.display()))?;

    for entry_result in entries {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();

        if path.is_dir() {
            scan_thread_dir(&path, source, archived, index_info, sessions)?;
            continue;
        }

        let is_jsonl = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("jsonl"))
            .unwrap_or(false);
        if is_jsonl {
            if let Ok(session) = parse_thread_session_file(&path, source, archived, index_info) {
                sessions.push(session);
            }
        }
    }

    Ok(())
}

fn parse_thread_session_file(
    path: &Path,
    source: &str,
    archived: bool,
    index_info: &SessionIndexInfo,
) -> Result<ThreadSession, String> {
    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "璇诲彇浼氳瘽鏂囦欢淇℃伅澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })?;
    let file = fs::File::open(path).map_err(|error| {
        format!(
            "鎵撳紑浼氳瘽鏂囦欢澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })?;
    let reader = BufReader::new(file);
    let mut id = String::new();
    let mut cwd = None;
    let mut originator = None;
    let mut cli_version = None;
    let mut thread_source = None;
    let mut created_at = None;
    let mut thread_name = None;
    let mut first_user_text = None;
    let mut latest_timestamp = None;
    let mut message_count = 0usize;
    let mut parse_errors = 0usize;

    for line_result in reader.lines() {
        let Ok(line) = line_result else {
            parse_errors += 1;
            continue;
        };
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            parse_errors += 1;
            continue;
        };

        if let Some(timestamp) = json_string_field(&value, "timestamp") {
            latest_timestamp = Some(timestamp);
        }

        if value.get("type").and_then(|item| item.as_str()) == Some("session_meta") {
            if let Some(payload) = value.get("payload") {
                if id.is_empty() {
                    id = json_string_field(payload, "id").unwrap_or_default();
                }
                if created_at.is_none() {
                    created_at = json_string_field(payload, "timestamp");
                }
                if cwd.is_none() {
                    cwd = json_string_field(payload, "cwd");
                }
                if originator.is_none() {
                    originator = json_string_field(payload, "originator");
                }
                if cli_version.is_none() {
                    cli_version = json_string_field(payload, "cli_version");
                }
                if thread_source.is_none() {
                    thread_source = json_string_field(payload, "thread_source");
                }
            }
            continue;
        }

        if let Some(payload) = value.get("payload") {
            let payload_type = payload.get("type").and_then(|item| item.as_str());
            let role = payload.get("role").and_then(|item| item.as_str());

            if payload_type.map(is_thread_title_event).unwrap_or(false) {
                if let Some(name) = ["thread_name", "threadName", "title", "name"]
                    .iter()
                    .find_map(|key| json_string_field(payload, key))
                    .filter(|name| valid_title(name).is_some())
                {
                    thread_name = Some(truncate_text(&name, MAX_THREAD_TITLE_CHARS));
                }
            }

            if payload_type == Some("message") && matches!(role, Some("user") | Some("assistant")) {
                message_count += 1;
            }

            if payload_type == Some("message") && role == Some("user") && first_user_text.is_none()
            {
                if let Some(text) = extract_title_text(payload) {
                    let candidate = extract_real_user_request(&text).or_else(|| {
                        (!is_synthetic_user_message(&text)).then(|| text.clone())
                    });
                    if let Some(candidate) = candidate.filter(|text| is_real_user_text(text)) {
                        first_user_text = Some(truncate_text(&candidate, MAX_THREAD_TITLE_CHARS));
                    }
                }
            }
        }
    }

    if id.is_empty() {
        id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("unknown-thread")
            .to_string();
    }

    let indexed = index_info.ids.contains(&id);
    let in_any_sidebar = index_info.sidebar_ids.contains(&id);
    let sidebar_missing = !in_any_sidebar;
    let state_needs_repair =
        thread_session_state_needs_repair(path, &id, cwd.as_deref(), archived, index_info);
    let sqlite_thread = index_info.sqlite_threads.get(&id);
    let title = sqlite_thread
        .and_then(|thread| valid_title(&thread.title))
        .or_else(|| index_info.titles.get(&id).and_then(|title| valid_title(title)))
        .or_else(|| thread_name.as_deref().and_then(valid_title))
        .or_else(|| first_user_text.as_deref().and_then(valid_title))
        .or_else(|| sqlite_thread.and_then(|thread| valid_title(&thread.first_user_message)))
        .unwrap_or_else(|| "未命名会话".to_string());
    let project_name = project_name_from_cwd(cwd.as_deref());
    let updated_at =
        latest_timestamp.or_else(|| metadata.modified().ok().map(system_time_to_unix_string));

    Ok(ThreadSession {
        id,
        title,
        file_path: path.display().to_string(),
        source: source.to_string(),
        archived,
        indexed,
        missing_from_index: !indexed,
        sidebar_missing,
        state_needs_repair,
        cwd,
        project_name,
        originator,
        cli_version,
        thread_source,
        created_at,
        updated_at,
        file_size: metadata.len(),
        message_count,
        first_user_text,
        parse_errors,
    })
}

#[allow(dead_code)]
fn is_synthetic_user_message(content: &str) -> bool {
    let value = content.trim().to_lowercase();
    value.starts_with("# files pasted by the user:")
        || value.starts_with("# files mentioned by the user:")
        || value.starts_with("# files attached by the user:")
        || value.contains("the attached pasted text file(s) contain the user's request")
        || value.contains("the attached file(s) contain the user's request")
}

fn decode_protocol_text_entities(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut index = 0;
    while let Some(relative_start) = text[index..].find('&') {
        let start = index + relative_start;
        result.push_str(&text[index..start]);
        let Some(relative_end) = text[start..].find(';') else {
            result.push_str(&text[start..]);
            index = text.len();
            break;
        };
        let end = start + relative_end;
        let entity = &text[start..=end];
        let decoded = match entity {
            "&#x20;" | "&#X20;" | "&nbsp;" => Some(' '),
            "&amp;" => Some('&'),
            "&lt;" => Some('<'),
            "&gt;" => Some('>'),
            "&quot;" => Some('"'),
            "&#39;" | "&apos;" => Some('\''),
            _ => entity
                .strip_prefix("&#x")
                .or_else(|| entity.strip_prefix("&#X"))
                .and_then(|value| value.strip_suffix(';'))
                .and_then(|value| u32::from_str_radix(value, 16).ok())
                .and_then(char::from_u32)
                .or_else(|| entity.strip_prefix("&#").and_then(|value| value.strip_suffix(';')).and_then(|value| value.parse::<u32>().ok()).and_then(char::from_u32)),
        };
        result.push_str(&decoded.map(|value| value.to_string()).unwrap_or_else(|| entity.to_string()));
        index = end + 1;
    }
    if index < text.len() {
        result.push_str(&text[index..]);
    }
    result
}

fn normalize_protocol_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) => *text = decode_protocol_text_entities(text),
        serde_json::Value::Array(items) => items.iter_mut().for_each(normalize_protocol_value),
        serde_json::Value::Object(map) => map.values_mut().for_each(normalize_protocol_value),
        _ => {}
    }
}

fn extract_title_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(decode_protocol_text_entities(text)),
        serde_json::Value::Array(items) => {
            let text = items.iter().filter_map(extract_title_text).filter(|text| !text.trim().is_empty()).collect::<Vec<_>>().join(" ");
            (!text.trim().is_empty()).then_some(text)
        }
        serde_json::Value::Object(map) => {
            ["text", "input_text", "output_text", "content", "part", "clip"]
                .iter()
                .find_map(|key| map.get(*key).and_then(extract_title_text))
        }
        _ => None,
    }
}

fn decode_title_entities(text: &str) -> String {
    decode_protocol_text_entities(text)
}


fn is_thread_title_event(event_type: &str) -> bool {
    let value = event_type.to_lowercase();
    value.contains("thread_name")
        || value.contains("threadname")
        || value.contains("title_updated")
        || value.contains("titleupdated")
        || value.contains("thread_renamed")
        || value.contains("threadrenamed")
}

fn extract_real_user_request(content: &str) -> Option<String> {
    let markers = ["## my request for codex:", "my request for codex:", "user request:", "request:"];
    let lower = content.to_lowercase();
    markers.iter().find_map(|marker| {
        let index = lower.find(marker)?;
        let request = content[index + marker.len()..].trim();
        (!request.is_empty() && !is_log_like_text(request)).then(|| normalize_fallback_title(request))
    })
}

fn valid_title(value: &str) -> Option<String> {
    let normalized = normalize_fallback_title(value);
    if normalized.is_empty() || normalized.eq_ignore_ascii_case("untitled") || is_synthetic_user_message(&normalized) || is_log_like_text(&normalized) {
        return None;
    }
    Some(truncate_text(&normalized, MAX_THREAD_TITLE_CHARS))
}

fn normalize_fallback_title(value: &str) -> String {
    let mut text = decode_title_entities(value)
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("```") && !line.ends_with("```"))
        .collect::<Vec<_>>()
        .join(" ");
    text = text.trim().to_string();
    for prefix in ["### ", "## ", "# "] {
        if text.starts_with(prefix) {
            text = text[prefix.len()..].trim_start().to_string();
            break;
        }
    }
    for (open, close) in [("**", "**"), ("__", "__"), ("~~", "~~"), ("`", "`"), ("*", "*"), ("_", "_")] {
        if text.len() >= open.len() + close.len() && text.starts_with(open) && text.ends_with(close) {
            text = text[open.len()..text.len() - close.len()].trim().to_string();
            break;
        }
    }
    text.chars()
        .map(|character| match character {
            '\u{00a0}' | '\u{2002}' | '\u{2003}' | '\u{2009}' | '\u{3000}' | '\n' | '\r' | '\t' => ' ',
            value => value,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_log_like_text(value: &str) -> bool {
    let lower = value.to_lowercase();
    value.len() > 500 || value.lines().count() > 8 || lower.contains("openjdk 64-bit server vm warning:") || lower.contains("exception in thread") || lower.contains("stack trace")
}

fn is_real_user_text(value: &str) -> bool {
    if is_synthetic_user_message(value) { extract_real_user_request(value).is_some() } else { valid_title(value).is_some() }
}

fn read_thread_id_from_jsonl(path: &Path) -> Result<Option<String>, String> {
    let file = fs::File::open(path).map_err(|error| {
        format!(
            "璇诲彇浼氳瘽鏂囦欢澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })?;
    let reader = BufReader::new(file);

    for line in reader.lines().map_while(Result::ok) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
            continue;
        };

        if value.get("type").and_then(|item| item.as_str()) == Some("session_meta") {
            if let Some(payload) = value.get("payload") {
                return Ok(json_string_field(payload, "id"));
            }
        }
    }

    Ok(None)
}

#[allow(dead_code)]
fn remove_threads_from_session_index(thread_ids: &HashSet<String>) -> Result<(), String> {
    if thread_ids.is_empty() {
        return Ok(());
    }

    let path = codex_session_index_path()?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "璇诲彇 session_index.jsonl 澶辫触：{}锛岃矾寰勶細{}",
                error,
                path.display()
            ))
        }
    };
    let mut kept_lines = Vec::new();

    for line in text.lines() {
        let should_remove = serde_json::from_str::<serde_json::Value>(line.trim())
            .ok()
            .and_then(|value| find_session_index_id(&value))
            .map(|id| thread_ids.contains(&id))
            .unwrap_or(false);

        if !should_remove {
            kept_lines.push(line);
        }
    }

    let next_text = if kept_lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", kept_lines.join("\n"))
    };
    fs::write(&path, next_text).map_err(|error| {
        format!(
            "鏇存柊 session_index.jsonl 澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })
}

fn rebuild_session_index_with_sessions(
    sessions: &[ThreadSession],
    move_to_recent: bool,
) -> Result<Option<PathBuf>, String> {
    let path = codex_session_index_path()?;
    ensure_parent_dir(&path)?;
    let restore_time = current_unix_timestamp();
    let mut selected_sessions = sessions
        .iter()
        .filter(|session| !session.id.trim().is_empty())
        .collect::<Vec<_>>();
    selected_sessions.sort_by(|left, right| {
        session_index_updated_at(right)
            .cmp(&session_index_updated_at(left))
            .then_with(|| left.title.cmp(&right.title))
    });
    let selected_rank = if move_to_recent {
        selected_sessions
            .into_iter()
            .enumerate()
            .map(|(index, session)| (session.id.clone(), index))
            .collect::<HashMap<_, _>>()
    } else {
        HashMap::new()
    };
    let selected_items = sessions
        .iter()
        .filter(|session| !session.id.trim().is_empty())
        .enumerate()
        .map(|(index, session)| {
            let updated_at = if move_to_recent {
                iso_from_thread_time(
                    Some(restore_time.saturating_sub((index as i64).saturating_mul(60))),
                    Some(
                        restore_time
                            .saturating_sub((index as i64).saturating_mul(60))
                            .saturating_mul(1000),
                    ),
                )
            } else {
                session_index_updated_at(session)
            };
            let mut item = serde_json::Map::new();
            item.insert(
                "id".to_string(),
                serde_json::Value::String(session.id.clone()),
            );
            item.insert(
                "thread_name".to_string(),
                serde_json::Value::String(session.title.clone()),
            );
            item.insert(
                "updated_at".to_string(),
                serde_json::Value::String(updated_at),
            );
            (session.id.clone(), serde_json::Value::Object(item))
        })
        .collect::<HashMap<_, _>>();

    let backup_path = if path.exists() {
        let backup_dir = workspace_backup_sessions_path()?
            .join(format!("single-index-restore-{}", current_log_time()));
        fs::create_dir_all(&backup_dir).map_err(|error| {
            format!(
                "创建 session_index 备份目录失败：{}，路径：{}",
                error,
                backup_dir.display()
            )
        })?;
        let backup_path = backup_dir.join("session_index.before.jsonl");
        fs::copy(&path, &backup_path).map_err(|error| {
            format!(
                "澶囦唤 session_index.jsonl 澶辫触：{}锛岃矾寰勶細{}",
                error,
                backup_path.display()
            )
        })?;
        Some(backup_path)
    } else {
        None
    };

    let existing_text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(format!(
                "璇诲彇 session_index.jsonl 澶辫触：{}锛岃矾寰勶細{}",
                error,
                path.display()
            ))
        }
    };
    let mut next_items = Vec::new();
    let mut seen_ids = HashSet::new();
    for line in existing_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        let Some(id) = find_session_index_id(&value) else {
            continue;
        };
        if seen_ids.insert(id.clone()) {
            if let Some(selected_item) = selected_items.get(&id) {
                next_items.push(selected_item.clone());
            } else {
                next_items.push(value);
            }
        }
    }

    for session in sessions {
        if !seen_ids.insert(session.id.clone()) {
            continue;
        }
        if let Some(selected_item) = selected_items.get(&session.id) {
            next_items.push(selected_item.clone());
        }
    }

    next_items.sort_by(|left, right| {
        let left_rank = find_session_index_id(left).and_then(|id| selected_rank.get(&id).copied());
        let right_rank =
            find_session_index_id(right).and_then(|id| selected_rank.get(&id).copied());
        match (left_rank, right_rank) {
            (Some(left_index), Some(right_index)) => left_index.cmp(&right_index),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => session_index_value_updated_at(right)
                .cmp(&session_index_value_updated_at(left))
                .then_with(|| find_session_index_id(left).cmp(&find_session_index_id(right))),
        }
    });
    let next_lines = next_items
        .into_iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let next_text = if next_lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", next_lines.join("\n"))
    };
    let existing_text = next_text;

    fs::write(&path, existing_text).map_err(|error| {
        format!(
            "鏇存柊 session_index.jsonl 澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })?;

    Ok(backup_path)
}

fn backup_codex_thread_state() -> Result<Option<PathBuf>, String> {
    let backup_dir =
        workspace_backup_sessions_path()?.join(format!("sidebar-recovery-{}", current_log_time()));
    fs::create_dir_all(&backup_dir).map_err(|error| {
        format!(
            "create recovery backup dir failed: {}, path: {}",
            error,
            backup_dir.display()
        )
    })?;

    let index_path = codex_session_index_path()?;
    if index_path.exists() {
        fs::copy(&index_path, backup_dir.join("session_index.before.jsonl")).map_err(|error| {
            format!(
                "backup session_index.jsonl failed: {}, path: {}",
                error,
                index_path.display()
            )
        })?;
    }

    let global_state_path = codex_global_state_path()?;
    if global_state_path.exists() {
        fs::copy(
            &global_state_path,
            backup_dir.join("codex-global-state.before.json"),
        )
        .map_err(|error| {
            format!(
                "backup .codex-global-state.json failed: {}, path: {}",
                error,
                global_state_path.display()
            )
        })?;
    }

    let global_state_backup_path = codex_global_state_backup_path()?;
    if global_state_backup_path.exists() {
        fs::copy(
            &global_state_backup_path,
            backup_dir.join("codex-global-state.before.json.bak"),
        )
        .map_err(|error| {
            format!(
                "backup .codex-global-state.json.bak failed: {}, path: {}",
                error,
                global_state_backup_path.display()
            )
        })?;
    }

    let db_path = codex_state_db_path()?;
    if db_path.exists() {
        fs::copy(&db_path, backup_dir.join("state_5.before.sqlite")).map_err(|error| {
            format!(
                "backup state_5.sqlite failed: {}, path: {}",
                error,
                db_path.display()
            )
        })?;
    }

    Ok(Some(backup_dir))
}

fn open_codex_state_db() -> Result<Connection, String> {
    let path = codex_state_db_path()?;
    if !path.exists() {
        return Err(format!(
            "Codex state database not found: {}",
            path.display()
        ));
    }

    Connection::open(&path).map_err(|error| {
        format!(
            "open state_5.sqlite failed: {}, path: {}",
            error,
            path.display()
        )
    })
}

fn checkpoint_codex_state_db() -> Result<(), String> {
    let connection = open_codex_state_db()?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| format!("set SQLite busy timeout failed: {}", error))?;
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|error| format!("刷新 Codex state_5.sqlite WAL 失败：{}", error))
}

fn load_restorable_sidebar_rows(ids: &HashSet<String>) -> Result<Vec<SidebarThreadRow>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let connection = open_codex_state_db()?;
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!(
        "SELECT id, COALESCE(NULLIF(title, ''), NULLIF(first_user_message, ''), id) AS title, COALESCE(cwd, '') AS cwd, \
                COALESCE(NULLIF(title, ''), NULLIF(first_user_message, ''), id) AS prompt_history_text \
         FROM threads \
         WHERE id IN ({})",
        placeholders,
    );
    let params = ids.iter().map(String::as_str).collect::<Vec<_>>();
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("prepare sidebar thread query failed: {}", error))?;
    let rows = statement
        .query_map(params_from_iter(params), |row| {
            Ok(SidebarThreadRow {
                id: row.get(0)?,
                title: row.get(1)?,
                cwd: normalize_codex_cwd(&row.get::<_, String>(2)?),
                prompt_history_text: row.get(1)?,
            })
        })
        .map_err(|error| format!("query sidebar threads failed: {}", error))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read sidebar thread rows failed: {}", error))
}

fn load_active_sqlite_sidebar_rows() -> Result<Vec<SidebarThreadRow>, String> {
    let connection = open_codex_state_db()?;
    let mut statement = connection
        .prepare(
        "SELECT id, COALESCE(NULLIF(title, ''), NULLIF(first_user_message, ''), id) AS title, COALESCE(cwd, '') AS cwd, \
                    COALESCE(NULLIF(title, ''), NULLIF(first_user_message, ''), id) AS prompt_history_text \
             FROM threads \
             WHERE COALESCE(archived, 0) = 0 \
               AND COALESCE(thread_source, '') <> 'subagent' \
             ORDER BY COALESCE(updated_at_ms, updated_at * 1000) DESC, id ASC",
        )
        .map_err(|error| format!("prepare active sqlite sidebar query failed: {}", error))?;
    let rows = statement
        .query_map([], |row| {
            Ok(SidebarThreadRow {
                id: row.get(0)?,
                title: row.get(1)?,
                cwd: normalize_codex_cwd(&row.get::<_, String>(2)?),
                prompt_history_text: row.get(1)?,
            })
        })
        .map_err(|error| format!("query active sqlite sidebar rows failed: {}", error))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read active sqlite sidebar rows failed: {}", error))
}

fn upsert_missing_sqlite_threads_from_sessions(
    sessions: &[ThreadSession],
    provider: &str,
) -> Result<usize, String> {
    if sessions.is_empty() {
        return Ok(0);
    }

    let connection = open_codex_state_db()?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| format!("set SQLite busy timeout failed: {}", error))?;
    let columns = sqlite_table_columns(&connection, "threads")?;
    if !columns.contains("id") {
        return Err("Codex state_5.sqlite threads 表缺少 id 字段，无法补齐缺失会话。".to_string());
    }

    let mut inserted_count = 0usize;
    let mut seen_ids = HashSet::new();
    for session in sessions {
        if session.id.trim().is_empty()
            || !seen_ids.insert(session.id.clone())
            || sqlite_thread_exists(&connection, &session.id)?
        {
            continue;
        }

        let mut names = Vec::new();
        let mut values = Vec::new();
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "id",
            SqlValue::Text(session.id.clone()),
        );
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "title",
            SqlValue::Text(session.title.clone()),
        );
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "first_user_message",
            SqlValue::Text(
                session
                    .first_user_text
                    .clone()
                    .unwrap_or_else(|| session.title.clone()),
            ),
        );
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "preview",
            SqlValue::Text(
                session
                    .first_user_text
                    .clone()
                    .unwrap_or_else(|| session.title.clone()),
            ),
        );
        if let Some(cwd) = session
            .cwd
            .as_deref()
            .map(sqlite_codex_cwd)
            .filter(|value| !value.trim().is_empty())
        {
            push_sqlite_thread_value(
                &columns,
                &mut names,
                &mut values,
                "cwd",
                SqlValue::Text(cwd),
            );
        }
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "rollout_path",
            SqlValue::Text(sqlite_codex_path(&session.file_path)),
        );
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "source",
            SqlValue::Text("vscode".to_string()),
        );
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "thread_source",
            SqlValue::Text("user".to_string()),
        );
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "has_user_event",
            SqlValue::Integer(1),
        );
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "archived",
            SqlValue::Integer(0),
        );
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "archived_at",
            SqlValue::Null,
        );
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "model_provider",
            SqlValue::Text(provider.to_string()),
        );

        let updated_at =
            session_updated_at_unix_seconds(session).unwrap_or_else(current_unix_timestamp);
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "updated_at",
            SqlValue::Integer(updated_at),
        );
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "updated_at_ms",
            SqlValue::Integer(updated_at.saturating_mul(1000)),
        );
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "created_at",
            SqlValue::Integer(updated_at),
        );
        push_sqlite_thread_value(
            &columns,
            &mut names,
            &mut values,
            "created_at_ms",
            SqlValue::Integer(updated_at.saturating_mul(1000)),
        );

        let placeholders = vec!["?"; names.len()].join(",");
        let sql = format!(
            "INSERT OR IGNORE INTO threads ({}) VALUES ({})",
            names.join(","),
            placeholders
        );
        inserted_count += connection
            .execute(&sql, params_from_iter(values.iter()))
            .map_err(|error| {
                format!(
                    "insert missing sqlite thread failed: {}, id: {}",
                    error, session.id
                )
            })?;
    }

    Ok(inserted_count)
}

fn sync_selected_sqlite_thread_metadata(
    sessions: &[ThreadSession],
    provider: &str,
    move_to_recent: bool,
) -> Result<usize, String> {
    if sessions.is_empty() {
        return Ok(0);
    }

    let connection = open_codex_state_db()?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| format!("set SQLite busy timeout failed: {}", error))?;
    let columns = sqlite_table_columns(&connection, "threads")?;
    let mut changed_count = 0usize;
    let mut seen_ids = HashSet::new();
    let restore_time = current_unix_timestamp();

    for (index, session) in sessions.iter().enumerate() {
        if session.id.trim().is_empty() || !seen_ids.insert(session.id.clone()) {
            continue;
        }

        let updated_at_ms = if move_to_recent {
            restore_time
                .saturating_mul(1000)
                .saturating_sub(index as i64)
        } else {
            session_updated_at_unix_seconds(session)
                .unwrap_or(restore_time)
                .saturating_mul(1000)
        };
        let updated_at = updated_at_ms / 1000;
        let mut assignments = Vec::new();
        let mut values = Vec::new();
        push_sqlite_thread_assignment(
            &columns,
            &mut assignments,
            &mut values,
            "model_provider",
            SqlValue::Text(provider.to_string()),
        );
        push_sqlite_thread_assignment(
            &columns,
            &mut assignments,
            &mut values,
            "archived",
            SqlValue::Integer(0),
        );
        push_sqlite_thread_assignment(
            &columns,
            &mut assignments,
            &mut values,
            "archived_at",
            SqlValue::Null,
        );
        push_sqlite_thread_assignment(
            &columns,
            &mut assignments,
            &mut values,
            "rollout_path",
            SqlValue::Text(sqlite_codex_path(&session.file_path)),
        );
        push_sqlite_thread_assignment(
            &columns,
            &mut assignments,
            &mut values,
            "source",
            SqlValue::Text("vscode".to_string()),
        );
        push_sqlite_thread_assignment(
            &columns,
            &mut assignments,
            &mut values,
            "thread_source",
            SqlValue::Text("user".to_string()),
        );
        push_sqlite_thread_assignment(
            &columns,
            &mut assignments,
            &mut values,
            "has_user_event",
            SqlValue::Integer(1),
        );
        push_sqlite_thread_assignment(
            &columns,
            &mut assignments,
            &mut values,
            "cwd",
            SqlValue::Text(
                session
                    .cwd
                    .as_deref()
                    .map(sqlite_codex_cwd)
                    .unwrap_or_default(),
            ),
        );
        push_sqlite_thread_assignment(
            &columns,
            &mut assignments,
            &mut values,
            "preview",
            SqlValue::Text(
                session
                    .first_user_text
                    .clone()
                    .unwrap_or_else(|| session.title.clone()),
            ),
        );
        push_sqlite_thread_assignment(
            &columns,
            &mut assignments,
            &mut values,
            "title",
            SqlValue::Text(session.title.clone()),
        );
        push_sqlite_thread_assignment(
            &columns,
            &mut assignments,
            &mut values,
            "first_user_message",
            SqlValue::Text(
                session
                    .first_user_text
                    .clone()
                    .unwrap_or_else(|| session.title.clone()),
            ),
        );
        push_sqlite_thread_assignment(
            &columns,
            &mut assignments,
            &mut values,
            "updated_at",
            SqlValue::Integer(updated_at),
        );
        push_sqlite_thread_assignment(
            &columns,
            &mut assignments,
            &mut values,
            "updated_at_ms",
            SqlValue::Integer(updated_at_ms),
        );

        if assignments.is_empty() {
            continue;
        }
        values.push(SqlValue::Text(session.id.clone()));
        let sql = format!("UPDATE threads SET {} WHERE id = ?", assignments.join(", "));
        changed_count += connection
            .execute(&sql, params_from_iter(values.iter()))
            .map_err(|error| {
                format!(
                    "update sqlite thread metadata failed: {}, id: {}",
                    error, session.id
                )
            })?;
        changed_count += connection
            .execute(
                "UPDATE thread_dynamic_tools SET namespace = 'codex_app' WHERE thread_id = ? AND namespace IS NULL",
                [session.id.as_str()],
            )
            .map_err(|error| format!("update sqlite thread dynamic tool namespaces failed: {}, id: {}", error, session.id))?;
    }

    Ok(changed_count)
}

fn sqlite_table_columns(connection: &Connection, table: &str) -> Result<HashSet<String>, String> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({})", table))
        .map_err(|error| format!("read sqlite table schema failed: {}", error))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("query sqlite table schema failed: {}", error))?;
    rows.collect::<Result<HashSet<_>, _>>()
        .map_err(|error| format!("collect sqlite table schema failed: {}", error))
}

fn sqlite_thread_exists(connection: &Connection, id: &str) -> Result<bool, String> {
    let count = connection
        .query_row("SELECT COUNT(1) FROM threads WHERE id = ?", [id], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| format!("query sqlite thread failed: {}, id: {}", error, id))?;
    Ok(count > 0)
}

fn push_sqlite_thread_value(
    columns: &HashSet<String>,
    names: &mut Vec<String>,
    values: &mut Vec<SqlValue>,
    name: &str,
    value: SqlValue,
) {
    if columns.contains(name) {
        names.push(name.to_string());
        values.push(value);
    }
}

fn push_sqlite_thread_assignment(
    columns: &HashSet<String>,
    assignments: &mut Vec<String>,
    values: &mut Vec<SqlValue>,
    name: &str,
    value: SqlValue,
) {
    if columns.contains(name) {
        assignments.push(format!("{} = ?", name));
        values.push(value);
    }
}

fn sqlite_codex_cwd(value: &str) -> String {
    let normalized = normalize_codex_cwd(value);
    if normalized.len() >= 3 && normalized.as_bytes().get(1) == Some(&b':') {
        format!("\\\\?\\{}", normalized)
    } else {
        normalized
    }
}

fn sqlite_codex_path(value: &str) -> String {
    let normalized = normalize_codex_cwd(value);
    if normalized.starts_with("\\\\?\\") {
        normalized
    } else if normalized.len() >= 3 && normalized.as_bytes().get(1) == Some(&b':') {
        format!("\\\\?\\{}", normalized)
    } else {
        normalized
    }
}

fn thread_session_state_needs_repair(
    path: &Path,
    id: &str,
    cwd: Option<&str>,
    archived: bool,
    index_info: &SessionIndexInfo,
) -> bool {
    if archived || id.trim().is_empty() {
        return false;
    }

    let Some(sqlite_thread) = index_info.sqlite_threads.get(id) else {
        return true;
    };
    if sqlite_thread.archived != 0 {
        return true;
    }

    if let Some(cwd) = cwd.map(str::trim).filter(|value| !value.is_empty()) {
        if !normalize_codex_cwd(&sqlite_thread.cwd).eq_ignore_ascii_case(&normalize_codex_cwd(cwd))
        {
            return true;
        }
    }

    let expected_rollout_path = sqlite_codex_path(&path.display().to_string());
    if sqlite_thread.rollout_path != expected_rollout_path {
        return true;
    }

    if let Some(title) = index_info
        .titles
        .get(id)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let prompt_history_text = index_info
            .prompt_history
            .get(id)
            .map(String::as_str)
            .unwrap_or_default()
            .trim();
        if prompt_history_text != title {
            return true;
        }
    }

    false
}

fn responses_tool_summary_for_log(payload: &serde_json::Value) -> String {
    fn summarize(tool: &serde_json::Value, parent: Option<&str>) -> String {
        if let Some(name) = tool.as_str() {
            return format!("string:{}", truncate_text(name, 80));
        }
        let tool_type = tool
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let name = extract_tool_definition_name(tool).unwrap_or("-");
        let namespace = tool_namespace_name(tool).or(parent).unwrap_or("-");
        let children = tool
            .get("tools")
            .or_else(|| tool.get("functions"))
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .take(20)
                    .map(|item| summarize(item, Some(namespace)))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        if children.is_empty() {
            format!(
                "type={},name={},namespace={}",
                truncate_text(tool_type, 40),
                truncate_text(name, 80),
                truncate_text(namespace, 80)
            )
        } else {
            format!(
                "type={},name={},namespace={},children=[{}]",
                truncate_text(tool_type, 40),
                truncate_text(name, 80),
                truncate_text(namespace, 80),
                children
            )
        }
    }

    payload
        .get("tools")
        .and_then(|value| value.as_array())
        .map(|tools| {
            tools
                .iter()
                .take(50)
                .map(|tool| summarize(tool, None))
                .collect::<Vec<_>>()
                .join(";")
        })
        .unwrap_or_else(|| "none".to_string())
}

fn session_updated_at_unix_seconds(session: &ThreadSession) -> Option<i64> {
    let value = session
        .updated_at
        .as_deref()
        .or(session.created_at.as_deref())?
        .trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(seconds) = value.parse::<i64>() {
        return Some(seconds);
    }
    OffsetDateTime::parse(value, &Rfc3339)
        .ok()
        .map(|time| time.unix_timestamp())
}

fn sync_all_threads_provider(provider: &str) -> Result<usize, String> {
    if provider.trim().is_empty() {
        return Ok(0);
    }

    let connection = match open_codex_state_db() {
        Ok(connection) => connection,
        Err(error) if is_sqlite_malformed_error(&error) => {
            quarantine_malformed_codex_state_db(&error)?;
            return Ok(0);
        }
        Err(error) => return Err(error),
    };
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| format!("set SQLite busy timeout failed: {}", error))?;

    match connection.execute(
        "UPDATE threads SET model_provider = ? WHERE COALESCE(model_provider, '') <> ?",
        [provider, provider],
    ) {
        Ok(count) => Ok(count),
        Err(error) if is_sqlite_malformed_error(&error.to_string()) => {
            quarantine_malformed_codex_state_db(&error.to_string())?;
            Ok(0)
        }
        Err(error) => Err(format!("update all thread providers failed: {}", error)),
    }
}

fn is_sqlite_malformed_error(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("database disk image is malformed")
        || normalized.contains("sqlite_corrupt")
        || normalized.contains("malformed")
}

fn quarantine_malformed_codex_state_db(reason: &str) -> Result<(), String> {
    let path = codex_state_db_path()?;
    if !path.exists() {
        return Ok(());
    }

    let backup_dir = workspace_backup_sessions_path()?
        .join(format!("malformed-state-db-{}", current_log_time()));
    fs::create_dir_all(&backup_dir).map_err(|error| {
        format!(
            "create malformed state db backup dir failed: {}, path: {}",
            error,
            backup_dir.display()
        )
    })?;
    let target = backup_dir.join("state_5.malformed.sqlite");
    fs::rename(&path, &target)
        .or_else(|_| {
            fs::copy(&path, &target)?;
            fs::remove_file(&path)
        })
        .map_err(|error| {
            format!(
                "quarantine malformed state_5.sqlite failed: {}, path: {}, reason: {}",
                error,
                path.display(),
                reason
            )
        })?;

    append_internal_app_log(
        "warn",
        "threads",
        "quarantine-state-db",
        "state_5.sqlite is malformed and has been moved aside.",
        Some(format!(
            "backupPath={}, reason={}",
            target.display(),
            reason
        )),
    );
    Ok(())
}

fn repair_rollout_sessions(
    sessions: &[ThreadSession],
    provider: &str,
    repair_id: bool,
    move_to_recent: bool,
) -> Result<usize, String> {
    if sessions.is_empty() || provider.trim().is_empty() {
        return Ok(0);
    }

    let mut changed_count = 0usize;
    for session in sessions {
        let path = PathBuf::from(&session.file_path);
        if !path.exists() {
            continue;
        }
        if repair_id {
            backup_rollout_session_file(&path, &session.id)?;
        }
        let original_modified = if move_to_recent {
            None
        } else {
            fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .ok()
        };
        if rewrite_rollout_session_meta(&path, session, provider, repair_id)? {
            if let Some(modified) = original_modified {
                restore_file_modified_time(&path, modified)?;
            }
            changed_count += 1;
        }
    }

    Ok(changed_count)
}

fn restore_file_modified_time(path: &Path, modified: SystemTime) -> Result<(), String> {
    let file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|error| {
            format!(
                "open rollout file for mtime restore failed: {}, path: {}",
                error,
                path.display()
            )
        })?;
    file.set_times(FileTimes::new().set_modified(modified))
        .map_err(|error| {
            format!(
                "restore rollout file mtime failed: {}, path: {}",
                error,
                path.display()
            )
        })
}

fn touch_rollout_session_files(sessions: &[ThreadSession]) -> Result<usize, String> {
    let mut changed_count = 0usize;

    for session in sessions {
        let path = PathBuf::from(&session.file_path);
        if !path.exists() {
            continue;
        }
        let content = fs::read(&path).map_err(|error| {
            format!(
                "read rollout file for mtime refresh failed: {}, path: {}",
                error,
                path.display()
            )
        })?;
        fs::write(&path, content).map_err(|error| {
            format!(
                "refresh rollout file mtime failed: {}, path: {}",
                error,
                path.display()
            )
        })?;
        changed_count += 1;
    }

    Ok(changed_count)
}

fn backup_rollout_session_file(path: &Path, thread_id: &str) -> Result<Option<PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let backup_dir =
        workspace_backup_sessions_path()?.join(format!("rollout-repair-{}", current_log_time()));
    fs::create_dir_all(&backup_dir).map_err(|error| {
        format!(
            "create rollout repair backup dir failed: {}, path: {}",
            error,
            backup_dir.display()
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("session.jsonl");
    let backup_path = backup_dir.join(format!("{}-{}", thread_id, file_name));
    fs::copy(path, &backup_path).map_err(|error| {
        format!(
            "backup rollout file failed: {}, path: {}",
            error,
            backup_path.display()
        )
    })?;
    Ok(Some(backup_path))
}

fn collect_all_thread_sessions() -> Result<Vec<ThreadSession>, String> {
    let index_info = load_session_index_info().unwrap_or_default();
    let mut sessions = Vec::new();
    scan_thread_root(
        &codex_sessions_path()?,
        "sessions",
        false,
        &index_info,
        &mut sessions,
    )?;
    scan_thread_root(
        &codex_archived_sessions_path()?,
        "archived_sessions",
        true,
        &index_info,
        &mut sessions,
    )?;
    Ok(sessions)
}

fn sync_codex_history_provider(
    provider: &str,
    rebuild_sidebar: bool,
    move_to_recent: bool,
) -> Result<usize, String> {
    let provider = provider.trim();
    if provider.is_empty() {
        return Ok(0);
    }

    backup_codex_thread_state()?;
    sync_all_threads_provider(provider)?;
    checkpoint_codex_state_db()?;
    let sidebar_rows = load_active_sqlite_sidebar_rows()?;
    let sqlite_ids = sidebar_rows
        .iter()
        .map(|row| row.id.clone())
        .collect::<HashSet<_>>();
    let sessions = collect_all_thread_sessions()?;
    let sqlite_sessions = sessions
        .into_iter()
        .filter(|session| sqlite_ids.contains(&session.id))
        .filter(|session| !is_subagent_thread_session(session))
        .collect::<Vec<_>>();
    repair_rollout_sessions(&sqlite_sessions, provider, false, move_to_recent)?;
    if move_to_recent {
        touch_rollout_session_files(&sqlite_sessions)?;
    }
    let restored_count = rebuild_session_index_from_sqlite()?;

    if rebuild_sidebar {
        rebuild_global_state_for_sidebar_rows(&sidebar_rows, false, false, move_to_recent)?;
    }

    Ok(restored_count)
}

fn rewrite_rollout_session_meta(
    path: &Path,
    session: &ThreadSession,
    provider: &str,
    repair_id: bool,
) -> Result<bool, String> {
    let text = fs::read_to_string(path).map_err(|error| {
        format!(
            "read rollout file failed: {}, path: {}",
            error,
            path.display()
        )
    })?;
    let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let target_cwd = session.cwd.as_deref().map(normalize_codex_cwd);
    let mut changed = false;
    let mut next_lines = Vec::new();

    for line in text.lines() {
        let mut value = match serde_json::from_str::<serde_json::Value>(line.trim_end_matches('\r'))
        {
            Ok(value) => value,
            Err(_) => {
                next_lines.push(line.to_string());
                continue;
            }
        };

        if value.get("type").and_then(|item| item.as_str()) == Some("session_meta") {
            let payload = value
                .get_mut("payload")
                .and_then(|item| item.as_object_mut())
                .ok_or_else(|| {
                    format!(
                        "rollout session_meta missing payload object: {}",
                        path.display()
                    )
                })?;
            let before = serde_json::Value::Object(payload.clone());

            if repair_id {
                payload.insert(
                    "id".to_string(),
                    serde_json::Value::String(session.id.clone()),
                );
            }
            if let Some(cwd) = target_cwd.as_ref().filter(|cwd| !cwd.trim().is_empty()) {
                payload.insert("cwd".to_string(), serde_json::Value::String(cwd.clone()));
            }
            payload.insert(
                "model_provider".to_string(),
                serde_json::Value::String(provider.to_string()),
            );

            if before != serde_json::Value::Object(payload.clone()) {
                changed = true;
            }
        }

        next_lines.push(value.to_string());
    }

    if !changed {
        return Ok(false);
    }

    let next_text = if next_lines.is_empty() {
        String::new()
    } else {
        format!("{}{}", next_lines.join(line_ending), line_ending)
    };
    fs::write(path, next_text).map_err(|error| {
        format!(
            "write rollout file failed: {}, path: {}",
            error,
            path.display()
        )
    })?;
    Ok(true)
}

fn rebuild_session_index_from_sqlite() -> Result<usize, String> {
    let connection = open_codex_state_db()?;
    let mut statement = connection
        .prepare(
            "SELECT id, COALESCE(NULLIF(title, ''), NULLIF(first_user_message, ''), id) AS title, updated_at, updated_at_ms \
             FROM threads \
             WHERE COALESCE(archived, 0) = 0 \
               AND COALESCE(thread_source, '') <> 'subagent' \
             ORDER BY COALESCE(updated_at_ms, updated_at * 1000) DESC, id ASC",
        )
        .map_err(|error| format!("prepare rebuild session_index query failed: {}", error))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2).ok().flatten(),
                row.get::<_, Option<i64>>(3).ok().flatten(),
            ))
        })
        .map_err(|error| format!("query session_index rows failed: {}", error))?;

    let mut lines = Vec::new();
    for row in rows {
        let (id, title, updated_at, updated_at_ms) =
            row.map_err(|error| format!("read session_index row failed: {}", error))?;
        lines.push(
            serde_json::json!({
                "id": id,
                "thread_name": title,
                "updated_at": iso_from_thread_time(updated_at, updated_at_ms),
            })
            .to_string(),
        );
    }

    let path = codex_session_index_path()?;
    ensure_parent_dir(&path)?;
    let text = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };
    fs::write(&path, text).map_err(|error| {
        format!(
            "write rebuilt session_index.jsonl failed: {}, path: {}",
            error,
            path.display()
        )
    })?;
    Ok(lines.len())
}

fn is_subagent_thread_session(session: &ThreadSession) -> bool {
    session.thread_source.as_deref() == Some("subagent")
}

fn merge_sidebar_rows_for_restore(
    sessions: &[ThreadSession],
    sqlite_rows: Vec<SidebarThreadRow>,
) -> Vec<SidebarThreadRow> {
    let mut sqlite_by_id = sqlite_rows
        .into_iter()
        .filter(|row| !row.id.trim().is_empty())
        .map(|row| (row.id.clone(), row))
        .collect::<HashMap<_, _>>();
    let mut session_refs = sessions
        .iter()
        .filter(|session| !session.id.trim().is_empty())
        .collect::<Vec<_>>();

    session_refs.sort_by(|left, right| {
        session_index_updated_at(right)
            .cmp(&session_index_updated_at(left))
            .then_with(|| left.title.cmp(&right.title))
    });

    let mut seen_ids = HashSet::new();
    let mut rows = Vec::new();
    for session in session_refs {
        if !seen_ids.insert(session.id.clone()) {
            continue;
        }

        let row = sqlite_by_id
            .remove(&session.id)
            .unwrap_or_else(|| SidebarThreadRow {
                id: session.id.clone(),
                title: session.title.clone(),
                cwd: session
                    .cwd
                    .as_deref()
                    .map(normalize_codex_cwd)
                    .unwrap_or_default(),
                prompt_history_text: session.title.clone(),
            });
        rows.push(row);
    }

    rows
}

fn restore_project_roots_for_display(sessions: &[ThreadSession]) -> Vec<String> {
    unique_strings(
        sessions
            .iter()
            .filter_map(|session| session.cwd.as_deref())
            .map(sidebar_project_cwd)
            .filter(|cwd| !cwd.trim().is_empty())
            .collect(),
    )
}

fn session_index_updated_at(session: &ThreadSession) -> String {
    session
        .updated_at
        .as_deref()
        .and_then(iso_from_session_time_text)
        .or_else(|| {
            session
                .created_at
                .as_deref()
                .and_then(iso_from_session_time_text)
        })
        .unwrap_or_else(current_auth_refresh_time)
}

fn iso_from_session_time_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.len() >= 10 && trimmed.as_bytes().get(4) == Some(&b'-') {
        return Some(trimmed.to_string());
    }
    let seconds = trimmed.parse::<i64>().ok()?;
    OffsetDateTime::from_unix_timestamp(seconds)
        .ok()?
        .format(&Rfc3339)
        .ok()
}

fn rebuild_global_state_for_sidebar_rows(
    rows: &[SidebarThreadRow],
    pin_projects: bool,
    pin_threads: bool,
    move_to_recent: bool,
) -> Result<(), String> {
    if rows.is_empty() {
        return Ok(());
    }

    let path = codex_global_state_path()?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => EMPTY_JSON_OBJECT_CONTENT.to_string(),
        Err(error) => {
            return Err(format!(
                "read .codex-global-state.json failed: {}, path: {}",
                error,
                path.display()
            ))
        }
    };
    let mut state =
        serde_json::from_str::<serde_json::Value>(&text).unwrap_or_else(|_| serde_json::json!({}));
    ensure_json_object(&mut state);

    let selected_ids = rows.iter().map(|row| row.id.clone()).collect::<Vec<_>>();
    let selected_projects = unique_strings(
        rows.iter()
            .map(|row| sidebar_project_cwd(&row.cwd))
            .filter(|cwd| !cwd.trim().is_empty())
            .collect::<Vec<_>>(),
    );
    let selected_project_variants = project_path_variants(&selected_projects);
    let selected_project_thread_ids = rows
        .iter()
        .filter(|row| !sidebar_project_cwd(&row.cwd).trim().is_empty())
        .map(|row| row.id.clone())
        .collect::<Vec<_>>();
    let selected_projectless_thread_ids = rows
        .iter()
        .filter(|row| sidebar_project_cwd(&row.cwd).trim().is_empty())
        .map(|row| row.id.clone())
        .collect::<Vec<_>>();

    {
        let root = state
            .as_object_mut()
            .ok_or_else(|| ".codex-global-state.json root is not an object".to_string())?;
        let orders = root
            .entry("sidebar-project-thread-orders".to_string())
            .or_insert_with(|| serde_json::json!({}));
        ensure_json_object(orders);
        let orders_object = orders
            .as_object_mut()
            .ok_or_else(|| "sidebar-project-thread-orders is not an object".to_string())?;
        let mut by_project: HashMap<String, Vec<String>> = HashMap::new();
        let selected_id_set = selected_ids.iter().cloned().collect::<HashSet<_>>();

        for row in rows {
            let project_cwd = sidebar_project_cwd(&row.cwd);
            if project_cwd.trim().is_empty() {
                continue;
            }
            by_project
                .entry(project_cwd)
                .or_default()
                .push(row.id.clone());
        }

        for selected_id in &selected_id_set {
            for value in orders_object.values_mut() {
                let current = thread_ids_from_order(value);
                *value = thread_order(current.into_iter().filter(|id| id != selected_id).collect());
            }
        }

        for (project, ids) in by_project {
            let variants = project_path_variants(std::slice::from_ref(&project));
            let mut current = Vec::new();
            for variant in &variants {
                current.extend(
                    orders_object
                        .remove(variant)
                        .map(|value| thread_ids_from_order(&value))
                        .unwrap_or_default(),
                );
            }
            current.retain(|id| !selected_id_set.contains(id));
            let next_order = if move_to_recent {
                front_unique(current, ids)
            } else {
                append_unique(current, ids)
            };
            orders_object.insert(project, thread_order(next_order));
        }
    }

    {
        let root = state
            .as_object_mut()
            .ok_or_else(|| ".codex-global-state.json root is not an object".to_string())?;
        let hints = root
            .entry("thread-workspace-root-hints".to_string())
            .or_insert_with(|| serde_json::json!({}));
        ensure_json_object(hints);
        if let Some(hints_object) = hints.as_object_mut() {
            for row in rows {
                let project_cwd = sidebar_project_cwd(&row.cwd);
                if !project_cwd.trim().is_empty() {
                    hints_object.insert(row.id.clone(), serde_json::Value::String(project_cwd));
                } else {
                    hints_object.remove(&row.id);
                }
            }
        }
    }

    upsert_prompt_history_for_sidebar_rows(&mut state, rows)?;

    {
        let root = state
            .as_object_mut()
            .ok_or_else(|| ".codex-global-state.json root is not an object".to_string())?;
        let current_chat_order = root
            .get("sidebar-chat-thread-order")
            .map(thread_ids_from_order)
            .unwrap_or_default();
        root.insert(
            "sidebar-chat-thread-order".to_string(),
            thread_order(if move_to_recent {
                front_unique(current_chat_order, selected_ids.clone())
            } else {
                append_unique(current_chat_order, selected_ids.clone())
            }),
        );

        let custom_sections = root
            .entry("sidebar-custom-sections".to_string())
            .or_insert_with(|| serde_json::json!([]));
        if !custom_sections.is_array() {
            *custom_sections = serde_json::json!([]);
        }

        let projectless = root
            .entry("projectless-thread-ids".to_string())
            .or_insert_with(|| serde_json::json!([]));
        if !projectless.is_array() {
            *projectless = serde_json::json!([]);
        }
        if let Some(projectless_items) = projectless.as_array_mut() {
            let selected_with_project = selected_project_thread_ids
                .iter()
                .cloned()
                .collect::<HashSet<_>>();
            projectless_items.retain(|item| {
                item.as_str()
                    .map(|id| !selected_with_project.contains(id))
                    .unwrap_or(true)
            });
            let current = projectless_items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect::<Vec<_>>();
            let next_projectless_items = if move_to_recent {
                front_unique(current, selected_projectless_thread_ids.clone())
            } else {
                append_unique(current, selected_projectless_thread_ids.clone())
            };
            *projectless_items = next_projectless_items
                .into_iter()
                .map(serde_json::Value::String)
                .collect();
        }

        if pin_projects {
            let current = json_string_array(root.get("pinned-project-ids"));
            root.insert(
                "pinned-project-ids".to_string(),
                serde_json::Value::Array(
                    merge_ordered_unique(current, selected_projects.clone(), move_to_recent)
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        } else if !root
            .get("pinned-project-ids")
            .map(|value| value.is_array())
            .unwrap_or(false)
        {
            root.insert("pinned-project-ids".to_string(), serde_json::json!([]));
        }

        let current_pinned_threads = root
            .get("pinned-thread-ids")
            .map(thread_ids_from_order)
            .unwrap_or_default();
        let pinned_threads = if pin_threads {
            front_unique(current_pinned_threads, selected_ids.clone())
        } else {
            current_pinned_threads
        };
        root.insert(
            "pinned-thread-ids".to_string(),
            thread_order(pinned_threads),
        );

        let saved_roots = normalize_project_root_list(json_string_array(
            root.get("electron-saved-workspace-roots"),
        ));
        root.insert(
            "electron-saved-workspace-roots".to_string(),
            serde_json::Value::Array(
                merge_ordered_unique(saved_roots, selected_projects.clone(), move_to_recent)
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );

        let active_roots =
            normalize_project_root_list(json_string_array(root.get("active-workspace-roots")));
        root.insert(
            "active-workspace-roots".to_string(),
            serde_json::Value::Array(
                merge_ordered_unique(active_roots, selected_projects.clone(), move_to_recent)
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );

        let project_order =
            normalize_project_root_list(json_string_array(root.get("project-order")));
        root.insert(
            "project-order".to_string(),
            serde_json::Value::Array(
                merge_ordered_unique(project_order, selected_projects.clone(), move_to_recent)
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }

    set_sidebar_atom_preferences(&mut state, &selected_project_variants)?;

    let next_text = serde_json::to_string_pretty(&state)
        .map_err(|error| format!("serialize .codex-global-state.json failed: {}", error))?;
    let next_content = format!("{}\n", next_text);
    fs::write(&path, &next_content).map_err(|error| {
        format!(
            "write .codex-global-state.json failed: {}, path: {}",
            error,
            path.display()
        )
    })?;
    sync_codex_global_state_backup_if_present(&next_content)
}

fn sync_codex_global_state_backup_if_present(content: &str) -> Result<(), String> {
    let backup_path = codex_global_state_backup_path()?;
    if !backup_path.exists() {
        return Ok(());
    }

    fs::write(&backup_path, content).map_err(|error| {
        format!(
            "write .codex-global-state.json.bak failed: {}, path: {}",
            error,
            backup_path.display()
        )
    })
}

fn verify_thread_restore_state(sessions: &[ThreadSession]) -> Result<(), String> {
    if sessions.is_empty() {
        return Ok(());
    }

    let sqlite_threads = load_sqlite_thread_state_map()
        .map_err(|error| format!("恢复后校验 sqlite 状态失败：{}", error))?;
    let index_ids = load_session_index_id_set()
        .map_err(|error| format!("恢复后校验 session_index 失败：{}", error))?;
    let global_state_path = codex_global_state_path()?;
    verify_thread_restore_global_state_file(&global_state_path, sessions)?;
    let global_state_backup_path = codex_global_state_backup_path()?;
    if global_state_backup_path.exists() {
        verify_thread_restore_global_state_file(&global_state_backup_path, sessions)?;
    }

    let mut errors = Vec::new();
    for session in sessions {
        if session.archived || session.id.trim().is_empty() {
            continue;
        }

        match sqlite_threads.get(&session.id) {
            Some(thread) => {
                if thread.archived != 0 {
                    errors.push(format!(
                        "{} sqlite archived={}",
                        session.id, thread.archived
                    ));
                }
                if let Some(cwd) = session
                    .cwd
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    let expected_cwd = normalize_codex_cwd(cwd);
                    let actual_cwd = normalize_codex_cwd(&thread.cwd);
                    if !actual_cwd.eq_ignore_ascii_case(&expected_cwd) {
                        errors.push(format!(
                            "{} sqlite cwd 不一致，期望 {}，实际 {}",
                            session.id, expected_cwd, actual_cwd
                        ));
                    }
                }
                let expected_rollout_path = sqlite_codex_path(&session.file_path);
                if thread.rollout_path != expected_rollout_path {
                    errors.push(format!(
                        "{} sqlite rollout_path 不一致，期望 {}，实际 {}",
                        session.id, expected_rollout_path, thread.rollout_path
                    ));
                }
            }
            None => errors.push(format!("{} sqlite threads ȱʧ", session.id)),
        }

        if !index_ids.contains(&session.id) {
            errors.push(format!("{} session_index ȱʧ", session.id));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "恢复写入后校验失败，已取消启动 Codex：{}",
            errors.join("；")
        ))
    }
}

fn reinforce_thread_restore_state_after_codex_start(
    sessions: &[ThreadSession],
    provider: &str,
    selected_ids: &HashSet<String>,
    move_to_recent: bool,
) -> Result<(), String> {
    for delay in [2_u64, 3, 5, 8, 13, 21] {
        thread::sleep(Duration::from_secs(delay));
        sync_selected_sqlite_thread_metadata(sessions, provider, move_to_recent)?;
        checkpoint_codex_state_db()?;
        repair_rollout_sessions(sessions, provider, true, move_to_recent)?;
        if move_to_recent {
            touch_rollout_session_files(sessions)?;
        }
        rebuild_session_index_with_sessions(sessions, move_to_recent)?;
        let sqlite_sidebar_rows = load_restorable_sidebar_rows(selected_ids).unwrap_or_default();
        let sidebar_rows = merge_sidebar_rows_for_restore(sessions, sqlite_sidebar_rows);
        rebuild_global_state_for_sidebar_rows(&sidebar_rows, false, false, move_to_recent)?;
    }
    verify_thread_restore_state(sessions)
}

fn load_session_index_id_set() -> Result<HashSet<String>, String> {
    let path = codex_session_index_path()?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(format!(
                "读取 session_index.jsonl 失败：{}，路径：{}",
                error,
                path.display()
            ))
        }
    };
    let mut ids = HashSet::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(id) = find_session_index_id(&value) {
                ids.insert(id);
            }
        }
    }
    Ok(ids)
}

fn verify_thread_restore_global_state_file(
    path: &Path,
    sessions: &[ThreadSession],
) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|error| {
        format!(
            "读取 global-state 失败：{}，路径：{}",
            error,
            path.display()
        )
    })?;
    let state = serde_json::from_str::<serde_json::Value>(&text).map_err(|error| {
        format!(
            "解析 global-state 失败：{}，路径：{}",
            error,
            path.display()
        )
    })?;
    let chat_ids = state
        .get("sidebar-chat-thread-order")
        .map(thread_ids_from_order)
        .unwrap_or_default();
    let hints = state
        .get("thread-workspace-root-hints")
        .and_then(|value| value.as_object());
    let orders = state
        .get("sidebar-project-thread-orders")
        .and_then(|value| value.as_object());
    let projectless_ids = state
        .get("projectless-thread-ids")
        .map(thread_ids_from_order)
        .unwrap_or_default();
    let prompt_history = thread_prompt_history_from_global_state(&state);
    let mut errors = Vec::new();

    for session in sessions {
        if session.archived || session.id.trim().is_empty() {
            continue;
        }
        if !chat_ids.contains(&session.id) {
            errors.push(format!("{} sidebar-chat-thread-order ȱʧ", session.id));
        }
        if let Some(cwd) = session
            .cwd
            .as_deref()
            .map(normalize_codex_cwd)
            .filter(|value| !value.trim().is_empty())
        {
            let sidebar_cwd = sidebar_project_cwd(&cwd);
            let hinted = hints
                .and_then(|items| items.get(&session.id))
                .and_then(|value| value.as_str())
                .map(normalize_codex_cwd)
                .unwrap_or_default();
            if sidebar_cwd.trim().is_empty() {
                if !hinted.trim().is_empty() {
                    errors.push(format!("{} thread-workspace-root-hints 应为空", session.id));
                }
                if !projectless_ids.contains(&session.id) {
                    errors.push(format!("{} projectless-thread-ids ȱʧ", session.id));
                }
                continue;
            }

            if !hinted.eq_ignore_ascii_case(&sidebar_cwd) {
                errors.push(format!("{} thread-workspace-root-hints 不一致", session.id));
            }

            let project_has_thread = orders
                .and_then(|items| items.get(&sidebar_cwd))
                .map(thread_ids_from_order)
                .map(|ids| ids.contains(&session.id))
                .unwrap_or(false);
            if !project_has_thread {
                errors.push(format!(
                    "{} sidebar-project-thread-orders 缺失项目 {}",
                    session.id, sidebar_cwd
                ));
            }
        }
        let actual_prompt = prompt_history
            .get(&session.id)
            .map(String::as_str)
            .unwrap_or_default()
            .trim();
        if actual_prompt != session.title.trim() {
            errors.push(format!(
                "{} prompt-history 不一致，期望 {}",
                session.id, session.title
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "恢复后校验 {} 失败：{}",
            path.display(),
            errors.join("；")
        ))
    }
}



fn thread_ids_from_order(value: &serde_json::Value) -> Vec<String> {
    if let Some(items) = value.as_array() {
        return items
            .iter()
            .filter_map(|item| item.as_str().map(ToString::to_string))
            .collect();
    }

    value
        .get("threadIds")
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn thread_order(ids: Vec<String>) -> serde_json::Value {
    serde_json::json!({ "threadIds": unique_strings(ids) })
}

fn load_sidebar_thread_ids_from_global_state() -> Result<HashSet<String>, String> {
    let path = codex_global_state_path()?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => EMPTY_JSON_OBJECT_CONTENT.to_string(),
        Err(error) => {
            return Err(format!(
                "read .codex-global-state.json failed: {}, path: {}",
                error,
                path.display()
            ))
        }
    };
    let state =
        serde_json::from_str::<serde_json::Value>(&text).unwrap_or_else(|_| serde_json::json!({}));
    let mut ids = HashSet::new();

    if let Some(orders) = state
        .get("sidebar-project-thread-orders")
        .and_then(|value| value.as_object())
    {
        for order in orders.values() {
            ids.extend(thread_ids_from_order(order));
        }
    }

    ids.extend(thread_ids_from_order(
        state
            .get("sidebar-chat-thread-order")
            .unwrap_or(&serde_json::Value::Null),
    ));
    ids.extend(json_string_array(state.get("projectless-thread-ids")));
    ids.extend(json_string_array(state.get("pinned-thread-ids")));
    Ok(ids)
}

fn load_thread_prompt_history_from_global_state() -> Result<HashMap<String, String>, String> {
    let path = codex_global_state_path()?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => EMPTY_JSON_OBJECT_CONTENT.to_string(),
        Err(error) => {
            return Err(format!(
                "read .codex-global-state.json failed: {}, path: {}",
                error,
                path.display()
            ))
        }
    };
    let state =
        serde_json::from_str::<serde_json::Value>(&text).unwrap_or_else(|_| serde_json::json!({}));
    Ok(thread_prompt_history_from_global_state(&state))
}

fn thread_prompt_history_from_global_state(state: &serde_json::Value) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for atom_key in ["persisted-atom-state", "electron-persisted-atom-state"] {
        let Some(prompt_history) = state
            .get(atom_key)
            .and_then(|atom| atom.get("prompt-history"))
            .and_then(|value| value.as_object())
        else {
            continue;
        };
        for (id, value) in prompt_history {
            if result.contains_key(id) {
                continue;
            }
            let text = value
                .as_array()
                .and_then(|items| items.first())
                .and_then(|item| item.as_str())
                .or_else(|| value.as_str())
                .unwrap_or_default()
                .to_string();
            result.insert(id.clone(), text);
        }
    }
    result
}

fn load_sqlite_thread_state_map() -> Result<HashMap<String, SqliteThreadState>, String> {
    let connection = open_codex_state_db()?;
    let mut statement = connection
        .prepare("SELECT id, COALESCE(title, ''), COALESCE(first_user_message, ''), COALESCE(cwd, ''), COALESCE(rollout_path, ''), COALESCE(archived, 0) FROM threads")
        .map_err(|error| format!("prepare sqlite thread state query failed: {}", error))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                SqliteThreadState {
                    title: row.get(1)?,
                    first_user_message: row.get(2)?,
                    cwd: row.get(3)?,
                    rollout_path: row.get(4)?,
                    archived: row.get(5)?,
                },
            ))
        })
        .map_err(|error| format!("query sqlite thread state failed: {}", error))?;

    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(|error| format!("read sqlite thread state failed: {}", error))
}











fn project_path_variants(projects: &[String]) -> Vec<String> {
    let mut variants = Vec::new();
    for project in projects {
        let normalized = normalize_codex_cwd(project);
        if normalized.trim().is_empty() {
            continue;
        }
        variants.push(normalized.clone());
        if normalized.len() >= 3 && normalized.as_bytes().get(1) == Some(&b':') {
            let lower_drive = format!("{}{}", normalized[..1].to_lowercase(), &normalized[1..]);
            let upper_drive = format!("{}{}", normalized[..1].to_uppercase(), &normalized[1..]);
            variants.push(lower_drive);
            variants.push(upper_drive);
        }
        if normalized.len() >= 3
            && normalized.as_bytes().get(1) == Some(&b':')
            && !normalized.starts_with("\\\\?\\")
        {
            variants.push(format!("\\\\?\\{}", normalized));
        }
    }
    unique_strings(variants)
}

fn normalize_project_root_list(projects: Vec<String>) -> Vec<String> {
    unique_strings(
        projects
            .into_iter()
            .map(|project| normalize_codex_cwd(&project))
            .filter(|project| !project.trim().is_empty())
            .collect(),
    )
}

fn set_sidebar_atom_preferences(
    state: &mut serde_json::Value,
    selected_projects: &[String],
) -> Result<(), String> {
    for key in ["persisted-atom-state", "electron-persisted-atom-state"] {
        let root = state
            .as_object_mut()
            .ok_or_else(|| ".codex-global-state.json root is not an object".to_string())?;
        let atom = root
            .entry(key.to_string())
            .or_insert_with(|| serde_json::json!({}));
        ensure_json_object(atom);
        let atom_object = atom
            .as_object_mut()
            .ok_or_else(|| format!("{} is not an object", key))?;

        atom_object.insert(
            "sidebar-workspace-filter-v2".to_string(),
            serde_json::Value::String("all".to_string()),
        );
        atom_object.insert(
            "sidebar-organize-mode-v1".to_string(),
            serde_json::Value::String("project".to_string()),
        );
        atom_object.insert(
            "sidebar-keep-projects-in-recent-v1".to_string(),
            serde_json::Value::Bool(true),
        );
        atom_object.insert(
            "projectless-sidebar-chats-first-v1".to_string(),
            serde_json::Value::Bool(false),
        );
        atom_object.insert(
            "thread-sort-key".to_string(),
            serde_json::Value::String("updated_at".to_string()),
        );
        atom_object.insert(
            "sidebar-move-updated-threads-in-front-v1".to_string(),
            serde_json::Value::Bool(true),
        );
        atom_object.insert(
            "sidebar-history-show".to_string(),
            serde_json::Value::String("all".to_string()),
        );
        atom_object.insert(
            "sidebar-history-organize".to_string(),
            serde_json::Value::String("project".to_string()),
        );
        atom_object.insert(
            "organize-mode-v1".to_string(),
            serde_json::Value::String("project".to_string()),
        );

        let collapsed_groups = atom_object
            .entry("sidebar-collapsed-groups".to_string())
            .or_insert_with(|| serde_json::json!({}));
        ensure_json_object(collapsed_groups);
        if let Some(groups_object) = collapsed_groups.as_object_mut() {
            for project in selected_projects {
                groups_object.remove(project);
            }
        }

        let collapsed_sections = atom_object
            .entry("sidebar-collapsed-sections-v1".to_string())
            .or_insert_with(|| serde_json::json!({}));
        ensure_json_object(collapsed_sections);
        if let Some(sections_object) = collapsed_sections.as_object_mut() {
            sections_object.insert("chats".to_string(), serde_json::Value::Bool(false));
            sections_object.insert("pinned".to_string(), serde_json::Value::Bool(false));
            sections_object.insert("threads".to_string(), serde_json::Value::Bool(false));
        }
    }

    Ok(())
}

fn upsert_prompt_history_for_sidebar_rows(
    state: &mut serde_json::Value,
    rows: &[SidebarThreadRow],
) -> Result<(), String> {
    for key in ["electron-persisted-atom-state", "persisted-atom-state"] {
        let root = state
            .as_object_mut()
            .ok_or_else(|| ".codex-global-state.json root is not an object".to_string())?;
        let atom = root
            .entry(key.to_string())
            .or_insert_with(|| serde_json::json!({}));
        ensure_json_object(atom);
        let atom_object = atom
            .as_object_mut()
            .ok_or_else(|| format!("{} is not an object", key))?;
        let prompt_history = atom_object
            .entry("prompt-history".to_string())
            .or_insert_with(|| serde_json::json!({}));
        ensure_json_object(prompt_history);

        if let Some(prompt_history_object) = prompt_history.as_object_mut() {
            for row in rows {
                let text = if row.prompt_history_text.trim().is_empty() {
                    row.title.clone()
                } else {
                    row.prompt_history_text.clone()
                };
                upsert_prompt_history_entry(prompt_history_object, &row.id, text);
            }
        }
    }

    Ok(())
}

fn upsert_prompt_history_entry(
    prompt_history_object: &mut serde_json::Map<String, serde_json::Value>,
    id: &str,
    text: String,
) {
    prompt_history_object.insert(id.to_string(), serde_json::json!([text]));
}

fn iso_from_thread_time(updated_at: Option<i64>, updated_at_ms: Option<i64>) -> String {
    let seconds = updated_at_ms.map(|value| value / 1000).or(updated_at);
    let Some(seconds) = seconds else {
        return current_auth_refresh_time();
    };

    OffsetDateTime::from_unix_timestamp(seconds)
        .ok()
        .and_then(|value| value.format(&Rfc3339).ok())
        .unwrap_or_else(current_auth_refresh_time)
}

fn normalize_codex_cwd(value: &str) -> String {
    let trimmed = value.trim();
    trimmed
        .strip_prefix("\\\\?\\")
        .unwrap_or(trimmed)
        .to_string()
}

fn sidebar_project_cwd(value: &str) -> String {
    let normalized = normalize_codex_cwd(value);
    if is_codex_projectless_workspace_root(&normalized) {
        String::new()
    } else {
        normalized
    }
}

fn is_codex_projectless_workspace_root(value: &str) -> bool {
    let normalized = normalize_codex_cwd(value).replace('/', "\\");
    let lower = normalized.to_lowercase();
    let Some((_, rest)) = lower.split_once("\\documents\\codex\\") else {
        return false;
    };
    let mut segments = rest
        .split('\\')
        .filter(|segment| !segment.trim().is_empty());
    let Some(date) = segments.next() else {
        return false;
    };
    let Some(_) = segments.next() else {
        return false;
    };

    date.len() == 10
        && date.as_bytes().get(4) == Some(&b'-')
        && date.as_bytes().get(7) == Some(&b'-')
        && date
            .chars()
            .enumerate()
            .all(|(index, ch)| matches!(index, 4 | 7) || ch.is_ascii_digit())
}

fn cleanup_codex_projectless_workspace_roots() -> Result<(), String> {
    cleanup_codex_projectless_roots_from_global_state()?;
    cleanup_codex_projectless_roots_from_config()
}

fn cleanup_codex_projectless_roots_from_global_state() -> Result<(), String> {
    let path = codex_global_state_path()?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "read .codex-global-state.json failed: {}, path: {}",
                error,
                path.display()
            ))
        }
    };
    let mut state =
        serde_json::from_str::<serde_json::Value>(&text).unwrap_or_else(|_| serde_json::json!({}));
    ensure_json_object(&mut state);
    let mut moved_projectless_ids = Vec::new();

    if let Some(root) = state.as_object_mut() {
        for key in [
            "electron-saved-workspace-roots",
            "active-workspace-roots",
            "project-order",
            "pinned-project-ids",
        ] {
            remove_projectless_roots_from_json_array(root.get_mut(key));
        }

        if let Some(orders) = root
            .get_mut("sidebar-project-thread-orders")
            .and_then(|value| value.as_object_mut())
        {
            let projectless_keys = orders
                .keys()
                .filter(|key| is_codex_projectless_workspace_root(key))
                .cloned()
                .collect::<Vec<_>>();
            for key in projectless_keys {
                if let Some(value) = orders.remove(&key) {
                    moved_projectless_ids.extend(thread_ids_from_order(&value));
                }
            }
        }

        if let Some(hints) = root
            .get_mut("thread-workspace-root-hints")
            .and_then(|value| value.as_object_mut())
        {
            hints.retain(|_, value| {
                value
                    .as_str()
                    .map(|cwd| !is_codex_projectless_workspace_root(cwd))
                    .unwrap_or(true)
            });
        }

        if !moved_projectless_ids.is_empty() {
            let projectless = root
                .entry("projectless-thread-ids".to_string())
                .or_insert_with(|| serde_json::json!([]));
            if !projectless.is_array() {
                *projectless = serde_json::json!([]);
            }
            if let Some(items) = projectless.as_array_mut() {
                let current = items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToString::to_string))
                    .collect::<Vec<_>>();
                *items = front_unique(current, moved_projectless_ids)
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect();
            }
        }

        for atom_key in ["electron-persisted-atom-state", "persisted-atom-state"] {
            if let Some(atom) = root
                .get_mut(atom_key)
                .and_then(|value| value.as_object_mut())
            {
                if let Some(groups) = atom
                    .get_mut("sidebar-collapsed-groups")
                    .and_then(|value| value.as_object_mut())
                {
                    groups.retain(|key, _| !is_codex_projectless_workspace_root(key));
                }
            }
        }
    }

    let next_text = serde_json::to_string_pretty(&state)
        .map_err(|error| format!("serialize .codex-global-state.json failed: {}", error))?;
    let next_content = format!("{}\n", next_text);
    if next_content != text {
        fs::write(&path, &next_content).map_err(|error| {
            format!(
                "write .codex-global-state.json failed: {}, path: {}",
                error,
                path.display()
            )
        })?;
        sync_codex_global_state_backup_if_present(&next_content)?;
    }

    Ok(())
}

fn remove_projectless_roots_from_json_array(value: Option<&mut serde_json::Value>) {
    if let Some(items) = value.and_then(|value| value.as_array_mut()) {
        items.retain(|item| {
            item.as_str()
                .map(|path| !is_codex_projectless_workspace_root(path))
                .unwrap_or(true)
        });
    }
}

fn cleanup_codex_projectless_roots_from_config() -> Result<(), String> {
    let path = codex_config_path()?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "读取 Codex config.toml 失败：{}，路径：{}",
                error,
                path.display()
            ))
        }
    };
    let next_text = remove_projectless_project_blocks_from_toml(&text);

    if next_text != text {
        fs::write(&path, next_text).map_err(|error| {
            format!(
                "写入 Codex config.toml 失败：{}，路径：{}",
                error,
                path.display()
            )
        })?;
    }

    Ok(())
}

fn remove_projectless_project_blocks_from_toml(text: &str) -> String {
    let mut result = String::new();
    let mut skipping_projectless_project = false;

    for line_with_ending in text.split_inclusive('\n') {
        let line = line_with_ending.trim_end_matches(['\r', '\n']);
        let trimmed = line.trim();
        let is_table = trimmed.starts_with('[') && trimmed.ends_with(']');

        if is_table {
            skipping_projectless_project = toml_project_section_path(trimmed)
                .map(|path| is_codex_projectless_workspace_root(&path))
                .unwrap_or(false);
        }

        if !skipping_projectless_project {
            result.push_str(line_with_ending);
        }
    }

    if text.ends_with('\n') || result.is_empty() {
        result
    } else {
        result.push('\n');
        result
    }
}

fn toml_project_section_path(trimmed: &str) -> Option<String> {
    let section = trimmed.strip_prefix('[')?.strip_suffix(']')?;
    let rest = section.strip_prefix("projects.")?;
    Some(unquote_toml(rest))
}

#[allow(dead_code)]
fn remove_threads_from_global_state(thread_ids: &HashSet<String>) -> Result<(), String> {
    if thread_ids.is_empty() {
        return Ok(());
    }

    let path = codex_global_state_path()?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "读取 .codex-global-state.json 失败：{}，路径：{}",
                error,
                path.display()
            ))
        }
    };
    let mut root = serde_json::from_str::<serde_json::Value>(&text).map_err(|error| {
        format!(
            "解析 .codex-global-state.json 失败：{}，路径：{}",
            error,
            path.display()
        )
    })?;

    if let Some(atom_state) = root
        .get_mut("electron-persisted-atom-state")
        .and_then(|value| value.as_object_mut())
    {
        if let Some(prompt_history) = atom_state
            .get_mut("prompt-history")
            .and_then(|value| value.as_object_mut())
        {
            for thread_id in thread_ids {
                prompt_history.remove(thread_id);
            }
        }

        if let Some(permissions) = atom_state
            .get_mut("heartbeat-thread-permissions-by-id")
            .and_then(|value| value.as_object_mut())
        {
            for thread_id in thread_ids {
                permissions.remove(thread_id);
            }
        }
    }

    remove_thread_ids_from_array(root.get_mut("projectless-thread-ids"), thread_ids);

    if let Some(hints) = root
        .get_mut("thread-workspace-root-hints")
        .and_then(|value| value.as_object_mut())
    {
        for thread_id in thread_ids {
            hints.remove(thread_id);
        }
    }

    let next_text = serde_json::to_string(&root)
        .map_err(|error| format!("序列化 .codex-global-state.json 失败：{}", error))?;
    fs::write(&path, &next_text).map_err(|error| {
        format!(
            "更新 .codex-global-state.json 失败：{}，路径：{}",
            error,
            path.display()
        )
    })?;
    sync_codex_global_state_backup_if_present(&next_text)
}

#[allow(dead_code)]
fn remove_thread_ids_from_array(
    value: Option<&mut serde_json::Value>,
    thread_ids: &HashSet<String>,
) {
    if let Some(serde_json::Value::Array(items)) = value {
        items.retain(|item| {
            item.as_str()
                .map(|id| !thread_ids.contains(id))
                .unwrap_or(true)
        });
    }
}

fn load_session_index_info() -> Result<SessionIndexInfo, String> {
    let path = codex_session_index_path()?;
    let mut info = SessionIndexInfo {
        sidebar_ids: load_sidebar_thread_ids_from_global_state().unwrap_or_default(),
        prompt_history: load_thread_prompt_history_from_global_state().unwrap_or_default(),
        sqlite_threads: load_sqlite_thread_state_map().unwrap_or_default(),
        ..SessionIndexInfo::default()
    };
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(info),
        Err(error) => {
            return Err(format!(
                "读取 session_index.jsonl 失败：{}，路径：{}",
                error,
                path.display()
            ))
        }
    };
    let reader = BufReader::new(file);

    for line in reader.lines().map_while(Result::ok) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
            continue;
        };
        let Some(id) = find_session_index_id(&value) else {
            continue;
        };
        info.ids.insert(id.clone());

        if let Some(title) = find_session_index_title(&value) {
            info.titles
                .insert(id, truncate_text(&title, MAX_THREAD_TITLE_CHARS));
        }
    }

    Ok(info)
}

fn find_session_index_id(value: &serde_json::Value) -> Option<String> {
    ["id", "thread_id", "threadId", "session_id", "sessionId"]
        .iter()
        .find_map(|key| json_string_field(value, key))
        .or_else(|| value.get("payload").and_then(find_session_index_id))
}

fn find_session_index_title(value: &serde_json::Value) -> Option<String> {
    ["thread_name", "threadName", "title", "name"]
        .iter()
        .find_map(|key| json_string_field(value, key))
        .filter(|title| !title.trim().is_empty())
        .or_else(|| value.get("payload").and_then(find_session_index_title))
}

fn session_index_value_updated_at(value: &serde_json::Value) -> String {
    ["updated_at", "updatedAt", "created_at", "createdAt"]
        .iter()
        .find_map(|key| json_string_field(value, key))
        .or_else(|| value.get("payload").map(session_index_value_updated_at))
        .unwrap_or_default()
}

fn extract_message_text(payload: &serde_json::Value) -> Option<String> {
    let content = payload.get("content")?;

    match content {
        serde_json::Value::String(text) => Some(text.trim().to_string()),
        serde_json::Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(|item| json_string_field(item, "text"))
                .collect::<Vec<_>>()
                .join(" ");
            if text.trim().is_empty() {
                None
            } else {
                Some(text.trim().to_string())
            }
        }
        _ => None,
    }
}





fn usage_window_from_value(value: &serde_json::Value) -> Option<CodexUsageWindow> {
    if value.is_null() {
        return None;
    }

    let remaining_percent = usage_remaining_percent_from_cached_value(value)?;

    Some(CodexUsageWindow {
        used_percent: 100u8.saturating_sub(remaining_percent.min(100)),
        resets_at: json_number_or_string_field(value, "reset_at")
            .or_else(|| json_number_or_string_field(value, "resetAt"))
            .or_else(|| json_number_or_string_field(value, "resets_at"))
            .or_else(|| json_number_or_string_field(value, "resetsAt")),
        limit_window_seconds: json_u64_number_field(value, "limit_window_seconds")
            .or_else(|| json_u64_number_field(value, "limitWindowSeconds"))
            .or_else(|| json_u64_number_field(value, "window_seconds"))
            .or_else(|| json_u64_number_field(value, "windowSeconds"))
            .or_else(|| {
                json_u64_number_field(value, "window_minutes")
                    .or_else(|| json_u64_number_field(value, "windowMinutes"))
                    .map(|minutes| minutes.saturating_mul(60))
            }),
        reset_after_seconds: json_u64_number_field(value, "reset_after_seconds")
            .or_else(|| json_u64_number_field(value, "resetAfterSeconds")),
    })
}

fn read_accounts_registry() -> Result<serde_json::Value, String> {
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

fn ensure_accounts_registry_file() -> Result<(), String> {
    let registry_path = codex_accounts_registry_path()?;
    if registry_path.exists() {
        return Ok(());
    }

    ensure_parent_dir(&registry_path)?;
    let auth_path = codex_auth_path()?;
    let auth_root = read_json_file_optional(&auth_path);
    let now = current_log_time().parse::<i64>().unwrap_or_default();
    let mut registry = serde_json::Map::new();
    registry.insert(
        "schemaVersion".to_string(),
        serde_json::Value::Number(1.into()),
    );
    registry.insert(
        "owner".to_string(),
        serde_json::Value::String("codexmate".to_string()),
    );
    registry.insert(
        "updatedAt".to_string(),
        serde_json::Value::Number(now.into()),
    );
    registry.insert(
        "activeAccountKey".to_string(),
        serde_json::Value::String(String::new()),
    );
    registry.insert("items".to_string(), serde_json::Value::Array(Vec::new()));

    if let Some(root) = auth_root.as_ref() {
        if let Some(account_key) = build_codex_account_key(root) {
            let snapshots_dir = codex_accounts_snapshots_path()?;
            fs::create_dir_all(&snapshots_dir).map_err(|error| {
                format!(
                    "创建账号快照目录失败：{}，路径：{}",
                    error,
                    snapshots_dir.display()
                )
            })?;
            let snapshot_path = snapshots_dir.join(format!(
                "{}.json",
                sanitize_account_key_for_filename(&account_key)
            ));
            fs::copy(&auth_path, &snapshot_path).map_err(|error| {
                format!(
                    "创建本项目账号快照失败：{}，路径：{}",
                    error,
                    snapshot_path.display()
                )
            })?;

            let email = find_string_by_keys(
                root,
                &[
                    "email",
                    "account_email",
                    "accountEmail",
                    "user_email",
                    "userEmail",
                ],
            )
            .unwrap_or_else(|| "未知账号".to_string());
            let name = find_string_by_keys(
                root,
                &[
                    "name",
                    "display_name",
                    "displayName",
                    "user_name",
                    "userName",
                ],
            )
            .unwrap_or_else(|| email.clone());
            let mut item = serde_json::Map::new();
            item.insert(
                "accountKey".to_string(),
                serde_json::Value::String(account_key.clone()),
            );
            item.insert(
                "snapshotPath".to_string(),
                serde_json::Value::String(snapshot_path.display().to_string()),
            );
            item.insert("email".to_string(), serde_json::Value::String(email));
            item.insert(
                "alias".to_string(),
                serde_json::Value::String(String::new()),
            );
            item.insert(
                "accountName".to_string(),
                serde_json::Value::String("Personal".to_string()),
            );
            item.insert(
                "workspaceName".to_string(),
                serde_json::Value::String("Personal".to_string()),
            );
            item.insert("profileName".to_string(), serde_json::Value::String(name));
            item.insert(
                "plan".to_string(),
                serde_json::Value::String("unknown".to_string()),
            );
            item.insert(
                "authMode".to_string(),
                serde_json::Value::String("chatgpt".to_string()),
            );
            item.insert(
                "hasActiveSubscription".to_string(),
                serde_json::Value::Bool(false),
            );
            item.insert(
                "subscriptionWillRenew".to_string(),
                serde_json::Value::Bool(false),
            );
            item.insert(
                "createdAt".to_string(),
                serde_json::Value::Number(now.into()),
            );
            item.insert(
                "lastUsedAt".to_string(),
                serde_json::Value::Number(now.into()),
            );
            item.insert("lastUsageAt".to_string(), serde_json::Value::Null);
            item.insert("cachedPrimaryWindow".to_string(), serde_json::Value::Null);
            item.insert("cachedSecondaryWindow".to_string(), serde_json::Value::Null);
            registry.insert(
                "activeAccountKey".to_string(),
                serde_json::Value::String(account_key),
            );
            registry.insert(
                "items".to_string(),
                serde_json::Value::Array(vec![serde_json::Value::Object(item)]),
            );
        }
    }

    let text = serde_json::to_string_pretty(&serde_json::Value::Object(registry))
        .map_err(|error| format!("搴忓垪鍖栨湰椤圭洰璐﹀彿 registry 澶辫触：{}", error))?;
    fs::write(&registry_path, text).map_err(|error| {
        format!(
            "写入本项目账号registry 失败：{}，路径：{}",
            error,
            registry_path.display()
        )
    })
}

fn sanitize_account_key_for_filename(account_key: &str) -> String {
    let sanitized = account_key
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();

    if sanitized.trim().is_empty() {
        "account".to_string()
    } else {
        sanitized
    }
}

fn write_accounts_registry(registry: &serde_json::Value) -> Result<(), String> {
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

fn sync_accounts_registry_from_snapshot_dir(
    registry: &mut serde_json::Value,
) -> Result<usize, String> {
    let snapshots_dir = codex_accounts_snapshots_path()?;
    if !snapshots_dir.exists() {
        return Ok(0);
    }

    let entries = fs::read_dir(&snapshots_dir).map_err(|error| {
        format!(
            "读取账号快照目录失败：{}，路径：{}",
            error,
            snapshots_dir.display()
        )
    })?;
    let mut synced_count = 0;

    for entry in entries {
        let entry = entry.map_err(|error| format!("读取账号快照条目失败：{}", error))?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| !extension.eq_ignore_ascii_case("json"))
            .unwrap_or(true)
        {
            continue;
        }

        let Some(snapshot_root) = read_json_file_optional(&path) else {
            continue;
        };
        let Some(account_key) = path
            .file_stem()
            .and_then(|value| value.to_str())
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty())
        else {
            continue;
        };

        if find_registry_account(registry, &account_key).is_some() {
            continue;
        }

        upsert_codex_auth_value_account_with_key(registry, &snapshot_root, &account_key, false)?;
        synced_count += 1;
    }

    Ok(synced_count)
}

fn complete_missing_account_snapshot_id_tokens(
    registry: &serde_json::Value,
) -> Result<usize, String> {
    let Some(items) = registry.get("items").and_then(|value| value.as_array()) else {
        return Ok(0);
    };
    let mut completed_count = 0usize;

    for item in items {
        let Some(snapshot_path_text) = json_string_field(item, "snapshotPath") else {
            continue;
        };
        let snapshot_path = PathBuf::from(snapshot_path_text);
        let Some(mut snapshot_root) = read_json_file_optional(&snapshot_path) else {
            continue;
        };
        let has_refresh_token =
            find_string_by_keys(&snapshot_root, &["refresh_token", "refreshToken"])
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false);
        if has_refresh_token || find_chatgpt_session_cookie_token(&snapshot_root).is_none() {
            continue;
        }
        if !ensure_synthetic_id_token_field(&mut snapshot_root) {
            continue;
        }
        write_account_snapshot_value(&snapshot_path, &snapshot_root)?;
        completed_count += 1;
    }

    Ok(completed_count)
}

fn upsert_current_codex_auth_account(registry: &mut serde_json::Value) -> Result<String, String> {
    let auth_path = codex_auth_path()?;
    if !auth_path.exists() {
        return Err(format!("未找到 Codex auth.json：{}", auth_path.display()));
    }

    let auth_root = read_json_file_optional(&auth_path)
        .ok_or_else(|| format!("无法解析 Codex auth.json：{}", auth_path.display()))?;
    let enriched_auth_root = enrich_codex_auth_identity(auth_root);
    upsert_codex_auth_value_account(registry, &enriched_auth_root, true)
}

#[allow(dead_code)]
fn upsert_current_codex_auth_account_legacy(
    registry: &mut serde_json::Value,
) -> Result<String, String> {
    let auth_path = codex_auth_path()?;
    if !auth_path.exists() {
        return Err(format!("未找到 Codex auth.json：{}", auth_path.display()));
    }

    let auth_root = read_json_file_optional(&auth_path)
        .ok_or_else(|| format!("无法解析 Codex auth.json：{}", auth_path.display()))?;
    let account_key = build_codex_account_key(&auth_root).ok_or_else(|| {
        "当前 Codex 登录缺少 account id / email / access token，无法保存为账号。".to_string()
    })?;

    let snapshots_dir = codex_accounts_snapshots_path()?;
    fs::create_dir_all(&snapshots_dir).map_err(|error| {
        format!(
            "创建账号快照目录失败：{}，路径：{}",
            error,
            snapshots_dir.display()
        )
    })?;
    let snapshot_path = snapshots_dir.join(format!(
        "{}.json",
        sanitize_account_key_for_filename(&account_key)
    ));
    fs::copy(&auth_path, &snapshot_path).map_err(|error| {
        format!(
            "保存账号快照失败：{}，路径：{}",
            error,
            snapshot_path.display()
        )
    })?;

    let now = current_log_time().parse::<i64>().unwrap_or_default();
    let email = find_string_by_keys(
        &auth_root,
        &[
            "email",
            "account_email",
            "accountEmail",
            "user_email",
            "userEmail",
        ],
    )
    .unwrap_or_else(|| "未知账号".to_string());
    let name = find_string_by_keys(
        &auth_root,
        &[
            "name",
            "display_name",
            "displayName",
            "user_name",
            "userName",
        ],
    )
    .unwrap_or_else(|| email.clone());
    let auth_mode = find_string_by_keys(&auth_root, &["auth_mode", "authMode"])
        .unwrap_or_else(|| "chatgpt".to_string());

    let root = registry
        .as_object_mut()
        .ok_or_else(|| "账号 registry 格式无效".to_string())?;
    root.insert(
        "schemaVersion".to_string(),
        serde_json::Value::Number(1.into()),
    );
    root.insert(
        "owner".to_string(),
        serde_json::Value::String("codexmate".to_string()),
    );
    root.insert(
        "updatedAt".to_string(),
        serde_json::Value::Number(now.into()),
    );
    root.insert(
        "activeAccountKey".to_string(),
        serde_json::Value::String(account_key.clone()),
    );

    if !root
        .get("items")
        .map(|value| value.is_array())
        .unwrap_or(false)
    {
        root.insert("items".to_string(), serde_json::Value::Array(Vec::new()));
    }

    let items = root
        .get_mut("items")
        .and_then(|value| value.as_array_mut())
        .ok_or_else(|| "账号 registry 缺少 items".to_string())?;
    let existing_index = items.iter().position(|item| {
        json_string_field(item, "accountKey").as_deref() == Some(account_key.as_str())
    });

    let mut item = existing_index
        .and_then(|index| {
            items
                .get(index)
                .and_then(|value| value.as_object())
                .cloned()
        })
        .unwrap_or_default();

    item.insert(
        "accountKey".to_string(),
        serde_json::Value::String(account_key.clone()),
    );
    item.insert(
        "snapshotPath".to_string(),
        serde_json::Value::String(snapshot_path.display().to_string()),
    );
    item.insert("email".to_string(), serde_json::Value::String(email));
    item.insert("profileName".to_string(), serde_json::Value::String(name));
    item.insert("authMode".to_string(), serde_json::Value::String(auth_mode));
    item.entry("alias".to_string())
        .or_insert_with(|| serde_json::Value::String(String::new()));
    item.entry("accountName".to_string())
        .or_insert_with(|| serde_json::Value::String("Personal".to_string()));
    item.entry("workspaceName".to_string())
        .or_insert_with(|| serde_json::Value::String("Personal".to_string()));
    item.insert(
        "plan".to_string(),
        serde_json::Value::String(
            find_string_by_keys(
                &auth_root,
                &[
                    "plan",
                    "plan_type",
                    "planType",
                    "chatgpt_plan_type",
                    "chatgptPlanType",
                ],
            )
            .unwrap_or_else(|| "unknown".to_string()),
        ),
    );
    item.entry("hasActiveSubscription".to_string())
        .or_insert(serde_json::Value::Bool(false));
    item.entry("subscriptionWillRenew".to_string())
        .or_insert(serde_json::Value::Bool(false));
    item.entry("createdAt".to_string())
        .or_insert_with(|| serde_json::Value::Number(now.into()));
    item.insert(
        "lastUsedAt".to_string(),
        serde_json::Value::Number(now.into()),
    );
    item.entry("lastUsageAt".to_string())
        .or_insert(serde_json::Value::Null);
    item.entry("cachedPrimaryWindow".to_string())
        .or_insert(serde_json::Value::Null);
    item.entry("cachedSecondaryWindow".to_string())
        .or_insert(serde_json::Value::Null);

    if let Some(index) = existing_index {
        items[index] = serde_json::Value::Object(item);
    } else {
        items.push(serde_json::Value::Object(item));
    }

    Ok(account_key)
}

fn stable_text_key(text: &str) -> String {
    let mut hash: u64 = 14695981039346656037;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{:016x}", hash)
}

fn upsert_codex_auth_value_account(
    registry: &mut serde_json::Value,
    auth_root: &serde_json::Value,
    activate: bool,
) -> Result<String, String> {
    let account_key = build_codex_account_key(auth_root).ok_or_else(|| {
        "OAuth 登录结果缺少 account id / email / access token，无法保存为账号。".to_string()
    })?;

    upsert_codex_auth_value_account_with_key(registry, auth_root, &account_key, activate)
}

fn upsert_codex_auth_value_account_with_key(
    registry: &mut serde_json::Value,
    auth_root: &serde_json::Value,
    account_key: &str,
    activate: bool,
) -> Result<String, String> {
    let snapshots_dir = codex_accounts_snapshots_path()?;
    fs::create_dir_all(&snapshots_dir).map_err(|error| {
        format!(
            "创建账号快照目录失败：{}，路径：{}",
            error,
            snapshots_dir.display()
        )
    })?;
    let snapshot_path = snapshots_dir.join(format!(
        "{}.json",
        sanitize_account_key_for_filename(&account_key)
    ));
    let snapshot_text = serde_json::to_string_pretty(auth_root)
        .map_err(|error| format!("序列化账号快照失败：{}", error))?;
    fs::write(&snapshot_path, snapshot_text).map_err(|error| {
        format!(
            "保存账号快照失败：{}，路径：{}",
            error,
            snapshot_path.display()
        )
    })?;

    let now = current_log_time().parse::<i64>().unwrap_or_default();
    let email = find_string_by_keys(
        auth_root,
        &[
            "email",
            "account_email",
            "accountEmail",
            "user_email",
            "userEmail",
        ],
    )
    .unwrap_or_else(|| "未知账号".to_string());
    let name = find_string_by_keys(
        auth_root,
        &[
            "name",
            "display_name",
            "displayName",
            "user_name",
            "userName",
        ],
    )
    .unwrap_or_else(|| email.clone());
    let auth_mode = find_string_by_keys(auth_root, &["auth_mode", "authMode"])
        .unwrap_or_else(|| "chatgpt".to_string());
    let plan = find_string_by_keys(
        auth_root,
        &[
            "plan",
            "plan_type",
            "planType",
            "chatgpt_plan_type",
            "chatgptPlanType",
        ],
    )
    .unwrap_or_else(|| "unknown".to_string());

    let root = registry
        .as_object_mut()
        .ok_or_else(|| "账号 registry 格式无效".to_string())?;
    root.insert(
        "schemaVersion".to_string(),
        serde_json::Value::Number(1.into()),
    );
    root.insert(
        "owner".to_string(),
        serde_json::Value::String("codexmate".to_string()),
    );
    root.insert(
        "updatedAt".to_string(),
        serde_json::Value::Number(now.into()),
    );
    if activate {
        root.insert(
            "activeAccountKey".to_string(),
            serde_json::Value::String(account_key.to_string()),
        );
    } else if !root
        .get("activeAccountKey")
        .map(|value| value.is_string())
        .unwrap_or(false)
    {
        root.insert(
            "activeAccountKey".to_string(),
            serde_json::Value::String(String::new()),
        );
    }

    if !root
        .get("items")
        .map(|value| value.is_array())
        .unwrap_or(false)
    {
        root.insert("items".to_string(), serde_json::Value::Array(Vec::new()));
    }

    let items = root
        .get_mut("items")
        .and_then(|value| value.as_array_mut())
        .ok_or_else(|| "账号 registry 缺少 items".to_string())?;
    let snapshot_path_text = snapshot_path.display().to_string();
    let normalized_email = normalize_email_key(&email);
    let existing_index = items.iter().position(|item| {
        json_string_field(item, "accountKey").as_deref() == Some(account_key)
            || json_string_field(item, "snapshotPath").as_deref()
                == Some(snapshot_path_text.as_str())
            || match normalized_email.as_deref() {
                Some(target_email) => {
                    json_string_field(item, "email")
                        .and_then(|value| normalize_email_key(&value))
                        .as_deref()
                        == Some(target_email)
                }
                None => false,
            }
    });
    let mut item = existing_index
        .and_then(|index| {
            items
                .get(index)
                .and_then(|value| value.as_object())
                .cloned()
        })
        .unwrap_or_default();

    let previous_account_key = item
        .get("accountKey")
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    item.insert(
        "accountKey".to_string(),
        serde_json::Value::String(account_key.to_string()),
    );
    item.insert(
        "snapshotPath".to_string(),
        serde_json::Value::String(snapshot_path_text),
    );
    item.insert("email".to_string(), serde_json::Value::String(email));
    item.insert("profileName".to_string(), serde_json::Value::String(name));
    item.insert("authMode".to_string(), serde_json::Value::String(auth_mode));
    item.entry("alias".to_string())
        .or_insert_with(|| serde_json::Value::String(String::new()));
    item.entry("accountName".to_string())
        .or_insert_with(|| serde_json::Value::String("Personal".to_string()));
    item.entry("workspaceName".to_string())
        .or_insert_with(|| serde_json::Value::String("Personal".to_string()));
    item.insert("plan".to_string(), serde_json::Value::String(plan));
    item.entry("hasActiveSubscription".to_string())
        .or_insert(serde_json::Value::Bool(false));
    item.entry("subscriptionWillRenew".to_string())
        .or_insert(serde_json::Value::Bool(false));
    item.entry("createdAt".to_string())
        .or_insert_with(|| serde_json::Value::Number(now.into()));
    item.insert(
        "lastUsedAt".to_string(),
        serde_json::Value::Number(now.into()),
    );
    item.entry("lastUsageAt".to_string())
        .or_insert(serde_json::Value::Null);
    item.entry("cachedPrimaryWindow".to_string())
        .or_insert(serde_json::Value::Null);
    item.entry("cachedSecondaryWindow".to_string())
        .or_insert(serde_json::Value::Null);

    if let Some(index) = existing_index {
        items[index] = serde_json::Value::Object(item);
    } else {
        items.push(serde_json::Value::Object(item));
    }

    if !activate {
        if let Some(previous_account_key) = previous_account_key {
            if root
                .get("activeAccountKey")
                .and_then(|value| value.as_str())
                == Some(previous_account_key.as_str())
            {
                root.insert(
                    "activeAccountKey".to_string(),
                    serde_json::Value::String(account_key.to_string()),
                );
            }
        }
    }

    Ok(account_key.to_string())
}

fn normalize_email_key(email: &str) -> Option<String> {
    let normalized = email.trim().to_ascii_lowercase();
    if normalized.contains('@') && !normalized.is_empty() {
        Some(normalized)
    } else {
        None
    }
}

fn dedupe_registry_accounts_by_email(registry: &mut serde_json::Value) -> Result<bool, String> {
    let root = registry
        .as_object_mut()
        .ok_or_else(|| "账号 registry 格式无效".to_string())?;
    let active_account_key = root
        .get("activeAccountKey")
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    let mut removed_to_kept_key: HashMap<String, String> = HashMap::new();
    let changed = {
        let items = root
            .get_mut("items")
            .and_then(|value| value.as_array_mut())
            .ok_or_else(|| "账号 registry 缺少 items".to_string())?;

        let original_len = items.len();
        let original_items = std::mem::take(items);
        let mut kept_reversed = Vec::with_capacity(original_items.len());
        let mut email_to_kept_key: HashMap<String, String> = HashMap::new();

        for item in original_items.into_iter().rev() {
            let email_key =
                json_string_field(&item, "email").and_then(|email| normalize_email_key(&email));
            let account_key = json_string_field(&item, "accountKey");

            if let Some(email_key) = email_key {
                if let Some(kept_key) = email_to_kept_key.get(&email_key) {
                    if let Some(account_key) = account_key {
                        removed_to_kept_key.insert(account_key, kept_key.clone());
                    }
                    continue;
                }

                if let Some(account_key) = account_key {
                    email_to_kept_key.insert(email_key, account_key);
                }
            }

            kept_reversed.push(item);
        }

        kept_reversed.reverse();
        *items = kept_reversed;
        items.len() != original_len
    };

    if let Some(active_account_key) = active_account_key {
        if let Some(next_active_key) = removed_to_kept_key.get(&active_account_key) {
            root.insert(
                "activeAccountKey".to_string(),
                serde_json::Value::String(next_active_key.clone()),
            );
        }
    }

    Ok(changed)
}

fn compute_token_expiry(snapshot_root: Option<&serde_json::Value>) -> (Option<String>, bool, bool) {
    let Some(root) = snapshot_root else {
        return (None, false, false);
    };
    let Some(access_token) = find_codex_access_token(root) else {
        return (None, false, false);
    };
    let Some(exp) = decode_jwt_payload(&access_token)
        .as_ref()
        .and_then(|claims| claims.get("exp"))
        .and_then(|value| value.as_u64())
    else {
        return (None, false, false);
    };

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let token_expires_at = OffsetDateTime::from_unix_timestamp(exp as i64)
        .ok()
        .map(format_beijing_timestamp);
    let token_expired = now_secs > exp;
    let token_needs_refresh = !token_expired && exp.saturating_sub(now_secs) < 86_400;

    (token_expires_at, token_needs_refresh, token_expired)
}

fn collect_accounts_from_registry(
    root: &serde_json::Value,
    current_account_id: &Option<String>,
    accounts: &mut Vec<CodexAccount>,
) {
    let Some(items) = root.get("items").and_then(|value| value.as_array()) else {
        return;
    };

    for item in items {
        let account_key = json_string_field(item, "accountKey")
            .unwrap_or_else(|| format!("account-{}", accounts.len() + 1));
        if accounts
            .iter()
            .any(|account| account.account_key == account_key)
        {
            continue;
        }

        let email = json_string_field(item, "email").unwrap_or_else(|| "未知账号".to_string());
        let name = json_string_field(item, "profileName")
            .or_else(|| json_string_field(item, "alias"))
            .or_else(|| json_string_field(item, "accountName"))
            .unwrap_or_else(|| email.clone());
        let snapshot_path = json_string_field(item, "snapshotPath");
        let snapshot_root = snapshot_path
            .as_deref()
            .and_then(|path| read_json_file_optional(Path::new(path)));
        let snapshot_key = build_registry_snapshot_key(item, snapshot_root.as_ref(), &account_key);

        let cached_primary = item
            .get("cachedPrimaryWindow")
            .and_then(usage_window_from_cached_value);
        let cached_secondary = item
            .get("cachedSecondaryWindow")
            .and_then(usage_window_from_cached_value);
        let is_current = current_account_id
            .as_ref()
            .map(|current_id| current_id == &account_key)
            .unwrap_or(false);
        let plan = json_string_field(item, "plan").unwrap_or_else(|| "Unknown".to_string());
        let cached_display_usage =
            display_usage_from_windows(cached_primary.as_ref(), cached_secondary.as_ref());
        let (token_expires_at, token_needs_refresh, token_expired) =
            compute_token_expiry(snapshot_root.as_ref());

        accounts.push(CodexAccount {
            id: account_key.clone(),
            account_key: account_key.clone(),
            email,
            name,
            plan,
            auth_mode: json_string_field(item, "authMode")
                .unwrap_or_else(|| "ChatGPT OAuth".to_string()),
            subscription_status: if item
                .get("hasActiveSubscription")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                "有效订阅".to_string()
            } else {
                "δ֪".to_string()
            },
            workspace_name: json_string_field(item, "workspaceName")
                .unwrap_or_else(|| "Personal".to_string()),
            access_token_mask: snapshot_key,
            is_current,
            five_hour_percent: cached_display_usage.five_hour_percent,
            weekly_percent: cached_display_usage.weekly_percent,
            five_hour_reset_at: cached_display_usage.five_hour_reset_at,
            weekly_reset_at: cached_display_usage.weekly_reset_at,
            expires_at: json_number_or_string_field(item, "subscriptionExpiresAt"),
            auto_renew: item
                .get("subscriptionWillRenew")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            snapshot_path,
            last_used_at: json_number_or_string_field(item, "lastUsedAt"),
            last_usage_at: json_number_or_string_field(item, "lastUsageAt"),
            usage_windows: account_usage_windows_from_cached(
                cached_primary.as_ref(),
                cached_secondary.as_ref(),
            ),
            token_expires_at,
            token_needs_refresh,
            token_expired,
            token_refresh_permanently_failed: item
                .get("tokenRefreshPermanentlyFailed")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
        });
    }
}

fn find_registry_account(
    registry: &serde_json::Value,
    account_key: &str,
) -> Option<serde_json::Value> {
    registry
        .get("items")?
        .as_array()?
        .iter()
        .find(|item| json_string_field(item, "accountKey").as_deref() == Some(account_key))
        .cloned()
}

fn set_registry_account_token_refresh_failed(
    registry: &mut serde_json::Value,
    account_key: &str,
    failed: bool,
) -> Result<(), String> {
    let root = registry
        .as_object_mut()
        .ok_or_else(|| "账号 registry 格式无效".to_string())?;
    root.insert(
        "updatedAt".to_string(),
        serde_json::Value::Number(current_log_time().parse::<i64>().unwrap_or_default().into()),
    );
    let items = root
        .get_mut("items")
        .and_then(|value| value.as_array_mut())
        .ok_or_else(|| "账号 registry 缺少 items".to_string())?;
    let item = items
        .iter_mut()
        .find(|item| json_string_field(item, "accountKey").as_deref() == Some(account_key))
        .ok_or_else(|| "未找到账号 registry 项".to_string())?;
    let map = item
        .as_object_mut()
        .ok_or_else(|| "账号 registry 项格式无效".to_string())?;
    map.insert(
        "tokenRefreshPermanentlyFailed".to_string(),
        serde_json::Value::Bool(failed),
    );
    Ok(())
}

fn update_registry_active_account(
    registry: &mut serde_json::Value,
    account_key: &str,
) -> Result<(), String> {
    let now = current_log_time().parse::<i64>().unwrap_or_default();
    let root = registry
        .as_object_mut()
        .ok_or_else(|| "账号 registry 格式无效".to_string())?;
    root.insert(
        "activeAccountKey".to_string(),
        serde_json::Value::String(account_key.to_string()),
    );
    root.insert(
        "updatedAt".to_string(),
        serde_json::Value::Number(now.into()),
    );

    let items = root
        .get_mut("items")
        .and_then(|value| value.as_array_mut())
        .ok_or_else(|| "账号 registry 缺少 items".to_string())?;
    for item in items {
        if json_string_field(item, "accountKey").as_deref() == Some(account_key) {
            if let Some(map) = item.as_object_mut() {
                map.insert(
                    "lastUsedAt".to_string(),
                    serde_json::Value::Number(now.into()),
                );
            }
        }
    }

    Ok(())
}

fn remove_registry_account(
    registry: &mut serde_json::Value,
    account_key: &str,
) -> Result<Option<String>, String> {
    let root = registry
        .as_object_mut()
        .ok_or_else(|| "账号 registry 格式无效".to_string())?;
    let current_active_key = root
        .get("activeAccountKey")
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    let items = root
        .get_mut("items")
        .and_then(|value| value.as_array_mut())
        .ok_or_else(|| "账号 registry 缺少 items".to_string())?;
    let index = items
        .iter()
        .position(|item| json_string_field(item, "accountKey").as_deref() == Some(account_key))
        .ok_or_else(|| "未找到要删除的账号。".to_string())?;
    let removed = items.remove(index);
    let removed_snapshot_path = json_string_field(&removed, "snapshotPath");

    if current_active_key.as_deref() == Some(account_key) {
        if let Some(next_key) = items
            .iter()
            .find_map(|item| json_string_field(item, "accountKey"))
        {
            root.insert(
                "activeAccountKey".to_string(),
                serde_json::Value::String(next_key),
            );
        } else {
            root.insert(
                "activeAccountKey".to_string(),
                serde_json::Value::String(String::new()),
            );
        }
    }

    root.insert(
        "updatedAt".to_string(),
        serde_json::Value::Number(current_log_time().parse::<i64>().unwrap_or_default().into()),
    );
    Ok(removed_snapshot_path)
}

fn refresh_accounts_usage_from_backend_api(registry: &mut serde_json::Value) -> usize {
    let Some(items) = registry
        .get("items")
        .and_then(|value| value.as_array())
        .cloned()
    else {
        return 0;
    };

    let mut updates = Vec::new();
    for item in items {
        let Some(account_key) = json_string_field(&item, "accountKey") else {
            continue;
        };
        let Some(snapshot_path) = json_string_field(&item, "snapshotPath") else {
            continue;
        };
        let Some(snapshot_root) = read_json_file_optional(Path::new(&snapshot_path)) else {
            continue;
        };
        let Some(access_token) = find_codex_access_token(&snapshot_root)
            .or_else(|| find_string_by_keys(&snapshot_root, &["OPENAI_API_KEY"]))
        else {
            continue;
        };
        let account_id =
            find_codex_account_id(&snapshot_root).unwrap_or_else(|| account_key.clone());
        let mut _batch_err: Option<String> = None;
        let Some(usage) =
            fetch_codex_usage_from_backend_api(&access_token, &account_id, false, &mut _batch_err)
        else {
            continue;
        };
        updates.push((account_key, usage));
    }

    let update_count = updates.len();
    for (account_key, usage) in updates {
        let _ = update_account_usage_cache(registry, &account_key, &usage);
    }

    update_count
}

fn refresh_account_usage_from_backend_api(
    registry: &mut serde_json::Value,
    account_key: &str,
    manual: bool,
) -> bool {
    let in_flight = ACCOUNT_USAGE_REFRESH_IN_FLIGHT.get_or_init(|| Mutex::new(HashSet::new()));
    {
        let mut keys = match in_flight.lock() {
            Ok(keys) => keys,
            Err(_) => return false,
        };
        if keys.contains(account_key) {
            return false;
        }
        keys.insert(account_key.to_string());
    }

    let refreshed = refresh_account_usage_from_backend_api_inner(registry, account_key, manual);
    if let Ok(mut keys) = in_flight.lock() {
        keys.remove(account_key);
    }

    refreshed
}

fn refresh_account_usage_from_backend_api_inner(
    registry: &mut serde_json::Value,
    account_key: &str,
    manual: bool,
) -> bool {
    let Some(item) = find_registry_account(registry, account_key) else {
        return false;
    };
    let Some(snapshot_path) = json_string_field(&item, "snapshotPath") else {
        return false;
    };
    let Some(snapshot_root) = read_json_file_optional(Path::new(&snapshot_path)) else {
        return false;
    };
    let Some(access_token) = find_codex_access_token(&snapshot_root)
        .or_else(|| find_string_by_keys(&snapshot_root, &["OPENAI_API_KEY"]))
    else {
        return false;
    };
    let account_id =
        find_codex_account_id(&snapshot_root).unwrap_or_else(|| account_key.to_string());
    if should_log_verbose_account_usage_refresh(manual) {
        append_internal_app_log(
            "info",
            "accounts",
            "refresh-usage",
            "开始主动刷新账号额度",
            Some(format!(
                "accountKey={}, chatgptAccountId={}",
                mask_secret(account_key),
                mask_secret(&account_id)
            )),
        );
    }

    let mut usage_error_detail: Option<String> = None;
    let Some(usage) = fetch_codex_usage_from_backend_api(
        &access_token,
        &account_id,
        manual,
        &mut usage_error_detail,
    ) else {
        set_account_usage_last_error(
            usage_error_detail
                .clone()
                .unwrap_or_else(|| "未返回有效额度数据".to_string()),
        );
        if manual {
            let error_detail = usage_error_detail.as_deref().unwrap_or("(no detail)");
            append_internal_app_log(
                "warn",
                "accounts",
                "refresh-usage",
                "主动刷新账号额度失败",
                Some(format!(
                    "accountKey={}, chatgptAccountId={}, error={}",
                    mask_secret(account_key),
                    mask_secret(&account_id),
                    error_detail
                )),
            );
        }
        return false;
    };

    let updated = update_account_usage_cache(registry, account_key, &usage).is_ok();
    if manual {
        append_internal_app_log(
            if updated { "info" } else { "warn" },
            "accounts",
            "refresh-usage",
            if updated {
                "主动刷新账号额度成功"
            } else {
                "主动刷新账号额度写入缓存失败"
            },
            Some(format!(
                "accountKey={}, primary={}, secondary={}",
                mask_secret(account_key),
                usage.primary.is_some(),
                usage.secondary.is_some()
            )),
        );
    }
    updated
}

fn set_account_usage_last_error(detail: String) {
    if let Ok(mut error) = ACCOUNT_USAGE_LAST_ERROR
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *error = Some(truncate_text(&detail, 240));
    }
}

fn take_account_usage_last_error() -> Option<String> {
    ACCOUNT_USAGE_LAST_ERROR
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|mut error| error.take())
}

fn refresh_active_account_usage_from_backend_api() -> Result<bool, String> {
    let mut registry = read_accounts_registry()?;
    let Some(account_key) =
        json_string_field(&registry, "activeAccountKey").filter(|value| !value.trim().is_empty())
    else {
        return Ok(false);
    };

    if !refresh_account_usage_from_backend_api(&mut registry, &account_key, false) {
        return Ok(false);
    }

    write_accounts_registry(&registry)?;
    Ok(true)
}

fn fetch_codex_usage_from_backend_api(
    access_token: &str,
    account_id: &str,
    manual: bool,
    collect_error: &mut Option<String>,
) -> Option<CodexUsageSnapshot> {
    fetch_codex_usage_from_url(
        OFFICIAL_CODEX_USAGE_URL,
        access_token,
        account_id,
        manual,
        collect_error,
    )
}

#[tauri::command]
async fn sync_official_catalog() -> Result<Vec<CatalogModelOption>, String> {
    tauri::async_runtime::spawn_blocking(sync_official_catalog_blocking)
        .await
        .map_err(|error| format!("同步官方模型任务执行失败：{}", error))?
}

fn sync_official_catalog_blocking() -> Result<Vec<CatalogModelOption>, String> {
    let auth_root = read_json_file_optional(&codex_auth_path()?);
    let (access_token, account_id) = if let Ok(registry) = read_accounts_registry() {
        let account_key = registry
            .get("activeAccountKey")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty());
        let account = account_key.and_then(|key| find_registry_account(&registry, key));
        let snapshot = account
            .as_ref()
            .and_then(|item| json_string_field(item, "snapshotPath"))
            .and_then(|path| read_json_file_optional(Path::new(&path)));
        let token = snapshot.as_ref().and_then(find_codex_access_token)
            .or_else(|| auth_root.as_ref().and_then(find_codex_access_token));
        let id = snapshot.as_ref().and_then(find_codex_account_id)
            .or_else(|| auth_root.as_ref().and_then(find_codex_account_id))
            .or_else(|| account_key.map(str::to_string));
        (token, id)
    } else {
        (
            auth_root.as_ref().and_then(find_codex_access_token),
            auth_root.as_ref().and_then(find_codex_account_id),
        )
    };
    let access_token = access_token.ok_or_else(|| "未找到当前账号的访问令牌".to_string())?;
    let account_id = account_id.ok_or_else(|| "未找到当前账号的 ChatGPT-Account-Id".to_string())?;
    let settings = load_official_codex_forward_settings();
    let request = match settings.proxy_url.as_deref().and_then(|url| ureq::Proxy::new(url).ok()) {
        Some(proxy) => ureq::builder().proxy(proxy).build().get(OFFICIAL_CODEX_MODELS_URL),
        None => ureq::get(OFFICIAL_CODEX_MODELS_URL),
    }
        .timeout(Duration::from_secs(ACCOUNT_USAGE_REQUEST_TIMEOUT_SECONDS))
        .set(HEADER_AUTHORIZATION, &format!("Bearer {}", access_token))
        .set(HEADER_CHATGPT_ACCOUNT_ID, &account_id)
        .set("Host", "chatgpt.com")
        .set(HEADER_ACCEPT, HEADER_JSON);
    let response = request.call().map_err(|error| {
        append_account_usage_router_log(
            OFFICIAL_CODEX_MODELS_URL,
            &account_id,
            "error",
            Some(&error.to_string()),
        );
        format!("同步官方模型失败：{}", error)
    })?;
    if !(200..300).contains(&response.status()) {
        append_account_usage_router_log(
            OFFICIAL_CODEX_MODELS_URL,
            &account_id,
            &response.status().to_string(),
            None,
        );
        return Err(format!("同步官方模型失败，HTTP 状态码：{}", response.status()));
    }
    append_account_usage_router_log(
        OFFICIAL_CODEX_MODELS_URL,
        &account_id,
        &response.status().to_string(),
        None,
    );
    let body = response.into_string().map_err(|error| format!("读取官方模型响应失败：{}", error))?;
    let root = serde_json::from_str::<serde_json::Value>(&body).map_err(|error| format!("解析官方模型响应失败：{}", error))?;
    if root.get(CATALOG_MODELS_KEY).and_then(|value| value.as_array()).is_none() {
        return Err("官方模型响应缺少 models 数组".to_string());
    }
    write_catalog_root(&catalog_base_config_path()?, &root)?;
    read_catalog_model_options()
}

fn fetch_codex_usage_from_url(
    url: &str,
    access_token: &str,
    account_id: &str,
    manual: bool,
    collect_error: &mut Option<String>,
) -> Option<CodexUsageSnapshot> {
    let settings = load_official_codex_forward_settings();
    let authorization = format!("Bearer {}", access_token);
    let body = send_codex_usage_request(url, &authorization, account_id, &settings, manual, collect_error)?;
    let root = match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(root) => root,
        Err(error) => {
            *collect_error = Some(format!(
                "url={}, account={}, error={}",
                url,
                mask_secret(account_id),
                error
            ));
            if should_log_verbose_account_usage_refresh(manual) {
                append_internal_app_log(
                    "warn",
                    "accounts",
                    "refresh-usage",
                    "解析额度接口响应失败",
                    Some(format!(
                        "url={}, account={}, error={}, body={}",
                        url,
                        mask_secret(account_id),
                        error,
                        truncate_text(&body, 800)
                    )),
                );
            }
            return None;
        }
    };
    let usage = usage_snapshot_from_backend_value(&root, manual);
    if usage.is_none() {
        *collect_error = Some(format!(
            "url={}, account={}, body={}",
            url,
            mask_secret(account_id),
            truncate_text(&body, 200)
        ));
        if should_log_verbose_account_usage_refresh(manual) {
            append_internal_app_log(
                "warn",
                "accounts",
                "refresh-usage",
                "额度接口响应中未找到有效额度窗口",
                Some(format!(
                    "url={}, account={}, body={}",
                    url,
                    mask_secret(account_id),
                    truncate_text(&body, 800)
                )),
            );
        }
    }
    usage
}

fn send_codex_usage_request(
    url: &str,
    authorization: &str,
    account_id: &str,
    settings: &OfficialCodexForwardSettings,
    manual: bool,
    collect_error: &mut Option<String>,
) -> Option<String> {
    let agent = match settings.proxy_url.as_deref() {
        Some(proxy_url) => match ureq::Proxy::new(proxy_url) {
            Ok(proxy) => Some(ureq::builder().proxy(proxy).build()),
            Err(_) => None,
        },
        None => None,
    };
    let request = match agent.as_ref() {
        Some(agent) => agent.get(url),
        None => ureq::get(url),
    }
    .timeout(Duration::from_secs(ACCOUNT_USAGE_REQUEST_TIMEOUT_SECONDS))
    .set(HEADER_ACCEPT, HEADER_JSON)
    .set(HEADER_CONTENT_TYPE, HEADER_JSON)
    .set(HEADER_AUTHORIZATION, authorization)
    .set(HEADER_CHATGPT_ACCOUNT_ID, account_id)
    .set("Host", "chatgpt.com")
    .set(HEADER_OPENAI_BETA, OFFICIAL_CODEX_BETA_HEADER_VALUE)
    .set(HEADER_ORIGINATOR, OFFICIAL_CODEX_ORIGINATOR)
    .set(HEADER_ORIGIN, "https://chatgpt.com")
    .set(HEADER_REFERER, "https://chatgpt.com/")
    .set(HEADER_USER_AGENT, "Mozilla/5.0 codex-router-shell");

    if should_log_verbose_account_usage_refresh(manual) {
        append_internal_app_log(
            "info",
            "accounts",
            "refresh-usage",
            "发送额度刷新请求",
            Some(format!(
                "method=GET, url={}, account={}",
                url,
                mask_secret(account_id)
            )),
        );
    }

    let response = request.call();

    let response = match response {
        Ok(response) => {
            append_account_usage_router_log(
                url,
                account_id,
                &response.status().to_string(),
                None,
            );
            if should_log_verbose_account_usage_refresh(manual) {
                append_internal_app_log(
                    "info",
                    "accounts",
                    "refresh-usage",
                    "额度刷新请求返回",
                    Some(format!(
                        "method=GET, url={}, account={}, status={}",
                        url,
                        mask_secret(account_id),
                        response.status()
                    )),
                );
            }
            let status = response.status();
            if !(200..300).contains(&status) {
                *collect_error = Some(format!(
                    "method=GET, url={}, account={}, status={}",
                    url,
                    mask_secret(account_id),
                    status
                ));
                if should_log_verbose_account_usage_refresh(manual) {
                    append_internal_app_log(
                        "warn",
                        "accounts",
                        "refresh-usage",
                        "额度接口返回非成功状态",
                        Some(format!(
                            "method=GET, url={}, account={}, status={}",
                            url,
                            mask_secret(account_id),
                            status
                        )),
                    );
                }
                return None;
            }
            response
        }
        Err(error) => {
            append_account_usage_router_log(url, account_id, "error", Some(&error.to_string()));
            *collect_error = Some(format!(
                "method=GET, url={}, account={}, error={}",
                url,
                mask_secret(account_id),
                error
            ));
            if should_log_verbose_account_usage_refresh(manual) {
                append_internal_app_log(
                    "warn",
                    "accounts",
                    "refresh-usage",
                    "请求额度接口失败",
                    Some(format!(
                        "method=GET, url={}, account={}, error={}",
                        url,
                        mask_secret(account_id),
                        error
                    )),
                );
            }
            return None;
        }
    };

    let body = response.into_string().ok()?;
    if should_log_verbose_account_usage_refresh(manual) {
        append_internal_app_log(
            "info",
            "accounts",
            "refresh-usage",
            "额度刷新响应内容",
            Some(format!(
                "method=GET, url={}, account={}, body={}",
                url,
                mask_secret(account_id),
                truncate_text(&body, 1200)
            )),
        );
    }
    Some(body)
}

fn should_log_verbose_account_usage_refresh(manual: bool) -> bool {
    manual && ACCOUNT_USAGE_VERBOSE_APP_LOGS
}

fn usage_snapshot_from_backend_value(
    root: &serde_json::Value,
    manual: bool,
) -> Option<CodexUsageSnapshot> {
    let rate_limits = find_rate_limits_value(root).unwrap_or(root);
    let has_explicit_usage_windows = rate_limits
        .get("primary")
        .or_else(|| rate_limits.get("primary_window"))
        .or_else(|| rate_limits.get("primaryWindow"))
        .is_some()
        || rate_limits
            .get("secondary")
            .or_else(|| rate_limits.get("secondary_window"))
            .or_else(|| rate_limits.get("secondaryWindow"))
            .is_some();
    let primary_value = rate_limits
        .get("primary")
        .or_else(|| rate_limits.get("primary_window"))
        .or_else(|| rate_limits.get("primaryWindow"));
    let secondary_value = rate_limits
        .get("secondary")
        .or_else(|| rate_limits.get("secondary_window"))
        .or_else(|| rate_limits.get("secondaryWindow"));
    let primary = primary_value.and_then(usage_window_from_value).or_else(|| {
        if primary_value.is_none() {
            find_usage_window_by_minutes(rate_limits, 300)
        } else {
            None
        }
    });
    let secondary = secondary_value
        .and_then(usage_window_from_value)
        .or_else(|| {
            if secondary_value.is_none() {
                find_usage_window_by_minutes(rate_limits, 10080)
            } else {
                None
            }
        });

    if primary.is_none() && secondary.is_none() {
        if has_explicit_usage_windows && should_log_verbose_account_usage_refresh(manual) {
            append_internal_app_log(
                "warn",
                "accounts",
                "refresh-usage",
                "额度接口返回了额度字段，但字段格式无效",
                Some(format!("body={}", truncate_text(&root.to_string(), 800))),
            );
        }
        return None;
    }

    Some(CodexUsageSnapshot {
        primary,
        secondary,
        plan: find_string_by_keys(
            root,
            &[
                "plan_type",
                "planType",
                "chatgpt_plan_type",
                "chatgptPlanType",
                "plan",
            ],
        ),
        user_id: find_codex_user_id(root),
        account_id: find_codex_account_id(root),
    })
}

fn find_usage_window_by_minutes(
    value: &serde_json::Value,
    window_minutes: u64,
) -> Option<CodexUsageWindow> {
    match value {
        serde_json::Value::Object(map) => {
            let matches_window = map
                .get("window_minutes")
                .or_else(|| map.get("windowMinutes"))
                .and_then(|item| item.as_u64())
                == Some(window_minutes);
            if matches_window {
                if let Some(window) = usage_window_from_value(value) {
                    return Some(window);
                }
            }

            for child in map.values() {
                if let Some(found) = find_usage_window_by_minutes(child, window_minutes) {
                    return Some(found);
                }
            }

            None
        }
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(|item| find_usage_window_by_minutes(item, window_minutes)),
        _ => None,
    }
}

fn find_rate_limits_value(value: &serde_json::Value) -> Option<&serde_json::Value> {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(rate_limits) = map
                .get("rate_limit")
                .or_else(|| map.get("rateLimit"))
                .or_else(|| map.get("rate_limits"))
                .or_else(|| map.get("rateLimits"))
            {
                return Some(rate_limits);
            }

            for child in map.values() {
                if let Some(found) = find_rate_limits_value(child) {
                    return Some(found);
                }
            }

            None
        }
        serde_json::Value::Array(items) => items.iter().find_map(find_rate_limits_value),
        _ => None,
    }
}

fn update_account_usage_cache(
    registry: &mut serde_json::Value,
    account_key: &str,
    usage: &CodexUsageSnapshot,
) -> Result<(), String> {
    let now = current_log_time().parse::<i64>().unwrap_or_default();
    let root = registry
        .as_object_mut()
        .ok_or_else(|| "账号 registry 格式无效".to_string())?;
    let items = root
        .get_mut("items")
        .and_then(|value| value.as_array_mut())
        .ok_or_else(|| "账号 registry 缺少 items".to_string())?;
    let item = items
        .iter_mut()
        .find(|item| json_string_field(item, "accountKey").as_deref() == Some(account_key))
        .ok_or_else(|| "未找到账户，无法更新额度".to_string())?;
    let map = item
        .as_object_mut()
        .ok_or_else(|| "账号条目格式无效".to_string())?;

    map.insert(
        "lastUsageAt".to_string(),
        serde_json::Value::Number(now.into()),
    );
    map.insert(
        "cachedPrimaryWindow".to_string(),
        usage
            .primary
            .as_ref()
            .map(usage_window_to_json)
            .unwrap_or(serde_json::Value::Null),
    );
    map.insert(
        "cachedSecondaryWindow".to_string(),
        usage
            .secondary
            .as_ref()
            .map(usage_window_to_json)
            .unwrap_or(serde_json::Value::Null),
    );
    if let Some(plan) = usage.plan.as_ref().filter(|plan| !plan.trim().is_empty()) {
        map.insert(
            "plan".to_string(),
            serde_json::Value::String(plan.to_string()),
        );
    }
    if let Some(user_id) = usage
        .user_id
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        map.insert(
            "userId".to_string(),
            serde_json::Value::String(user_id.to_string()),
        );
    }
    if let Some(account_id) = usage
        .account_id
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        map.insert(
            "usageAccountId".to_string(),
            serde_json::Value::String(account_id.to_string()),
        );
    }

    root.insert(
        "updatedAt".to_string(),
        serde_json::Value::Number(now.into()),
    );
    Ok(())
}

fn usage_window_to_json(window: &CodexUsageWindow) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert(
        "usedPercent".to_string(),
        serde_json::Value::Number((window.used_percent as i64).into()),
    );
    map.insert(
        "remainingPercent".to_string(),
        serde_json::Value::Number((remaining_percent_from_used(window.used_percent) as i64).into()),
    );
    if let Some(limit_window_seconds) = window.limit_window_seconds {
        map.insert(
            "limitWindowSeconds".to_string(),
            serde_json::Value::Number(serde_json::Number::from(limit_window_seconds)),
        );
    }
    if let Some(reset_after_seconds) = window.reset_after_seconds {
        map.insert(
            "resetAfterSeconds".to_string(),
            serde_json::Value::Number(serde_json::Number::from(reset_after_seconds)),
        );
    }
    if let Some(resets_at) = window.resets_at.as_ref() {
        if let Ok(number) = resets_at.parse::<i64>() {
            map.insert(
                "resetsAt".to_string(),
                serde_json::Value::Number(number.into()),
            );
            map.insert(
                "resetAt".to_string(),
                serde_json::Value::Number(number.into()),
            );
        } else {
            map.insert(
                "resetsAt".to_string(),
                serde_json::Value::String(resets_at.clone()),
            );
            map.insert(
                "resetAt".to_string(),
                serde_json::Value::String(resets_at.clone()),
            );
        }
    }
    serde_json::Value::Object(map)
}

fn usage_remaining_percent_from_cached_value(value: &serde_json::Value) -> Option<u8> {
    find_u8_by_keys(
        value,
        &[
            "remainingPercent",
            "remaining_percent",
            "remaining",
            "availablePercent",
            "available_percent",
        ],
    )
    .or_else(|| {
        find_u8_by_keys(
            value,
            &[
                "usedPercent",
                "used_percent",
                "usagePercent",
                "usage_percent",
                "percent",
                "percentage",
            ],
        )
        .map(remaining_percent_from_used)
    })
}

fn usage_window_from_cached_value(value: &serde_json::Value) -> Option<CodexUsageWindow> {
    if value.is_null() {
        return None;
    }

    let remaining_percent = usage_remaining_percent_from_cached_value(value)?;
    Some(CodexUsageWindow {
        used_percent: 100u8.saturating_sub(remaining_percent.min(100)),
        resets_at: json_number_or_string_field(value, "resetAt")
            .or_else(|| json_number_or_string_field(value, "reset_at"))
            .or_else(|| json_number_or_string_field(value, "resetsAt"))
            .or_else(|| json_number_or_string_field(value, "endAt"))
            .or_else(|| json_number_or_string_field(value, "resets_at")),
        limit_window_seconds: json_u64_number_field(value, "limitWindowSeconds")
            .or_else(|| json_u64_number_field(value, "limit_window_seconds")),
        reset_after_seconds: json_u64_number_field(value, "resetAfterSeconds")
            .or_else(|| json_u64_number_field(value, "reset_after_seconds")),
    })
}

struct DisplayUsageWindows {
    five_hour_percent: Option<u8>,
    weekly_percent: Option<u8>,
    five_hour_reset_at: Option<String>,
    weekly_reset_at: Option<String>,
}

fn display_usage_from_windows(
    primary: Option<&CodexUsageWindow>,
    secondary: Option<&CodexUsageWindow>,
) -> DisplayUsageWindows {
    let mut result = DisplayUsageWindows {
        five_hour_percent: None,
        weekly_percent: None,
        five_hour_reset_at: None,
        weekly_reset_at: None,
    };

    let windows = [primary, secondary]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let five_hour = windows
        .iter()
        .copied()
        .find(|window| window.limit_window_seconds == Some(18_000));
    let weekly = windows
        .iter()
        .copied()
        .find(|window| window.limit_window_seconds == Some(604_800));

    if let Some(five_hour) = five_hour {
        assign_five_hour_usage(&mut result, five_hour);
    }
    if let Some(weekly) = weekly {
        assign_weekly_usage(&mut result, weekly);
    }

    result
}

fn assign_five_hour_usage(result: &mut DisplayUsageWindows, window: &CodexUsageWindow) {
    result.five_hour_percent = Some(remaining_percent_from_used(window.used_percent));
    result.five_hour_reset_at = window.resets_at.clone();
}

fn assign_weekly_usage(result: &mut DisplayUsageWindows, window: &CodexUsageWindow) {
    result.weekly_percent = Some(remaining_percent_from_used(window.used_percent));
    result.weekly_reset_at = window.resets_at.clone();
}

fn account_usage_windows_from_cached(
    primary: Option<&CodexUsageWindow>,
    secondary: Option<&CodexUsageWindow>,
) -> Vec<CodexAccountUsageWindow> {
    let mut windows = [primary, secondary]
        .into_iter()
        .flatten()
        .map(|window| CodexAccountUsageWindow {
            remaining_percent: remaining_percent_from_used(window.used_percent),
            reset_at: window.resets_at.clone(),
            limit_window_seconds: window.limit_window_seconds,
            reset_after_seconds: window.reset_after_seconds,
        })
        .collect::<Vec<_>>();
    windows.sort_by_key(|window| window.limit_window_seconds.unwrap_or(u64::MAX));
    windows
}

fn remaining_percent_from_used(used_percent: u8) -> u8 {
    100u8.saturating_sub(used_percent.min(100))
}

fn backup_current_auth_file() -> Result<(), String> {
    let auth_path = codex_auth_path()?;
    if !auth_path.exists() {
        return Ok(());
    }

    let backup_dir = codex_accounts_backups_path()?;
    fs::create_dir_all(&backup_dir).map_err(|error| {
        format!(
            "创建账号备份目录失败：{}，路径：{}",
            error,
            backup_dir.display()
        )
    })?;
    let backup_path = backup_dir.join(format!("auth-before-switch-{}.json", current_log_time()));
    fs::copy(&auth_path, &backup_path).map_err(|error| {
        format!(
            "备份当前 auth.json 失败：{}，路径：{}",
            error,
            backup_path.display()
        )
    })?;
    Ok(())
}







fn find_codex_access_token(root: &serde_json::Value) -> Option<String> {
    find_string_by_keys(root, CODEX_ACCESS_TOKEN_KEYS)
}







fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(truncate_text(trimmed, MAX_THREAD_TITLE_CHARS))
    }
}

fn project_name_from_cwd(cwd: Option<&str>) -> String {
    let Some(cwd) = cwd.map(str::trim).filter(|value| !value.is_empty()) else {
        return "Unknown Project".to_string();
    };
    let normalized = cwd.replace('/', "\\");
    let trimmed = normalized.trim_end_matches('\\');

    if trimmed.ends_with("\\Documents\\Codex")
        || trimmed.contains("\\Documents\\Codex\\")
        || trimmed == "Documents\\Codex"
    {
        return "对话".to_string();
    }

    normalized
        .split('\\')
        .filter(|segment| !segment.trim().is_empty())
        .last()
        .unwrap_or("Unknown Project")
        .to_string()
}

fn is_dialog_project_name(project_name: &str) -> bool {
    matches!(project_name, "对话" | "瀵硅瘽")
}

fn session_active_day(session: &ThreadSession) -> Option<String> {
    session
        .created_at
        .as_deref()
        .or(session.updated_at.as_deref())
        .map(|value| {
            if value.len() >= 10 && value.as_bytes().get(4) == Some(&b'-') {
                value[..10].to_string()
            } else {
                value.to_string()
            }
        })
}





fn load_app_settings() -> Result<AppSettings, String> {
    ensure_app_settings_file()?;
    let path = app_settings_path()?;
    let text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "璇诲彇搴旂敤璁剧疆澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })?;

    if text.trim().is_empty() {
        return Ok(AppSettings::default());
    }

    serde_json::from_str::<AppSettings>(&text).map_err(|error| {
        format!(
            "瑙ｆ瀽搴旂敤璁剧疆澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })
}

fn save_app_settings(settings: &AppSettings) -> Result<(), String> {
    let path = app_settings_path()?;
    ensure_parent_dir(&path)?;
    let text = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("序列化应用设置失败：{}", error))?;
    fs::write(&path, text).map_err(|error| {
        format!(
            "鍐欏叆搴旂敤璁剧疆澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })
}

fn default_account_usage_refresh_seconds() -> u64 {
    DEFAULT_ACCOUNT_USAGE_REFRESH_SECONDS
}

fn default_system_version() -> String {
    SERVICE_VERSION.to_string()
}

fn current_app_activation_time() -> String {
    let beijing_offset = UtcOffset::from_hms(8, 0, 0).unwrap_or(UtcOffset::UTC);
    format_beijing_timestamp(OffsetDateTime::now_utc().to_offset(beijing_offset))
}

fn normalize_app_activation_time(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if is_plain_beijing_timestamp(trimmed) {
        return Some(format!("{}+08:00", trimmed.replace(' ', "T")));
    }

    OffsetDateTime::parse(trimmed, &Rfc3339)
        .ok()
        .map(|time| {
            let beijing_offset = UtcOffset::from_hms(8, 0, 0).unwrap_or(UtcOffset::UTC);
            format_beijing_timestamp(time.to_offset(beijing_offset))
        })
        .or_else(|| Some(trimmed.to_string()))
}





fn normalize_system_version(version: String) -> String {
    let trimmed = version.trim().trim_start_matches('v').to_string();
    if trimmed.is_empty() {
        default_system_version()
    } else {
        trimmed
    }
}

fn fetch_latest_codexhub_msi_asset() -> Result<ReleaseMsiAsset, String> {
    let releases = fetch_codexhub_release_roots()?;
    let mut candidates = Vec::new();
    for root in releases {
        let release_page_url = json_string_field(&root, "html_url")
            .unwrap_or_else(|| "https://github.com/xiaoashuo/CodexHub/releases".to_string());
        let Some(assets) = root.get("assets").and_then(|value| value.as_array()) else {
            continue;
        };

        for asset in assets {
            let Some(asset_name) = json_string_field(asset, "name") else {
                continue;
            };
            let Some(version) = parse_codex_companion_msi_version(&asset_name) else {
                continue;
            };
            let Some(download_url) = json_string_field(asset, "browser_download_url") else {
                continue;
            };
            candidates.push(ReleaseMsiAsset {
                version,
                asset_name,
                download_url,
                release_page_url: release_page_url.clone(),
            });
        }
    }

    candidates
        .into_iter()
        .max_by(|left, right| {
            compare_semverish(&left.version, &right.version)
                .cmp(&0)
                .then_with(|| left.version.cmp(&right.version))
        })
        .ok_or_else(|| {
            "Latest release does not contain a CodexHub_version_arch_locale.msi asset.".to_string()
        })
}

fn fetch_codexhub_release_roots() -> Result<Vec<serde_json::Value>, String> {
    match request_github_json(CODEXHUB_LATEST_RELEASE_API_URL) {
        Ok(root) => return Ok(vec![root]),
        Err((Some(404), _)) => {}
        Err((_, message)) => return Err(message),
    }

    let root = request_github_json(CODEXHUB_RELEASES_API_URL).map_err(|(_, message)| message)?;
    root.as_array()
        .cloned()
        .filter(|items| !items.is_empty())
        .ok_or_else(|| "GitHub Releases 列表为空。".to_string())
}

fn request_github_json(url: &str) -> Result<serde_json::Value, (Option<u16>, String)> {
    let response = match ureq::get(url)
        .set(HEADER_USER_AGENT, "CodexHub")
        .set(HEADER_ACCEPT, "application/vnd.github+json")
        .timeout(Duration::from_secs(VERSION_CHECK_TIMEOUT_SECONDS))
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            return Err((
                Some(status),
                format!(
                    "检测 GitHub Release 失败：HTTP {} {}",
                    status,
                    truncate_text(&body, 120)
                ),
            ));
        }
        Err(ureq::Error::Transport(error)) => {
            return Err((None, format!("连接 GitHub Release 失败：{}", error)))
        }
    };
    let body = response
        .into_string()
        .map_err(|error| (None, format!("读取 GitHub Release 响应失败：{}", error)))?;
    serde_json::from_str::<serde_json::Value>(&body)
        .map_err(|error| (None, format!("解析 GitHub Release 响应失败：{}", error)))
}

fn parse_codex_companion_msi_version(asset_name: &str) -> Option<String> {
    let name = asset_name.trim();
    if !name.to_ascii_lowercase().ends_with(".msi") {
        return None;
    }

    let stem = &name[..name.len().saturating_sub(4)];
    let mut parts = stem.split('_');
    let product_name = parts.next()?.trim();
    if product_name != "CodexHub" {
        return None;
    }

    let version = parts.next()?.trim();
    if !is_version_token(version) {
        return None;
    }

    Some(normalize_system_version(version.to_string()))
}

fn is_version_token(value: &str) -> bool {
    let trimmed = value.trim().trim_start_matches('v');
    !trimmed.is_empty()
        && trimmed.contains('.')
        && trimmed.split('.').all(|part| {
            !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
        })
}

fn is_version_newer(latest: &str, current: &str) -> bool {
    compare_semverish(latest, current) > 0
}

fn compare_semverish(left: &str, right: &str) -> i8 {
    let left_parts = semverish_parts(left);
    let right_parts = semverish_parts(right);
    let len = left_parts.len().max(right_parts.len()).max(3);

    for index in 0..len {
        let left_value = *left_parts.get(index).unwrap_or(&0);
        let right_value = *right_parts.get(index).unwrap_or(&0);
        if left_value > right_value {
            return 1;
        }
        if left_value < right_value {
            return -1;
        }
    }

    0
}

fn semverish_parts(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches('v')
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

fn default_account_proxy_api_key() -> String {
    let mut bytes = [0u8; 32];
    if getrandom::getrandom(&mut bytes).is_ok() {
        return format!("sk_{}", URL_SAFE_NO_PAD.encode(bytes));
    }

    let fallback = format!("{}:{}", current_log_millis(), std::process::id());
    format!(
        "sk_{}",
        URL_SAFE_NO_PAD.encode(Sha256::digest(fallback.as_bytes()))
    )
}

fn default_account_proxy_url(port: u16) -> String {
    format!("http://{}:{}/v1", ROUTER_HOST, port)
}

fn normalize_account_proxy_settings(
    mut settings: AccountProxySettings,
    oauth_callback_port: u16,
) -> AccountProxySettings {
    settings.account_proxy_url = if settings.account_proxy_url.trim().is_empty() {
        default_account_proxy_url(oauth_callback_port)
    } else {
        settings.account_proxy_url.trim().to_string()
    };
    settings.api_key = normalize_account_proxy_api_key(&settings.api_key);
    settings
}

fn normalize_account_proxy_api_key(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with("sk_") && trimmed.len() >= 24 {
        trimmed.to_string()
    } else {
        default_account_proxy_api_key()
    }
}

fn default_router_port() -> u16 {
    ROUTER_PORT
}

fn default_router_concurrency_limit() -> usize {
    DEFAULT_ROUTER_CONCURRENCY_LIMIT
}

fn default_oauth_callback_port() -> u16 {
    OAUTH_CALLBACK_PORT
}

fn normalize_port(port: u16, default_port: u16) -> u16 {
    if port == 0 {
        default_port
    } else {
        port
    }
}

fn configured_router_port() -> u16 {
    load_app_settings()
        .map(|settings| normalize_port(settings.router_port, default_router_port()))
        .unwrap_or_else(|_| default_router_port())
}

fn normalize_router_concurrency_limit(limit: usize) -> usize {
    limit.clamp(MIN_ROUTER_CONCURRENCY_LIMIT, MAX_ROUTER_CONCURRENCY_LIMIT)
}

fn configured_router_concurrency_limit() -> usize {
    load_app_settings()
        .map(|settings| normalize_router_concurrency_limit(settings.router_concurrency_limit))
        .unwrap_or_else(|_| default_router_concurrency_limit())
}

fn configured_oauth_callback_port() -> u16 {
    load_app_settings()
        .map(|settings| normalize_port(settings.oauth_callback_port, default_oauth_callback_port()))
        .unwrap_or_else(|_| default_oauth_callback_port())
}

fn normalize_account_usage_refresh_seconds(seconds: u64) -> u64 {
    if ACCOUNT_USAGE_REFRESH_ALLOWED_SECONDS.contains(&seconds) {
        seconds
    } else {
        DEFAULT_ACCOUNT_USAGE_REFRESH_SECONDS
    }
}

fn sync_active_auth_to_snapshot() -> Result<(), String> {
    let auth_path = codex_auth_path()?;
    if !auth_path.exists() {
        return Ok(());
    }
    let Some(auth_root) = read_json_file_optional(&auth_path) else {
        return Ok(());
    };
    let Some(account_key) = build_codex_account_key(&auth_root) else {
        return Ok(());
    };

    let mut registry = read_accounts_registry()?;
    let is_registered = find_registry_account(&registry, &account_key).is_some();

    if !is_registered {
        let enriched = enrich_codex_auth_identity(auth_root);
        upsert_codex_auth_value_account(&mut registry, &enriched, true)?;
        write_accounts_registry(&registry)?;
        return Ok(());
    }

    let snapshots_dir = codex_accounts_snapshots_path()?;
    let snapshot_path = snapshots_dir.join(format!(
        "{}.json",
        sanitize_account_key_for_filename(&account_key)
    ));
    let auth_mtime = fs::metadata(&auth_path)
        .ok()
        .and_then(|metadata| metadata.modified().ok());
    let snapshot_mtime = fs::metadata(&snapshot_path)
        .ok()
        .and_then(|metadata| metadata.modified().ok());
    let needs_sync = match (auth_mtime, snapshot_mtime) {
        (Some(auth_time), Some(snapshot_time)) => auth_time > snapshot_time,
        (Some(_), None) => true,
        _ => false,
    };

    if needs_sync {
        let enriched = enrich_codex_auth_identity(auth_root);
        upsert_codex_auth_value_account(&mut registry, &enriched, true)?;
        write_accounts_registry(&registry)?;
    }

    Ok(())
}

fn ensure_account_usage_refresh_worker() {
    ACCOUNT_USAGE_REFRESH_WORKER.get_or_init(|| {
        thread::spawn(|| {
            let mut last_refresh_at: Option<Instant> = Some(
                Instant::now()
                    - Duration::from_secs(
                        DEFAULT_ACCOUNT_USAGE_REFRESH_SECONDS
                            .saturating_sub(ACCOUNT_USAGE_INITIAL_DELAY_SECONDS),
                    ),
            );

            loop {
                let refresh_seconds = load_app_settings()
                    .map(|settings| {
                        normalize_account_usage_refresh_seconds(
                            settings.account_usage_refresh_seconds,
                        )
                    })
                    .unwrap_or(DEFAULT_ACCOUNT_USAGE_REFRESH_SECONDS);
                let refresh_interval = Duration::from_secs(refresh_seconds);
                let should_refresh = last_refresh_at
                    .map(|last_refresh_at| last_refresh_at.elapsed() >= refresh_interval)
                    .unwrap_or(true);

                if should_refresh {
                    let _ = refresh_active_account_usage_from_backend_api();
                    last_refresh_at = Some(Instant::now());
                }

                let _ = sync_active_auth_to_snapshot();

                thread::sleep(Duration::from_secs(ACCOUNT_USAGE_REFRESH_POLL_SECONDS));
            }
        });
    });
}

fn detect_codex_exe_path() -> Option<String> {
    find_app_path_by_appx_package("ChatGPT", "ChatGPT.exe")
        .or_else(|| find_app_path_by_appx_package("Codex", "Codex.exe"))
        .or_else(find_codex_path_by_where)
        .or_else(find_codex_path_by_powershell)
        .or_else(find_codex_path_from_common_locations)
}

fn find_codex_path_by_where() -> Option<String> {
    let output = hidden_command("where.exe").arg("codex").output().ok()?;

    if !output.status.success() {
        return None;
    }

    select_best_codex_candidate(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToString::to_string)
            .collect(),
    )
}

fn find_codex_path_by_powershell() -> Option<String> {
    let output = hidden_command("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-Command codex -ErrorAction SilentlyContinue).Source",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    select_best_codex_candidate(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToString::to_string)
            .collect(),
    )
}

fn find_app_path_by_appx_package(package_name: &str, executable_name: &str) -> Option<String> {
    let command = format!(
        "Get-AppxPackage *{}* | Select-Object -ExpandProperty InstallLocation",
        package_name.replace('\'', "''")
    );
    let output = hidden_command("powershell")
        .args(["-NoProfile", "-Command", &command])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find_map(|install_location| {
            find_app_exe_under_install_location(Path::new(install_location), executable_name)
        })
}

fn select_best_codex_candidate(candidates: Vec<String>) -> Option<String> {
    candidates
        .iter()
        .find(|line| line.to_ascii_lowercase().ends_with("codex.exe"))
        .cloned()
        .or_else(|| candidates.into_iter().find(|line| !line.is_empty()))
}

fn select_existing_codex_candidate(candidates: Vec<PathBuf>) -> Option<String> {
    candidates
        .into_iter()
        .find_map(|path| resolve_existing_path_case_insensitive(&path))
        .map(|path| path.display().to_string())
}

fn find_app_exe_under_install_location(
    install_location: &Path,
    executable_name: &str,
) -> Option<String> {
    [
        &["app", executable_name][..],
        &["app", "resources", executable_name][..],
        &[executable_name][..],
    ]
    .iter()
    .find_map(|segments| resolve_child_path_case_insensitive(install_location, segments))
    .map(|path| path.display().to_string())
}

fn find_codex_exe_under_install_location(install_location: &Path) -> Option<String> {
    find_app_exe_under_install_location(install_location, "Codex.exe")
}

fn resolve_existing_path_case_insensitive(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    let file_name = path.file_name()?.to_str()?;
    find_child_case_insensitive(parent, file_name).or_else(|| {
        if path.exists() {
            path.canonicalize()
                .ok()
                .or_else(|| Some(path.to_path_buf()))
        } else {
            None
        }
    })
}

fn resolve_child_path_case_insensitive(root: &Path, segments: &[&str]) -> Option<PathBuf> {
    let mut current = root.to_path_buf();
    for segment in segments {
        current = find_child_case_insensitive(&current, segment)?;
    }

    Some(current)
}

fn find_child_case_insensitive(parent: &Path, child_name: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(parent).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if file_name.eq_ignore_ascii_case(child_name) {
            return Some(path);
        }
    }

    None
}

fn find_codex_path_from_common_locations() -> Option<String> {
    let mut candidates = vec![
        env::var("LOCALAPPDATA").ok().map(|path| {
            PathBuf::from(path)
                .join("Microsoft")
                .join("WindowsApps")
                .join("ChatGPT.exe")
        }),
        env::var("LOCALAPPDATA").ok().map(|path| {
            PathBuf::from(path)
                .join("Microsoft")
                .join("WindowsApps")
                .join("chatgpt.exe")
        }),
        env::var("LOCALAPPDATA").ok().map(|path| {
            PathBuf::from(path)
                .join("Programs")
                .join("ChatGPT")
                .join("ChatGPT.exe")
        }),
        env::var("LOCALAPPDATA").ok().map(|path| {
            PathBuf::from(path)
                .join("Programs")
                .join("ChatGPT")
                .join("chatgpt.exe")
        }),
        env::var("LOCALAPPDATA").ok().map(|path| {
            PathBuf::from(path)
                .join("Microsoft")
                .join("WindowsApps")
                .join("codex.exe")
        }),
        env::var("LOCALAPPDATA").ok().map(|path| {
            PathBuf::from(path)
                .join("Microsoft")
                .join("WindowsApps")
                .join("Codex.exe")
        }),
        env::var("LOCALAPPDATA").ok().map(|path| {
            PathBuf::from(path)
                .join("Programs")
                .join("Codex")
                .join("codex.exe")
        }),
        env::var("LOCALAPPDATA").ok().map(|path| {
            PathBuf::from(path)
                .join("Programs")
                .join("Codex")
                .join("Codex.exe")
        }),
        env::var("PROGRAMFILES")
            .ok()
            .map(|path| PathBuf::from(path).join("Codex").join("codex.exe")),
        env::var("PROGRAMFILES")
            .ok()
            .map(|path| PathBuf::from(path).join("Codex").join("Codex.exe")),
        env::var("PROGRAMFILES(X86)")
            .ok()
            .map(|path| PathBuf::from(path).join("Codex").join("codex.exe")),
        env::var("PROGRAMFILES(X86)")
            .ok()
            .map(|path| PathBuf::from(path).join("Codex").join("Codex.exe")),
    ];

    if let Ok(program_files) = env::var("PROGRAMFILES") {
        let windows_apps = PathBuf::from(program_files).join("WindowsApps");
        if let Ok(entries) = fs::read_dir(windows_apps) {
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                    continue;
                };
                if file_name
                    .to_ascii_lowercase()
                    .starts_with("openai.chatgpt_")
                {
                    if let Some(app_path) =
                        find_app_exe_under_install_location(&path, "ChatGPT.exe")
                    {
                        candidates.push(Some(PathBuf::from(app_path)));
                    }
                }
                if file_name.to_ascii_lowercase().starts_with("openai.codex_") {
                    if let Some(codex_path) = find_codex_exe_under_install_location(&path) {
                        candidates.push(Some(PathBuf::from(codex_path)));
                    }
                }
            }
        }
    }

    select_existing_codex_candidate(candidates.into_iter().flatten().collect())
}

#[allow(dead_code)]
fn find_codex_path_from_common_locations_legacy() -> Option<String> {
    String::from_utf8_lossy(&[])
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.to_string())
}

fn sync_catalog_from_provider_config() -> Result<SyncCatalogResult, String> {
    let target_path = catalog_config_path()?;
    let managed_slugs = provider_route_slugs()?;
    let enabled_routes = enabled_provider_routes()?;
    let (source_path, mut base_root) = load_catalog_base_root(&managed_slugs)?;
    let base_models = base_root
        .get("models")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "base catalog missing non-empty models".to_string())?;

    if base_models.is_empty() {
        return Err("base catalog missing non-empty models".to_string());
    }

    let mut target_models = base_models.clone();
    let template = base_models[0].clone();
    let mut synced_count = 0usize;
    let mut synced_slugs = Vec::new();
    let mut router_models = Vec::new();

    for item in enabled_routes {
        let slug = item.slug;
        let route = item.route;
        let mut custom_model = template.clone();
        let model_object = custom_model
            .as_object_mut()
            .ok_or_else(|| "base catalog first model is not an object".to_string())?;
        let display_name = if route.display_name.trim().is_empty() {
            slug.clone()
        } else {
            route.display_name.trim().to_string()
        };

        model_object.insert("slug".to_string(), serde_json::Value::String(slug.clone()));
        model_object.insert(
            "display_name".to_string(),
            serde_json::Value::String(display_name),
        );
        model_object.insert(
            "description".to_string(),
            serde_json::Value::String(CODEX_ROUTER_MODEL_DESCRIPTION.to_string()),
        );
        apply_router_model_context_fields(model_object, &route);
        rewrite_router_model_prompts(model_object, &route.real_model);
        model_object.insert(
            "priority".to_string(),
            serde_json::Value::Number(serde_json::Number::from(-10)),
        );
        model_object.insert("availability_nux".to_string(), serde_json::Value::Null);
        model_object.insert(
            "visibility".to_string(),
            serde_json::Value::String("list".to_string()),
        );
        model_object.insert(
            "supported_in_api".to_string(),
            serde_json::Value::Bool(true),
        );
        model_object.insert(
            CODEX_PROXY_MANAGED_KEY.to_string(),
            serde_json::Value::Bool(true),
        );
        target_models.retain(|model| {
            model.get("slug").and_then(|value| value.as_str()) != Some(slug.as_str())
        });
        router_models.push(custom_model.clone());
        target_models.push(custom_model);
        synced_count += 1;
        synced_slugs.push(slug);
    }

    base_root
        .as_object_mut()
        .ok_or_else(|| "base catalog root is not an object".to_string())?
        .insert(
            "models".to_string(),
            serde_json::Value::Array(target_models.clone()),
        );
    ensure_parent_dir(&target_path)?;
    let text = serde_json::to_string_pretty(&base_root)
        .map_err(|error| format!("serialize catalog failed: {}", error))?;
    fs::write(&target_path, text).map_err(|error| {
        format!(
            "write catalog failed: {}, path: {}",
            error,
            target_path.display()
        )
    })?;
    sync_models_cache_router_models(&router_models, &managed_slugs)?;

    Ok(SyncCatalogResult {
        source_path: source_path.display().to_string(),
        target_path: target_path.display().to_string(),
        synced_count,
        total_count: target_models.len(),
        synced_slugs,
    })
}

fn load_catalog_base_root(
    managed_slugs: &HashSet<String>,
) -> Result<(PathBuf, serde_json::Value), String> {
    let models_cache = models_cache_path()?;
    if let Some(root) = read_clean_catalog_root_if_available(&models_cache, managed_slugs)? {
        write_catalog_root(&catalog_base_config_path()?, &root)?;
        return Ok((models_cache, root));
    }

    let base_path = catalog_base_config_path()?;
    if let Some(root) = read_clean_catalog_root_if_available(&base_path, managed_slugs)? {
        return Ok((base_path, root));
    }

    let target_path = catalog_config_path()?;
    if let Some(root) = read_clean_catalog_root_if_available(&target_path, managed_slugs)? {
        return Ok((target_path, root));
    }

    let mut fallback_root = serde_json::Map::new();
    fallback_root.insert(
        "models".to_string(),
        serde_json::Value::Array(build_fallback_catalog_base_models()),
    );
    Ok((
        PathBuf::from("built-in fallback catalog"),
        serde_json::Value::Object(fallback_root),
    ))
}

fn read_clean_catalog_root_if_available(
    path: &Path,
    managed_slugs: &HashSet<String>,
) -> Result<Option<serde_json::Value>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let text = fs::read_to_string(path)
        .map_err(|error| format!("read catalog failed: {}, path: {}", error, path.display()))?;
    let root = serde_json::from_str::<serde_json::Value>(&text)
        .map_err(|error| format!("parse catalog failed: {}, path: {}", error, path.display()))?;

    Ok(clean_catalog_root(root, managed_slugs))
}

fn clean_catalog_root(
    mut root: serde_json::Value,
    managed_slugs: &HashSet<String>,
) -> Option<serde_json::Value> {
    let models = root
        .get_mut("models")
        .and_then(|value| value.as_array_mut())?;
    models.retain(|model| !is_router_catalog_model(model, managed_slugs));

    if models.is_empty() {
        None
    } else {
        Some(root)
    }
}

fn write_catalog_root(path: &Path, root: &serde_json::Value) -> Result<(), String> {
    ensure_parent_dir(path)?;
    let text = serde_json::to_string_pretty(root)
        .map_err(|error| format!("serialize catalog failed: {}", error))?;
    fs::write(path, text)
        .map_err(|error| format!("write catalog failed: {}, path: {}", error, path.display()))
}

fn sync_models_cache_router_models(
    router_models: &[serde_json::Value],
    managed_slugs: &HashSet<String>,
) -> Result<(), String> {
    let path = models_cache_path()?;
    if !path.exists() {
        return Ok(());
    }

    let text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "read models cache failed: {}, path: {}",
            error,
            path.display()
        )
    })?;
    let mut root = serde_json::from_str::<serde_json::Value>(&text).map_err(|error| {
        format!(
            "parse models cache failed: {}, path: {}",
            error,
            path.display()
        )
    })?;
    let models = root
        .get_mut("models")
        .and_then(|value| value.as_array_mut())
        .ok_or_else(|| "models cache missing models array".to_string())?;

    models.retain(|model| !is_router_catalog_model(model, managed_slugs));
    models.extend(router_models.iter().cloned());

    let next_text = serde_json::to_string_pretty(&root)
        .map_err(|error| format!("serialize models cache failed: {}", error))?;
    fs::write(&path, next_text).map_err(|error| {
        format!(
            "write models cache failed: {}, path: {}",
            error,
            path.display()
        )
    })
}

fn apply_router_model_context_fields(
    model_object: &mut serde_json::Map<String, serde_json::Value>,
    route: &ProviderRouteFileItem,
) {
    if let Some(value) = normalize_positive_u64(route.context_window) {
        model_object.insert(
            "context_window".to_string(),
            serde_json::Value::Number(serde_json::Number::from(value)),
        );
    }

    if let Some(value) = normalize_positive_u64(route.max_context_window) {
        model_object.insert(
            "max_context_window".to_string(),
            serde_json::Value::Number(serde_json::Number::from(value)),
        );
    }

    if let Some(value) = normalize_percent(route.effective_context_window_percent) {
        model_object.insert(
            "effective_context_window_percent".to_string(),
            serde_json::Value::Number(serde_json::Number::from(value)),
        );
    }
}

fn remove_router_models_from_models_cache() -> Result<(), String> {
    let path = models_cache_path()?;
    if !path.exists() {
        return Ok(());
    }
    let managed_slugs = provider_route_slugs()?;

    let text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "read models cache failed: {}, path: {}",
            error,
            path.display()
        )
    })?;
    let mut root = serde_json::from_str::<serde_json::Value>(&text).map_err(|error| {
        format!(
            "parse models cache failed: {}, path: {}",
            error,
            path.display()
        )
    })?;
    let Some(models) = root
        .get_mut("models")
        .and_then(|value| value.as_array_mut())
    else {
        return Ok(());
    };
    let before = models.len();
    models.retain(|model| !is_router_catalog_model(model, &managed_slugs));

    if models.len() == before {
        return Ok(());
    }

    let next_text = serde_json::to_string_pretty(&root)
        .map_err(|error| format!("serialize models cache failed: {}", error))?;
    fs::write(&path, next_text).map_err(|error| {
        format!(
            "write models cache failed: {}, path: {}",
            error,
            path.display()
        )
    })
}

fn is_router_catalog_model(model: &serde_json::Value, managed_slugs: &HashSet<String>) -> bool {
    let slug = model
        .get("slug")
        .and_then(|value| value.as_str())
        .unwrap_or("");

    managed_slugs.contains(slug)
        || model
            .get(CODEX_PROXY_MANAGED_KEY)
            .and_then(|value| value.as_bool())
            == Some(true)
}

fn rewrite_router_model_prompts(
    model_object: &mut serde_json::Map<String, serde_json::Value>,
    real_model: &str,
) {
    if let Some(base_instructions) = model_object
        .get("base_instructions")
        .and_then(|value| value.as_str())
        .map(|value| rewrite_router_model_identity(value, real_model))
    {
        model_object.insert(
            "base_instructions".to_string(),
            serde_json::Value::String(base_instructions),
        );
    }

    if let Some(model_messages) = model_object
        .get_mut("model_messages")
        .and_then(|value| value.as_object_mut())
    {
        if let Some(instructions_template) = model_messages
            .get("instructions_template")
            .and_then(|value| value.as_str())
            .map(|value| rewrite_router_model_identity(value, real_model))
        {
            model_messages.insert(
                "instructions_template".to_string(),
                serde_json::Value::String(instructions_template),
            );
        }
    }
}

fn rewrite_router_model_identity(template: &str, real_model: &str) -> String {
    const OFFICIAL_CODEX_IDENTITY_PREFIXES: &[&str] = &[
        "You are Codex, a coding agent based on GPT-5.",
        "You are GPT-5.2 running in the Codex CLI, a terminal-based coding assistant.",
    ];

    let router_identity = build_router_model_identity(real_model);

    for official_identity in OFFICIAL_CODEX_IDENTITY_PREFIXES {
        if let Some(rest) = template.strip_prefix(official_identity) {
            return format!("{}{}", router_identity, rest);
        }
    }

    if template.trim().is_empty() {
        router_identity
    } else {
        format!("{} {}", router_identity, template)
    }
}

fn build_router_model_identity(real_model: &str) -> String {
    let upstream_model = real_model.trim();
    if upstream_model.is_empty() {
        return CODEX_ROUTER_IDENTITY_PREFIX.to_string();
    }

    format!(
        "{} The active upstream model for this route is {}.",
        CODEX_ROUTER_IDENTITY_PREFIX, upstream_model
    )
}

fn restore_official_model_catalog() -> Result<(), String> {
    ensure_catalog_base_config_file()?;
    let base_path = catalog_base_config_path()?;
    let target_path = catalog_config_path()?;
    ensure_parent_dir(&target_path)?;
    fs::copy(&base_path, &target_path)
        .map(|_| ())
        .map_err(|error| {
            format!(
                "杩樺師瀹樻柟妯″瀷 Catalog 澶辫触：{}锛屾潵婧愶細{}锛岀洰鏍囷細{}",
                error,
                base_path.display(),
                target_path.display()
            )
        })
}

fn append_router_log_entry(log_entry: &RouterLogEntry) -> Result<(), String> {
    enqueue_router_log_write(RouterLogWriteTask::RouterLog(log_entry.clone()))
}

fn append_router_debug_log(reason: &str, detail: serde_json::Value) {
    if !load_app_settings()
        .map(|settings| settings.router_debug_mode)
        .unwrap_or(false)
    {
        return;
    }
    let entry = serde_json::json!({
        "time": current_log_time(),
        "reason": reason,
        "detail": sanitize_router_debug_value(detail)
    });
    if let Err(error) = append_router_debug_log_sync(&entry) {
        eprintln!("router debug log write error: {}", error);
    }
}

fn append_router_full_debug_log(reason: &str, detail: serde_json::Value) {
    if !ROUTER_FULL_DEBUG_LOG_ENABLED {
        return;
    }
    let entry = serde_json::json!({
        "time": current_log_time(),
        "reason": reason,
        "detail": sanitize_router_full_debug_value(detail)
    });
    if let Err(error) = append_router_full_debug_log_sync(&entry) {
        eprintln!("router full debug log write error: {}", error);
    }
}

fn append_router_full_debug_log_sync(entry: &serde_json::Value) -> Result<(), String> {
    static ROUTER_FULL_DEBUG_LOG_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _write_guard = ROUTER_FULL_DEBUG_LOG_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|error| error.to_string())?;
    let path = router_full_debug_log_path()?;
    ensure_parent_dir(&path)?;
    let line = serde_json::to_string(entry)
        .map_err(|error| format!("serialize router full debug log failed: {}", error))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| {
            format!(
                "open router full debug log failed: {}, path: {}",
                error,
                path.display()
            )
        })?;
    writeln!(file, "{}", line).map_err(|error| {
        format!(
            "write router full debug log failed: {}, path: {}",
            error,
            path.display()
        )
    })?;
    file.flush().map_err(|error| {
        format!(
            "flush router full debug log failed: {}, path: {}",
            error,
            path.display()
        )
    })
}

fn append_router_debug_log_sync(entry: &serde_json::Value) -> Result<(), String> {
    static ROUTER_DEBUG_LOG_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _write_guard = ROUTER_DEBUG_LOG_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|error| error.to_string())?;
    let path = router_debug_log_path()?;
    ensure_parent_dir(&path)?;
    let line = serde_json::to_string(entry)
        .map_err(|error| format!("serialize router debug log failed: {}", error))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| {
            format!(
                "open router debug log failed: {}, path: {}",
                error,
                path.display()
            )
        })?;
    writeln!(file, "{}", line).map_err(|error| {
        format!(
            "write router debug log failed: {}, path: {}",
            error,
            path.display()
        )
    })?;
    file.flush().map_err(|error| {
        format!(
            "flush router debug log failed: {}, path: {}",
            error,
            path.display()
        )
    })
}

fn sanitize_router_debug_value(value: serde_json::Value) -> serde_json::Value {
    sanitize_router_debug_value_for_key(None, value)
}

fn sanitize_router_full_debug_value(value: serde_json::Value) -> serde_json::Value {
    sanitize_router_full_debug_value_for_key(None, value)
}

fn sanitize_router_full_debug_value_for_key(
    key: Option<&str>,
    value: serde_json::Value,
) -> serde_json::Value {
    if key.map(router_debug_key_is_sensitive).unwrap_or(false) {
        return serde_json::Value::String("<redacted>".to_string());
    }

    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let sanitized = sanitize_router_full_debug_value_for_key(Some(&key), value);
                    (key, sanitized)
                })
                .collect(),
        ),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(|item| sanitize_router_full_debug_value_for_key(None, item))
                .collect(),
        ),
        value => value,
    }
}

fn sanitize_router_debug_value_for_key(
    key: Option<&str>,
    value: serde_json::Value,
) -> serde_json::Value {
    if key.map(router_debug_key_is_sensitive).unwrap_or(false) {
        return serde_json::Value::String("<redacted>".to_string());
    }

    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let sanitized = sanitize_router_debug_value_for_key(Some(&key), value);
                    (key, sanitized)
                })
                .collect(),
        ),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(|item| sanitize_router_debug_value_for_key(None, item))
                .collect(),
        ),
        serde_json::Value::String(text) => {
            serde_json::Value::String(limit_router_debug_string(&text))
        }
        value => value,
    }
}

fn router_debug_key_is_sensitive(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("authorization")
        || key.contains("api_key")
        || key.contains("apikey")
        || key == "key"
        || key.contains("token")
        || key.contains("cookie")
        || key.contains("secret")
        || key.contains("password")
}

fn limit_router_debug_string(text: &str) -> String {
    if text.len() <= ROUTER_DEBUG_STRING_LIMIT {
        return text.to_string();
    }
    let mut truncated = text
        .chars()
        .take(ROUTER_DEBUG_STRING_LIMIT)
        .collect::<String>();
    truncated.push_str(&format!(
        "\n<truncated: original_bytes={}, kept_bytes={}>",
        text.len(),
        truncated.len()
    ));
    truncated
}

fn router_debug_body_value(body: &str) -> serde_json::Value {
    let limited = limit_router_debug_body(body);
    serde_json::from_str::<serde_json::Value>(&limited)
        .unwrap_or_else(|_| serde_json::Value::String(limited))
}

fn router_full_debug_body_value(body: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(body)
        .map(sanitize_router_full_debug_value)
        .unwrap_or_else(|_| serde_json::Value::String(redact_sensitive_text(body)))
}

fn router_full_debug_sse_line_value(line: &str) -> serde_json::Value {
    let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
    let Some(data) = trimmed.trim_start().strip_prefix("data:") else {
        return serde_json::Value::String(redact_sensitive_text(trimmed));
    };
    let data = data.trim();
    if data == "[DONE]" {
        return serde_json::json!({ "line": "data: [DONE]" });
    }
    match serde_json::from_str::<serde_json::Value>(data) {
        Ok(value) => serde_json::json!({
            "line_prefix": "data:",
            "data": sanitize_router_full_debug_value(value)
        }),
        Err(_) => serde_json::Value::String(redact_sensitive_text(trimmed)),
    }
}



fn limit_router_debug_body(body: &str) -> String {
    if body.len() <= ROUTER_DEBUG_BODY_LIMIT {
        return body.to_string();
    }
    let mut truncated = body
        .chars()
        .take(ROUTER_DEBUG_BODY_LIMIT)
        .collect::<String>();
    truncated.push_str(&format!(
        "\n<truncated body: original_bytes={}, kept_bytes={}>",
        body.len(),
        truncated.len()
    ));
    truncated
}

fn append_router_log_entry_sync(log_entry: &RouterLogEntry) -> Result<(), String> {
    static ROUTER_LOG_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _write_guard = ROUTER_LOG_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|error| error.to_string())?;
    let path = router_log_path()?;
    ensure_parent_dir(&path)?;
    let line = serde_json::to_string(log_entry)
        .map_err(|error| format!("serialize router log failed: {}", error))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| {
            format!(
                "open router log failed: {}, path: {}",
                error,
                path.display()
            )
        })?;
    writeln!(file, "{}", line).map_err(|error| {
        format!(
            "write router log failed: {}, path: {}",
            error,
            path.display()
        )
    })?;
    file.flush().map_err(|error| {
        format!(
            "flush router log failed: {}, path: {}",
            error,
            path.display()
        )
    })?;

    if let Err(error) = insert_audit_log(
        &log_entry.time,
        &log_entry.source_ip,
        &log_entry.method,
        &log_entry.path,
        &log_entry.status,
        &log_entry.target_provider,
        &log_entry.cost,
        log_entry.input_tokens,
        log_entry.output_tokens,
        log_entry.cached_input_tokens,
        log_entry.total_tokens,
        &log_entry.usage_source,
        &log_entry.error_detail,
    ) {
        eprintln!("audit log write error: {}", error);
    }

    Ok(())
}

fn append_account_usage_router_log(
    url: &str,
    account_id: &str,
    status: &str,
    error_detail: Option<&str>,
) {
    push_router_log(RouterLogEntry {
        time: current_log_time(),
        source_ip: "127.0.0.1".to_string(),
        method: "GET".to_string(),
        path: url.to_string(),
        status: status.to_string(),
        target_provider: "official".to_string(),
        cost: EMPTY_LOG_VALUE.to_string(),
        input_tokens: 0,
        output_tokens: 0,
        cached_input_tokens: 0,
        total_tokens: 0,
        usage_source: "account_usage".to_string(),
        error_detail: error_detail
            .map(|value| format!("account={}, error={}", mask_secret(account_id), value))
            .unwrap_or_else(|| format!("account={}", mask_secret(account_id))),
    });
}

fn read_router_log_entries(limit: usize) -> Result<Vec<RouterLogEntry>, String> {
    let path = router_log_path()?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(format!(
                "read router log failed: {}, path: {}",
                error,
                path.display()
            ))
        }
    };
    let mut logs = Vec::new();
    for line in text.lines().rev() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(log) = serde_json::from_str::<RouterLogEntry>(line) {
            logs.push(log);
            if logs.len() >= limit {
                break;
            }
        }
    }
    Ok(logs)
}

fn read_request_log_token_usage_summary() -> Result<TokenUsageSummary, String> {
    let mut summary = TokenUsageSummary::default();
    for log in read_router_log_entries(REQUEST_LOG_LIMIT)? {
        summary.router_input_tokens = summary.router_input_tokens.saturating_add(log.input_tokens);
        summary.router_output_tokens = summary
            .router_output_tokens
            .saturating_add(log.output_tokens);
        summary.router_cached_input_tokens = summary
            .router_cached_input_tokens
            .saturating_add(log.cached_input_tokens);
    }
    for log in read_account_proxy_log_entries(REQUEST_LOG_LIMIT)? {
        summary.account_proxy_input_tokens = summary
            .account_proxy_input_tokens
            .saturating_add(log.input_tokens);
        summary.account_proxy_output_tokens = summary
            .account_proxy_output_tokens
            .saturating_add(log.output_tokens);
        summary.account_proxy_cached_input_tokens = summary
            .account_proxy_cached_input_tokens
            .saturating_add(log.cached_input_tokens);
    }
    Ok(summary)
}

fn append_account_proxy_log_entry(log_entry: &AccountProxyLogEntry) -> Result<(), String> {
    enqueue_router_log_write(RouterLogWriteTask::AccountProxy(log_entry.clone()))
}

fn append_account_proxy_log_entry_sync(log_entry: &AccountProxyLogEntry) -> Result<(), String> {
    static ACCOUNT_PROXY_LOG_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _write_guard = ACCOUNT_PROXY_LOG_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|error| error.to_string())?;
    let path = account_proxy_log_path()?;
    ensure_parent_dir(&path)?;
    let line = serde_json::to_string(log_entry)
        .map_err(|error| format!("serialize account proxy log failed: {}", error))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| {
            format!(
                "open account proxy log failed: {}, path: {}",
                error,
                path.display()
            )
        })?;
    writeln!(file, "{}", line).map_err(|error| {
        format!(
            "write account proxy log failed: {}, path: {}",
            error,
            path.display()
        )
    })?;
    file.flush().map_err(|error| {
        format!(
            "flush account proxy log failed: {}, path: {}",
            error,
            path.display()
        )
    })
}

fn enqueue_router_log_write(task: RouterLogWriteTask) -> Result<(), String> {
    match router_log_write_sender().send(task) {
        Ok(()) => Ok(()),
        Err(error) => write_router_log_task_sync(error.0),
    }
}

fn router_log_write_sender() -> &'static mpsc::Sender<RouterLogWriteTask> {
    static ROUTER_LOG_WRITE_SENDER: OnceLock<mpsc::Sender<RouterLogWriteTask>> = OnceLock::new();
    ROUTER_LOG_WRITE_SENDER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<RouterLogWriteTask>();
        thread::spawn(move || {
            for task in receiver {
                if let Err(error) = write_router_log_task_sync(task) {
                    eprintln!("router log write error: {}", error);
                }
            }
        });
        sender
    })
}

fn write_router_log_task_sync(task: RouterLogWriteTask) -> Result<(), String> {
    match task {
        RouterLogWriteTask::RouterLog(log_entry) => append_router_log_entry_sync(&log_entry),
        RouterLogWriteTask::AccountProxy(log_entry) => {
            append_account_proxy_log_entry_sync(&log_entry)
        }
    }
}

fn read_account_proxy_log_entries(limit: usize) -> Result<Vec<AccountProxyLogEntry>, String> {
    let path = account_proxy_log_path()?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(format!(
                "read account proxy log failed: {}, path: {}",
                error,
                path.display()
            ))
        }
    };
    let mut logs = Vec::new();
    for line in text.lines().rev() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(log) = serde_json::from_str::<AccountProxyLogEntry>(line) {
            logs.push(log);
            if logs.len() >= limit {
                break;
            }
        }
    }
    Ok(logs)
}

fn append_app_log_entry(log_entry: &AppOperationLogEntry) -> Result<(), String> {
    static APP_LOG_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _write_guard = APP_LOG_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|error| error.to_string())?;
    if is_recent_duplicate_app_log(log_entry) {
        return Ok(());
    }
    rotate_app_log_if_needed()?;
    let path = app_log_path()?;
    ensure_parent_dir(&path)?;
    let line = serde_json::to_string(log_entry)
        .map_err(|error| format!("序列化应用日志失败：{}", error))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| {
            format!(
                "鎵撳紑搴旂敤鏃ュ織澶辫触：{}锛岃矾寰勶細{}",
                error,
                path.display()
            )
        })?;

    writeln!(file, "{}", line).map_err(|error| {
        format!(
            "鍐欏叆搴旂敤鏃ュ織澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })?;
    file.flush().map_err(|error| {
        format!(
            "鍒锋柊搴旂敤鏃ュ織澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })?;
    drop(file);
    rotate_app_log_if_needed()
}

fn is_recent_duplicate_app_log(log_entry: &AppOperationLogEntry) -> bool {
    static RECENT_APP_LOG: OnceLock<Mutex<Option<RecentAppOperationLog>>> = OnceLock::new();
    let Ok(mut recent_log) = RECENT_APP_LOG.get_or_init(|| Mutex::new(None)).lock() else {
        return false;
    };

    if let Some(recent) = recent_log.as_ref() {
        let same_log = recent.level == log_entry.level
            && recent.module == log_entry.module
            && recent.action == log_entry.action
            && recent.message == log_entry.message
            && recent.detail == log_entry.detail;
        if same_log && recent.at.elapsed().as_millis() <= APP_LOG_DEDUP_WINDOW_MS {
            return true;
        }
    }

    *recent_log = Some(RecentAppOperationLog {
        level: log_entry.level.clone(),
        module: log_entry.module.clone(),
        action: log_entry.action.clone(),
        message: log_entry.message.clone(),
        detail: log_entry.detail.clone(),
        at: Instant::now(),
    });
    false
}

fn rotate_app_log_if_needed() -> Result<(), String> {
    let path = app_log_path()?;
    let Ok(metadata) = fs::metadata(&path) else {
        return Ok(());
    };

    if metadata.len() <= APP_LOG_MAX_SIZE_BYTES {
        return Ok(());
    }

    let text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "璇诲彇搴旂敤鏃ュ織澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })?;
    let keep_start = text.len().saturating_sub(APP_LOG_TRIM_KEEP_BYTES);
    let mut trimmed = text[keep_start..].to_string();

    if let Some(newline_index) = trimmed.find('\n') {
        trimmed = trimmed[(newline_index + 1)..].to_string();
    }

    fs::write(&path, trimmed).map_err(|error| {
        format!(
            "娓呯悊搴旂敤鏃ュ織澶辫触：{}锛岃矾寰勶細{}",
            error,
            path.display()
        )
    })
}

fn app_log_matches_keyword(log: &AppOperationLogEntry, keyword: &str) -> bool {
    log.level.to_lowercase().contains(keyword)
        || log.module.to_lowercase().contains(keyword)
        || log.action.to_lowercase().contains(keyword)
        || log.message.to_lowercase().contains(keyword)
        || log
            .detail
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
            .contains(keyword)
}

fn normalize_log_level(level: &str) -> String {
    match level {
        "warn" => "warn".to_string(),
        "error" => "error".to_string(),
        _ => "info".to_string(),
    }
}

fn select_provider_route(
    config: &RouterProviderConfig,
    requested_model: Option<&str>,
) -> Option<ProviderRoute> {
    let model = requested_model?;
    select_provider_route_by_slug(config, model)
}

fn select_provider_route_by_slug(
    config: &RouterProviderConfig,
    model: &str,
) -> Option<ProviderRoute> {
    select_provider_routes_by_model(config, model).into_iter().next()
}

/// Builds a priority-aware weighted route queue.  A model is routed by slug,
/// display name, or an explicit `modelMappings` entry.  This keeps legacy
/// single-model configuration working while allowing multiple provider
/// entries to serve the same Codex model.
fn select_provider_routes_by_model(
    config: &RouterProviderConfig,
    model: &str,
) -> Vec<ProviderRoute> {
    let model = model.trim();
    let mut routes = HashMap::new();
    let mut candidates = Vec::new();

    for (slug, route_value) in &config.0 {
        let Ok(route_item) = serde_json::from_value::<ProviderRouteFileItem>(route_value.clone()) else {
            continue;
        };
        let matches = slug == model
            || route_item.display_name.trim() == model
            || route_item.model_mappings.iter().any(|mapping| mapping.trim() == model);
        if !matches {
            continue;
        }
        if let Some(route) = provider_route_from_item(slug, route_item) {
            candidates.push(DispatchCandidate {
                key: slug.clone(),
                priority: route.priority,
                weight: route.weight,
            });
            routes.insert(slug.clone(), route);
        }
    }

    router_dispatcher::order_candidates(model, candidates)
        .into_iter()
        .filter_map(|candidate| routes.remove(&candidate.key))
        .collect()
}

fn provider_route_from_item(
    slug: &str,
    route_item: ProviderRouteFileItem,
) -> Option<ProviderRoute> {
    if !route_item.enabled {
        return None;
    }

    Some(ProviderRoute {
        provider: slug.to_string(),
        base_url: route_item
            .base_url
            .trim()
            .trim_matches('`')
            .trim()
            .to_string(),
        api_key: route_item
            .api_key
            .trim()
            .trim_matches('`')
            .trim()
            .to_string(),
        real_model: route_item
            .real_model
            .trim()
            .trim_matches('`')
            .trim()
            .to_string(),
        proxy_url: normalize_proxy_url(&route_item.proxy_url).unwrap_or_default(),
        proxy_mode: normalize_provider_proxy_mode(&route_item.proxy_mode),
        protocol_type: normalize_protocol_type(&route_item.protocol_type),
        endpoint_path: normalize_endpoint_path(&route_item.endpoint_path),
        priority: route_item.priority,
        weight: normalize_provider_weight(route_item.weight),
    })
}

fn resolve_provider_route_proxy_url(route: &ProviderRoute) -> Option<String> {
    match normalize_provider_proxy_mode(&route.proxy_mode).as_str() {
        "manual" => normalize_proxy_url(&route.proxy_url),
        "direct" => None,
        _ => load_app_settings()
            .ok()
            .and_then(|settings| normalize_proxy_url(&settings.official_proxy_url)),
    }
}

fn normalize_provider_proxy_mode(proxy_mode: &str) -> String {
    match proxy_mode.trim().to_ascii_lowercase().as_str() {
        "manual" | "proxy" => "manual".to_string(),
        "direct" | "none" => "direct".to_string(),
        _ => "default".to_string(),
    }
}

fn normalize_proxy_url(proxy_url: &str) -> Option<String> {
    let value = proxy_url.trim().trim_matches('`').trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn build_upstream_get_request(
    url: &str,
    proxy_url: Option<&str>,
    timeout_seconds: u64,
) -> ureq::Request {
    upstream_agent(proxy_url)
        .get(url)
        .timeout(Duration::from_secs(timeout_seconds))
}

fn build_upstream_post_request(
    url: &str,
    proxy_url: Option<&str>,
    timeout_seconds: u64,
) -> ureq::Request {
    upstream_agent(proxy_url)
        .post(url)
        .timeout(Duration::from_secs(timeout_seconds))
}

fn upstream_agent(proxy_url: Option<&str>) -> ureq::Agent {
    static UPSTREAM_AGENTS: OnceLock<Mutex<HashMap<String, ureq::Agent>>> = OnceLock::new();
    let normalized_proxy_url = proxy_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let cache_key = normalized_proxy_url
        .as_deref()
        .map(|proxy_url| format!("proxy:{}", proxy_url))
        .unwrap_or_else(|| "direct".to_string());

    let agents = UPSTREAM_AGENTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = agents.lock() {
        if let Some(agent) = guard.get(&cache_key) {
            return agent.clone();
        }
        let agent = build_upstream_agent(normalized_proxy_url.as_deref());
        guard.insert(cache_key, agent.clone());
        return agent;
    }

    build_upstream_agent(normalized_proxy_url.as_deref())
}

fn build_upstream_agent(proxy_url: Option<&str>) -> ureq::Agent {
    match proxy_url.and_then(|proxy_url| ureq::Proxy::new(proxy_url).ok()) {
        Some(proxy) => ureq::builder().proxy(proxy).build(),
        None => ureq::builder().build(),
    }
}

fn build_custom_upstream_post_request(
    url: &str,
    proxy_url: Option<&str>,
    protocol_type: &str,
    api_key: &str,
    authorization: &str,
    timeout_seconds: u64,
) -> ureq::Request {
    let _ = authorization; // Kept in the signature for existing call sites.
    let protocol = ProviderProtocol::from_config(protocol_type);
    protocol
        .adapter()
        .apply_authentication(
            build_upstream_post_request(url, proxy_url, timeout_seconds)
                .set(HEADER_CONTENT_TYPE, HEADER_JSON),
            api_key,
        )
}

fn send_custom_upstream_request_with_retries(
    url: &str,
    proxy_url: Option<&str>,
    protocol_type: &str,
    api_key: &str,
    authorization: &str,
    body: &str,
    timeout_seconds: u64,
) -> Result<ureq::Response, ureq::Error> {
    let mut last_error = None;

    for attempt in 1..=UPSTREAM_NETWORK_RETRY_ATTEMPTS {
        let request = build_custom_upstream_post_request(
            url,
            proxy_url,
            protocol_type,
            api_key,
            authorization,
            timeout_seconds,
        );

        match request.send_string(body) {
            Ok(response) => return Ok(response),
            Err(error)
                if attempt < UPSTREAM_NETWORK_RETRY_ATTEMPTS
                    && upstream_error_is_retryable(&error) =>
            {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(UPSTREAM_NETWORK_RETRY_DELAY_MS));
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.expect("retry loop should store the last upstream error"))
}

fn custom_upstream_timeout_seconds(uses_image_generation_tool: bool) -> u64 {
    if uses_image_generation_tool {
        IMAGE_GENERATION_FORWARD_TIMEOUT_SECONDS
    } else {
        FORWARD_TIMEOUT_SECONDS
    }
}

fn upstream_error_is_retryable(error: &ureq::Error) -> bool {
    if !matches!(error, ureq::Error::Transport(_)) {
        return false;
    }

    let message = error.to_string().to_ascii_lowercase();
    message.contains("network error")
        || message.contains("timed out")
        || message.contains("timeout")
        || message.contains("status line")
        || message.contains("failed to connect")
        || message.contains("connection")
        || message.contains("connection reset")
        || message.contains("forcibly closed")
        || message.contains("os error 10054")
        || message.contains("os error 10060")
        || message.contains("强迫关闭")
}

fn format_custom_upstream_error(error: &ureq::Error) -> String {
    if upstream_error_is_retryable(error) && UPSTREAM_NETWORK_RETRY_ATTEMPTS > 1 {
        format!(
            "{}; rebuilt upstream connection and retried {} time(s), still failed",
            error,
            UPSTREAM_NETWORK_RETRY_ATTEMPTS - 1
        )
    } else {
        error.to_string()
    }
}

fn normalize_protocol_type(protocol_type: &str) -> String {
    ProviderProtocol::from_config(protocol_type)
        .as_str()
        .to_string()
}

fn normalize_endpoint_path(endpoint_path: &str) -> String {
    let trimmed = endpoint_path.trim().trim_matches('`').trim();

    if trimmed.is_empty() {
        return String::new();
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with('/')
    {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    }
}

fn is_anthropic_protocol(protocol_type: &str) -> bool {
    ProviderProtocol::from_config(protocol_type) == ProviderProtocol::Anthropic
}

fn build_upstream_endpoint_url(
    base_url: &str,
    endpoint_path: &str,
    default_endpoint_path: &str,
) -> String {
    let endpoint = normalize_endpoint_path(endpoint_path);

    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return endpoint;
    }

    let endpoint = if endpoint.is_empty() {
        default_endpoint_path
    } else {
        endpoint.as_str()
    };
    format!("{}{}", base_url.trim().trim_end_matches('/'), endpoint)
}

fn build_upstream_models_url(base_url: &str) -> String {
    let trimmed_base_url = base_url.trim().trim_end_matches('/');

    if trimmed_base_url.ends_with(MODELS_ENDPOINT_SUFFIX) {
        trimmed_base_url.to_string()
    } else if trimmed_base_url.ends_with(V1_PATH_SUFFIX) {
        format!("{}{}", trimmed_base_url, MODELS_PATH)
    } else {
        format!("{}{}{}", trimmed_base_url, V1_PATH_SUFFIX, MODELS_PATH)
    }
}

fn parse_provider_model_ids(body: &str) -> Result<Vec<String>, String> {
    let root = serde_json::from_str::<serde_json::Value>(body)
        .map_err(|error| format!("瑙ｆ瀽妯″瀷鍒楄〃鍝嶅簲澶辫触：{}", error))?;
    let data = root
        .get("data")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "模型列表响应缺少 data 数组".to_string())?;
    let mut models = Vec::new();

    for item in data {
        if let Some(id) = item
            .get("id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            models.push(id.to_string());
        }
    }

    if models.is_empty() {
        return Err("模型列表为空或缺少id 字段".to_string());
    }

    models.sort();
    models.dedup();
    Ok(models)
}







fn write_http_response(
    stream: &mut TcpStream,
    status_line: &str,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let headers = format!(
        "{}\r\nContent-Type: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, x-api-key\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status_line,
        content_type,
        body.len(),
    );

    stream.write_all(headers.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn codex_sse_transport_status_code(status_code: u16, content_type: &str) -> u16 {
    if status_code >= 400
        && content_type
            .to_ascii_lowercase()
            .contains("text/event-stream")
    {
        HTTP_OK
    } else {
        status_code
    }
}

fn write_streaming_response_headers(
    stream: &mut TcpStream,
    status_line: &str,
    content_type: &str,
) -> std::io::Result<()> {
    let headers = format!(
        "{}\r\nContent-Type: {}\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, x-api-key\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nConnection: close\r\n\r\n",
        status_line, content_type
    );
    stream.write_all(headers.as_bytes())?;
    stream.flush()
}























fn push_router_log(log_entry: RouterLogEntry) {
    let _ = append_router_log_entry(&log_entry);
    if let Ok(mut logs) = router_logs().lock() {
        logs.insert(0, log_entry);
        logs.truncate(REQUEST_LOG_LIMIT);
    }
}







fn build_health_response(started_at: Instant, router_port: u16) -> String {
    format!(
        "{{\"status\":\"ok\",\"service\":\"{}\",\"version\":\"{}\",\"host\":\"{}\",\"port\":{},\"pid\":{},\"healthPath\":\"{}\",\"uptimeSeconds\":{},\"started\":true,\"forwardingEnabled\":{}}}",
        SERVICE_NAME,
        SERVICE_VERSION,
        ROUTER_HOST,
        router_port,
        std::process::id(),
        HEALTH_PATH,
        started_at.elapsed().as_secs(),
        load_provider_config().is_ok(),
    )
}

pub use app::{run, run_router_only};

#[cfg(test)]
mod tests {
    use super::{
        active_rollout_path, build_openai_chat_body, codex_completed_sse_from_text,
        codex_sse_error_response, codex_sse_transport_status_code, collect_active_exec_cell_ids,
        collect_namespace_tool_mappings, custom_empty_response_is_image_generation_unsupported,
        custom_image_generation_response_needs_notice, custom_upstream_timeout_seconds,
        ensure_official_sse_completed, extract_tool_definition_name, guard_wait_tool_for_upstream,
        is_hosted_image_generation_tool,
        normalize_official_codex_payload, normalize_repeated_tool_names_in_body,
        normalize_repeated_tool_names_in_body_with_available,
        normalize_repeated_tool_names_in_sse_line, normalize_upstream_tool_name,
        openai_chat_sse_body_to_codex_sse,
        openai_chat_tool_calls_to_codex_sse, remove_codex_router_top_level_keys,
        remove_managed_codex_router_block, request_uses_image_generation_tool,
        response_contains_tool_call_request, rollout_file_date, sanitize_custom_responses_payload,
        sanitize_inactive_wait_sse_line, sse_line_is_done, sse_text_has_response_completed,
        stream_raw_upstream_sse, target_process_filter_script, wait_tool_call_is_valid,
        ProviderRoute,
        CODEX_ROUTER_TOP_MANAGED_END_MARKER,
        CODEX_ROUTER_TOP_MANAGED_START_MARKER, CUSTOM_IMAGE_GENERATION_UNSUPPORTED_MESSAGE,
        CUSTOM_UPSTREAM_EMPTY_RESPONSE_MESSAGE, FORWARD_TIMEOUT_SECONDS, HEADER_EVENT_STREAM,
        HEADER_JSON, HTTP_BAD_GATEWAY, HTTP_OK, HTTP_TOO_MANY_REQUESTS,
        IMAGE_GENERATION_FORWARD_TIMEOUT_SECONDS,
    };
    use std::path::Path;

    #[test]
    fn parses_rollout_file_date() {
        assert_eq!(
            rollout_file_date(
                "rollout-2026-05-24T14-36-57-019e58b3-7a72-7ee3-8544-6c7c063b35aa.jsonl"
            ),
            Some("2026-05-24".to_string())
        );
    }

    #[test]
    fn rejects_invalid_rollout_file_date() {
        assert_eq!(rollout_file_date("rollout-invalid.jsonl"), None);
        assert_eq!(rollout_file_date("session-2026-05-24.jsonl"), None);
    }

    #[test]
    fn maps_archived_rollout_to_active_date_directory() {
        let file_name = "rollout-2026-05-24T14-36-57-019e58b3-7a72-7ee3-8544-6c7c063b35aa.jsonl";
        assert_eq!(
            active_rollout_path(Path::new("sessions"), "2026-05-24", file_name),
            Path::new("sessions")
                .join("2026")
                .join("05")
                .join("24")
                .join(file_name)
        );
    }

    #[test]
    fn removes_only_top_level_router_keys() {
        let text = r#"model = "gpt-5.5"
model_reasoning_effort = "medium"
model_provider = "ai-router"
model_catalog_json = "catalog.json"
model_providers."ai-router".name = "Router"

[windows]
sandbox = "elevated"

[projects.'example']
model = "project-model"
"#;

        let cleaned = remove_codex_router_top_level_keys(text);

        assert!(!cleaned.contains("model_provider = \"ai-router\""));
        assert!(!cleaned.contains("model_catalog_json = \"catalog.json\""));
        assert!(!cleaned.contains("model_providers.\"ai-router\".name"));
        assert!(!cleaned.contains("model = \"gpt-5.5\""));
        assert!(cleaned.contains("model_reasoning_effort = \"medium\""));
        assert!(cleaned.contains("[windows]\nsandbox = \"elevated\""));
        assert!(cleaned.contains("[projects.'example']\nmodel = \"project-model\""));
    }

    #[test]
    fn removes_managed_block_without_touching_windows_sandbox() {
        let text = format!(
            "{start}\nmodel_provider = \"ai-router\"\nmodel_providers.\"ai-router\".name = \"Router\"\n{end}\n\n[windows]\nsandbox = \"elevated\"\n",
            start = CODEX_ROUTER_TOP_MANAGED_START_MARKER,
            end = CODEX_ROUTER_TOP_MANAGED_END_MARKER,
        );

        let cleaned = remove_managed_codex_router_block(&text);

        assert!(!cleaned.contains("model_provider = \"ai-router\""));
        assert!(cleaned.contains("[windows]\nsandbox = \"elevated\""));
    }

    #[test]
    fn codex_sse_errors_use_ok_transport_and_done_frame() {
        let response = codex_sse_error_response(
            HTTP_BAD_GATEWAY,
            "upstream_empty_response",
            CUSTOM_UPSTREAM_EMPTY_RESPONSE_MESSAGE,
            "test-provider".to_string(),
        );

        assert_eq!(response.status_code, HTTP_BAD_GATEWAY);
        assert_eq!(
            codex_sse_transport_status_code(response.status_code, &response.content_type),
            HTTP_OK
        );
        assert!(response.content_type.contains("text/event-stream"));
        assert!(response.body.contains("event: response.failed"));
        assert!(response.body.contains("\"status\":\"failed\""));
        assert!(response
            .body
            .contains("\"code\":\"upstream_empty_response\""));
        assert!(response
            .body
            .contains("\"type\":\"upstream_empty_response\""));
        assert!(!response.body.contains("event: response.completed"));
        assert!(response
            .body
            .contains(CUSTOM_UPSTREAM_EMPTY_RESPONSE_MESSAGE));
        assert!(response.body.ends_with("data: [DONE]\n\n"));
        assert!(response.error_detail.contains("upstream_empty_response"));
    }

    #[test]
    fn raw_upstream_read_error_terminates_client_sse() {
        let upstream_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let upstream_address = upstream_listener.local_addr().unwrap();
        let upstream = std::thread::spawn(move || {
            let (mut socket, _) = upstream_listener.accept().unwrap();
            let response = concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Type: text/event-stream\r\n",
                "Transfer-Encoding: chunked\r\n",
                "Connection: close\r\n\r\n",
                "100\r\n",
                "data: {\"type\":\"response.created\"}\n\n"
            );
            std::io::Write::write_all(&mut socket, response.as_bytes()).unwrap();
            std::io::Write::flush(&mut socket).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(100));
            socket.shutdown(std::net::Shutdown::Write).unwrap();
        });

        let downstream_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let downstream_address = downstream_listener.local_addr().unwrap();
        let mut downstream_client = std::net::TcpStream::connect(downstream_address).unwrap();
        downstream_client
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let (mut downstream_server, _) = downstream_listener.accept().unwrap();

        let response = ureq::get(&format!("http://{upstream_address}"))
            .call()
            .unwrap();
        let result =
            stream_raw_upstream_sse(response, &mut downstream_server, None, &[], &[]);
        assert!(result.is_err());
        drop(downstream_server);
        upstream.join().unwrap();

        let mut body = String::new();
        std::io::Read::read_to_string(&mut downstream_client, &mut body).unwrap();
        assert!(body.contains("event: response.failed"));
        assert!(body.contains("\"code\":\"upstream_stream_disconnected\""));
        assert!(body.ends_with("data: [DONE]\n\n"));
    }

    #[test]
    fn chatgpt_restart_filter_includes_codex_desktop_package_processes() {
        let script = target_process_filter_script(1234, "chatgpt");
        assert!(script.contains("$_.Name -ieq 'ChatGPT'"));
        assert!(script.contains("$_.Path -like '*\\OpenAI.Codex_*'"));
        assert!(script.contains("$_.Id -ne $currentPid"));
    }

    #[test]
    fn rate_limit_is_rendered_without_failed_event_to_avoid_reconnect_loop() {
        let response = codex_sse_error_response(
            HTTP_TOO_MANY_REQUESTS,
            "upstream_rate_limited",
            "model cooldown",
            "test-provider".to_string(),
        );

        assert_eq!(response.status_code, HTTP_TOO_MANY_REQUESTS);
        assert!(response.body.contains("event: response.completed"));
        assert!(response.body.contains("model cooldown"));
        assert!(!response.body.contains("event: response.failed"));
        assert!(response.body.ends_with("data: [DONE]\n\n"));
    }

    #[test]
    fn image_generation_errors_are_normalized_for_codex_sse() {
        let response = codex_sse_error_response(
            HTTP_BAD_GATEWAY,
            "custom_image_generation_not_supported",
            "ignored provider message",
            "test-provider".to_string(),
        );

        assert_eq!(
            codex_sse_transport_status_code(response.status_code, HEADER_EVENT_STREAM),
            HTTP_OK
        );
        assert!(response
            .body
            .contains(CUSTOM_IMAGE_GENERATION_UNSUPPORTED_MESSAGE));
        assert!(response.body.contains("event: response.output_text.delta"));
        assert!(response.body.contains("\"status\":\"completed\""));
        assert!(response.body.ends_with("data: [DONE]\n\n"));
    }

    #[test]
    fn empty_image_generation_response_is_classified_as_image_unsupported() {
        assert!(custom_empty_response_is_image_generation_unsupported(
            true,
            r#"{"output":[]}"#,
            HEADER_JSON,
            "cpamc",
        ));
        assert!(!custom_empty_response_is_image_generation_unsupported(
            false,
            r#"{"output":[]}"#,
            HEADER_JSON,
            "cpamc",
        ));
        assert!(!custom_empty_response_is_image_generation_unsupported(
            true,
            r#"{"output":[{"type":"output_image","image_url":"https://example.test/cat.png"}]}"#,
            HEADER_JSON,
            "cpamc",
        ));
    }

    #[test]
    fn image_generation_plain_text_response_needs_notice() {
        assert!(custom_image_generation_response_needs_notice(
            r#"{"choices":[{"message":{"role":"assistant","content":"Now I will generate a cute kitten image."}}]}"#,
            HEADER_JSON,
        ));
    }

    #[test]
    fn image_generation_tool_call_response_does_not_need_notice() {
        let body = r#"{"choices":[{"message":{"role":"assistant","tool_calls":[{"id":"call_1","type":"function","function":{"name":"generate_image","arguments":"{\"prompt\":\"kitten\"}"}}]}}]}"#;

        assert!(response_contains_tool_call_request(body));
        assert!(!custom_image_generation_response_needs_notice(
            body,
            HEADER_JSON,
        ));
    }

    #[test]
    fn available_image_generation_tool_does_not_mark_plain_followup_as_image_request() {
        let payload = serde_json::json!({
            "model": "custom-model",
            "tools": [{"type": "image_generation"}],
            "input": [
                {"role": "user", "content": "给我生图"},
                {"type": "image_generation_call", "id": "img_1"},
                {"role": "user", "content": "这段代码哪里有问题？"}
            ]
        });

        assert!(!request_uses_image_generation_tool(&payload));
    }

    #[test]
    fn latest_user_image_prompt_marks_request_as_image_generation() {
        let payload = serde_json::json!({
            "model": "custom-model",
            "tools": [{"type": "image_generation"}],
            "input": [
                {"role": "user", "content": "先解释一下路由"},
                {"role": "assistant", "content": "好的"},
                {"role": "user", "content": "帮我生成一张路由工作流插画"}
            ]
        });

        assert!(request_uses_image_generation_tool(&payload));
    }

    #[test]
    fn chinese_image_prompt_marks_request_as_image_generation() {
        let payload = serde_json::json!({
            "model": "custom-model",
            "tools": [{"type": "image_generation"}],
            "input": [
                {"role": "user", "content": "给我生成一张可爱的小猫图片"}
            ]
        });

        assert!(request_uses_image_generation_tool(&payload));
    }

    #[test]
    fn explicit_image_tool_choice_marks_request_as_image_generation() {
        let payload = serde_json::json!({
            "model": "custom-model",
            "tools": [{"type": "image_generation"}],
            "tool_choice": {"type": "image_generation"},
            "input": "普通文本"
        });

        assert!(request_uses_image_generation_tool(&payload));
    }

    #[test]
    fn image_generation_requests_use_longer_upstream_timeout() {
        assert_eq!(
            custom_upstream_timeout_seconds(false),
            FORWARD_TIMEOUT_SECONDS
        );
        assert_eq!(
            custom_upstream_timeout_seconds(true),
            IMAGE_GENERATION_FORWARD_TIMEOUT_SECONDS
        );
    }

    #[test]
    fn completed_sse_without_done_gets_done_frame() {
        let body = "event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n";
        let completed = ensure_official_sse_completed(body.to_string());

        assert!(completed.contains("event: response.completed"));
        assert!(completed.ends_with("data: [DONE]\n\n"));
    }

    #[test]
    fn done_before_missing_completed_is_reordered() {
        let body =
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\ndata: [DONE]\n\n";
        let completed = ensure_official_sse_completed(body.to_string());
        let completed_index = completed.find("event: response.completed").unwrap();
        let done_index = completed.rfind("data: [DONE]").unwrap();

        assert!(completed_index < done_index);
        assert_eq!(completed.matches("data: [DONE]").count(), 1);
    }

    #[test]
    fn existing_completed_with_early_done_is_reordered() {
        let body = "event: response.created\ndata: {}\n\ndata: [DONE]\n\nevent: response.completed\ndata: {\"type\":\"response.completed\"}\n\ndata: [DONE]\n\n";
        let completed = ensure_official_sse_completed(body.to_string());
        let completed_index = completed.find("event: response.completed").unwrap();
        let done_index = completed.rfind("data: [DONE]").unwrap();

        assert!(completed_index < done_index);
        assert_eq!(completed.matches("data: [DONE]").count(), 1);
    }

    #[test]
    fn openai_chat_sse_body_is_converted_to_responses_sse() {
        let route = ProviderRoute {
            provider: "test".to_string(),
            base_url: "https://example.test/v1".to_string(),
            api_key: String::new(),
            real_model: "deepseek-v4-pro".to_string(),
            proxy_mode: "direct".to_string(),
            proxy_url: String::new(),
            protocol_type: "openai".to_string(),
            endpoint_path: "/chat/completions".to_string(),
            priority: 0,
            weight: 1,
        };
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}],\"usage\":null}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" world\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":2,\"total_tokens\":3}}\n\n",
            "data: [DONE]\n\n",
        );

        let converted = openai_chat_sse_body_to_codex_sse(body, &route, &[], &[], &[]).unwrap();

        assert!(converted.contains("event: response.output_text.delta"));
        assert!(converted.contains("hello world"));
        assert!(converted.contains("event: response.completed"));
        assert!(converted.ends_with("data: [DONE]\n\n"));
        assert!(!converted.contains("chat.completion.chunk"));
    }

    #[test]
    fn duplicated_upstream_tool_name_is_normalized_when_known() {
        let body = r#"{"choices":[{"message":{"role":"assistant","tool_calls":[{"id":"call_1","type":"function","function":{"name":"execexec","arguments":"{\"command\":\"pwd\"}"}}]}}]}"#;
        let root = serde_json::from_str::<serde_json::Value>(body).unwrap();
        let converted =
            openai_chat_tool_calls_to_codex_sse(&root, None, &["exec".to_string()], &[], &[])
                .unwrap();

        assert!(converted.contains(r#""name":"exec""#));
        assert!(!converted.contains(r#""name":"execexec""#));
        assert!(converted.contains("event: response.function_call_arguments.delta"));
        assert!(converted.contains("event: response.function_call_arguments.done"));
        assert!(converted.contains(r#""status":"in_progress""#));

        let events = converted
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .filter_map(|data| serde_json::from_str::<serde_json::Value>(data).ok())
            .collect::<Vec<_>>();
        assert_eq!(events[1]["item"]["name"], "exec");
        assert_eq!(events[4]["item"]["name"], "exec");
        assert_eq!(events[5]["response"]["output"][0]["name"], "exec");
    }

    #[test]
    fn codex_custom_tool_is_wrapped_for_chat_upstream() {
        let route = ProviderRoute {
            provider: "test".to_string(),
            base_url: "https://example.test/v1".to_string(),
            api_key: String::new(),
            real_model: "gpt-test".to_string(),
            proxy_mode: "direct".to_string(),
            proxy_url: String::new(),
            protocol_type: "openai".to_string(),
            endpoint_path: "/chat/completions".to_string(),
            priority: 0,
            weight: 1,
        };
        let payload = serde_json::json!({
            "input": [{"role": "user", "content": "run a command"}],
            "tools": [{"type": "custom", "name": "exec", "description": "shell"}],
            "tool_choice": {"type": "custom", "name": "exec"}
        });

        let body = build_openai_chat_body(&payload, &route);

        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["function"]["name"], "exec");
        assert_eq!(
            body["tools"][0]["function"]["parameters"]["required"][0],
            "input"
        );
        assert_eq!(body["tool_choice"]["function"]["name"], "exec");
    }

    #[test]
    fn chat_bridge_only_exposes_wait_for_a_real_active_exec_cell() {
        let route = ProviderRoute {
            provider: "test".to_string(),
            base_url: "https://example.test/v1".to_string(),
            api_key: String::new(),
            real_model: "gpt-test".to_string(),
            proxy_mode: "direct".to_string(),
            proxy_url: String::new(),
            protocol_type: "openai".to_string(),
            endpoint_path: "/chat/completions".to_string(),
            priority: 0,
            weight: 1,
        };
        let mut payload = serde_json::json!({
            "input": [{"role": "user", "content": "run a command"}],
            "tools": [
                {"type": "custom", "name": "exec", "description": "shell"},
                {
                    "type": "function",
                    "name": "wait",
                    "description": "wait for an exec cell",
                    "parameters": {
                        "type": "object",
                        "properties": {"cell_id": {"type": "string"}},
                        "required": ["cell_id"]
                    }
                }
            ],
            "parallel_tool_calls": true
        });

        let without_active_cell = build_openai_chat_body(&payload, &route);
        assert_eq!(without_active_cell["parallel_tool_calls"], false);
        assert_eq!(without_active_cell["tools"].as_array().unwrap().len(), 1);
        assert_eq!(without_active_cell["tools"][0]["function"]["name"], "exec");

        payload["input"] = serde_json::json!([
            {
                "type": "custom_tool_call",
                "call_id": "call_exec",
                "name": "exec",
                "input": "run"
            },
            {
                "type": "custom_tool_call_output",
                "call_id": "call_exec",
                "output": "Script running with cell ID cell-24"
            }
        ]);
        let with_active_cell = build_openai_chat_body(&payload, &route);
        let wait_tool = with_active_cell["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["function"]["name"] == "wait")
            .unwrap();
        assert_eq!(
            wait_tool["function"]["parameters"]["properties"]["cell_id"]["enum"],
            serde_json::json!(["cell-24"])
        );
        assert!(wait_tool["function"]["description"]
            .as_str()
            .unwrap()
            .contains("Never invent"));

        payload["input"].as_array_mut().unwrap().extend(
            serde_json::json!([
                {
                    "type": "function_call",
                    "call_id": "call_wait",
                    "name": "wait",
                    "arguments": "{\"cell_id\":\"cell-24\"}"
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_wait",
                    "output": "Script completed\nExit code: 0"
                }
            ])
            .as_array()
            .unwrap()
            .clone(),
        );
        let after_completion = build_openai_chat_body(&payload, &route);
        assert!(after_completion["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|tool| tool["function"]["name"] != "wait"));
    }

    #[test]
    fn responses_guard_filters_wait_namespace_until_exec_cell_is_running() {
        let mut payload = serde_json::json!({
            "input": [{"role": "user", "content": "work"}],
            "tools": [
                {
                    "type": "namespace",
                    "name": "exec",
                    "tools": [{"type": "custom", "name": "exec"}]
                },
                {
                    "type": "namespace",
                    "name": "wait",
                    "tools": [{
                        "type": "function",
                        "name": "wait",
                        "parameters": {
                            "type": "object",
                            "properties": {"cell_id": {"type": "string"}}
                        }
                    }]
                }
            ],
            "tool_choice": {"type": "function", "namespace": "wait", "name": "wait"},
            "parallel_tool_calls": true
        });

        guard_wait_tool_for_upstream(&mut payload);
        assert_eq!(payload["tools"].as_array().unwrap().len(), 1);
        assert_eq!(payload["tools"][0]["name"], "exec");
        assert_eq!(payload["tool_choice"], "auto");
        assert_eq!(payload["parallel_tool_calls"], false);

        payload["input"] = serde_json::json!([
            {
                "type": "custom_tool_call",
                "call_id": "call_exec",
                "name": "exec",
                "input": "run"
            },
            {
                "type": "custom_tool_call_output",
                "call_id": "call_exec",
                "output": "Script running with cell ID live-7"
            }
        ]);
        payload["tools"] = serde_json::json!([{
            "type": "namespace",
            "name": "wait",
            "tools": [{
                "type": "function",
                "name": "wait",
                "parameters": {
                    "type": "object",
                    "properties": {"cell_id": {"type": "string"}}
                }
            }]
        }]);
        guard_wait_tool_for_upstream(&mut payload);
        assert_eq!(
            payload["tools"][0]["tools"][0]["parameters"]["properties"]["cell_id"]["enum"],
            serde_json::json!(["live-7"])
        );
    }

    #[test]
    fn responses_stream_suppresses_wait_call_when_no_exec_cell_is_active() {
        let mut suppressed = std::collections::HashSet::new();
        let added = "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"fc_wait\",\"type\":\"function_call\",\"name\":\"wait\",\"arguments\":\"\"}}\n";
        let delta = "data: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc_wait\",\"delta\":\"{\\\"cell_id\\\":\\\"noop\\\"}\"}\n";
        let done = "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"fc_wait\",\"type\":\"function_call\",\"name\":\"wait\",\"arguments\":\"{\\\"cell_id\\\":\\\"noop\\\"}\"}}\n";
        let completed = "data: {\"type\":\"response.completed\",\"response\":{\"output\":[{\"id\":\"fc_wait\",\"type\":\"function_call\",\"name\":\"wait\",\"arguments\":\"{\\\"cell_id\\\":\\\"noop\\\"}\"},{\"id\":\"msg_1\",\"type\":\"message\",\"content\":[]}]}}\n";

        assert!(sanitize_inactive_wait_sse_line(added, &[], &mut suppressed).is_none());
        assert!(suppressed.contains("fc_wait"));
        assert!(sanitize_inactive_wait_sse_line(delta, &[], &mut suppressed).is_none());
        assert!(sanitize_inactive_wait_sse_line(done, &[], &mut suppressed).is_none());
        let completed = sanitize_inactive_wait_sse_line(completed, &[], &mut suppressed).unwrap();
        assert!(!completed.contains(r#""name":"wait""#));
        assert!(completed.contains(r#""type":"message""#));
    }

    #[test]
    fn wait_tool_call_requires_an_exact_active_cell_id() {
        let active = ["27".to_string()];
        for cell_id in ["noop", "none", "none2", "", "28"] {
            let item = serde_json::json!({
                "type": "function_call",
                "name": "wait",
                "arguments": serde_json::json!({"cell_id": cell_id}).to_string()
            });
            assert!(!wait_tool_call_is_valid(&item, &active), "{cell_id}");
        }
        let valid_string = serde_json::json!({
            "type": "function_call",
            "name": "wait",
            "arguments": "{\"cell_id\":\"27\"}"
        });
        let valid_object = serde_json::json!({
            "type": "custom_tool_call",
            "namespace": "wait",
            "input": {"cell_id": "27"}
        });
        assert!(wait_tool_call_is_valid(&valid_string, &active));
        assert!(wait_tool_call_is_valid(&valid_object, &active));
    }

    #[test]
    fn stale_running_exec_cells_are_cleared_by_later_activity() {
        for later_item in [
            serde_json::json!({"type": "message", "role": "assistant", "content": []}),
            serde_json::json!({
                "type": "function_call",
                "name": "exec",
                "call_id": "call_later",
                "arguments": "{}"
            }),
        ] {
            let payload = serde_json::json!({
                "input": [
                    {
                        "type": "custom_tool_call_output",
                        "call_id": "call_old",
                        "output": "Script running with cell ID 1"
                    },
                    later_item
                ]
            });
            assert!(collect_active_exec_cell_ids(&payload).is_empty());
        }
    }

    #[test]
    fn chat_bridge_round_trips_namespace_tool_name() {
        let route = ProviderRoute {
            provider: "test".to_string(),
            base_url: "https://example.test/v1".to_string(),
            api_key: String::new(),
            real_model: "gpt-test".to_string(),
            proxy_mode: "direct".to_string(),
            proxy_url: String::new(),
            protocol_type: "openai".to_string(),
            endpoint_path: "/chat/completions".to_string(),
            priority: 0,
            weight: 1,
        };
        let payload = serde_json::json!({
            "input": [{"role": "user", "content": "send"}],
            "tools": [{
                "type": "namespace",
                "name": "collaboration",
                "description": "agent tools",
                "tools": [{
                    "type": "function",
                    "name": "send_message",
                    "description": "send a message",
                    "parameters": {
                        "type": "object",
                        "properties": {"message": {"type": "string"}},
                        "required": ["message"]
                    }
                }]
            }]
        });

        let chat_body = build_openai_chat_body(&payload, &route);
        assert_eq!(
            chat_body["tools"][0]["function"]["name"],
            "collaboration__send_message"
        );
        let mappings = collect_namespace_tool_mappings(&payload);
        let root = serde_json::json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "call_send",
                        "type": "function",
                        "function": {
                            "name": "collaboration__send_message",
                            "arguments": "{\"message\":\"hello\"}"
                        }
                    }]
                }
            }]
        });
        let converted =
            openai_chat_tool_calls_to_codex_sse(&root, None, &[], &[], &mappings).unwrap();
        assert!(converted.contains(r#""name":"send_message""#));
        assert!(converted.contains(r#""namespace":"collaboration""#));
        assert!(!converted.contains(r#""name":"collaboration__send_message""#));
    }

    #[test]
    fn chat_bridge_deduplicates_identical_parallel_tool_calls() {
        let root = serde_json::json!({
            "choices": [{
                "message": {
                    "tool_calls": [
                        {
                            "id": "call_a",
                            "type": "function",
                            "function": {"name": "wait", "arguments": "{\"cell_id\":\"live-1\"}"}
                        },
                        {
                            "id": "call_b",
                            "type": "function",
                            "function": {"name": "wait", "arguments": "{\"cell_id\":\"live-1\"}"}
                        }
                    ]
                }
            }]
        });

        let converted = openai_chat_tool_calls_to_codex_sse(&root, None, &[], &[], &[]).unwrap();
        assert_eq!(
            converted
                .matches("event: response.output_item.done")
                .count(),
            1
        );
    }

    #[test]
    fn chat_bridge_converts_orphan_tool_output_to_user_context() {
        let route = ProviderRoute {
            provider: "test".to_string(),
            base_url: "https://example.test/v1".to_string(),
            api_key: String::new(),
            real_model: "gpt-test".to_string(),
            proxy_mode: "direct".to_string(),
            proxy_url: String::new(),
            protocol_type: "openai".to_string(),
            endpoint_path: "/chat/completions".to_string(),
            priority: 0,
            weight: 1,
        };
        let payload = serde_json::json!({
            "input": [{
                "type": "function_call_output",
                "call_id": "missing_call",
                "output": "result"
            }]
        });

        let chat_body = build_openai_chat_body(&payload, &route);
        assert_eq!(chat_body["messages"][0]["role"], "user");
        assert!(chat_body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("unavailable call missing_call"));
    }

    #[test]
    fn chat_tool_call_is_restored_as_codex_custom_tool_call() {
        let root = serde_json::json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "call_exec",
                        "type": "function",
                        "function": {
                            "name": "exec",
                            "arguments": "{\"input\":\"Get-ChildItem\"}"
                        }
                    }]
                }
            }]
        });

        let converted = openai_chat_tool_calls_to_codex_sse(
            &root,
            None,
            &["exec".to_string()],
            &["exec".to_string()],
            &[],
        )
        .unwrap();

        assert!(converted.contains(r#""type":"custom_tool_call""#));
        assert!(converted.contains(r#""name":"exec""#));
        assert!(converted.contains(r#""input":"Get-ChildItem""#));
        assert!(!converted.contains(r#""type":"function_call""#));

        let events = converted
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .filter_map(|data| serde_json::from_str::<serde_json::Value>(data).ok())
            .collect::<Vec<_>>();
        let event_types = events
            .iter()
            .filter_map(|event| event.get("type").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            event_types,
            vec![
                "response.created",
                "response.output_item.added",
                "response.custom_tool_call_input.delta",
                "response.custom_tool_call_input.done",
                "response.output_item.done",
                "response.completed"
            ]
        );
        let added = &events[1]["item"];
        assert_eq!(added["status"], "in_progress");
        assert_eq!(added["name"], "exec");
        assert_eq!(added["input"], "");
        let done = &events[4]["item"];
        assert_eq!(done["status"], "completed");
        assert_eq!(done["name"], "exec");
        assert_eq!(done["input"], "Get-ChildItem");
        assert_eq!(events[2]["item_id"], done["id"]);
        assert_eq!(events[3]["item_id"], done["id"]);
        assert_eq!(events[5]["response"]["output"][0]["name"], "exec");
    }

    #[test]
    fn cpamc_body_normalization_rewrites_repeated_tool_names() {
        let body = r#"{"type":"response.output_item.added","item":{"type":"custom_tool_call","name":"execexec","call_id":"call_1","input":"{}"}}"#;
        let normalized = normalize_repeated_tool_names_in_body(body);

        assert!(normalized.contains(r#""type":"custom_tool_call""#));
        assert!(normalized.contains(r#""name":"exec""#));
        assert!(normalized.contains(r#""input":"{}""#));
        assert!(!normalized.contains(r#""type":"function_call""#));
        assert!(!normalized.contains(r#""name":"execexec""#));
    }

    #[test]
    fn cpamc_body_normalization_uses_tools_declared_by_codex() {
        let body = r#"{"type":"response.output_item.added","item":{"type":"custom_tool_call","name":"execexecexec","call_id":"call_1","input":"{}"}}"#;
        let normalized = normalize_repeated_tool_names_in_body_with_available(
            body,
            &["exec".to_string(), "wait".to_string()],
        );

        assert!(normalized.contains(r#""name":"exec""#));
        assert!(!normalized.contains("execexec"));
    }

    #[test]
    fn cpamc_body_normalization_preserves_declared_repeated_looking_name() {
        let body = r#"{"type":"response.output_item.added","item":{"type":"custom_tool_call","name":"mama","call_id":"call_1","input":"{}"}}"#;
        let normalized =
            normalize_repeated_tool_names_in_body_with_available(body, &["mama".to_string()]);

        assert!(normalized.contains(r#""name":"mama""#));
    }

    #[test]
    fn cpamc_sse_line_normalization_rewrites_repeated_tool_names() {
        let line = "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"custom_tool_call\",\"name\":\"execexec\",\"call_id\":\"call_1\",\"input\":\"{}\"}}\n\n";
        let normalized = normalize_repeated_tool_names_in_sse_line(line);

        assert!(normalized.contains(r#""type":"custom_tool_call""#));
        assert!(normalized.contains(r#""name":"exec""#));
        assert!(normalized.contains(r#""input":"{}""#));
        assert!(!normalized.contains(r#""type":"function_call""#));
        assert!(!normalized.contains(r#""name":"execexec""#));
    }

    #[test]
    fn cpamc_compact_sse_line_normalization_rewrites_custom_tool_call() {
        let line = "data:{\"type\":\"response.output_item.added\",\"item\":{\"type\":\"custom_tool_call\",\"name\":\"execexec\",\"call_id\":\"call_1\",\"input\":\"{}\"}}\n";
        let normalized = normalize_repeated_tool_names_in_sse_line(line);

        assert!(normalized.starts_with("data: {"));
        assert!(normalized.ends_with('\n'));
        assert!(normalized.contains(r#""type":"custom_tool_call""#));
        assert!(normalized.contains(r#""name":"exec""#));
        assert!(normalized.contains(r#""input":"{}""#));
        assert!(!normalized.contains(r#""type":"function_call""#));
        assert!(!normalized.contains("execexec"));
    }

    #[test]
    fn cpamc_sse_body_normalization_handles_misreported_content_type() {
        let body = "event: response.output_item.added\ndata:{\"type\":\"response.output_item.added\",\"item\":{\"type\":\"custom_tool_call\",\"name\":\"execexec\",\"call_id\":\"call_1\",\"input\":\"{}\"}}\n\n";
        let normalized = normalize_repeated_tool_names_in_body(body);

        assert!(normalized.contains(r#""type":"custom_tool_call""#));
        assert!(normalized.contains(r#""name":"exec""#));
        assert!(normalized.contains(r#""input":"{}""#));
        assert!(!normalized.contains(r#""type":"function_call""#));
        assert!(!normalized.contains("execexec"));
    }

    #[test]
    fn cpamc_custom_tool_argument_events_are_preserved() {
        let body = "event: response.custom_tool_call_input.delta\ndata:{\"type\":\"response.custom_tool_call_input.delta\",\"delta\":\"{}\"}\n\nevent: response.custom_tool_call_input.done\ndata: {\"type\":\"response.custom_tool_call_input.done\",\"input\":\"{}\"}\n\n";
        let normalized = normalize_repeated_tool_names_in_body(body);

        assert!(normalized.contains("event: response.custom_tool_call_input.delta"));
        assert!(normalized.contains(r#""type":"response.custom_tool_call_input.delta""#));
        assert!(normalized.contains("event: response.custom_tool_call_input.done"));
        assert!(normalized.contains(r#""type":"response.custom_tool_call_input.done""#));
        assert!(!normalized.contains("function_call_arguments"));
    }

    #[test]
    fn cpamc_done_event_keeps_custom_tool_name() {
        let line = "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"custom_tool_call\",\"name\":\"execexec\",\"namespace\":\"exec\",\"call_id\":\"call_1\",\"input\":\"{}\"}}\n\n";
        let normalized = normalize_repeated_tool_names_in_sse_line(line);

        assert!(normalized.contains(r#""name":"exec""#));
        assert!(!normalized.contains("namespace"));
        assert!(!normalized.contains("execexec"));
    }

    #[test]
    fn cpamc_completed_event_removes_replayed_namespace() {
        let line = "data: {\"type\":\"response.completed\",\"response\":{\"output\":[{\"type\":\"custom_tool_call\",\"name\":\"exec\",\"namespace\":\"exec\",\"call_id\":\"call_1\",\"input\":\"{}\"}]}}\n\n";
        let normalized = normalize_repeated_tool_names_in_sse_line(line);

        assert!(normalized.contains(r#""type":"custom_tool_call""#));
        assert!(normalized.contains(r#""name":"exec""#));
        assert!(!normalized.contains("namespace"));
    }

    #[test]
    fn official_payload_normalization_removes_input_namespace_recursively() {
        let mut payload = serde_json::json!({
            "model": "gpt-5.6-sol",
            "input": [{
                "type": "message",
                "role": "user",
                "namespace": "codex_app",
                "content": [{
                    "type": "input_text",
                    "text": "hello",
                    "namespace": "nested"
                }]
            }]
        });

        normalize_official_codex_payload(&mut payload);

        let serialized = payload.to_string();
        assert!(!serialized.contains("namespace"));
        assert!(serialized.contains(r#""input""#));
    }

    #[test]
    fn responses_payload_prefers_hosted_image_generation_over_conflicting_namespace_tool() {
        let mut payload = serde_json::json!({
            "tools": [
                {"type": "image_generation"},
                {
                    "type": "namespace",
                    "name": "image_gen",
                    "tools": [
                        {"type": "function", "name": "imagegen", "parameters": {}},
                        {"type": "function", "name": "inspect", "parameters": {}}
                    ]
                },
                {"type": "function", "name": "unrelated", "parameters": {}}
            ],
            "tool_choice": {
                "type": "function",
                "namespace": "image_gen",
                "name": "imagegen"
            }
        });

        sanitize_custom_responses_payload(&mut payload);

        let tools = payload["tools"].as_array().unwrap();
        assert!(tools.iter().any(|tool| {
            tool.get("type").and_then(|value| value.as_str()) == Some("image_generation")
        }));
        let image_namespace = tools
            .iter()
            .find(|tool| tool.get("name").and_then(|value| value.as_str()) == Some("image_gen"))
            .unwrap();
        let children = image_namespace["tools"].as_array().unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0]["name"], "inspect");
        assert!(tools
            .iter()
            .any(|tool| tool.get("name").and_then(|value| value.as_str()) == Some("unrelated")));
        assert_eq!(payload["tool_choice"], "auto");
    }

    #[test]
    fn responses_payload_keeps_local_image_tool_without_hosted_tool() {
        let mut payload = serde_json::json!({
            "tools": [{
                "type": "namespace",
                "name": "image_gen",
                "tools": [{"type": "function", "name": "imagegen", "parameters": {}}]
            }]
        });

        sanitize_custom_responses_payload(&mut payload);

        assert_eq!(payload["tools"][0]["name"], "image_gen");
        assert_eq!(payload["tools"][0]["tools"][0]["name"], "imagegen");
        assert_eq!(
            payload["tools"][0]["description"],
            "Tools in the image_gen namespace."
        );
    }

    #[test]
    fn responses_payload_removes_nested_image_namespace_when_tool_search_is_present() {
        let mut payload = serde_json::json!({
            "input": [{
                "role": "user",
                "tools": [
                    {
                        "type": "namespace",
                        "name": "image_gen",
                        "tools": [{
                            "type": "function",
                            "name": "imagegen",
                            "parameters": {}
                        }]
                    },
                    {
                        "type": "namespace",
                        "name": "unrelated",
                        "tools": [{
                            "type": "function",
                            "name": "inspect",
                            "parameters": {}
                        }]
                    },
                    {"type": "tool_search"}
                ],
                "tool_choice": {
                    "type": "function",
                    "namespace": "image_gen",
                    "name": "imagegen"
                }
            }]
        });

        sanitize_custom_responses_payload(&mut payload);

        let tools = payload["input"][0]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert!(tools.iter().any(|tool| {
            tool.get("type").and_then(|value| value.as_str()) == Some("tool_search")
        }));
        assert!(tools
            .iter()
            .any(|tool| tool.get("name").and_then(|value| value.as_str()) == Some("unrelated")));
        assert!(!tools
            .iter()
            .any(|tool| tool.get("name").and_then(|value| value.as_str()) == Some("image_gen")));
        assert_eq!(payload["input"][0]["tool_choice"], "auto");
    }

    #[test]
    fn responses_payload_removes_flattened_image_tool_schema_variants() {
        let mut payload = serde_json::json!({
            "tools": [
                {"type": "image_generation"},
                {"type": "function", "namespace": "image_gen", "name": "imagegen"},
                {"type": "function", "server_label": "image_gen", "name": "imagegen"},
                {
                    "type": "function",
                    "function": {"namespace": "image_gen", "name": "imagegen"}
                },
                {"type": "function", "name": "image_gen.imagegen"},
                {"type": "function", "name": "unrelated"}
            ],
            "tool_choice": {
                "type": "function",
                "function": {"namespace": "image_gen", "name": "imagegen"}
            }
        });

        sanitize_custom_responses_payload(&mut payload);

        let tools = payload["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert!(tools.iter().any(is_hosted_image_generation_tool));
        assert!(tools
            .iter()
            .any(|tool| extract_tool_definition_name(tool) == Some("unrelated")));
        assert_eq!(payload["tool_choice"], "auto");
    }

    #[test]
    fn responses_payload_removes_image_gen_mcp_when_hosted_image_tool_exists() {
        let mut payload = serde_json::json!({
            "tools": [
                {"type": "image_generation"},
                {
                    "type": "mcp",
                    "server_label": "image_gen",
                    "server_url": "https://example.test/mcp",
                    "allowed_tools": ["imagegen"]
                },
                {
                    "type": "mcp",
                    "server_label": "unrelated",
                    "server_url": "https://example.test/other"
                }
            ]
        });

        sanitize_custom_responses_payload(&mut payload);

        let tools = payload["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert!(tools.iter().any(is_hosted_image_generation_tool));
        assert!(tools.iter().any(|tool| {
            tool.get("server_label").and_then(|value| value.as_str()) == Some("unrelated")
        }));
        assert!(!tools.iter().any(|tool| {
            tool.get("server_label").and_then(|value| value.as_str()) == Some("image_gen")
        }));
    }

    #[test]
    fn responses_payload_adds_required_names_to_codex_tool_variants() {
        let mut payload = serde_json::json!({
            "tools": [
                {"type": "image_generation"},
                {
                    "type": "namespace",
                    "namespace": "functions",
                    "tools": [{
                        "type": "function",
                        "function": {
                            "name": "shell_command",
                            "description": "Run a command",
                            "parameters": {"type": "object", "properties": {}}
                        }
                    }]
                },
                {
                    "type": "function",
                    "function": {
                        "name": "standalone",
                        "parameters": {"type": "object", "properties": {}}
                    }
                },
                {"type": "function", "description": "invalid without a name"}
            ]
        });

        sanitize_custom_responses_payload(&mut payload);

        let tools = payload["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[1]["name"], "functions");
        assert!(tools[1].get("namespace").is_none());
        assert_eq!(
            tools[1]["description"],
            "Tools in the functions namespace."
        );
        assert_eq!(tools[1]["tools"][0]["name"], "shell_command");
        assert!(tools[1]["tools"][0].get("function").is_none());
        assert_eq!(tools[2]["name"], "standalone");
        assert!(tools[2].get("function").is_none());
    }

    #[test]
    fn duplicated_upstream_tool_name_is_normalized_even_when_tool_list_missing() {
        let body = r#"{"choices":[{"message":{"role":"assistant","tool_calls":[{"id":"call_1","type":"function","function":{"name":"execexec","arguments":"{\"command\":\"pwd\"}"}}]}}]}"#;
        let root = serde_json::from_str::<serde_json::Value>(body).unwrap();
        let converted = openai_chat_tool_calls_to_codex_sse(&root, None, &[], &[], &[]).unwrap();

        assert!(converted.contains(r#""name":"exec""#));
        assert!(!converted.contains(r#""name":"execexec""#));
    }

    #[test]
    fn chat_tool_alias_maps_back_to_dotted_codex_name() {
        let available = ["image_gen.imagegen".to_string()];
        assert_eq!(
            normalize_upstream_tool_name("image_gen__imagegen", &available),
            "image_gen.imagegen"
        );
    }

    #[test]
    fn duplicated_stream_tool_name_is_normalized_when_known() {
        let route = ProviderRoute {
            provider: "test".to_string(),
            base_url: "https://example.test/v1".to_string(),
            api_key: String::new(),
            real_model: "deepseek-v4-pro".to_string(),
            proxy_mode: "direct".to_string(),
            proxy_url: String::new(),
            protocol_type: "openai".to_string(),
            endpoint_path: "/chat/completions".to_string(),
            priority: 0,
            weight: 1,
        };
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"waitwait\",\"arguments\":\"{\\\"timeout_ms\\\":\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"1000}\"}}]}}]}\n\n",
            "data: [DONE]\n\n",
        );

        let converted =
            openai_chat_sse_body_to_codex_sse(body, &route, &["wait".to_string()], &[], &[])
                .unwrap();

        assert!(converted.contains(r#""name":"wait""#));
        assert!(!converted.contains(r#""name":"waitwait""#));
    }

    #[test]
    fn duplicated_stream_tool_name_is_normalized_even_when_tool_list_missing() {
        let route = ProviderRoute {
            provider: "test".to_string(),
            base_url: "https://example.test/v1".to_string(),
            api_key: String::new(),
            real_model: "deepseek-v4-pro".to_string(),
            proxy_mode: "direct".to_string(),
            proxy_url: String::new(),
            protocol_type: "openai".to_string(),
            endpoint_path: "/chat/completions".to_string(),
            priority: 0,
            weight: 1,
        };
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"waitwait\",\"arguments\":\"{\\\"timeout_ms\\\":\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"1000}\"}}]}}]}\n\n",
            "data: [DONE]\n\n",
        );

        let converted = openai_chat_sse_body_to_codex_sse(body, &route, &[], &[], &[]).unwrap();

        assert!(converted.contains(r#""name":"wait""#));
        assert!(!converted.contains(r#""name":"waitwait""#));
    }

    #[test]
    fn detects_response_completed_from_event_or_data_line() {
        assert!(sse_text_has_response_completed(
            "event: response.completed\n"
        ));
        assert!(sse_text_has_response_completed(
            "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n"
        ));
        assert!(!sse_text_has_response_completed("data: [DONE]\n"));
    }

    #[test]
    fn detects_sse_done_line() {
        assert!(sse_line_is_done("data: [DONE]\n"));
        assert!(!sse_line_is_done(
            "data: {\"type\":\"response.completed\"}\n"
        ));
    }

    #[test]
    fn json_response_completed_sse_has_done_frame() {
        let body = codex_completed_sse_from_text("ok", None);

        assert!(body.contains("event: response.completed"));
        assert!(body.ends_with("data: [DONE]\n\n"));
    }
}
