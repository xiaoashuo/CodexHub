import { useCallback, useEffect, useMemo, useRef, useState, type ReactElement } from 'react';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { Sidebar, TitleBar } from './components/business/Layout';
import { ModelDialog } from './components/business/ModelDialog';
import { Button } from './components/ui/Button';
import { navItems, type NavItem } from './data/seedData';
import { AccountManagementPage } from './features/accounts/AccountManagementPage';
import { DashboardPage } from './features/dashboard/DashboardPage';
import { LogsPage } from './features/logs/LogsPage';
import { ModelsPage } from './features/models/ModelsPage';
import { RouterCommandProgressDialog, RouterPage, RouterStartupChecklistDialog } from './features/router/RouterPage';
import { SettingsPage } from './features/settings/SettingsPage';
import { ServicesPage } from './features/services/ServicesPage';
import { ThreadManagerPage } from './features/threads/ThreadManagerPage';
import { APP_VERSION, ROUTER_BASE_PATH, ROUTER_HEALTH_URL, ROUTER_HOST, ROUTER_PORT } from './lib/constants';
import {
  invokeClearAppLogs,
  invokeClearRouterRequestLogs,
  invokeCheckLatestVersion,
  invokeCodexOAuthCallbackListenerStatus,
  invokeCancelUpdateDownload,
  invokeDashboardQuickCounts,
  invokeDownloadAndInstallUpdate,
  invokeExportProviderConfig,
  invokeImportProviderConfig,
  invokeLocalConfigPaths,
  invokePreviewLocalFile,
  invokeQuickCodexThreadSummary,
  invokeReadAppSettings,
  invokeReadCatalogModelOptions,
  invokeReadRouterConfig,
  invokeReadProviderConfig,
  invokeRestartCodexApp,
  invokePrepareRouterStartup,
  invokePortOccupancyCheck,
  invokeRouterCommand,
  invokeRouterRequestLogs,
  invokeSearchAppLogs,
  invokeSyncEnabledModelsToCatalog,
  invokeSyncOfficialCatalog,
  invokeTestProviderModel,
  invokeTestProviderModelChat,
  invokeTokenUsageSummary,
  invokeWriteAppSettings,
  invokeWriteRouterConfig,
  invokeWriteProviderConfig,
  ROUTER_COMMANDS,
} from './lib/tauriBridge';
import type {
  AppOperationLogEntry,
  AppSettings,
  CatalogModelOption,
  FilePreviewResult,
  LatestVersionCheckResult,
  LocalConfigPaths,
  ModelConfig,
  ProviderConfigFile,
  RouterCommandProgressState,
  RouterLogEntry,
  RouterRuntimeInfo,
  RouterStartupChecklistState,
  RouterStartupChecklistStep,
  RouterStatus,
  RouterConfig,
  SyncCatalogResult,
  ToastState,
  UpdateDownloadProgress,
} from './types';
import type { DashboardSnapshot, ModelDialogFormValues, ModelDialogMode, ModelDialogState, PageContext } from './lib/appTypes';

const DASHBOARD_REFRESH_INTERVAL_MS = 60_000;
const DASHBOARD_INITIAL_REFRESH_DELAY_MS = 1_500;

const routerStartupSteps: RouterStartupChecklistStep[] = [
  { key: 'files', label: '\u68c0\u6d4b\u6570\u636e\u6587\u4ef6', status: 'pending', message: '\u7b49\u5f85\u68c0\u6d4b config.toml \u548c models_cache.json...' },
  { key: 'catalog', label: '\u5907\u4efd\u5e76\u5408\u5e76\u6a21\u578b', status: 'pending', message: '\u7b49\u5f85\u5907\u4efd models_cache.json \u5e76\u5408\u5e76\u6fc0\u6d3b\u6a21\u578b...' },
  { key: 'port', label: '\u68c0\u6d4b Router \u7aef\u53e3', status: 'pending', message: '\u7b49\u5f85\u68c0\u6d4b\u7aef\u53e3...' },
  { key: 'start', label: '\u542f\u52a8 Router', status: 'pending', message: '\u7b49\u5f85\u542f\u52a8\u8def\u7531...' },
];

const routerStopSteps: RouterStartupChecklistStep[] = [
  { key: 'stop', label: '\u505c\u6b62\u672c\u5730 Router', status: 'pending', message: '\u7b49\u5f85\u505c\u6b62\u670d\u52a1...' },
];

function markStepRunning(steps: RouterStartupChecklistStep[], key: string): RouterStartupChecklistStep[] {
  return steps.map((step) => ({ ...step, status: step.key === key ? 'running' : step.status, message: step.key === key ? '\u6b63\u5728\u5904\u7406...' : step.message }));
}

function markFirstRunningAsError(steps: RouterStartupChecklistStep[], message: string): RouterStartupChecklistStep[] {
  const runningIndex = steps.findIndex((step) => step.status === 'running');
  const errorIndex = runningIndex >= 0 ? runningIndex : steps.findIndex((step) => step.status === 'pending');
  return steps.map((step, index) => (index === errorIndex ? { ...step, status: 'error', message } : step));
}


const LOCKED_SCROLL_NAV_ITEMS = new Set<NavItem>(['\u8def\u7531\u7ba1\u7406', '\u8d26\u53f7\u7ba1\u7406', '\u6a21\u578b\u7ba1\u7406', '\u4f1a\u8bdd\u7ba1\u7406', '\u670d\u52a1', '\u8bbe\u7f6e', '\u65e5\u5fd7']);
const DEFAULT_NAV_ITEM = navItems[0];
const ACTIVE_NAV_STORAGE_KEY = 'codex-proxy.activeNav';

function isNavItem(value: string): value is NavItem {
  return (navItems as readonly string[]).includes(value);
}

function readSavedNav(): NavItem {
  if (typeof window === 'undefined') return DEFAULT_NAV_ITEM;

  try {
    const savedNav = window.localStorage.getItem(ACTIVE_NAV_STORAGE_KEY);
    return savedNav && isNavItem(savedNav) ? savedNav : DEFAULT_NAV_ITEM;
  } catch {
    return DEFAULT_NAV_ITEM;
  }
}

export function App() {
  const [activeNav, setActiveNav] = useState<NavItem>(() => readSavedNav());
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [catalogModels, setCatalogModels] = useState<CatalogModelOption[]>([]);
  const [routerStatus, setRouterStatus] = useState<RouterStatus>('stopped');
  const [routerRuntimeInfo, setRouterRuntimeInfo] = useState<RouterRuntimeInfo>(createDefaultRouterRuntimeInfo());
  const [routerLogs, setRouterLogs] = useState<RouterLogEntry[]>([]);
  const [routerStartupChecklist, setRouterStartupChecklist] = useState<RouterStartupChecklistState>(createDefaultRouterStartupChecklistState());
  const [routerCommandProgress, setRouterCommandProgress] = useState<RouterCommandProgressState>(createDefaultRouterCommandProgressState());
  const [routerActionRunning, setRouterActionRunning] = useState(false);
  const [appOperationLogs, setAppOperationLogs] = useState<AppOperationLogEntry[]>([]);
  const [appSettings, setAppSettings] = useState<AppSettings>(createDefaultAppSettings());
  const [routerConfig, setRouterConfig] = useState<RouterConfig>(createDefaultRouterConfig());
  const [localConfigPaths, setLocalConfigPaths] = useState<LocalConfigPaths>(createDefaultLocalConfigPaths());
  const [filePreview, setFilePreview] = useState<FilePreviewResult | null>(null);
  const [syncCatalogPreview, setSyncCatalogPreview] = useState<SyncCatalogResult | null>(null);
  const [modelDialog, setModelDialog] = useState<ModelDialogState | null>(null);
  const [checkingVersion, setCheckingVersion] = useState(false);
  const [updateDialog, setUpdateDialog] = useState<LatestVersionCheckResult | null>(null);
  const [updating, setUpdating] = useState(false);
  const [updateError, setUpdateError] = useState('');
  const [updateProgress, setUpdateProgress] = useState<UpdateDownloadProgress>(createIdleUpdateProgress());
  const [toast, setToast] = useState<ToastState>({ title: '', description: '' });
  const [modelConfigNotice, setModelConfigNotice] = useState<ToastState>({ title: '', description: '' });
  const [dashboardSnapshot, setDashboardSnapshot] = useState(createDefaultDashboardSnapshot());
  const dashboardRefreshInFlightRef = useRef(false);
  const versionCheckInFlightRef = useRef(false);
  const settingsSupportLoadedRef = useRef(false);

  const enabledModels = useMemo(() => models.filter((model) => model.enabled).length, [models]);
  const routerUrl = `http://${ROUTER_HOST}:${appSettings.router_port || ROUTER_PORT}${ROUTER_BASE_PATH}`;
  const handleNavChange = useCallback((nav: NavItem) => {
    setActiveNav(nav);
    try {
      window.localStorage.setItem(ACTIVE_NAV_STORAGE_KEY, nav);
    } catch {
      // Ignore storage failures so navigation still works in restricted WebViews.
    }
  }, []);

  useEffect(() => {
    void refreshBaseState();
    const startupVersionCheckTimer = window.setTimeout(() => {
      void runVersionCheck({ source: 'startup' });
    }, 0);
    return () => window.clearTimeout(startupVersionCheckTimer);
  }, []);


  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    void listen<UpdateDownloadProgress>('update-download-progress', (event) => {
      if (!disposed) {
        setUpdateProgress(event.payload);
      }
    }).then((cleanup) => {
      if (disposed) {
        cleanup();
      } else {
        unlisten = cleanup;
      }
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (activeNav !== '\u603b\u89c8') return;
    const initialTimer = window.setTimeout(() => {
      void refreshDashboardSnapshot();
    }, DASHBOARD_INITIAL_REFRESH_DELAY_MS);
    const timer = window.setInterval(() => {
      void refreshDashboardSnapshot();
    }, DASHBOARD_REFRESH_INTERVAL_MS);
    return () => {
      window.clearTimeout(initialTimer);
      window.clearInterval(timer);
    };
  }, [activeNav]);

  useEffect(() => {
    if (activeNav !== '\u8bbe\u7f6e' || settingsSupportLoadedRef.current) return;
    settingsSupportLoadedRef.current = true;
    void refreshSettingsSupportState();
  }, [activeNav]);

  useEffect(() => {
    if (activeNav !== '\u65e5\u5fd7') return;
    void handleRouterLogsRefresh();
    void handleAppLogsSearch('', 'all');
  }, [activeNav]);

  useEffect(() => {
    if (!toast.title && !toast.description) return;
    const timer = window.setTimeout(() => setToast({ title: '', description: '' }), 2600);
    return () => window.clearTimeout(timer);
  }, [toast]);

  const loadCatalogModels = async () => {
    try {
      setCatalogModels(await invokeReadCatalogModelOptions());
    } catch {
      setCatalogModels([]);
    }
  };

  const refreshBaseState = async () => {
    await Promise.allSettled([
      invokeReadAppSettings().then(setAppSettings),
      invokeReadRouterConfig().then(setRouterConfig),
      invokeReadProviderConfig().then((config) => setModels((current) => providerConfigToModels(config, current))),
      invokeRouterCommand(ROUTER_COMMANDS.status).then(applyRouterCommandResult),
      loadCatalogModels(),
    ]);
  };

  const refreshSettingsSupportState = async () => {
    await Promise.allSettled([
      invokeLocalConfigPaths().then(setLocalConfigPaths),
      invokeSearchAppLogs({ keyword: '', level: 'all', limit: 100 }).then(setAppOperationLogs),
    ]);
  };

  const refreshDashboardSnapshot = async () => {
    if (dashboardRefreshInFlightRef.current) return;
    dashboardRefreshInFlightRef.current = true;
    setDashboardSnapshot((current) => ({ ...current, loading: true }));

    try {
      const [countsResult, tokenResult, listenerResult] = await Promise.allSettled([
        invokeDashboardQuickCounts(),
        invokeTokenUsageSummary(),
        invokeCodexOAuthCallbackListenerStatus(),
      ]);

      setDashboardSnapshot((current) => {
        const next = { ...current };
        if (countsResult.status === 'fulfilled') {
          next.accountCount = countsResult.value.accountCount;
          next.skillCount = countsResult.value.skillCount;
          next.mcpSummary = {
            total: countsResult.value.mcpTotal,
            enabled: countsResult.value.mcpEnabled,
          };
        }
        if (tokenResult.status === 'fulfilled') {
          next.tokenUsageSummary = tokenResult.value;
        }
        if (listenerResult.status === 'fulfilled') {
          next.oauthListenerStatus = listenerResult.value;
        } else {
          next.oauthListenerStatus = {
            running: false,
            host: '127.0.0.1',
            port: 1455,
            callbackUrl: 'http://localhost:1455/auth/callback',
            message: formatUnknownError(listenerResult.reason),
          };
        }
        next.lastUpdatedAt = Date.now();
        return next;
      });

      const threadResult = await invokeQuickCodexThreadSummary()
        .then((value) => ({ status: 'fulfilled' as const, value }))
        .catch((reason) => ({ status: 'rejected' as const, reason }));

      setDashboardSnapshot((current) => {
        const next = { ...current };
        if (threadResult.status === 'fulfilled') {
          next.threadSummary = threadResult.value;
        }
        next.loading = false;
        next.lastUpdatedAt = Date.now();
        return next;
      });
    } finally {
      dashboardRefreshInFlightRef.current = false;
    }
  };

  const handleRouterToggle = async () => {
    if (routerActionRunning) return;

    if (routerStatus === 'running') {
      await runRouterStop();
      return;
    }

    await runRouterStart();
  };

  const handleRouterRestart = async () => {
    if (routerActionRunning) return;
    if (routerStatus !== 'running') {
      setToast({ title: '\u0052outer \u672a\u542f\u52a8', description: '\u8bf7\u5148\u542f\u52a8 Router\u3002' });
      return;
    }
    await runRouterStart('restart');
  };

  const handleRouterHealthCheck = async () => {
    try {
      const result = await invokeRouterCommand(ROUTER_COMMANDS.status);
      applyRouterCommandResult(result);
      setToast({ title: result.started ? '\u0052outer \u6b63\u5728\u8fd0\u884c' : '\u0052outer \u672a\u542f\u52a8', description: result.health_url });
    } catch (error) {
      setToast({ title: '\u0052outer \u72b6\u6001\u68c0\u67e5\u5931\u8d25', description: formatUnknownError(error) });
    }
  };

  const handleRouterLogsRefresh = async () => {
    try {
      setRouterLogs(await invokeRouterRequestLogs());
    } catch (error) {
      setToast({ title: '\u0052outer \u65e5\u5fd7\u5237\u65b0\u5931\u8d25', description: formatUnknownError(error) });
    }
  };

  const handleRouterLogsClear = async () => {
    try {
      setRouterLogs(await invokeClearRouterRequestLogs());
    } catch (error) {
      setToast({ title: '\u0052outer \u65e5\u5fd7\u6e05\u7a7a\u5931\u8d25', description: formatUnknownError(error) });
    }
  };

  const runRouterStart = async (operation: 'start' | 'restart' = 'start') => {
    setRouterActionRunning(true);
    setRouterStartupChecklist({ open: true, running: true, completed: false, visibleCount: routerStartupSteps.length, steps: markStepRunning(routerStartupSteps, 'files') });

    try {
      const routerMode = routerConfig.runtime.router_mode;
      const preparation = await invokePrepareRouterStartup(routerMode);
      setRouterStartupChecklist((current) => ({
        ...current,
        steps: current.steps.map((step) => {
          if (step.key === 'files') return { ...step, status: 'success', message: '\u5df2\u68c0\u6d4b config.toml \u548c models_cache.json' };
          if (step.key === 'catalog') return { ...step, status: 'success', message: `\u5df2\u5907\u4efd\u5e76\u751f\u6210\u6a21\u578b catalog\uff1a${preparation.catalog_path}` };
          return step;
        }),
      }));

      setRouterStartupChecklist((current) => ({
        ...current,
        steps: current.steps.map((step) => (step.key === 'port' ? { ...step, status: 'running', message: '\u6b63\u5728\u68c0\u6d4b\u7aef\u53e3...' } : step)),
      }));
      const occupancy = await invokePortOccupancyCheck();
      if (occupancy.occupied && routerStatus !== 'running') {
        setRouterStartupChecklist((current) => ({
          ...current,
          running: false,
          steps: current.steps.map((step) =>
            step.key === 'port'
              ? {
                  ...step,
                  status: 'error',
                  message: `\u7aef\u53e3\u88ab\u5360\u7528\uff08pid=${occupancy.pid ?? '\u672a\u77e5'}${occupancy.process_name ? '\uff0c\u8fdb\u7a0b=' + occupancy.process_name : ''}\uff09\uff0c\u8bf7\u5148\u91ca\u653e\u7aef\u53e3\u540e\u518d\u542f\u52a8\u3002`,
                }
              : step,
          ),
        }));
        setToast({ title: '\u0052outer \u542f\u52a8\u5931\u8d25', description: '\u68c0\u6d4b\u5230\u7aef\u53e3\u88ab\u5360\u7528\uff0c\u5df2\u5728\u300c\u68c0\u6d4b\u7aef\u53e3\u300d\u6b65\u9aa4\u6682\u505c\u3002' });
        return;
      }
      setRouterStartupChecklist((current) => ({
        ...current,
        steps: current.steps.map((step) => (step.key === 'port' ? { ...step, status: 'success', message: routerMode === 0 ? '\u7aef\u53e3\u53ef\u7528' : '\u4e09\u65b9\u8def\u7531\u4e0d\u542f\u52a8\u672c\u5730\u7aef\u53e3' } : step)),
      }));

      setRouterStartupChecklist((current) => ({
        ...current,
        steps: current.steps.map((step) => (step.key === 'start' ? { ...step, status: 'running', message: operation === 'restart' ? '\u6b63\u5728\u91cd\u542f Router...' : (routerMode === 0 ? '\u6b63\u5728\u542f\u52a8\u672c\u5730 Router...' : '\u6b63\u5728\u542f\u52a8\u4e09\u65b9\u8def\u7531...') } : step)),
      }));
      const result = await invokeRouterCommand(ROUTER_COMMANDS.start);
      applyRouterCommandResult(result);
      setRouterStartupChecklist((current) => ({
        ...current,
        running: false,
        completed: true,
        steps: current.steps.map((step) => {
           if (step.key === 'start') return { ...step, status: 'success', message: routerMode === 0 ? `Router ${operation === 'restart' ? '\u5df2\u91cd\u542f' : '\u5df2\u542f\u52a8'}\uff1a${result.health_url}` : '\u4e09\u65b9\u8def\u7531\u5df2\u542f\u52a8' };
          return step;
        }),
      }));
      setToast({ title: operation === 'restart' ? '\u0052outer \u5df2\u91cd\u542f' : '\u0052outer \u5df2\u542f\u52a8', description: result.health_url });
    } catch (error) {
      setRouterStartupChecklist((current) => ({ ...current, running: false, completed: false, steps: markFirstRunningAsError(current.steps, formatUnknownError(error)) }));
      setToast({ title: operation === 'restart' ? '\u0052outer \u91cd\u542f\u5931\u8d25' : '\u0052outer \u542f\u52a8\u5931\u8d25', description: formatUnknownError(error) });
    } finally {
      setRouterActionRunning(false);
    }
  };

  const runRouterStop = async () => {
    setRouterActionRunning(true);
    setRouterCommandProgress({ open: true, running: true, completed: false, title: '\u505c\u6b62 Router', description: '', steps: markStepRunning(routerStopSteps, 'stop') });
    try {
      const result = await invokeRouterCommand(ROUTER_COMMANDS.stop);
      applyRouterCommandResult(result);
      setRouterCommandProgress((current) => ({
        ...current,
        running: false,
        completed: true,
        steps: current.steps.map((step) => ({ ...step, status: 'success', message: '\u5df2\u5b8c\u6210' })),
      }));
      setToast({ title: '\u0052outer \u5df2\u505c\u6b62', description: '' });
    } catch (error) {
      setRouterCommandProgress((current) => ({ ...current, running: false, completed: false, steps: markFirstRunningAsError(current.steps, formatUnknownError(error)) }));
      setToast({ title: '\u0052outer \u505c\u6b62\u5931\u8d25', description: formatUnknownError(error) });
    } finally {
      setRouterActionRunning(false);
    }
  };

  const applyRouterCommandResult = (result: Awaited<ReturnType<typeof invokeRouterCommand>>) => {
    setRouterStatus(result.started ? 'running' : 'stopped');
    setRouterRuntimeInfo(normalizeRouterRuntimeInfo(result));
  };

  const handleModelDialogOpen = (mode: ModelDialogMode, model?: ModelConfig) => setModelDialog({ mode, model });
  const handleModelDialogClose = () => setModelDialog(null);

  const handleModelDialogSave = async (values: ModelDialogFormValues) => {
    const parsedContextWindow = parseOptionalPositiveInteger(values.contextWindow);
    const nextModel: ModelConfig = {
      slug: values.slug || `model_${Date.now()}`,
      displayName: values.displayName,
      baseUrl: values.baseUrl,
      apiKey: values.apiKey,
      apiKeyMask: values.apiKey ? `${values.apiKey.slice(0, 6)}****` : '',
      realModel: values.realModel,
      contextWindow: parsedContextWindow,
      maxContextWindow: parsedContextWindow,
      effectiveContextWindowPercent: parseOptionalPercent(values.effectiveContextWindowPercent),
      proxyMode: values.proxyMode,
      proxyUrl: values.proxyUrl,
      protocolType: values.protocolType,
      endpointPath: values.endpointPath,
      modelMappings: [...new Set(values.modelMappings.split(',').map((item) => item.trim()).filter(Boolean))],
      priority: Number.isFinite(Number(values.priority)) ? Math.trunc(Number(values.priority)) : 0,
      weight: Math.max(1, Number.isFinite(Number(values.weight)) ? Math.trunc(Number(values.weight)) : 1),
      enabled: values.enabled,
      active: values.enabled && (modelDialog?.model?.active || !models.some((model) => model.enabled && model.active)),
      latency: '-',
      status: values.enabled ? 'ready' : 'disabled',
    };
    const nextModels = modelDialog?.mode === 'edit'
      ? models.map((model) => (model.slug === modelDialog.model?.slug ? nextModel : model))
      : [nextModel, ...models];
    const saved = await invokeWriteProviderConfig(buildProviderConfigFile(normalizeActiveModel(nextModels)));
    setModels(providerConfigToModels(saved, nextModels));
    setModelDialog(null);
  };

  const handleModelDelete = async (model: ModelConfig) => {
    const nextModels = normalizeActiveModel(models.filter((item) => item.slug !== model.slug));
    const saved = await invokeWriteProviderConfig(buildProviderConfigFile(nextModels));
    setModels(providerConfigToModels(saved, nextModels));
  };

  const handleModelEnabledToggle = async (model: ModelConfig) => {
    const nextModels = normalizeActiveModel(models.map((item) => (item.slug === model.slug ? { ...item, enabled: !item.enabled } : item)));
    const saved = await invokeWriteProviderConfig(buildProviderConfigFile(nextModels));
    setModels(providerConfigToModels(saved, nextModels));
  };

  const handleModelSetActive = async (model: ModelConfig) => {
    if (!model.enabled || model.active) return;
    const nextModels = models.map((item) => ({ ...item, active: item.slug === model.slug }));
    const saved = await invokeWriteProviderConfig(buildProviderConfigFile(nextModels));
    setModels(providerConfigToModels(saved, nextModels));
    setToast({ title: '\u5f53\u524d\u8def\u7531\u6a21\u578b\u5df2\u66f4\u65b0', description: `${model.displayName}\uff0c\u91cd\u65b0\u542f\u52a8 Router \u540e\u751f\u6548\u3002` });
  };

  const handleModelProxySave = async (model: ModelConfig, proxyMode: ModelConfig['proxyMode'], proxyUrl: string) => {
    const nextModels = models.map((item) => (item.slug === model.slug ? { ...item, proxyMode, proxyUrl } : item));
    const saved = await invokeWriteProviderConfig(buildProviderConfigFile(nextModels));
    setModels(providerConfigToModels(saved, nextModels));
    const modeText = proxyMode === 'manual' ? '\u5c06\u4f7f\u7528\u6a21\u578b\u4ee3\u7406\u3002' : proxyMode === 'direct' ? '\u5c06\u5f3a\u5236\u76f4\u8fde\u3002' : '\u5c06\u8d70\u5168\u5c40\u6a21\u5f0f\u3002';
    setToast({ title: '\u6a21\u578b\u4ee3\u7406\u914d\u7f6e\u5df2\u4fdd\u5b58', description: `${model.displayName} ${modeText}` });
  };

  const handleModelConnectivityTest = async (model: ModelConfig) => {
    setModels((current) => current.map((item) => (item.slug === model.slug ? { ...item, status: 'testing' } : item)));
    try {
      const result = await invokeTestProviderModel(model.slug);
      setModels((current) => current.map((item) => (item.slug === model.slug ? { ...item, latency: result.latency, status: result.success ? 'ready' : 'error' } : item)));
      setToast({ title: result.success ? '连通正常' : '连通失败', description: `${model.displayName} 延迟 ${result.latency}`, tone: result.success ? 'success' : 'error' });
    } catch (error) {
      setModels((current) => current.map((item) => (item.slug === model.slug ? { ...item, status: 'error' } : item)));
      setToast({ title: '\u8fde\u901a\u6d4b\u8bd5\u5931\u8d25', description: formatUnknownError(error), tone: 'error' });
    }
  };

  const handleModelChatTest = async (model: ModelConfig) => {
    setModels((current) => current.map((item) => (item.slug === model.slug ? { ...item, status: 'testing' } : item)));
    try {
      const result = await invokeTestProviderModelChat(model.slug);
      setModels((current) => current.map((item) => (item.slug === model.slug ? { ...item, latency: result.latency, status: result.success ? 'ready' : 'error' } : item)));
      setToast({
        title: result.success ? '模型测试成功' : '模型测试失败',
        description: result.message || `${model.displayName} ${result.latency}`,
        tone: result.success ? 'success' : 'error',
      });
    } catch (error) {
      setModels((current) => current.map((item) => (item.slug === model.slug ? { ...item, status: 'error' } : item)));
      setToast({ title: '\u6a21\u578b\u6d4b\u8bd5\u5931\u8d25', description: formatUnknownError(error), tone: 'error' });
    }
  };

  const handleModelConfigExport = async () => {
    try {
      const result = await invokeExportProviderConfig();
      setModelConfigNotice({ title: '\u6a21\u578b\u914d\u7f6e\u5df2\u5bfc\u51fa', description: result.exportPath });
    } catch (error) {
      setModelConfigNotice({ title: '\u6a21\u578b\u914d\u7f6e\u5bfc\u51fa\u5931\u8d25', description: formatUnknownError(error) });
    }
  };

  const handleModelConfigImport = async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        title: '\u9009\u62e9\u6a21\u578b\u914d\u7f6e\u5bfc\u51fa\u6587\u4ef6',
        filters: [{ name: 'JSON', extensions: ['json'] }],
      });
      if (typeof selected !== 'string') {
        return;
      }

      const result = await invokeImportProviderConfig(selected);
      setModels((current) => (Object.keys(result.config).length === 0 ? [] : providerConfigToModels(result.config, current)));
      setModelConfigNotice({
        title: '\u6a21\u578b\u914d\u7f6e\u5df2\u5bfc\u5165',
        description: result.backupPath ? `\u539f\u914d\u7f6e\u5df2\u5907\u4efd\uff1a${result.backupPath}` : '\u5bfc\u5165\u5b8c\u6210\uff0c\u539f\u914d\u7f6e\u4e0d\u5b58\u5728\u65e0\u9700\u5907\u4efd\u3002',
      });
    } catch (error) {
      setModelConfigNotice({ title: '\u6a21\u578b\u914d\u7f6e\u5bfc\u5165\u5931\u8d25', description: formatUnknownError(error) });
    }
  };

  const handleSyncModelsToCatalog = async () => {
    await invokeSyncOfficialCatalog();
    setSyncCatalogPreview(await invokeSyncEnabledModelsToCatalog());
    await loadCatalogModels();
  };

  const handleAppLogsSearch = async (keyword: string, level: AppOperationLogEntry['level'] | 'all') => {
    setAppOperationLogs(await invokeSearchAppLogs({ keyword, level, limit: 100 }));
  };

  const handleAppLogsClear = async () => {
    setAppOperationLogs(await invokeClearAppLogs());
  };

  const handleAppSettingsSave = async (settings: AppSettings) => {
    const saved = await invokeWriteAppSettings(settings);
    setAppSettings(saved);
  };

  const handleRouterConfigSave = async (config: RouterConfig) => {
    const saved = await invokeWriteRouterConfig(config);
    setRouterConfig(saved);
  };

  const runVersionCheck = async ({ source }: { source: 'manual' | 'startup' }) => {
    if (versionCheckInFlightRef.current) {
      if (source === 'manual') {
        setToast({ title: '\u6b63\u5728\u68c0\u6d4b\u66f4\u65b0', description: '\u542f\u52a8\u540e\u53f0\u68c0\u6d4b\u5c1a\u672a\u5b8c\u6210\uff0c\u8bf7\u7a0d\u540e\u518d\u8bd5\u3002' });
      }
      return;
    }

    const isManual = source === 'manual';
    versionCheckInFlightRef.current = true;
    if (isManual) {
      setToast({ title: '', description: '' });
      setCheckingVersion(true);
    }
    try {
      const result = await invokeCheckLatestVersion();
      if (result.updateAvailable) {
        setUpdateProgress(createIdleUpdateProgress());
        setUpdateDialog(result);
      } else if (isManual) {
        setToast({ title: '\u5f53\u524d\u5df2\u662f\u6700\u65b0\u7248\u672c', description: result.message });
      }
      setAppSettings(await invokeReadAppSettings());
    } catch (error) {
      if (isManual) {
        setToast({ title: '\u7248\u672c\u68c0\u6d4b\u5931\u8d25', description: error instanceof Error ? error.message : String(error) });
      }
    } finally {
      versionCheckInFlightRef.current = false;
      if (isManual) {
        setCheckingVersion(false);
      }
    }
  };

  const handleVersionCheck = async () => {
    await runVersionCheck({ source: 'manual' });
  };

  const handleStartUpdate = async () => {
    if (updating || !updateDialog) return;
    setUpdateError('');
    if (!updateDialog.downloadUrl || !updateDialog.assetName) {
      setUpdateError('\u6ca1\u6709\u53ef\u7528\u7684\u5b89\u88c5\u5305\u4e0b\u8f7d\u5730\u5740\u3002');
      return;
    }

    setUpdating(true);
    setUpdateProgress({
      phase: 'downloading',
      downloadedBytes: 0,
      totalBytes: null,
      percent: 0,
      message: 'Preparing download',
    });
    try {
      await invokeDownloadAndInstallUpdate(updateDialog.downloadUrl, updateDialog.assetName, updateDialog.latestVersion);
      setUpdateProgress((progress) => ({
        ...progress,
        phase: 'installing',
        percent: 100,
        message: 'Installer started',
      }));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (!message.toLowerCase().includes('canceled')) {
        setUpdateError(message);
      }
      setUpdating(false);
    }
  };

  const handleCancelUpdate = async () => {
    if (!updating) {
      setUpdateDialog(null);
      setUpdateError('');
      setUpdateProgress(createIdleUpdateProgress());
      return;
    }
    await invokeCancelUpdateDownload();
    setUpdating(false);
    setUpdateProgress((progress) => ({
      ...progress,
      phase: 'canceled',
      message: 'Download canceled',
    }));
  };

  const handleLocalFilePreview = async (path: string) => {
    setFilePreview(await invokePreviewLocalFile(path));
  };

  const context: PageContext = {
    models,
    catalogModels,
    enabledModels,
    dashboardSnapshot,
    routerStatus,
    routerActionRunning,
    routerRuntimeInfo,
    routerLogs,
    appOperationLogs,
    appSettings,
    routerConfig,
    localConfigPaths,
    syncCatalogPreview,
    routerUrl,
    toast,
    modelDialog,
    portOccupancyInfo: null,
    handlePreviewAction: (action) => setToast({ title: action, description: action }),
    handleModelDialogOpen,
    handleModelDialogClose,
    handleModelDialogSave,
    handleModelDelete,
    handleModelEnabledToggle,
    handleModelSetActive,
    handleModelProxySave,
    handleModelConnectivityTest,
    handleModelChatTest,
    handleModelConfigExport,
    handleModelConfigImport,
    handleSyncModelsToCatalog,
    handleRouterToggle,
    handleRouterRestart,
    handleCodexRestart: invokeRestartCodexApp,
    routerStartupChecklist,
    routerCommandProgress,
    handleRouterStartupChecklistClose: () => { if (!routerStartupChecklist.running) setRouterStartupChecklist(createDefaultRouterStartupChecklistState()); },
    handleRouterCommandProgressClose: () => { if (!routerCommandProgress.running) setRouterCommandProgress(createDefaultRouterCommandProgressState()); },
    handleRouterHealthCheck,
    handleRouterLogsRefresh,
    handleRouterLogsClear,
    handleAppLogsSearch,
    handleAppLogsClear,
    handleAppSettingsSave,
    handleRouterConfigSave,
    handleSyncCatalogPreviewClose: () => setSyncCatalogPreview(null),
    filePreview,
    handleLocalFilePreview,
    handleLocalFilePreviewClose: () => setFilePreview(null),
  };

  return (
    <div className="min-h-screen bg-gradient-to-br from-indigo-50 via-slate-50 to-cyan-50">
      <div className="flex h-screen flex-col overflow-hidden">
        <TitleBar checkingVersion={checkingVersion} handleVersionCheck={handleVersionCheck} />
        <div className="flex min-h-0 flex-1 overflow-x-hidden">
          <Sidebar activeNav={activeNav} navItems={navItems} setActiveNav={handleNavChange} routerStatus={routerStatus} routerConfig={routerConfig} />
          <main className={`flex min-w-0 flex-1 flex-col overflow-x-hidden px-8 py-6 ${LOCKED_SCROLL_NAV_ITEMS.has(activeNav) ? 'overflow-y-hidden' : 'overflow-y-auto'}`}>
            <div className={LOCKED_SCROLL_NAV_ITEMS.has(activeNav) ? 'flex flex-col min-h-0 flex-1 overflow-hidden' : ''}>
              {renderActivePage(activeNav, context)}
            </div>
          </main>
        </div>
        {modelDialog && <ModelDialog state={modelDialog} handleModelDialogClose={handleModelDialogClose} handleModelDialogSave={handleModelDialogSave} handleModelChatTest={handleModelChatTest} />}
        {routerStartupChecklist.open && (
          <RouterStartupChecklistDialog
            state={routerStartupChecklist}
            handleClose={context.handleRouterStartupChecklistClose}
          />
        )}
        {routerCommandProgress.open && (
          <RouterCommandProgressDialog
            state={routerCommandProgress}
            handleClose={context.handleRouterCommandProgressClose}
          />
        )}
        {updateDialog && <UpdateAvailableDialog result={updateDialog} progress={updateProgress} updating={updating} updateError={updateError} handleClose={handleCancelUpdate} handleStartUpdate={handleStartUpdate} />}
        {!checkingVersion && !updateDialog && <ToastView toast={toast} />}
        <ModelConfigNoticeDialog notice={modelConfigNotice} handleClose={() => setModelConfigNotice({ title: '', description: '' })} />
      </div>
    </div>
  );
}

function UpdateAvailableDialog({
  result,
  progress,
  updating,
  updateError,
  handleClose,
  handleStartUpdate,
}: {
  result: LatestVersionCheckResult;
  progress: UpdateDownloadProgress;
  updating: boolean;
  updateError: string;
  handleClose: () => void;
  handleStartUpdate: () => void;
}) {
  const percent = Math.max(0, Math.min(100, progress.percent ?? 0));
  const statusText = updating
    ? progress.phase === 'installing'
      ? '\u6b63\u5728\u542f\u52a8\u5b89\u88c5\u7a0b\u5e8f...'
      : `\u6b63\u5728\u4e0b\u8f7d ${percent}%`
    : progress.phase === 'canceled'
      ? '\u5df2\u53d6\u6d88\u4e0b\u8f7d'
    : '\u51c6\u5907\u5f00\u59cb\u66f4\u65b0';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center overflow-y-auto bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="box-border flex max-h-[86vh] w-full max-w-lg flex-col overflow-hidden rounded-3xl bg-white shadow-2xl">
        <div className="shrink-0 px-6 pt-6">
          <div className="flex min-w-0 items-start justify-between gap-4">
            <div className="min-w-0">
              <h3 className="text-xl font-bold text-slate-950">{'\u68c0\u6d4b\u5230\u65b0\u7248\u672c'} {result.latestVersion}</h3>
              <p className="mt-2 break-words text-sm leading-6 text-slate-500">{'\u5f53\u524d\u7248\u672c'} {result.currentVersion}</p>
            </div>
            <button className="shrink-0 text-2xl leading-none text-slate-400 hover:text-slate-700" type="button" disabled={updating} onClick={handleClose}>{'\u00d7'}</button>
          </div>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-6">
          <div className="mt-5 text-sm leading-6 text-slate-600">
            <div className="flex items-center justify-between gap-3 text-xs font-semibold text-slate-500">
              <span>{statusText}</span>
              <span className="font-mono text-slate-700">{percent}%</span>
            </div>
            <div className="mt-3 h-2 overflow-hidden rounded-full bg-slate-100">
              <div className="h-full rounded-full bg-slate-900 transition-[width] duration-300" style={{ width: `${percent}%` }} />
            </div>
            <div className="mt-4 break-all text-xs text-slate-500">{result.assetName}</div>
            <div className="mt-1 text-xs text-slate-400">{formatUpdateBytes(progress)}</div>
            {updateError && <div className="mt-4 break-words border-l-2 border-rose-500 pl-3 text-xs leading-5 text-rose-700">{updateError}</div>}
          </div>
        </div>
        <div className="shrink-0 flex justify-end gap-3 px-6 py-5">
          <Button variant="secondary" onClick={handleClose}>{'\u53d6\u6d88'}</Button>
          <Button onClick={handleStartUpdate} disabled={updating}>
            {updating && <span className="mr-2 h-3.5 w-3.5 animate-spin rounded-full border-2 border-white/50 border-t-white" />}
            {updating ? '\u66f4\u65b0\u4e2d' : '\u5f00\u59cb\u66f4\u65b0'}
          </Button>
        </div>
      </div>
    </div>
  );
}

function formatUpdateBytes(progress: UpdateDownloadProgress) {
  if (!progress.totalBytes) {
    return progress.downloadedBytes > 0 ? `${formatBytes(progress.downloadedBytes)} downloaded` : '\u7b49\u5f85\u4e0b\u8f7d';
  }
  return `${formatBytes(progress.downloadedBytes)} / ${formatBytes(progress.totalBytes)}`;
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  const digits = unitIndex === 0 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(digits)} ${units[unitIndex]}`;
}

function ToastView({ toast }: { toast: ToastState }) {
  if (!toast.title && !toast.description) return null;
  const toneBg = toast.tone === 'success'
    ? 'bg-emerald-600'
    : toast.tone === 'error'
      ? 'bg-rose-600'
      : 'bg-slate-800';
  const text = toast.description
    ? `${toast.title}：${toast.description}`
    : toast.title;
  return (
    <div className={`pointer-events-none fixed left-1/2 top-14 z-50 -translate-x-1/2 rounded-lg px-4 py-2 text-sm font-medium leading-5 text-white shadow ${toneBg}`}>
      <span className="block whitespace-normal break-words" style={{ maxWidth: 'min(92vw, 520px)', overflowWrap: 'anywhere' }}>{text}</span>
    </div>
  );
}

function ModelConfigNoticeDialog({ notice, handleClose }: { notice: ToastState; handleClose: () => void }) {
  if (!notice.title && !notice.description) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="box-border max-h-[70vh] w-full max-w-lg overflow-y-auto rounded-2xl bg-white px-5 py-5 shadow-2xl">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <div className="break-words text-lg font-bold text-slate-950">{notice.title}</div>
            {notice.description && <div className="mt-2 whitespace-pre-wrap break-words text-sm leading-6 text-slate-500">{notice.description}</div>}
          </div>
          <button className="shrink-0 text-2xl leading-none text-slate-400 hover:text-slate-700" type="button" onClick={handleClose}>{'\u00d7'}</button>
        </div>
        <div className="mt-6 flex justify-end">
          <Button variant="secondary" onClick={handleClose}>{'\u5173\u95ed'}</Button>
        </div>
      </div>
    </div>
  );
}

function renderActivePage(activeNav: NavItem, context: PageContext): ReactElement {
  const pageMap: Record<NavItem, ReactElement> = {
    '\u603b\u89c8': <DashboardPage {...context} />,
    '\u8d26\u53f7\u7ba1\u7406': <AccountManagementPage appSettings={context.appSettings} />,
    '\u6a21\u578b\u7ba1\u7406': <ModelsPage models={context.models} appSettings={context.appSettings} handlePreviewAction={context.handlePreviewAction} handleModelDialogOpen={context.handleModelDialogOpen} handleModelDelete={context.handleModelDelete} handleModelEnabledToggle={context.handleModelEnabledToggle} handleModelSetActive={context.handleModelSetActive} handleModelProxySave={context.handleModelProxySave} handleModelConnectivityTest={context.handleModelConnectivityTest} handleModelChatTest={context.handleModelChatTest} handleModelConfigExport={context.handleModelConfigExport} handleModelConfigImport={context.handleModelConfigImport} handleSyncModelsToCatalog={context.handleSyncModelsToCatalog} />,
    '\u4f1a\u8bdd\u7ba1\u7406': <ThreadManagerPage />,
     '\u8def\u7531\u7ba1\u7406': <RouterPage routerStatus={context.routerStatus} routerActionRunning={context.routerActionRunning} appSettings={context.appSettings} routerConfig={context.routerConfig} catalogModels={context.catalogModels} localConfigPaths={context.localConfigPaths} handleRouterToggle={context.handleRouterToggle} handleRouterRestart={context.handleRouterRestart} handleRouterHealthCheck={context.handleRouterHealthCheck} handleAppSettingsSave={context.handleAppSettingsSave} handleRouterConfigSave={context.handleRouterConfigSave} handleSyncModelsToCatalog={context.handleSyncModelsToCatalog} handleCodexRestart={context.handleCodexRestart} />,
    '\u65e5\u5fd7': <LogsPage routerLogs={context.routerLogs} appOperationLogs={context.appOperationLogs} handleRouterLogsRefresh={context.handleRouterLogsRefresh} handleRouterLogsClear={context.handleRouterLogsClear} handleAppLogsSearch={context.handleAppLogsSearch} handleAppLogsClear={context.handleAppLogsClear} />,
    '\u670d\u52a1': <ServicesPage />,
    '\u8bbe\u7f6e': <SettingsPage appSettings={context.appSettings} localConfigPaths={context.localConfigPaths} filePreview={context.filePreview} handleLocalFilePreview={context.handleLocalFilePreview} handleLocalFilePreviewClose={context.handleLocalFilePreviewClose} handleAppSettingsSave={context.handleAppSettingsSave} handleCodexRestart={context.handleCodexRestart} />,
  };

  return pageMap[activeNav];
}


function createAccountProxyApiKey() {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  const token = btoa(String.fromCharCode(...bytes)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
  return `sk_${token}`;
}

function createDefaultAppSettings(): AppSettings {
  return {
    system_version: APP_VERSION,
    activation_time: createBeijingTimestamp(),
    codex_exe_path: '',
    app_restart_target: 'chatgpt',
    official_proxy_url: '',
    account_usage_refresh_seconds: 60,
    router_port: ROUTER_PORT,
    router_concurrency_limit: 8,
    oauth_callback_port: 1455,
    router_debug_mode: false,
    token_auto_renew_enabled: false,
    image_generation_compat_mode: false,
    account_proxy: {
      account_proxy_enabled: false,
      account_proxy_url: 'http://127.0.0.1:1455/v1',
      api_key: createAccountProxyApiKey(),
    },
    router_name: 'Codex伴侣',
    router_base_url: '',
    router_auth_method: 'native',
    router_auth_external_token: '',
    router_auth_env_key: '',
    router_model_catalog_json: '',
    router_default_model: '',
    router_mode: 'system',
    router_auto_restart: false,
  };
}

function createDefaultRouterConfig(): RouterConfig {
  const common = {
    router_name: '',
    base_url: '',
    auth_method: 'native' as const,
    auth_external_token: '',
    auth_env_key: '',
    model_catalog_json: '',
    default_model: '',
  };
  return {
    system_config: { ...common, router_name: 'Codex伴侣', router_port: ROUTER_PORT, concurrency_limit: 8 },
    external_config: { ...common },
    runtime: { router_mode: 0, restart: 0 },
  };
}

function createBeijingTimestamp() {
  const now = new Date();
  const beijing = new Date(now.getTime() + 8 * 60 * 60 * 1000);
  const year = beijing.getUTCFullYear();
  const month = String(beijing.getUTCMonth() + 1).padStart(2, '0');
  const day = String(beijing.getUTCDate()).padStart(2, '0');
  const hour = String(beijing.getUTCHours()).padStart(2, '0');
  const minute = String(beijing.getUTCMinutes()).padStart(2, '0');
  const second = String(beijing.getUTCSeconds()).padStart(2, '0');
  return `${year}-${month}-${day}T${hour}:${minute}:${second}+08:00`;
}

function createIdleUpdateProgress(): UpdateDownloadProgress {
  return {
    phase: 'idle',
    downloadedBytes: 0,
    totalBytes: null,
    percent: 0,
    message: '',
  };
}

function createDefaultLocalConfigPaths(): LocalConfigPaths {
  return {
    user_home_path: '',
    codex_config_path: '',
    catalog_path: '',
    provider_config_path: '',
    app_settings_path: '',
    router_config_path: '',
    app_log_path: '',
    router_debug_log_path: '',
  };
}

function createDefaultDashboardSnapshot(): DashboardSnapshot {
  return {
    threadSummary: null,
    accountCount: 0,
    skillCount: 0,
    mcpSummary: { total: 0, enabled: 0 },
    oauthListenerStatus: null,
    tokenUsageSummary: {
      router_input_tokens: 0,
      router_output_tokens: 0,
      router_cached_input_tokens: 0,
      account_proxy_input_tokens: 0,
      account_proxy_output_tokens: 0,
      account_proxy_cached_input_tokens: 0,
    },
    loading: false,
    lastUpdatedAt: null,
  };
}

function createDefaultRouterRuntimeInfo(): RouterRuntimeInfo {
  return {
    status: 'stopped',
    service: 'codex-router',
    version: APP_VERSION,
    host: ROUTER_HOST,
    port: ROUTER_PORT,
    pid: null,
    healthPath: '/health',
    healthUrl: ROUTER_HEALTH_URL,
    uptimeSeconds: 0,
    started: false,
    forwardingEnabled: false,
    concurrencyLimit: 8,
  };
}

function createDefaultRouterStartupChecklistState(): RouterStartupChecklistState {
  return { open: false, running: false, completed: false, visibleCount: 0, steps: [] };
}

function createDefaultRouterCommandProgressState(): RouterCommandProgressState {
  return { open: false, running: false, completed: false, title: '', description: '', steps: [] };
}

function normalizeRouterRuntimeInfo(result: { status: string; service: string; version: string; host: string; port: number; pid: number | null; health_path: string; health_url: string; uptime_seconds: number; started: boolean; forwarding_enabled: boolean; concurrency_limit?: number }): RouterRuntimeInfo {
  return {
    status: result.status,
    service: result.service,
    version: result.version,
    host: result.host,
    port: result.port,
    pid: result.pid,
    healthPath: result.health_path,
    healthUrl: result.health_url,
    uptimeSeconds: result.uptime_seconds,
    started: result.started,
    forwardingEnabled: result.forwarding_enabled,
    concurrencyLimit: result.concurrency_limit || 8,
  };
}

function buildProviderConfigFile(models: ModelConfig[]): ProviderConfigFile {
  return Object.fromEntries(models.map((model) => [
    model.slug,
    {
      displayName: model.displayName,
      baseUrl: model.baseUrl,
      apiKey: model.apiKey,
      realModel: model.realModel,
      contextWindow: model.contextWindow,
      maxContextWindow: model.maxContextWindow,
      effectiveContextWindowPercent: model.effectiveContextWindowPercent,
      proxyMode: model.proxyMode,
      proxyUrl: model.proxyUrl,
      protocolType: model.protocolType,
      endpointPath: model.endpointPath,
      modelMappings: model.modelMappings,
      priority: model.priority,
      weight: model.weight,
      enabled: model.enabled,
      active: model.active,
    },
  ]));
}

function providerConfigToModels(config: ProviderConfigFile, currentModels: ModelConfig[]): ModelConfig[] {
  const entries = Object.entries(config);
  if (entries.length === 0) {
    return currentModels;
  }

  return normalizeActiveModel(entries.map(([slug, item]) => {
    const existing = currentModels.find((model) => model.slug === slug);
    return {
      slug,
      displayName: item.displayName || slug,
      baseUrl: item.baseUrl,
      apiKey: item.apiKey,
      apiKeyMask: item.apiKey ? `${item.apiKey.slice(0, 6)}****` : '',
      realModel: item.realModel,
      contextWindow: normalizeOptionalNumber(item.contextWindow),
      maxContextWindow: normalizeOptionalNumber(item.maxContextWindow),
      effectiveContextWindowPercent: normalizeOptionalNumber(item.effectiveContextWindowPercent),
      proxyMode: item.proxyMode || 'default',
      proxyUrl: item.proxyUrl || '',
      protocolType: item.protocolType || 'openai',
      endpointPath: item.endpointPath || '/chat/completions',
      modelMappings: Array.isArray(item.modelMappings)
        ? item.modelMappings
        : Array.isArray(item.modelAliases) ? item.modelAliases : [],
      priority: typeof item.priority === 'number' ? item.priority : 0,
      weight: typeof item.weight === 'number' && item.weight > 0 ? item.weight : 1,
      enabled: item.enabled,
      active: Boolean(item.active),
      latency: existing?.latency ?? '-',
      status: item.enabled ? 'ready' : 'disabled',
    };
  }));
}

function normalizeActiveModel(models: ModelConfig[]): ModelConfig[] {
  const activeSlug = models.find((model) => model.enabled && model.active)?.slug
    || models.find((model) => model.enabled)?.slug;
  return models.map((model) => ({ ...model, active: model.enabled && model.slug === activeSlug }));
}

function normalizeOptionalNumber(value: number | null | undefined): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? Math.trunc(value) : null;
}

function parseOptionalPositiveInteger(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) && parsed > 0 ? Math.trunc(parsed) : null;
}

function parseOptionalPercent(value: string): number | null {
  const parsed = parseOptionalPositiveInteger(value);
  if (parsed === null) return null;
  return Math.min(100, Math.max(1, parsed));
}

function formatUnknownError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return typeof error === 'string' ? error : String(error);
}




