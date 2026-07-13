import type { AppOperationLogEntry, AppSettings, CodexRestartMode, FilePreviewResult, LocalConfigPaths, ModelConfig, PortOccupancyInfo, RouterCommandProgressState, RouterLogEntry, RouterRuntimeInfo, RouterStartupChecklistState, RouterStatus, ScanSummary, SyncCatalogResult, ToastState, TokenUsageSummary } from '../types';

export type ModelDialogMode = 'create' | 'edit';

export type ModelDialogFormValues = {
  displayName: string;
  slug: string;
  baseUrl: string;
  apiKey: string;
  realModel: string;
  contextWindow: string;
  effectiveContextWindowPercent: string;
  proxyMode: ModelConfig['proxyMode'];
  proxyUrl: string;
  protocolType: string;
  endpointPath: string;
  enabled: boolean;
};

export type ModelDialogState = {
  mode: ModelDialogMode;
  model?: ModelConfig;
};

export type DashboardListenerStatus = {
  running: boolean;
  host: string;
  port: number;
  callbackUrl: string;
  message: string;
};

export type DashboardSnapshot = {
  threadSummary: ScanSummary | null;
  accountCount: number;
  skillCount: number;
  mcpSummary: { total: number; enabled: number };
  oauthListenerStatus: DashboardListenerStatus | null;
  tokenUsageSummary: TokenUsageSummary;
  loading: boolean;
  lastUpdatedAt: number | null;
};

export type PageContext = {
  models: ModelConfig[];
  enabledModels: number;
  dashboardSnapshot: DashboardSnapshot;
  routerStatus: RouterStatus;
  routerRuntimeInfo: RouterRuntimeInfo;
  routerLogs: RouterLogEntry[];
  appOperationLogs: AppOperationLogEntry[];
  appSettings: AppSettings;
  localConfigPaths: LocalConfigPaths;
  syncCatalogPreview: SyncCatalogResult | null;
  routerUrl: string;
  toast: ToastState;
  modelDialog: ModelDialogState | null;
  portOccupancyInfo: PortOccupancyInfo | null;
  handlePreviewAction: (action: string) => void;
  handleModelDialogOpen: (mode: ModelDialogMode, model?: ModelConfig) => void;
  handleModelDialogClose: () => void;
  handleModelDialogSave: (values: ModelDialogFormValues) => Promise<void>;
  handleModelDelete: (model: ModelConfig) => Promise<void>;
  handleModelEnabledToggle: (model: ModelConfig) => Promise<void>;
  handleModelSetActive: (model: ModelConfig) => Promise<void>;
  handleModelProxySave: (model: ModelConfig, proxyMode: ModelConfig['proxyMode'], proxyUrl: string) => Promise<void>;
  handleModelConnectivityTest: (model: ModelConfig) => Promise<void>;
  handleModelChatTest: (model: ModelConfig) => Promise<void>;
  handleModelConfigExport: () => Promise<void>;
  handleModelConfigImport: () => Promise<void>;
  handleSyncModelsToCatalog: () => Promise<void>;
  handleRouterToggle: (codexRestartMode?: CodexRestartMode) => Promise<void>;
  handleRouterRestart: (codexRestartMode?: CodexRestartMode) => Promise<void>;
  handleCodexRestart: () => Promise<{ success: boolean; message: string }>;
  routerStartupChecklist: RouterStartupChecklistState;
  routerCommandProgress: RouterCommandProgressState;
  handleRouterStartupChecklistClose: () => void;
  handleRouterCommandProgressClose: () => void;
  handleRouterHealthCheck: () => Promise<void>;
  handleRouterLogsRefresh: () => Promise<void>;
  handleRouterLogsClear: () => Promise<void>;
  handleAppLogsSearch: (keyword: string, level: AppOperationLogEntry['level'] | 'all') => Promise<void>;
  handleAppLogsClear: () => Promise<void>;
  handleAppSettingsSave: (settings: AppSettings) => Promise<void>;
  handleSyncCatalogPreviewClose: () => void;
  filePreview: FilePreviewResult | null;
  handleLocalFilePreview: (path: string) => Promise<void>;
  handleLocalFilePreviewClose: () => void;
};
