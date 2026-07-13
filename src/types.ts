export type RouterStatus = 'running' | 'stopped';
export type ModelProxyMode = 'default' | 'direct' | 'manual';

export type ChecklistStepStatus = 'pending' | 'running' | 'success' | 'warning' | 'error';

export interface RouterStartupChecklistStep {
  key: string;
  label: string;
  status: ChecklistStepStatus;
  message: string;
  detail?: string;
}

export interface RouterStartupChecklistState {
  open: boolean;
  running: boolean;
  completed: boolean;
  visibleCount: number;
  steps: RouterStartupChecklistStep[];
}

export interface RouterCommandProgressState {
  open: boolean;
  running: boolean;
  completed: boolean;
  title: string;
  description: string;
  steps: RouterStartupChecklistStep[];
}

export interface RouterStartupPreparationResult {
  codex_config_path: string;
  catalog_path: string;
  provider_config_path: string;
  sync_catalog_result: SyncCatalogResult;
  port_occupancy: PortOccupancyInfo;
  killed_port_owner: boolean;
  thread_restore_restored_count: number;
  thread_restore_skipped_count: number;
  thread_restore_message: string;
  codex_restart_attempted: boolean;
  codex_restart_message: string;
}

export type CodexRestartMode = 'restart' | 'skip';

export interface LocalConfigPaths {
  user_home_path: string;
  codex_config_path: string;
  catalog_path: string;
  provider_config_path: string;
  app_settings_path: string;
  app_log_path: string;
  router_debug_log_path: string;
}

export type McpTransport = 'stdio' | 'http' | 'sse';

export interface McpServerSummary {
  name: string;
  transport: McpTransport;
  enabled: boolean;
  sourcePath: string;
  command?: string | null;
  args: string[];
  url?: string | null;
  headers: Record<string, string>;
  environment: Record<string, string>;
}

export interface McpServerListResult {
  total: number;
  sourcePath: string;
  items: McpServerSummary[];
}

export interface UpsertMcpServerRequest {
  name: string;
  transport: McpTransport;
  enabled: boolean;
  command?: string | null;
  args: string[];
  url?: string | null;
  headers: Record<string, string>;
  environment: Record<string, string>;
}

export interface InstalledSkillSummary {
  id: string;
  name: string;
  title?: string | null;
  summary?: string | null;
  relativePath: string;
  directoryPath: string;
  skillFilePath: string;
  updatedAt?: number | null;
}

export interface SkillListResult {
  total: number;
  rootPath: string;
  items: InstalledSkillSummary[];
}

export interface PluginSkillSummary {
  id: string;
  name: string;
  fullName: string;
  title?: string | null;
  summary?: string | null;
  enabled: boolean;
  relativePath: string;
  directoryPath: string;
  skillFilePath: string;
  updatedAt?: number | null;
}

export interface CodexPluginSummary {
  id: string;
  name: string;
  displayName: string;
  source: string;
  version: string;
  description?: string | null;
  shortDescription?: string | null;
  developerName?: string | null;
  category?: string | null;
  enabled: boolean;
  directoryPath: string;
  manifestPath: string;
  skillCount: number;
  skills: PluginSkillSummary[];
}

export interface PluginListResult {
  total: number;
  rootPath: string;
  items: CodexPluginSummary[];
}

export interface SkillBackupSummary {
  id: string;
  skillId: string;
  name: string;
  title?: string | null;
  relativePath: string;
  backupPath: string;
  createdAt: number;
}

export interface SkillBackupListResult {
  total: number;
  rootPath: string;
  items: SkillBackupSummary[];
}

export interface SkillImportResult {
  skill: InstalledSkillSummary;
  replacedExisting: boolean;
  backup?: SkillBackupSummary | null;
}

export interface SkillRemoveResult {
  removedSkillId: string;
  backup: SkillBackupSummary;
  remainingInstalledCount: number;
}

export interface SkillRestoreResult {
  restoredSkill: InstalledSkillSummary;
  backup: SkillBackupSummary;
  rollbackBackup?: SkillBackupSummary | null;
}

export interface MaintenanceCleanResult {
  message: string;
  backupDeletedCount: number;
  backupDeletedBytes: number;
  cacheDeletedCount: number;
  cacheDeletedBytes: number;
  invalidSnapshotDeletedCount: number;
  invalidSnapshotDeletedBytes: number;
}

export interface MigrationBackupResult {
  backupPath: string;
  fileCount: number;
  totalBytes: number;
  includedSections: string[];
  skippedItems: string[];
  message: string;
}

export interface MigrationRestoreResult {
  restoredCount: number;
  restoredBytes: number;
  backupPath: string;
  restoredSections: string[];
  skippedItems: string[];
  message: string;
}

export interface MigrationMissingProjectSummary {
  cwd: string;
  sessionCount: number;
}

export interface MigrationBackupInspectionResult {
  sessionCount: number;
  projectCount: number;
  missingProjectCount: number;
  affectedSessionCount: number;
  missingProjects: MigrationMissingProjectSummary[];
  message: string;
}

export interface LatestVersionCheckResult {
  currentVersion: string;
  latestVersion: string;
  updateAvailable: boolean;
  assetName?: string | null;
  downloadUrl?: string | null;
  releasePageUrl?: string | null;
  message: string;
}

export interface UpdateDownloadProgress {
  phase: 'idle' | 'downloading' | 'installing' | 'done' | 'canceled';
  downloadedBytes: number;
  totalBytes?: number | null;
  percent?: number | null;
  message: string;
}

export interface UpdateInstallResult {
  installerPath: string;
  message: string;
}

export interface RouterRuntimeInfo {
  status: string;
  service: string;
  version: string;
  host: string;
  port: number;
  pid: number | null;
  healthPath: string;
  healthUrl: string;
  uptimeSeconds: number;
  started: boolean;
  forwardingEnabled: boolean;
  concurrencyLimit: number;
}

export interface PortOccupancyInfo {
  occupied: boolean;
  host: string;
  port: number;
  pid: number | null;
  process_name: string;
  process_path: string;
}

export interface RouterLogEntry {
  time: string;
  source_ip: string;
  method: string;
  path: string;
  status: string;
  target_provider: string;
  cost: string;
  input_tokens?: number;
  output_tokens?: number;
  cached_input_tokens?: number;
  total_tokens?: number;
  usage_source?: string;
  error_detail: string;
}

export interface AccountProxyLogEntry {
  time: string;
  source_ip: string;
  method: string;
  path: string;
  protocol: string;
  model: string;
  stream: boolean;
  status: string;
  cost: string;
  account: string;
  input_tokens?: number;
  output_tokens?: number;
  cached_input_tokens?: number;
  total_tokens?: number;
  usage_source?: string;
  error_detail: string;
}

export interface TokenUsageSummary {
  router_input_tokens: number;
  router_output_tokens: number;
  router_cached_input_tokens: number;
  account_proxy_input_tokens: number;
  account_proxy_output_tokens: number;
  account_proxy_cached_input_tokens: number;
}

export interface AppOperationLogEntry {
  id: string;
  time: string;
  level: 'info' | 'warn' | 'error';
  module: string;
  action: string;
  message: string;
  detail?: string;
}

export interface AppLogQuery {
  keyword: string;
  level: 'all' | AppOperationLogEntry['level'];
  limit: number;
}

export interface AppLogFileInfo {
  path: string;
  size: number;
  max_size: number;
  count: number;
}

export interface ProviderConfigItem {
  displayName: string;
  baseUrl: string;
  apiKey: string;
  realModel: string;
  contextWindow?: number | null;
  maxContextWindow?: number | null;
  effectiveContextWindowPercent?: number | null;
  proxyMode?: ModelProxyMode;
  proxyUrl?: string;
  protocolType?: string;
  endpointPath?: string;
  enabled: boolean;
  active?: boolean;
}

export type ProviderConfigFile = Record<string, ProviderConfigItem>;

export interface ProviderConfigExportResult {
  exportPath: string;
}

export interface ProviderConfigImportResult {
  config: ProviderConfigFile;
  backupPath?: string;
}

export interface ProviderModelListResult {
  models: string[];
  url: string;
}

export interface ProviderModelTestResult {
  slug: string;
  success: boolean;
  status_code: number;
  latency_ms: number;
  latency: string;
  url: string;
  message: string;
}

export interface ProviderModelChatTestResult extends ProviderModelTestResult {
  protocol_type: string;
  request_body: string;
  response_text: string;
}

export interface ProxyTestResult {
  success: boolean;
  proxy_url: string;
  latency_ms: number;
  latency: string;
  status_code: number;
  message: string;
}

export interface FilePreviewResult {
  path: string;
  exists: boolean;
  content: string;
  truncated: boolean;
}

export interface AccountProxySettings {
  account_proxy_enabled: boolean;
  account_proxy_url: string;
  api_key: string;
}

export type RestartAppTarget = 'codex' | 'chatgpt';

export interface AppSettings {
  system_version: string;
  activation_time: string;
  codex_exe_path: string;
  app_restart_target: RestartAppTarget;
  official_proxy_url: string;
  account_usage_refresh_seconds: number;
  token_auto_renew_enabled: boolean;
  router_port: number;
  router_concurrency_limit: number;
  oauth_callback_port: number;
  router_debug_mode: boolean;
  image_generation_compat_mode: boolean;
  account_proxy: AccountProxySettings;
}

export interface SyncCatalogResult {
  source_path: string;
  target_path: string;
  synced_count: number;
  total_count: number;
  synced_slugs: string[];
}

export interface ThreadSession {
  id: string;
  title: string;
  filePath: string;
  source: 'sessions' | 'archived_sessions';
  archived: boolean;
  indexed: boolean;
  missingFromIndex: boolean;
  sidebarMissing: boolean;
  stateNeedsRepair: boolean;
  cwd?: string;
  projectName: string;
  originator?: string;
  cliVersion?: string;
  threadSource?: string;
  createdAt?: string;
  updatedAt?: string;
  fileSize: number;
  messageCount: number;
  firstUserText?: string;
  parseErrors: number;
}

export interface ProjectGroup {
  projectName: string;
  cwd?: string;
  threadCount: number;
  totalSize: number;
  activeDays: number;
  sessions: ThreadSession[];
}

export interface ScanSummary {
  totalThreads: number;
  totalSize: number;
  activeDays: number;
  averageThreadsPerDay: number;
  indexedThreads: number;
  missingFromIndex: number;
  archivedThreads: number;
  projectCount: number;
  scannedAt: string;
}

export interface ThreadScanResult {
  summary: ScanSummary;
  projects: ProjectGroup[];
}

export interface ThreadRestoreResult {
  restoredCount: number;
  skippedCount: number;
  backupPath?: string;
  message: string;
  scan: ThreadScanResult;
}

export interface ThreadRestoreCheckResult {
  restoreCount: number;
  skippedCount: number;
  requiresCodexRestart: boolean;
  codexRunning: boolean;
  projectRoots: string[];
  message: string;
}

export interface CodexAccount {
  id: string;
  accountKey: string;
  email: string;
  name: string;
  plan: string;
  authMode: string;
  subscriptionStatus: string;
  workspaceName: string;
  accessTokenMask: string;
  isCurrent: boolean;
  fiveHourPercent?: number | null;
  weeklyPercent?: number | null;
  fiveHourResetAt?: string;
  weeklyResetAt?: string;
  expiresAt?: string;
  autoRenew: boolean;
  snapshotPath?: string;
  lastUsedAt?: string;
  lastUsageAt?: string;
  usageWindows?: CodexAccountUsageWindow[];
  tokenExpiresAt?: string;
  tokenNeedsRefresh: boolean;
  tokenExpired: boolean;
  tokenRefreshPermanentlyFailed: boolean;
}

export interface CodexAccountUsageWindow {
  remainingPercent?: number | null;
  resetAt?: string;
  limitWindowSeconds?: number | null;
  resetAfterSeconds?: number | null;
}

export interface CodexAccountScanResult {
  accounts: CodexAccount[];
  currentAccountId?: string;
  apiHealthy: boolean;
  scannedAt: string;
}

export interface CodexAccountOperationResult {
  message: string;
  path?: string;
  scan: CodexAccountScanResult;
}

export interface ModelConfig {
  slug: string;
  displayName: string;
  baseUrl: string;
  apiKey: string;
  apiKeyMask: string;
  realModel: string;
  contextWindow: number | null;
  maxContextWindow: number | null;
  effectiveContextWindowPercent: number | null;
  proxyMode: ModelProxyMode;
  proxyUrl: string;
  protocolType: string;
  endpointPath: string;
  enabled: boolean;
  active: boolean;
  latency: string;
  status: 'ready' | 'testing' | 'disabled' | 'error';
}

export interface ToastState {
  title: string;
  description: string;
  tone?: 'success' | 'error';
  detail?: string;
}

export type ConfirmDialogVariant = 'warning' | 'danger' | 'info';

export interface ConfirmDialogState {
  title: string;
  description: string;
  detail?: string;
  confirmText: string;
  cancelText: string;
  variant: ConfirmDialogVariant;
  onConfirm: () => void | Promise<void>;
}
