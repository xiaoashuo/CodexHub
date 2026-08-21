import { useEffect, useState } from 'react';
import { Button } from '../../components/ui/Button';
import { Card, CardContent, CardHeader } from '../../components/ui/Card';
import { Switch } from '../../components/ui/Switch';
import { open } from '@tauri-apps/plugin-dialog';
import { Eye, EyeOff } from 'lucide-react';
import { ROUTER_BASE_PATH, ROUTER_HEALTH_PATH, ROUTER_HOST, ROUTER_PORT, DEFAULT_MODEL_SLUG } from '../../lib/constants';
import type { AppSettings, CatalogModelOption, ChecklistStepStatus, LocalConfigPaths, RouterCommandProgressState, RouterConfig, RouterStartupChecklistState, RouterStatus } from '../../types';

const AUTH_OPTIONS: { value: AppSettings['router_auth_method']; label: string; hint: string }[] = [
  { value: 'native', label: '原生认证', hint: 'requires_openai_auth = true，使用官方令牌' },
  { value: 'external', label: '外部密钥', hint: 'experimental_bearer_token，使用自定义令牌' },
  { value: 'env', label: '环境变量', hint: 'env_key，指定环境变量名称' },
];

const ROUTER_MODE_OPTIONS: { value: 'system' | 'third'; label: string; hint: string }[] = [
  { value: 'system', label: '系统路由', hint: '运行本地路由，可配置端口与并发上限' },
  { value: 'third', label: '三方路由', hint: '使用第三方代理，无需运行配置' },
];

export function RouterPage({
  routerStatus,
  routerActionRunning,
  appSettings,
  routerConfig,
  catalogModels,
  localConfigPaths,
  handleRouterToggle,
  handleRouterRestart,
  handleRouterHealthCheck,
  handleAppSettingsSave,
  handleRouterConfigSave,
  handleSyncModelsToCatalog,
  handleCodexRestart,
}: {
  routerStatus: RouterStatus;
  routerActionRunning: boolean;
  appSettings: AppSettings;
  routerConfig: RouterConfig;
  catalogModels: CatalogModelOption[];
  localConfigPaths: LocalConfigPaths;
  handleRouterToggle: () => Promise<void>;
  handleRouterRestart: () => Promise<void>;
  handleRouterHealthCheck: () => Promise<void>;
  handleAppSettingsSave: (settings: AppSettings) => Promise<void>;
  handleRouterConfigSave: (config: RouterConfig) => Promise<void>;
  handleSyncModelsToCatalog: () => Promise<void>;
  handleCodexRestart: () => Promise<{ success: boolean; message: string }>;
}) {
  const [editableRouterConfig, setEditableRouterConfig] = useState<RouterConfig>(routerConfig);
  const [routerMode, setRouterMode] = useState<'system' | 'third'>(routerConfig.runtime.router_mode === 1 ? 'third' : 'system');
  const [autoRestart, setAutoRestart] = useState<boolean>(routerConfig.runtime.restart === 1);
  const [saving, setSaving] = useState(false);
  const [savedMsg, setSavedMsg] = useState('');
  const [previewOpen, setPreviewOpen] = useState(false);
  const [externalTokenVisible, setExternalTokenVisible] = useState(false);
  const [syncingModels, setSyncingModels] = useState(false);
  const [syncModelsFailed, setSyncModelsFailed] = useState(false);
  const [actionPending, setActionPending] = useState(false);
  const actionLocked = routerActionRunning || actionPending;
  const routeLocked = routerStatus === 'running' || actionLocked;

  useEffect(() => {
    setEditableRouterConfig(routerConfig);
    setRouterMode(routerConfig.runtime.router_mode === 1 ? 'third' : 'system');
    setAutoRestart(routerConfig.runtime.restart === 1);
  }, [appSettings, localConfigPaths, routerConfig]);

  const activeKey = routerMode === 'system' ? 'system_config' : 'external_config';
  const activeConfig = editableRouterConfig[activeKey];
  const updateActiveConfig = (patch: Partial<typeof activeConfig>) => {
    setEditableRouterConfig((current) => ({ ...current, [activeKey]: { ...current[activeKey], ...patch } }));
  };
  const routerPort = editableRouterConfig.system_config.router_port;
  const concurrencyLimit = editableRouterConfig.system_config.concurrency_limit;
  const routerName = activeConfig.router_name;
  const systemBaseUrl = `http://${ROUTER_HOST}:${routerPort || ROUTER_PORT}${ROUTER_BASE_PATH}`;
  const baseUrl = routerMode === 'system' ? systemBaseUrl : activeConfig.base_url;
  const authMethod = activeConfig.auth_method as AppSettings['router_auth_method'];
  const externalToken = activeConfig.auth_external_token;
  const envKey = activeConfig.auth_env_key;
  const modelCatalogJson = activeConfig.model_catalog_json;
  const defaultModel = activeConfig.default_model;
  const setRouterName = (value: string) => updateActiveConfig({ router_name: value });
  const setBaseUrl = (value: string) => {
    if (routerMode === 'third') updateActiveConfig({ base_url: value });
  };
  const setAuthMethod = (value: AppSettings['router_auth_method']) => updateActiveConfig({ auth_method: value });
  const setExternalToken = (value: string) => updateActiveConfig({ auth_external_token: value });
  const setEnvKey = (value: string) => updateActiveConfig({ auth_env_key: value });
  const setModelCatalogJson = (value: string) => updateActiveConfig({ model_catalog_json: value });
  const setDefaultModel = (value: string) => updateActiveConfig({ default_model: value });
  const setRouterPort = (value: number) => setEditableRouterConfig((current) => ({
    ...current,
    system_config: { ...current.system_config, router_port: value },
  }));
  const setConcurrencyLimit = (value: number) => setEditableRouterConfig((current) => ({
    ...current,
    system_config: { ...current.system_config, concurrency_limit: value },
  }));

  const handleSyncModels = async () => {
    if (syncingModels) return;
    setSyncingModels(true);
    setSyncModelsFailed(false);
    setSavedMsg('');
    try {
      await handleSyncModelsToCatalog();
      setSavedMsg('同步成功');
    } catch (error) {
      setSyncModelsFailed(true);
      setSavedMsg(error instanceof Error ? `同步失败：${error.message}` : '同步失败，请重试');
    } finally {
      setSyncingModels(false);
    }
  };

  const saveRuntimeBeforeAction = async () => {
    await handleRouterConfigSave({
      ...routerConfig,
      runtime: { router_mode: routerMode === 'third' ? 1 : 0, restart: autoRestart ? 1 : 0 },
    });
  };

  const handleRouterAction = async () => {
    if (actionLocked) return;
    setActionPending(true);
    try {
      await saveRuntimeBeforeAction();
      await handleRouterToggle();
    } finally {
      setActionPending(false);
    }
  };

  const handleRouterRestartAction = async () => {
    if (actionLocked) return;
    setActionPending(true);
    try {
      await saveRuntimeBeforeAction();
      await handleRouterRestart();
    } finally {
      setActionPending(false);
    }
  };

  const handlePickCatalog = async () => {
    const selected = await open({ multiple: false, filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (typeof selected === 'string') {
      setModelCatalogJson(selected);
    }
  };

  const handleSave = async () => {
    if (saving) return;
    setSaving(true);
    setSavedMsg('');
    try {
      await handleAppSettingsSave({
        ...appSettings,
        router_name: routerName.trim(),
        router_base_url: baseUrl.trim(),
        router_auth_method: authMethod,
        router_auth_external_token: externalToken.trim(),
        router_auth_env_key: envKey.trim(),
        router_model_catalog_json: modelCatalogJson.trim() || localConfigPaths.catalog_path || '',
        router_default_model: defaultModel.trim(),
        router_port: Number(routerPort) || ROUTER_PORT,
        router_concurrency_limit: Math.min(64, Math.max(1, Number(concurrencyLimit) || 8)),
      });
      await handleRouterConfigSave({
        ...editableRouterConfig,
        [activeKey]: {
          ...activeConfig,
          router_name: routerName.trim(),
          base_url: routerMode === 'system' ? systemBaseUrl : baseUrl.trim(),
          auth_method: authMethod,
          auth_external_token: externalToken.trim(),
          auth_env_key: envKey.trim(),
          model_catalog_json: modelCatalogJson.trim() || localConfigPaths.catalog_path || '',
          default_model: defaultModel.trim(),
          ...(routerMode === 'system' ? {
            router_port: Number(routerPort) || ROUTER_PORT,
            concurrency_limit: Math.min(64, Math.max(1, Number(concurrencyLimit) || 8)),
          } : {}),
        },
        runtime: editableRouterConfig.runtime,
      });
      setSavedMsg('路由配置已保存');
    } finally {
      setSaving(false);
    }
  };

  const previewText = buildRouterConfigPreview({
    routerName,
    baseUrl,
    authMethod,
    externalToken,
    envKey,
    modelCatalogJson: modelCatalogJson || localConfigPaths.catalog_path || '',
    defaultModel,
  });

  return (
    <div className="grid h-full min-h-0 grid-cols-12 gap-6 overflow-hidden">
      <section className="col-span-4 min-h-0 space-y-6 overflow-y-auto">
        <Card>
          <CardHeader>
            <h3 className="text-lg font-bold text-slate-950">路由管理</h3>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="rounded-3xl bg-slate-950 p-6 text-white">
              <div className="text-sm text-slate-300">当前状态</div>
              <div className="mt-3 text-3xl font-bold">{routerStatus === 'running' ? '运行中' : '已停止'}</div>
         <div className="mt-3 break-all font-mono text-sm leading-6 text-indigo-200">{baseUrl || '未配置 Base URL'}</div>
            </div>
            <Field label="路由模式">
              <div className="grid grid-cols-2 gap-2">
                {ROUTER_MODE_OPTIONS.map((opt) => (
                  <label
                    key={opt.value}
                    onClick={(event) => {
                      if (routeLocked) {
                        event.preventDefault();
                        return;
                      }
                      setRouterMode(opt.value);
                    }}
                    className={`flex items-center gap-2 rounded-xl border px-3 py-2.5 text-sm transition ${
                      routerMode === opt.value
                        ? 'border-indigo-400 bg-indigo-50 text-indigo-700'
                        : 'border-slate-200 bg-white text-slate-600 hover:border-slate-300'
                    } ${routeLocked ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'}`}
                  >
                    <input
                      type="radio"
                      name="router_mode"
                      checked={routerMode === opt.value}
                       onChange={() => setRouterMode(opt.value)}
                       disabled={routeLocked}
                      className="h-4 w-4 accent-indigo-600"
                    />
                    <span className="font-medium">{opt.label}</span>
                  </label>
                ))}
              </div>
              <p className="mt-1.5 text-xs text-slate-400">
                {ROUTER_MODE_OPTIONS.find((opt) => opt.value === routerMode)?.hint}
              </p>
            </Field>
            <div className="flex items-center justify-between rounded-xl border border-slate-200 px-3 py-2.5">
              <div>
                <div className="text-sm font-medium text-slate-700">重启</div>
                <div className="mt-0.5 text-xs text-slate-400">保存配置后自动重启</div>
              </div>
                <Switch checked={autoRestart} onChange={setAutoRestart} disabled={routeLocked} />
            </div>
             <Button className="w-full" onClick={() => void handleRouterAction()}>{routerStatus === 'running' ? '停止 Router' : '启动 Router'}</Button>
             <Button className="w-full" variant="secondary" disabled={routerStatus !== 'running'} onClick={() => void handleRouterRestartAction()}>重启 Router</Button>
            <Button className="w-full" variant="secondary" onClick={handleRouterHealthCheck}>检查 {ROUTER_HEALTH_PATH}</Button>
          </CardContent>
        </Card>
      </section>

      <section className={`col-span-8 flex min-h-0 flex-col gap-6 ${routeLocked ? 'pointer-events-none opacity-60' : ''}`}>
        <Card className="flex min-h-0 flex-1 flex-col">
          <CardHeader>
            <h3 className="text-lg font-bold text-slate-950">路由配置</h3>
          </CardHeader>
          <CardContent className="min-h-0 flex-1 overflow-y-auto">
            <div className="mb-5 flex gap-1 rounded-2xl bg-slate-100 p-1">
                <TabButton active={routerMode === 'system'} disabled={routeLocked} onClick={() => setRouterMode('system')}>系统路由</TabButton>
               <TabButton active={routerMode === 'third'} disabled={routeLocked} onClick={() => setRouterMode('third')}>三方路由</TabButton>
            </div>
            <div className="space-y-5">
            <Field label="路由名称">
              <input
                type="text"
                value={routerName}
                onChange={(e) => setRouterName(e.target.value)}
                placeholder="Codex伴侣"
                className="h-10 w-full rounded-xl border border-slate-200 px-3 text-sm text-slate-700 outline-none focus:border-indigo-400"
              />
            </Field>

            <Field label="Base URL">
              <input
                type="text"
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
                placeholder={routerMode === 'system' ? `http://${ROUTER_HOST}:${ROUTER_PORT}/v1` : 'https://example.com'}
                readOnly={routerMode === 'system'}
                className={`h-10 w-full rounded-xl border border-slate-200 px-3 font-mono text-sm outline-none focus:border-indigo-400 ${routerMode === 'system' ? 'cursor-not-allowed bg-slate-100 text-slate-500' : 'text-slate-700'}`}
              />
            </Field>

            {routerMode === 'system' && (
              <div className="my-1 border-t border-slate-100 pt-5">
                <div className="mb-4 text-sm font-semibold text-slate-500">运行配置</div>
                <div className="grid grid-cols-2 gap-5">
                  <Field label="路由端口">
                    <input
                      type="number"
                      value={routerPort}
                      onChange={(e) => setRouterPort(Number(e.target.value))}
                      className="h-10 w-full rounded-xl border border-slate-200 px-3 font-mono text-sm text-slate-700 outline-none focus:border-indigo-400"
                    />
                  </Field>

                  <Field label="并发模式">
                    <input
                      type="number"
                      min={1}
                      max={64}
                      value={concurrencyLimit}
                      onChange={(e) => setConcurrencyLimit(Number(e.target.value))}
                      className="h-10 w-full rounded-xl border border-slate-200 px-3 font-mono text-sm text-slate-700 outline-none focus:border-indigo-400"
                    />
                    <p className="mt-1.5 text-xs text-slate-400">并发上限（1–64，默认 8）</p>
                  </Field>
                </div>
              </div>
            )}

            <Field label="鉴权方式">
              <div className="grid grid-cols-3 gap-2">
                {AUTH_OPTIONS.map((opt) => (
                  <button
                    key={opt.value}
                    type="button"
                    onClick={() => setAuthMethod(opt.value)}
                    className={`rounded-xl border px-3 py-2 text-sm font-medium transition ${
                      authMethod === opt.value
                        ? 'border-indigo-400 bg-indigo-50 text-indigo-700'
                        : 'border-slate-200 bg-white text-slate-600 hover:border-slate-300'
                    }`}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
              <p className="mt-1.5 text-xs text-slate-400">
                {AUTH_OPTIONS.find((opt) => opt.value === authMethod)?.hint}
              </p>
            </Field>

            {authMethod === 'external' && (
              <Field label="自定义令牌">
                <div className="relative">
                  <input
                    type={externalTokenVisible ? 'text' : 'password'}
                    value={externalToken}
                    onChange={(e) => setExternalToken(e.target.value)}
                    placeholder="sk-..."
                    className="h-10 w-full rounded-xl border border-slate-200 px-3 pr-10 font-mono text-sm text-slate-700 outline-none focus:border-indigo-400"
                  />
                  <button
                    type="button"
                    aria-label={externalTokenVisible ? '隐藏自定义令牌' : '显示自定义令牌'}
                    onClick={() => setExternalTokenVisible((visible) => !visible)}
                    className="absolute right-2 top-1/2 -translate-y-1/2 rounded-md p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-700"
                  >
                    {externalTokenVisible ? <EyeOff size={17} /> : <Eye size={17} />}
                  </button>
                </div>
              </Field>
            )}

            {authMethod === 'env' && (
              <Field label="环境变量名称">
                <input
                  type="text"
                  value={envKey}
                  onChange={(e) => setEnvKey(e.target.value)}
                  placeholder="AI_KEY"
                  className="h-10 w-full rounded-xl border border-slate-200 px-3 font-mono text-sm text-slate-700 outline-none focus:border-indigo-400"
                />
              </Field>
            )}

            <Field label="自定义模型">
              <div className="flex items-center gap-2">
                <input
                  type="text"
                  value={modelCatalogJson}
                  onChange={(e) => setModelCatalogJson(e.target.value)}
                  placeholder={`C:\\Users\\14128\\.codex\\ai-router-workspace\\config\\codex_router_catalog.json`}
                  className="h-10 min-w-0 flex-1 rounded-xl border border-slate-200 px-3 font-mono text-sm text-slate-700 outline-none focus:border-indigo-400"
                />
                <Button variant="secondary" onClick={() => void handlePickCatalog()} className="h-10 shrink-0">
                  选择文件
                </Button>
              </div>
            </Field>

            <Field label="默认模型">
              <div className="flex items-center gap-2">
                <select
                  value={defaultModel}
                  onChange={(e) => setDefaultModel(e.target.value)}
                  className="h-10 min-w-0 flex-1 rounded-xl border border-slate-200 bg-white px-3 text-sm text-slate-700 outline-none focus:border-indigo-400"
                >
                  <option value="">使用当前默认模型</option>
                  {catalogModels.map((m) => (
                    <option key={m.value} value={m.value}>
                      {m.label || m.value}
                    </option>
                  ))}
                </select>
                 <Button variant="secondary" onClick={() => void handleSyncModels()} disabled={syncingModels} className="h-10 min-w-[76px] shrink-0">
                   {syncingModels && <span className="mr-2 h-3.5 w-3.5 animate-spin rounded-full border-2 border-slate-400/50 border-t-slate-700" />}
                   {syncingModels ? '同步中...' : syncModelsFailed ? '重试' : '同步'}
                 </Button>
              </div>
            </Field>

            <div className="flex items-center justify-between pt-1">
              <span className="text-sm text-emerald-600">{savedMsg}</span>
              <div className="flex gap-2">
                <Button variant="ghost" onClick={() => setPreviewOpen(true)}>预览</Button>
                <Button onClick={() => void handleSave()} disabled={saving}>
                  {saving ? '保存中...' : '保存'}
                </Button>
              </div>
            </div>
            </div>
          </CardContent>
        </Card>
      </section>

      {previewOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 px-4" onClick={() => setPreviewOpen(false)}>
          <div className="w-[640px] max-w-full rounded-2xl bg-white p-6 shadow-xl" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-lg font-bold text-slate-950">配置预览</h3>
            <p className="mt-1 text-sm text-slate-500">启动路由后才会将当前配置写入 Codex config.toml 的托管区块。</p>
            <pre className="mt-4 max-h-[50vh] overflow-auto rounded-xl bg-slate-900 p-4 text-xs leading-relaxed text-slate-100">
              {previewText}
            </pre>
            <div className="mt-5 flex justify-end">
              <Button variant="ghost" onClick={() => setPreviewOpen(false)}>关闭</Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function Field({ label, labelRight, children }: { label: string; labelRight?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div>
      <div className="mb-1.5 flex items-center justify-between text-sm font-medium text-slate-700">
        <span>{label}</span>
        {labelRight}
      </div>
      {children}
    </div>
  );
}

function TabButton({ active, disabled = false, onClick, children }: { active: boolean; disabled?: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={`flex-1 rounded-xl px-4 py-2 text-sm font-semibold transition ${
        active ? 'bg-white text-indigo-700 shadow-sm' : 'text-slate-500 hover:text-slate-800'
      } ${disabled ? 'cursor-not-allowed opacity-50' : ''}`}
    >
      {children}
    </button>
  );
}

function buildRouterConfigPreview(v: {
  routerName: string;
  baseUrl: string;
  authMethod: AppSettings['router_auth_method'];
  externalToken: string;
  envKey: string;
  modelCatalogJson: string;
  defaultModel: string;
}): string {
  const name = v.routerName.trim() || 'Codex伴侣';
  const baseUrl = v.baseUrl.trim() || `http://${ROUTER_HOST}:${ROUTER_PORT}/v1`;
  const catalog = v.modelCatalogJson.trim() || '<默认 catalog 路径>';
  const model = v.defaultModel.trim() || DEFAULT_MODEL_SLUG;

  const authBlock =
    v.authMethod === 'external'
      ? `#直接指定外部key形式\nexperimental_bearer_token = "${v.externalToken}"`
      : v.authMethod === 'env'
        ? `#配置自定义模型apikey形式\nenv_key = "${v.envKey}"`
        : '#官方常规登录形式\nrequires_openai_auth = true';

  return `# <<< codex-router top managed start
model_provider = "ai-router"
model = "${model}"
model_catalog_json = "${catalog}"

[model_providers.ai-router]
name = "${name}"
base_url = "${baseUrl}"
wire_api = "responses"
${authBlock}
# <<< codex-router top managed end`;
}

function StepStatusBadge({ status }: { status: ChecklistStepStatus }) {
  const map: Record<ChecklistStepStatus, string> = {
    pending: 'bg-slate-100 text-slate-500',
    running: 'bg-blue-100 text-blue-600',
    success: 'bg-emerald-100 text-emerald-600',
    warning: 'bg-amber-100 text-amber-600',
    error: 'bg-rose-100 text-rose-600',
  };
  const labelMap: Record<ChecklistStepStatus, string> = {
    pending: '等待中',
    running: '进行中',
    success: '成功',
    warning: '警告',
    error: '失败',
  };
  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${map[status]}`}>{labelMap[status]}</span>
  );
}

function RouterProgressCard({
  title,
  percent,
  completed,
  activeLabel,
  activeMessage,
  activeStatus,
  steps,
  handleClose,
  running,
}: {
  title: string;
  percent: number;
  completed: boolean;
  activeLabel: string;
  activeMessage: string;
  activeStatus: ChecklistStepStatus;
  steps: { key: string; label: string; status: ChecklistStepStatus; message?: string; detail?: string }[];
  handleClose: () => void;
  running: boolean;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 px-4" onClick={handleClose}>
      <div className="w-[520px] max-w-full rounded-2xl bg-white p-6 shadow-xl" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-bold text-slate-950">{title}</h3>
          <span className="text-sm font-semibold text-slate-400">{percent}%</span>
        </div>

        <div className="mt-4 h-2.5 w-full overflow-hidden rounded-full bg-slate-100">
          <div
            className={`h-full rounded-full transition-all duration-300 ${completed ? 'bg-emerald-500' : 'bg-indigo-600'}`}
            style={{ width: `${percent}%` }}
          />
        </div>

        <div className="mt-4 rounded-xl bg-slate-50 p-4">
          <div className="flex items-center gap-2">
            <StepStatusBadge status={activeStatus} />
            <span className="text-sm font-semibold text-slate-800">{activeLabel}</span>
          </div>
          <p className="mt-1.5 text-sm text-slate-500">{activeMessage}</p>
        </div>

        <div className="mt-4 space-y-2">
          {steps.map((step) => (
            <div key={step.key} className="flex items-center justify-between text-sm">
              <span className={step.status === 'pending' ? 'text-slate-400' : 'text-slate-700'}>{step.label}</span>
              <StepStatusBadge status={step.status} />
            </div>
          ))}
        </div>

        <div className="mt-5 flex justify-end">
          <Button variant="ghost" onClick={handleClose} disabled={running}>
            {running ? '进行中...' : '关闭'}
          </Button>
        </div>
      </div>
    </div>
  );
}

export function RouterStartupChecklistDialog({ state, handleClose }: { state: RouterStartupChecklistState; handleClose: () => void }) {
  const total = state.steps.length || 1;
  const doneCount = state.steps.filter((s) => s.status === 'success' || s.status === 'warning').length;
  const percent = state.completed ? 100 : Math.min(100, Math.round((doneCount / total) * 100));
  const activeStep = state.steps.find((s) => s.status === 'running') ?? state.steps[state.steps.length - 1];
  const activeLabel = state.completed ? '启动完成' : (activeStep?.label ?? '启动中');
  const activeMessage = state.completed
    ? '本地 Router 已启动，可以开始使用路由能力。'
    : (activeStep?.message ?? activeStep?.label ?? '正在准备启动本地 Router...');
  const activeStatus = state.completed ? 'success' : (activeStep?.status ?? 'running');
  return (
    <RouterProgressCard
      title="路由启动"
      percent={percent}
      completed={state.completed}
      activeLabel={activeLabel}
      activeMessage={activeMessage}
      activeStatus={activeStatus}
      steps={state.steps}
      handleClose={handleClose}
      running={state.running}
    />
  );
}

export function RouterCommandProgressDialog({ state, handleClose }: { state: RouterCommandProgressState; handleClose: () => void }) {
  const total = state.steps.length || 1;
  const doneCount = state.steps.filter((s) => s.status === 'success' || s.status === 'warning').length;
  const percent = state.completed ? 100 : Math.min(100, Math.round((doneCount / total) * 100));
  const activeStep = state.steps.find((s) => s.status === 'running') ?? state.steps[state.steps.length - 1];
  const activeLabel = state.completed ? '操作完成' : (activeStep?.label ?? state.title);
  const activeMessage = state.completed
    ? `${state.title}已完成。`
    : (activeStep?.message ?? activeStep?.label ?? state.description ?? '正在处理...');
  const activeStatus = state.completed ? 'success' : (activeStep?.status ?? 'running');
  return (
    <RouterProgressCard
      title={state.title}
      percent={percent}
      completed={state.completed}
      activeLabel={activeLabel}
      activeMessage={activeMessage}
      activeStatus={activeStatus}
      steps={state.steps}
      handleClose={handleClose}
      running={state.running}
    />
  );
}
