import { useState, type ButtonHTMLAttributes, type MouseEvent } from 'react';
import { MoreHorizontal, Network, Pause, Pencil, Play, Settings2, Star, Trash2, type LucideIcon } from 'lucide-react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card, CardHeader } from '../ui/Card';
import type { AppSettings, ModelConfig } from '../../types';
import type { ModelDialogMode } from '../../lib/appTypes';

const text = {
  title: '模型管理',
  description: '管理模型映射配置。',
  export: '导出',
  exporting: '导出中',
  import: '导入',
  importing: '导入中',
  create: '新增模型',
  displayName: '显示名称',
  realModel: '模型',
  enabledStatus: '状态',
  actions: '操作',
  enabled: '已启用',
  current: '当前模型',
  disabled: '已禁用',
  connectivityTest: '连通测试',
  proxyConfig: '代理配置',
  disabledTest: '已禁用，无法测试',
  disable: '禁用',
  enable: '启用',
  setCurrent: '设为当前路由模型',
  edit: '编辑',
  delete: '删除',
  moreActions: '更多操作',
  empty: '暂无模型，请点击新增模型添加数据。',
  checking: '检测中',
};

type ActionMenuPosition = {
  top: number;
  right: number;
};

export function ModelTable({
  models,
  appSettings,
  handleModelDialogOpen,
  handleModelDelete,
  handleModelEnabledToggle,
  handleModelSetActive,
  handleModelProxySave,
  handleModelConnectivityTest,
  handleModelConfigExport,
  handleModelConfigImport,
  compact = false,
}: {
  models: ModelConfig[];
  appSettings: AppSettings;
  handlePreviewAction: (action: string) => void;
  handleModelDialogOpen: (mode: ModelDialogMode, model?: ModelConfig) => void;
  handleModelDelete: (model: ModelConfig) => Promise<void>;
  handleModelEnabledToggle: (model: ModelConfig) => Promise<void>;
  handleModelSetActive: (model: ModelConfig) => Promise<void>;
  handleModelProxySave: (model: ModelConfig, proxyMode: ModelConfig['proxyMode'], proxyUrl: string) => Promise<void>;
  handleModelConnectivityTest: (model: ModelConfig) => Promise<void>;
  handleModelChatTest: (model: ModelConfig) => Promise<void>;
  handleModelConfigExport: () => Promise<void>;
  handleModelConfigImport: () => Promise<void>;
  handleSyncModelsToCatalog?: () => Promise<void>;
  compact?: boolean;
}) {
  const [testingActionBySlug, setTestingActionBySlug] = useState<Record<string, boolean | undefined>>({});
  const [configBusyAction, setConfigBusyAction] = useState<'export' | 'import' | null>(null);
  const [proxyDialogModel, setProxyDialogModel] = useState<ModelConfig | null>(null);
  const [actionMenuSlug, setActionMenuSlug] = useState<string | null>(null);
  const [actionMenuPosition, setActionMenuPosition] = useState<ActionMenuPosition | null>(null);

  const runConnectivityTest = async (model: ModelConfig) => {
    if (!model.enabled || model.status === 'testing' || testingActionBySlug[model.slug]) return;
    setTestingActionBySlug((current) => ({ ...current, [model.slug]: true }));
    try {
      await handleModelConnectivityTest(model);
    } finally {
      setTestingActionBySlug((current) => {
        const next = { ...current };
        delete next[model.slug];
        return next;
      });
    }
  };

  const runConfigAction = async (action: 'export' | 'import') => {
    if (configBusyAction) return;
    setConfigBusyAction(action);
    try {
      if (action === 'export') {
        await handleModelConfigExport();
      } else {
        await handleModelConfigImport();
      }
    } finally {
      setConfigBusyAction(null);
    }
  };

  const closeActionMenu = () => {
    setActionMenuSlug(null);
    setActionMenuPosition(null);
  };

  const toggleActionMenu = (event: MouseEvent<HTMLButtonElement>, model: ModelConfig, opensUp: boolean) => {
    if (actionMenuSlug === model.slug) {
      closeActionMenu();
      return;
    }

    const rect = event.currentTarget.getBoundingClientRect();
    const menuHeight = 88;
    const gap = 6;
    setActionMenuPosition({
      top: opensUp ? Math.max(gap, rect.top - menuHeight - gap) : rect.bottom + gap,
      right: Math.max(gap, window.innerWidth - rect.right),
    });
    setActionMenuSlug(model.slug);
  };

  return (
    <Card className="flex h-full min-h-0 flex-col overflow-hidden">
      <CardHeader className="shrink-0">
        <div className="flex items-center justify-between gap-4">
          <div>
            <h3 className="text-lg font-bold text-slate-950">渠道与路由管理</h3>
            <p className="mt-1 text-sm text-slate-500">管理上游渠道、对外模型映射与负载均衡策略。</p>
          </div>
          <div className="flex flex-wrap justify-end gap-2">
            <Button variant="secondary" onClick={() => runConfigAction('export')} disabled={configBusyAction !== null}>
              {configBusyAction === 'export' ? text.exporting : text.export}
            </Button>
            <Button variant="secondary" onClick={() => runConfigAction('import')} disabled={configBusyAction !== null}>
              {configBusyAction === 'import' ? text.importing : text.import}
            </Button>
            <Button onClick={() => handleModelDialogOpen('create')}>新增渠道</Button>
          </div>
        </div>
      </CardHeader>

      <div className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden px-6 py-5 pr-4" onScroll={closeActionMenu}>
        <div className="min-w-0 overflow-hidden rounded-2xl border border-slate-200">
          <table className="w-full table-fixed text-left text-base">
            <thead className="bg-slate-50 text-slate-500">
              <tr>
                <th className="px-3 py-3" style={{ width: '24%' }}>渠道名称</th>
                <th className="px-3 py-3" style={{ width: '16%' }}>渠道 ID</th>
                <th className="px-3 py-3" style={{ width: '24%' }}>上游模型与路由</th>
                <th className="px-3 py-3" style={{ width: '9%' }}>{text.enabledStatus}</th>
                <th className="px-3 py-3" style={{ width: '29%' }}>{text.actions}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100 bg-white">
              {models.map((model, index) => {
                const testDisabled = !model.enabled || model.status === 'testing' || testingActionBySlug[model.slug];
                const menuOpensUp = index >= models.length - 2;
                return (
                  <tr key={model.slug}>
                    <td className="overflow-hidden px-3 py-4">
                      <div className="truncate font-semibold text-slate-900" title={model.displayName}>{model.displayName}</div>
                      <div className="mt-1 flex min-w-0 items-center gap-1"><Badge tone="slate">{model.protocolType}</Badge><span className="truncate text-xs text-slate-500" title={model.baseUrl}>{model.baseUrl}</span></div>
                    </td>
                    <td className="overflow-hidden px-3 py-4">
                      <span className="break-all font-mono text-sm text-slate-700">{model.slug}</span>
                    </td>
                    <td className="overflow-hidden px-3 py-4">
                      <span className="block truncate font-mono text-sm text-slate-700" title={model.realModel}>{model.realModel}</span>
                      <div className="mt-1 truncate text-xs text-slate-500" title={model.modelMappings.join(', ')}>{model.modelMappings.length ? `对外：${model.modelMappings.join(', ')}` : '对外：渠道 ID'}</div>
                      <div className="mt-1 text-xs text-slate-500">优先级 {model.priority} · 权重 {model.weight}</div>
                    </td>
                    <td className="px-3 py-4">
                      <div className="flex flex-col items-start gap-2">
                        <Badge tone={model.enabled ? 'green' : 'slate'}>{model.enabled ? text.enabled : text.disabled}</Badge>
                        {model.active && <Badge tone="blue">{text.current}</Badge>}
                      </div>
                    </td>
                    <td className="px-3 py-4">
                      <div className="flex flex-nowrap items-center gap-1">
                        <IconButton label={text.setCurrent} icon="star" onClick={() => handleModelSetActive(model)} disabled={!model.enabled || model.active} />
                        <IconButton label={model.enabled ? text.connectivityTest : text.disabledTest} icon="network" loading={testingActionBySlug[model.slug]} onClick={() => runConnectivityTest(model)} disabled={testDisabled} />
                        <IconButton label={model.enabled ? text.disable : text.enable} icon={model.enabled ? 'pause' : 'play'} onClick={() => handleModelEnabledToggle(model)} />
                        {!compact && <IconButton label={text.edit} icon="edit" variant="ghost" onClick={() => handleModelDialogOpen('edit', model)} />}
                        <div className="relative">
                          <IconButton label={text.moreActions} icon="more" onClick={(event) => toggleActionMenu(event, model, menuOpensUp)} />
                          {actionMenuSlug === model.slug && <div className="fixed inset-0 z-10" onClick={closeActionMenu} />}
                          {actionMenuSlug === model.slug && actionMenuPosition && (
                            <div className="fixed z-20 w-36 rounded-xl border border-slate-200 bg-white py-1 shadow-lg" style={{ top: actionMenuPosition.top, right: actionMenuPosition.right }}>
                              <button className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm text-slate-700 hover:bg-slate-50" onClick={() => { closeActionMenu(); setProxyDialogModel(model); }}>
                                <Settings2 size={16} className="text-slate-400" />
                                <span>{text.proxyConfig}</span>
                              </button>
                              <button className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm text-rose-600 hover:bg-rose-50" onClick={() => { closeActionMenu(); handleModelDelete(model); }}>
                                <Trash2 size={16} className="text-rose-500" />
                                <span>{text.delete}</span>
                              </button>
                            </div>
                          )}
                        </div>
                      </div>
                    </td>
                  </tr>
                );
              })}
              {models.length === 0 && (
                <tr>
                  <td className="px-4 py-10 text-center text-sm text-slate-400" colSpan={5}>{text.empty}</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      {proxyDialogModel && (
        <ModelProxyConfigDialog
          model={proxyDialogModel}
          appSettings={appSettings}
          handleClose={() => setProxyDialogModel(null)}
          handleSave={async (proxyMode, proxyUrl) => {
            await handleModelProxySave(proxyDialogModel, proxyMode, proxyUrl);
            setProxyDialogModel(null);
          }}
        />
      )}
    </Card>
  );
}

type ModelActionIcon = 'star' | 'network' | 'pause' | 'play' | 'edit' | 'more';

const modelActionIcon: Record<ModelActionIcon, LucideIcon> = {
  star: Star,
  network: Network,
  pause: Pause,
  play: Play,
  edit: Pencil,
  more: MoreHorizontal,
};

function IconButton({
  label,
  icon,
  loading = false,
  variant = 'secondary',
  style,
  ...props
}: {
  label: string;
  icon: ModelActionIcon;
  loading?: boolean;
  variant?: 'secondary' | 'danger' | 'ghost';
} & ButtonHTMLAttributes<HTMLButtonElement>) {
  const Icon = modelActionIcon[icon];
  const iconColor = variant === 'danger' ? '#e11d48' : '#334155';

  return (
    <Button
      variant={variant}
      className="h-8 w-8 shrink-0 border border-slate-200 bg-white p-0 text-slate-700 shadow-sm hover:bg-slate-50 disabled:opacity-50"
      style={{ width: '2rem', height: '2rem', padding: 0, ...style }}
      title={loading ? text.checking : label}
      aria-label={loading ? `${label}，${text.checking}` : label}
      {...props}
    >
      {loading ? <SpinnerIcon /> : <Icon size={16} color={iconColor} strokeWidth={2.2} style={{ width: 16, height: 16, maxWidth: 'none' }} aria-hidden="true" />}
    </Button>
  );
}

function SpinnerIcon() {
  return (
    <svg className="h-4 w-4 animate-spin" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <circle className="opacity-25" cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="3" />
      <path className="opacity-75" d="M21 12a9 9 0 0 0-9-9" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
    </svg>
  );
}

function ModelProxyConfigDialog({
  model,
  appSettings,
  handleClose,
  handleSave,
}: {
  model: ModelConfig;
  appSettings: AppSettings;
  handleClose: () => void;
  handleSave: (proxyMode: ModelConfig['proxyMode'], proxyUrl: string) => Promise<void>;
}) {
  const defaultMode = 'default';
  const directMode = 'direct';
  const manualMode = 'manual';
  const appProxyUrl = appSettings.official_proxy_url.trim();
  const [proxyMode, setProxyMode] = useState<ModelConfig['proxyMode']>(model.proxyMode || defaultMode);
  const [proxyUrl, setProxyUrl] = useState(model.proxyUrl.trim() || appProxyUrl || 'http://127.0.0.1:4002');
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState('');
  const manual = proxyMode === manualMode;
  const proxyModeDescription =
    proxyMode === defaultMode
      ? '默认继承全局代理配置。'
      : proxyMode === directMode
        ? '当前强制走非代理模式。'
        : '当前模型使用单独的代理地址。';

  const handleProxyModeChange = (mode: ModelConfig['proxyMode']) => {
    setProxyMode(mode);
    if (mode === manualMode && !proxyUrl.trim()) {
      setProxyUrl(appProxyUrl || 'http://127.0.0.1:4002');
    }
  };

  const handleSubmit = async () => {
    if (saving) return;
    const nextProxyUrl = manual ? proxyUrl.trim() : '';
    if (manual && !nextProxyUrl) {
      setMessage('请先填写代理地址，或切换为默认/强制直连。');
      return;
    }

    setSaving(true);
    setMessage('');
    try {
      await handleSave(proxyMode, nextProxyUrl);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="w-full max-w-xl rounded-3xl bg-white p-5 shadow-2xl">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-lg font-bold text-slate-950">代理配置</h3>
            <p className="mt-2 text-sm leading-6 text-slate-500">{model.displayName} 的模型级代理，默认走全局模式。</p>
          </div>
          <button className="text-2xl leading-none text-slate-400 hover:text-slate-700" type="button" onClick={handleClose} disabled={saving}>{'\u00d7'}</button>
        </div>

        <div className="mt-5">
          <div className="text-sm font-semibold text-slate-800">代理模式</div>
          <div className="mt-3 inline-flex rounded-full bg-slate-100 p-1">
            <button type="button" className={`rounded-full px-4 py-2 text-sm ${proxyMode === defaultMode ? 'bg-white text-slate-950 shadow-sm' : 'text-slate-500'}`} onClick={() => handleProxyModeChange(defaultMode)}>默认</button>
            <button type="button" className={`rounded-full px-4 py-2 text-sm ${proxyMode === directMode ? 'bg-white text-slate-950 shadow-sm' : 'text-slate-500'}`} onClick={() => handleProxyModeChange(directMode)}>强制直连</button>
            <button type="button" className={`rounded-full px-4 py-2 text-sm ${proxyMode === manualMode ? 'bg-white text-slate-950 shadow-sm' : 'text-slate-500'}`} onClick={() => handleProxyModeChange(manualMode)}>手动代理</button>
          </div>
        </div>

        <div className="mt-5">
          {manual ? (
            <>
              <div className="mb-3 text-sm font-semibold text-slate-800">代理地址</div>
              <input className="w-full rounded-xl border border-slate-200 px-4 py-3 text-sm text-slate-700 outline-none focus:border-indigo-400 disabled:bg-slate-50 disabled:text-slate-400" value={proxyUrl} onChange={(event) => setProxyUrl(event.target.value)} placeholder="http://127.0.0.1:4002" disabled={saving} />
            </>
          ) : (
            <div className="rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 text-sm leading-6 text-slate-600">
              {proxyModeDescription}
            </div>
          )}
        </div>

        {message && <div className="mt-4 rounded-xl bg-amber-50 px-4 py-3 text-sm leading-6 text-amber-700">{message}</div>}
        <div className="mt-5 flex justify-end gap-3">
          <Button variant="secondary" onClick={handleClose} disabled={saving}>取消</Button>
          <Button onClick={handleSubmit} disabled={saving}>{saving ? '保存中' : '保存'}</Button>
        </div>
      </div>
    </div>
  );
}
