import { invoke } from '@tauri-apps/api/core';
import type { AccountProxyLogEntry, AppLogFileInfo, AppLogQuery, AppOperationLogEntry, AppSettings, CodexAccountOperationResult, CodexAccountScanResult, CodexRestartMode, FilePreviewResult, LatestVersionCheckResult, LocalConfigPaths, MaintenanceCleanResult, McpServerListResult, McpServerSummary, MigrationBackupInspectionResult, MigrationBackupResult, MigrationRestoreResult, PluginListResult, PortOccupancyInfo, ProviderConfigExportResult, ProviderConfigFile, ProviderConfigImportResult, ProviderModelChatTestResult, ProviderModelListResult, ProviderModelTestResult, ProxyTestResult, RouterLogEntry, RouterStartupPreparationResult, SkillBackupListResult, SkillImportResult, SkillListResult, SkillRemoveResult, SkillRestoreResult, SyncCatalogResult, ThreadRestoreCheckResult, ThreadRestoreResult, ThreadScanResult, TokenUsageSummary, UpdateInstallResult, UpsertMcpServerRequest } from '../types';

export const ROUTER_COMMANDS = {
  start: 'start_router',
  stop: 'stop_router',
  status: 'router_status',
  restart: 'restart_router',
} as const;

export type RouterCommand = (typeof ROUTER_COMMANDS)[keyof typeof ROUTER_COMMANDS];

export type TauriRouterCommandResult = {
  status: string;
  service: string;
  version: string;
  host: string;
  port: number;
  pid: number | null;
  health_path: string;
  health_url: string;
  uptime_seconds: number;
  started: boolean;
  forwarding_enabled: boolean;
  concurrency_limit: number;
  codex_restart_message?: string | null;
};

export async function invokeRouterCommand(command: RouterCommand) {
  return invoke<TauriRouterCommandResult>(command);
}

export async function invokePortOccupancyCheck() {
  return invoke<PortOccupancyInfo>('check_router_port_occupancy');
}

export async function invokeRouterRequestLogs() {
  return invoke<RouterLogEntry[]>('router_request_logs');
}

export async function invokeClearRouterRequestLogs() {
  return invoke<RouterLogEntry[]>('clear_router_request_logs');
}

export async function invokeAccountProxyRequestLogs() {
  return invoke<AccountProxyLogEntry[]>('account_proxy_request_logs');
}

export async function invokeClearAccountProxyRequestLogs() {
  return invoke<AccountProxyLogEntry[]>('clear_account_proxy_request_logs');
}

export async function invokeTokenUsageSummary() {
  return invoke<TokenUsageSummary>('token_usage_summary');
}

export async function invokeDashboardQuickCounts() {
  return invoke<{ accountCount: number; skillCount: number; mcpTotal: number; mcpEnabled: number }>('dashboard_quick_counts');
}

export async function invokeAppendAppLog(log: Omit<AppOperationLogEntry, 'id' | 'time'>) {
  return invoke<AppOperationLogEntry[]>('append_app_log', { log });
}

export async function invokeSearchAppLogs(query: AppLogQuery) {
  return invoke<AppOperationLogEntry[]>('search_app_logs', { query });
}

export async function invokeClearAppLogs() {
  return invoke<AppOperationLogEntry[]>('clear_app_logs');
}

export async function invokeCleanMaintenanceData() {
  return invoke<MaintenanceCleanResult>('clean_maintenance_data');
}

export async function invokeCreateMigrationBackup() {
  return invoke<MigrationBackupResult>('create_migration_backup');
}

export async function invokeInspectMigrationBackup(sourcePath: string) {
  return invoke<MigrationBackupInspectionResult>('inspect_migration_backup', { request: { sourcePath } });
}

export async function invokeImportMigrationBackup(sourcePath: string) {
  return invoke<MigrationRestoreResult>('import_migration_backup', { request: { sourcePath } });
}

export async function invokeCheckLatestVersion() {
  return invoke<LatestVersionCheckResult>('check_latest_version');
}

export async function invokeDownloadAndInstallUpdate(downloadUrl: string, assetName: string, latestVersion: string) {
  return invoke<UpdateInstallResult>('download_and_install_update', {
    request: { downloadUrl, assetName, latestVersion },
  });
}

export async function invokeCancelUpdateDownload() {
  return invoke<void>('cancel_update_download');
}

export async function invokeAppLogFileInfo() {
  return invoke<AppLogFileInfo>('app_log_file_info');
}

export async function invokeReadProviderConfig() {
  return invoke<ProviderConfigFile>('read_provider_config');
}

export async function invokeWriteProviderConfig(config: ProviderConfigFile) {
  return invoke<ProviderConfigFile>('write_provider_config', { config });
}

export async function invokeExportProviderConfig() {
  return invoke<ProviderConfigExportResult>('export_provider_config');
}

export async function invokeImportProviderConfig(sourcePath: string) {
  return invoke<ProviderConfigImportResult>('import_provider_config', { request: { sourcePath } });
}

export async function invokeFetchProviderModels(baseUrl: string, apiKey: string, protocolType: string, proxyUrl = '') {
  return invoke<ProviderModelListResult>('fetch_provider_models', { request: { baseUrl, apiKey, protocolType, proxyUrl } });
}

export async function invokeTestProviderModel(slug: string) {
  return invoke<ProviderModelTestResult>('test_provider_model', { request: { slug } });
}

export async function invokeTestProviderModelChat(slug: string) {
  return invoke<ProviderModelChatTestResult>('test_provider_model_chat', { request: { slug } });
}

export async function invokeTestProxyConnection(proxyUrl: string) {
  return invoke<ProxyTestResult>('test_proxy_connection', { request: { proxyUrl } });
}

export async function invokeDetectProxyConnection() {
  return invoke<ProxyTestResult>('detect_proxy_connection');
}

export async function invokePreviewLocalFile(path: string) {
  return invoke<FilePreviewResult>('preview_local_file', { request: { path } });
}

export async function invokeEnsureRequiredConfigFiles() {
  return invoke<string[]>('ensure_required_config_files');
}

export async function invokeReadAppSettings() {
  return invoke<AppSettings>('read_app_settings');
}

export async function invokeDetectCodexExePath() {
  return invoke<string>('detect_codex_exe_path_for_settings');
}

export async function invokeLocalConfigPaths() {
  return invoke<LocalConfigPaths>('local_config_paths');
}

export async function invokeLoadMcpServers() {
  return invoke<McpServerListResult>('load_mcp_servers');
}

export async function invokeUpsertMcpServer(request: UpsertMcpServerRequest) {
  return invoke<McpServerSummary>('upsert_mcp_server', { request });
}

export async function invokeSetMcpServerEnabled(name: string, enabled: boolean) {
  return invoke<McpServerSummary>('set_mcp_server_enabled', { request: { name, enabled } });
}

export async function invokeRemoveMcpServer(name: string) {
  return invoke<McpServerListResult>('remove_mcp_server', { request: { name } });
}

export async function invokeLoadInstalledSkills() {
  return invoke<SkillListResult>('load_installed_skills');
}

export async function invokeLoadCodexPlugins() {
  return invoke<PluginListResult>('load_codex_plugins');
}

export async function invokeSetCodexPluginEnabled(id: string, enabled: boolean) {
  return invoke<PluginListResult>('set_codex_plugin_enabled', { request: { id, enabled } });
}

export async function invokeSetCodexPluginSkillEnabled(fullName: string, enabled: boolean) {
  return invoke<PluginListResult>('set_codex_plugin_skill_enabled', { request: { fullName, enabled } });
}

export async function invokeLoadSkillBackups() {
  return invoke<SkillBackupListResult>('load_skill_backups');
}

export async function invokeImportSkill(sourcePath: string) {
  return invoke<SkillImportResult>('import_skill', { request: { sourcePath } });
}

export async function invokeRemoveSkill(id: string) {
  return invoke<SkillRemoveResult>('remove_skill', { request: { id } });
}

export async function invokeRestoreSkillBackup(id: string) {
  return invoke<SkillRestoreResult>('restore_skill_backup', { request: { id } });
}

export async function invokeDeleteSkillBackup(id: string) {
  return invoke<SkillBackupListResult>('delete_skill_backup', { request: { id } });
}

export async function invokeToggleCodexTokenAutoRenew(enabled: boolean) {
  return invoke<AppSettings>('toggle_codex_token_auto_renew', { enabled });
}

export async function invokeWriteAppSettings(settings: AppSettings) {
  return invoke<AppSettings>('write_app_settings', { settings });
}

export async function invokeSyncEnabledModelsToCatalog() {
  return invoke<SyncCatalogResult>('sync_enabled_models_to_catalog');
}

export async function invokePrepareRouterStartup(codexRestartMode: CodexRestartMode = 'restart') {
  return invoke<RouterStartupPreparationResult>('prepare_router_startup', { request: { codexRestartMode } });
}

export async function invokeScanCodexThreads() {
  return invoke<ThreadScanResult>('scan_codex_threads');
}

export async function invokeQuickCodexThreadSummary() {
  return invoke<ThreadScanResult['summary']>('quick_codex_thread_summary');
}

export async function invokeDeleteCodexThreadFiles(filePaths: string[]) {
  return invoke<ThreadScanResult>('delete_codex_thread_files', { request: { filePaths } });
}

export async function invokeCheckRestoreCodexThreadIndex(filePaths: string[], restoreAll = false) {
  return invoke<ThreadRestoreCheckResult>('check_restore_codex_thread_index', { request: { filePaths, restoreAll } });
}

export async function invokeRestoreCodexThreadIndex(filePaths: string[], restoreAll = false, allowCodexRestart = false, moveToRecent = false) {
  return invoke<ThreadRestoreResult>('restore_codex_thread_index', { request: { filePaths, restoreAll, allowCodexRestart, moveToRecent } });
}

export async function invokeScanCodexAccounts() {
  return invoke<CodexAccountScanResult>('scan_codex_accounts');
}

export async function invokeRefreshCodexAccountsUsage() {
  return invoke<CodexAccountOperationResult>('refresh_codex_accounts_usage');
}

export async function invokeRefreshCodexAccountUsage(accountKey: string, manual = true) {
  return invoke<CodexAccountOperationResult>('refresh_codex_account_usage', { request: { accountKey, manual } });
}

export async function invokeRefreshCodexAccountToken(accountKey: string) {
  return invoke<CodexAccountOperationResult>('refresh_codex_account_token', { request: { accountKey } });
}

export async function invokeUpdateCodexAccountExpiration(accountKey: string, expiresAt: string | null) {
  return invoke<CodexAccountOperationResult>('update_codex_account_expiration', { request: { accountKey, expiresAt } });
}

export async function invokeStartCodexAccountLogin() {
  return invoke<CodexAccountOperationResult>('start_codex_account_login');
}

export async function invokeCodexOAuthLoginStatus() {
  return invoke<{ status: string; message: string; accountKey?: string; accountEmail?: string }>('codex_oauth_login_status');
}

export async function invokeCodexOAuthCallbackListenerStatus() {
  return invoke<{ running: boolean; host: string; port: number; callbackUrl: string; message: string }>('codex_oauth_callback_listener_status');
}

export async function invokeImportCurrentCodexAccount() {
  return invoke<CodexAccountOperationResult>('import_current_codex_account');
}

export async function invokeImportChatGptSessionAccount(sessionJson: string) {
  return invoke<CodexAccountOperationResult>('import_chatgpt_session_account', { request: { sessionJson } });
}

export async function invokeImportCpaAccount(cpaJson: string) {
  return invoke<CodexAccountOperationResult>('import_cpa_account', { request: { cpaJson } });
}

export async function invokeStartCodexClientLogin() {
  return invoke<CodexAccountOperationResult>('start_codex_client_login');
}

export async function invokeOpenExternalUrl(url: string) {
  return invoke<void>('open_external_url', { request: { url } });
}

export async function invokeSwitchCodexAccount(accountKey: string) {
  return invoke<CodexAccountOperationResult>('switch_codex_account', { request: { accountKey } });
}

export async function invokeRestartCodexApp() {
  return invoke<{ success: boolean; message: string }>('restart_codex_app');
}

export async function invokeRemoveCodexAccountSnapshot(accountKey: string) {
  return invoke<CodexAccountOperationResult>('remove_codex_account_snapshot', { request: { accountKey } });
}

export async function invokeExportCodexAccounts() {
  return invoke<CodexAccountOperationResult>('export_codex_accounts');
}
