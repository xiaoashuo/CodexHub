import { useMemo, useState, type ReactNode } from 'react';
import { Button } from '../../components/ui/Button';
import { Card, CardContent, CardHeader } from '../../components/ui/Card';
import type { AppOperationLogEntry, AppSettings, FilePreviewResult, LocalConfigPaths, ProxyTestResult, RestartAppTarget } from '../../types';
import { APP_VERSION } from '../../lib/constants';
import { invokeDetectCodexExePath, invokeDetectProxyConnection, invokeTestProxyConnection, invokeToggleCodexTokenAutoRenew } from '../../lib/tauriBridge';

type SettingsTab = 'paths' | 'appLogs';
type ProxyMode = 'direct' | 'manual';

const DEFAULT_PROXY_URL = 'http://127.0.0.1:4002';
const DEFAULT_ROUTER_PORT = 25817;
const DEFAULT_LISTENER_PORT = 1455;

const settingsTabs: { key: SettingsTab; label: string }[] = [
  { key: 'paths', label: '配置路径' },
  { key: 'appLogs', label: '应用日志' },
];

const logLevelOptions: { value: AppOperationLogEntry['level'] | 'all'; label: string }[] = [
  { value: 'all', label: '全部' },
  { value: 'info', label: '信息' },
  { value: 'warn', label: '警告' },
  { value: 'error', label: '错误' },
];

export function SettingsPage({
  appOperationLogs,
  appSettings,
  localConfigPaths,
  filePreview,
  handleLocalFilePreview,
  handleLocalFilePreviewClose,
  handleAppLogsSearch,
  handleAppLogsClear,
  handleAppSettingsSave,
}: {
  appOperationLogs: AppOperationLogEntry[];
  appSettings: AppSettings;
  localConfigPaths: LocalConfigPaths;
  filePreview: FilePreviewResult | null;
  handleLocalFilePreview: (path: string) => Promise<void>;
  handleLocalFilePreviewClose: () => void;
  handleAppLogsSearch: (keyword: string, level: AppOperationLogEntry['level'] | 'all') => Promise<void>;
  handleAppLogsClear: () => Promise<void>;
  handleAppSettingsSave: (settings: AppSettings) => Promise<void>;
}) {
  const [activeTab, setActiveTab] = useState<SettingsTab>('paths');
  const [keyword, setKeyword] = useState('');
  const [level, setLevel] = useState<AppOperationLogEntry['level'] | 'all'>('all');
  const filteredLogCount = useMemo(() => appOperationLogs.length, [appOperationLogs]);

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 overflow-hidden">
      <div className="shrink-0">
        <h2 className="text-2xl font-bold text-slate-950">设置</h2>
      </div>

      <Card className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <CardHeader className="shrink-0 py-4">
          <div className="flex flex-wrap gap-4">
            {settingsTabs.map((tab) => (
              <button
                key={tab.key}
                type="button"
                className={`border-b-2 px-1 py-1 text-sm font-semibold transition ${
                  activeTab === tab.key ? 'border-indigo-600 text-indigo-700' : 'border-transparent text-slate-500 hover:text-slate-900'
                }`}
                onClick={() => setActiveTab(tab.key)}
              >
                {tab.label}
              </button>
            ))}
          </div>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 overflow-hidden pt-4">
          {activeTab === 'paths' ? (
            <PathSettings
              appSettings={appSettings}
              localConfigPaths={localConfigPaths}
              filePreview={filePreview}
              handleLocalFilePreview={handleLocalFilePreview}
              handleLocalFilePreviewClose={handleLocalFilePreviewClose}
              handleAppSettingsSave={handleAppSettingsSave}
            />
          ) : (
            <ApplicationLogs
              logs={appOperationLogs}
              keyword={keyword}
              level={level}
              filteredLogCount={filteredLogCount}
              setKeyword={setKeyword}
              setLevel={setLevel}
              handleSearch={() => handleAppLogsSearch(keyword, level)}
              handleClear={handleAppLogsClear}
            />
          )}
        </CardContent>
      </Card>
    </div>
  );
}

function PathSettings({
  appSettings,
  localConfigPaths,
  filePreview,
  handleLocalFilePreview,
  handleLocalFilePreviewClose,
  handleAppSettingsSave,
}: {
  appSettings: AppSettings;
  localConfigPaths: LocalConfigPaths;
  filePreview: FilePreviewResult | null;
  handleLocalFilePreview: (path: string) => Promise<void>;
  handleLocalFilePreviewClose: () => void;
  handleAppSettingsSave: (settings: AppSettings) => Promise<void>;
}) {
  const [codexDialogOpen, setCodexDialogOpen] = useState(false);
  const [restartTargetDialogOpen, setRestartTargetDialogOpen] = useState(false);
  const [proxyDialogOpen, setProxyDialogOpen] = useState(false);
  const [portDialogOpen, setPortDialogOpen] = useState(false);
  const [coreFilesOpen, setCoreFilesOpen] = useState(false);

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 overflow-hidden">
      <div className="min-h-0 overflow-y-auto rounded-xl border border-slate-200 bg-white">
        <SettingsListRow title="Codex 启动命令" description="用于重启 Codex/ChatGPT 客户端的可执行文件路径。" actionLabel="设置" onAction={() => setCodexDialogOpen(true)} />
        <SettingsListRow
          title="重新启动应用"
          description="选择 Router 启停、账号切换等操作后要重启的客户端。"
          value={getRestartTargetLabel(appSettings.app_restart_target)}
          actionLabel="设置"
          onAction={() => setRestartTargetDialogOpen(true)}
        />
        <SettingsListRow
          title="代理配置"
          description="仅用于官方 Codex API 请求。"
          value={appSettings.official_proxy_url || '直连'}
          actionLabel="设置"
          onAction={() => setProxyDialogOpen(true)}
        />
        <SettingsListRow
          title="端口配置"
          description="Router 服务端口和账号登录回调监听端口。"
          value={<PortSummaryValue routerPort={appSettings.router_port || DEFAULT_ROUTER_PORT} listenerPort={appSettings.oauth_callback_port || DEFAULT_LISTENER_PORT} />}
          actionLabel="设置"
          onAction={() => setPortDialogOpen(true)}
        />
        <SettingsListRow title="核心文件" description="当前工作目录、配置、Catalog、日志等路径。" actionLabel="查看" onAction={() => setCoreFilesOpen(true)} />
        <TokenAutoRenewRow enabled={appSettings.token_auto_renew_enabled || false} onToggle={async (enabled: boolean) => { await invokeToggleCodexTokenAutoRenew(enabled); await handleAppSettingsSave({ ...appSettings, token_auto_renew_enabled: enabled }); }} />
        <SettingsListRow title="当前版本" value={normalizeDisplayVersion(appSettings.system_version)} valueVariant="plain" valuePlacement="right" />
      </div>

      {codexDialogOpen && <CodexPathDialog appSettings={appSettings} handleClose={() => setCodexDialogOpen(false)} handleAppSettingsSave={handleAppSettingsSave} />}
      {restartTargetDialogOpen && <RestartTargetDialog appSettings={appSettings} handleClose={() => setRestartTargetDialogOpen(false)} handleAppSettingsSave={handleAppSettingsSave} />}
      {proxyDialogOpen && <ProxyConfigDialog appSettings={appSettings} handleClose={() => setProxyDialogOpen(false)} handleAppSettingsSave={handleAppSettingsSave} />}
      {portDialogOpen && <PortConfigDialog appSettings={appSettings} handleClose={() => setPortDialogOpen(false)} handleAppSettingsSave={handleAppSettingsSave} />}
      {coreFilesOpen && <CoreFilesDialog localConfigPaths={localConfigPaths} handleClose={() => setCoreFilesOpen(false)} handleLocalFilePreview={handleLocalFilePreview} />}
      {filePreview && <FilePreviewDialog preview={filePreview} handleLocalFilePreviewClose={handleLocalFilePreviewClose} />}
    </div>
  );
}

function TokenAutoRenewRow({ enabled, onToggle }: { enabled: boolean; onToggle: (enabled: boolean) => Promise<void> }) {
  const [toggling, setToggling] = useState(false);
  const handleToggle = async () => {
    setToggling(true);
    try {
      await onToggle(!enabled);
    } finally {
      setToggling(false);
    }
  };
  return (
    <div className="grid min-h-[68px] items-center gap-3 border-b border-slate-100 px-4 py-3 last:border-b-0 md:grid-cols-[260px_minmax(0,1fr)_auto]">
      <div className="min-w-0">
        <div className="text-sm font-semibold text-slate-900">Token 自动续期</div>
        <div className="mt-1 text-xs leading-5 text-slate-500">定时刷新 access token</div>
      </div>
      <div />
      <button
        type="button"
        disabled={toggling}
        onClick={handleToggle}
        className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 disabled:opacity-50 ${enabled ? 'bg-indigo-600' : 'bg-slate-300'}`}
      >
        <span className={`inline-block h-4 w-4 transform rounded-full bg-white shadow-sm transition-transform ${enabled ? 'translate-x-6' : 'translate-x-1'}`} />
      </button>
    </div>
  );
}

function SettingsListRow({
  title,
  description,
  value,
  valueVariant = 'boxed',
  valuePlacement = 'content',
  action,
  actionLabel,
  onAction,
}: {
  title: string;
  description?: string;
  value?: ReactNode;
  valueVariant?: 'boxed' | 'plain';
  valuePlacement?: 'content' | 'right';
  action?: ReactNode;
  actionLabel?: string;
  onAction?: () => void;
}) {
  const valueNode = value ? (
    <div className={`min-w-0 ${typeof value === 'string' ? 'truncate text-xs' : ''} ${valueVariant === 'boxed' ? 'text-slate-600' : 'text-slate-500'}`}>
      {value}
    </div>
  ) : null;

  return (
    <div className="grid min-h-[68px] items-center gap-3 border-b border-slate-100 px-4 py-3 last:border-b-0 md:grid-cols-[260px_minmax(0,1fr)_auto]">
      <div className="min-w-0">
        <div className="text-sm font-semibold text-slate-900">{title}</div>
        {description && <div className="mt-1 text-xs leading-5 text-slate-500">{description}</div>}
      </div>
      {valuePlacement === 'content' ? valueNode || <div /> : <div />}
      {action || (actionLabel && onAction ? <Button variant="secondary" onClick={onAction}>{actionLabel}</Button> : valuePlacement === 'right' ? valueNode || <div /> : <div />)}
    </div>
  );
}

function PortSummaryValue({ routerPort, listenerPort }: { routerPort: number; listenerPort: number }) {
  return (
    <div className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-1 text-xs">
      <span className="inline-flex min-w-0 items-baseline gap-1.5">
        <span className="font-semibold text-slate-500">Router</span>
        <span className="font-mono font-semibold text-slate-900">{routerPort}</span>
      </span>
      <span className="inline-flex min-w-0 items-baseline gap-1.5">
        <span className="font-semibold text-slate-500">监听</span>
        <span className="font-mono font-semibold text-slate-900">{listenerPort}</span>
      </span>
    </div>
  );
}

function getRestartTargetLabel(target: RestartAppTarget) {
  return target === 'codex' ? 'Codex' : 'ChatGPT';
}

function RestartTargetDialog({ appSettings, handleClose, handleAppSettingsSave }: { appSettings: AppSettings; handleClose: () => void; handleAppSettingsSave: (settings: AppSettings) => Promise<void> }) {
  const [target, setTarget] = useState<RestartAppTarget>(appSettings.app_restart_target || 'chatgpt');
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    if (saving) return;
    setSaving(true);
    try {
      await handleAppSettingsSave({ ...appSettings, app_restart_target: target });
      handleClose();
    } finally {
      setSaving(false);
    }
  };

  return (
    <DialogFrame title="重新启动应用" description="选择需要自动关闭并重新启动的客户端。" handleClose={handleClose} maxWidth="max-w-xl">
      <div className="grid gap-3 sm:grid-cols-2">
        <RestartTargetOption target="chatgpt" currentTarget={target} title="ChatGPT" description="适用于新版客户端。" onSelect={setTarget} />
        <RestartTargetOption target="codex" currentTarget={target} title="Codex" description="适用于旧版 Codex 客户端。" onSelect={setTarget} />
      </div>
      <DialogActions handleClose={handleClose} saving={saving} handleSave={handleSave} />
    </DialogFrame>
  );
}

function RestartTargetOption({ target, currentTarget, title, description, onSelect }: { target: RestartAppTarget; currentTarget: RestartAppTarget; title: string; description: string; onSelect: (target: RestartAppTarget) => void }) {
  const selected = target === currentTarget;
  return (
    <button
      type="button"
      className={`rounded-xl border px-4 py-3 text-left transition ${selected ? 'border-indigo-500 bg-indigo-50 text-indigo-900' : 'border-slate-200 bg-white text-slate-700 hover:border-slate-300'}`}
      onClick={() => onSelect(target)}
    >
      <div className="text-sm font-semibold">{title}</div>
      <div className="mt-1 text-xs leading-5 text-slate-500">{description}</div>
    </button>
  );
}

function CodexPathDialog({ appSettings, handleClose, handleAppSettingsSave }: { appSettings: AppSettings; handleClose: () => void; handleAppSettingsSave: (settings: AppSettings) => Promise<void> }) {
  const [codexExePath, setCodexExePath] = useState(appSettings.codex_exe_path);
  const [saving, setSaving] = useState(false);
  const [detecting, setDetecting] = useState(false);
  const [message, setMessage] = useState('');

  const handleDetect = async () => {
    if (detecting) return;
    setDetecting(true);
    setMessage('');
    try {
      const detectedPath = await invokeDetectCodexExePath();
      setCodexExePath(detectedPath);
      setMessage(`已检测到：${detectedPath}`);
    } catch (error) {
      setMessage(formatUnknownError(error));
    } finally {
      setDetecting(false);
    }
  };

  const handleSave = async () => {
    if (saving) return;
    setSaving(true);
    try {
      await handleAppSettingsSave({ ...appSettings, codex_exe_path: codexExePath });
      handleClose();
    } finally {
      setSaving(false);
    }
  };

  return (
    <DialogFrame title="Codex 启动命令" description="填写 codex.exe 完整路径，或让应用自动检测当前可用命令。" handleClose={handleClose} maxWidth="max-w-2xl">
      <div className="grid gap-3 lg:grid-cols-[1fr_auto]">
        <input className="w-full rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm text-slate-700 outline-none focus:border-indigo-400" value={codexExePath} onChange={(event) => setCodexExePath(event.target.value)} placeholder="例如 C:\\Program Files\\Codex\\codex.exe" />
        <Button variant="secondary" onClick={handleDetect} disabled={detecting}>{detecting ? '检测中' : '检测'}</Button>
      </div>
      {message && <div className="mt-2 break-all text-xs text-slate-500">{message}</div>}
      <DialogActions handleClose={handleClose} saving={saving} handleSave={handleSave} />
    </DialogFrame>
  );
}

function PortConfigDialog({ appSettings, handleClose, handleAppSettingsSave }: { appSettings: AppSettings; handleClose: () => void; handleAppSettingsSave: (settings: AppSettings) => Promise<void> }) {
  const [routerPort, setRouterPort] = useState(String(appSettings.router_port || DEFAULT_ROUTER_PORT));
  const [oauthCallbackPort, setOauthCallbackPort] = useState(String(appSettings.oauth_callback_port || DEFAULT_LISTENER_PORT));
  const [saving, setSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState('');

  const handleSave = async () => {
    if (saving) return;
    const nextRouterPort = normalizePortInput(routerPort, DEFAULT_ROUTER_PORT);
    const nextOauthCallbackPort = normalizePortInput(oauthCallbackPort, DEFAULT_LISTENER_PORT);
    if (nextRouterPort === nextOauthCallbackPort) {
      setErrorMessage('Router 端口和监听端口不能相同。');
      return;
    }
    setSaving(true);
    setErrorMessage('');
    try {
      await handleAppSettingsSave({ ...appSettings, router_port: nextRouterPort, oauth_callback_port: nextOauthCallbackPort });
      handleClose();
    } finally {
      setSaving(false);
    }
  };

  return (
    <DialogFrame title="端口配置" description="Router 端口保存后下次启动 Router 生效；监听端口保存后下次启动应用生效。" handleClose={handleClose} maxWidth="max-w-xl">
      <div className="grid gap-4 sm:grid-cols-2">
        <PortInput label="Router 端口" value={routerPort} defaultValue={DEFAULT_ROUTER_PORT} onChange={setRouterPort} />
        <PortInput label="监听端口" value={oauthCallbackPort} defaultValue={DEFAULT_LISTENER_PORT} onChange={setOauthCallbackPort} />
      </div>
      {errorMessage && <div className="mt-4 rounded-xl bg-rose-50 px-4 py-3 text-sm text-rose-700">{errorMessage}</div>}
      <DialogActions handleClose={handleClose} saving={saving} handleSave={handleSave} />
    </DialogFrame>
  );
}

function ProxyConfigDialog({ appSettings, handleClose, handleAppSettingsSave }: { appSettings: AppSettings; handleClose: () => void; handleAppSettingsSave: (settings: AppSettings) => Promise<void> }) {
  const [proxyMode, setProxyMode] = useState<ProxyMode>(appSettings.official_proxy_url ? 'manual' : 'direct');
  const [proxyUrl, setProxyUrl] = useState(appSettings.official_proxy_url || DEFAULT_PROXY_URL);
  const [busy, setBusy] = useState(false);
  const [testResult, setTestResult] = useState<ProxyTestResult | null>(null);
  const [errorMessage, setErrorMessage] = useState('');
  const manualMode = proxyMode === 'manual';

  const handleDetect = async () => {
    if (busy) return;
    setBusy(true);
    setErrorMessage('');
    setTestResult(null);
    try {
      const result = await invokeDetectProxyConnection();
      setProxyMode('manual');
      setProxyUrl(result.proxy_url);
      setTestResult(result);
    } catch (error) {
      setErrorMessage(formatUnknownError(error));
    } finally {
      setBusy(false);
    }
  };

  const handleTest = async () => {
    if (busy) return;
    if (!manualMode) {
      setTestResult({ success: true, proxy_url: '', latency_ms: 0, latency: '-', status_code: 0, message: '当前为直连模式。' });
      setErrorMessage('');
      return;
    }
    const nextProxyUrl = proxyUrl.trim();
    if (!nextProxyUrl) {
      setErrorMessage('请先填写代理地址，或切换为直连。');
      setTestResult(null);
      return;
    }
    setBusy(true);
    setErrorMessage('');
    setTestResult(null);
    try {
      setTestResult(await invokeTestProxyConnection(nextProxyUrl));
    } catch (error) {
      setErrorMessage(formatUnknownError(error));
    } finally {
      setBusy(false);
    }
  };

  const handleSave = async () => {
    if (busy) return;
    const nextProxyUrl = manualMode ? proxyUrl.trim() : '';
    if (manualMode && !nextProxyUrl) {
      setErrorMessage('请先填写代理地址，或切换为直连。');
      return;
    }
    setBusy(true);
    setErrorMessage('');
    try {
      await handleAppSettingsSave({ ...appSettings, official_proxy_url: nextProxyUrl });
      handleClose();
    } finally {
      setBusy(false);
    }
  };

  return (
    <DialogFrame title="代理配置" description="仅为官方 Codex API 请求配置代理，其它请求使用模型自身配置。" handleClose={handleClose} maxWidth="max-w-xl">
      <div>
        <div className="text-sm font-semibold text-slate-800">代理模式</div>
        <div className="mt-3 inline-flex rounded-full bg-slate-100 p-1">
          <button type="button" className={`rounded-full px-4 py-2 text-sm ${proxyMode === 'direct' ? 'bg-white text-slate-950 shadow-sm' : 'text-slate-500'}`} onClick={() => setProxyMode('direct')}>直连</button>
          <button type="button" className={`rounded-full px-4 py-2 text-sm ${proxyMode === 'manual' ? 'bg-white text-slate-950 shadow-sm' : 'text-slate-500'}`} onClick={() => setProxyMode('manual')}>手动代理</button>
        </div>
      </div>
      {manualMode ? (
        <div className="mt-5 grid gap-3 sm:grid-cols-[1fr_auto]">
          <input className="w-full rounded-xl border border-slate-200 px-4 py-3 text-sm text-slate-700 outline-none focus:border-indigo-400" value={proxyUrl} onChange={(event) => setProxyUrl(event.target.value)} placeholder={DEFAULT_PROXY_URL} disabled={busy} />
          <Button variant="secondary" onClick={handleDetect} disabled={busy}>一键检测</Button>
        </div>
      ) : (
        <div className="mt-5 rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 text-sm leading-6 text-slate-600">当前为直连模式。</div>
      )}
      {(testResult || errorMessage) && <div className={`mt-4 rounded-xl px-4 py-3 text-sm leading-6 ${testResult?.success ? 'bg-emerald-50 text-emerald-700' : 'bg-rose-50 text-rose-700'}`}>{testResult ? `${testResult.message} 延迟：${testResult.latency}` : errorMessage}</div>}
      <div className="mt-5 flex justify-end gap-3">
        <Button variant="secondary" onClick={handleClose} disabled={busy}>取消</Button>
        <Button variant="secondary" onClick={handleTest} disabled={busy}>测试</Button>
        <Button onClick={handleSave} disabled={busy}>保存</Button>
      </div>
    </DialogFrame>
  );
}

function CoreFilesDialog({ localConfigPaths, handleClose, handleLocalFilePreview }: { localConfigPaths: LocalConfigPaths; handleClose: () => void; handleLocalFilePreview: (path: string) => Promise<void> }) {
  return (
    <DialogFrame title="核心文件" description="查看应用工作目录和核心配置文件路径。" handleClose={handleClose} maxWidth="max-w-4xl">
      <div className="max-h-[70vh] overflow-y-auto rounded-xl border border-slate-100">
        <PathPreviewRow label="用户根目录" value={localConfigPaths.user_home_path} handleLocalFilePreview={handleLocalFilePreview} previewable={false} />
        {buildConfigPathItems(localConfigPaths).map((item) => (
          <PathPreviewRow key={item.label} label={item.label} value={item.value} handleLocalFilePreview={handleLocalFilePreview} />
        ))}
      </div>
    </DialogFrame>
  );
}

function PathPreviewRow({ label, value, handleLocalFilePreview, previewable = true }: { label: string; value: string; handleLocalFilePreview: (path: string) => Promise<void>; previewable?: boolean }) {
  return (
    <div className="grid min-h-[58px] items-center gap-3 border-b border-slate-100 px-4 py-3 last:border-b-0 lg:grid-cols-[160px_minmax(0,1fr)_auto]">
      <span className="text-sm font-semibold text-slate-800">{label}</span>
      <input className="min-w-0 rounded-lg border border-slate-200 bg-slate-50 px-3 py-2.5 font-mono text-xs text-slate-700 outline-none" value={value || '路径加载中...'} readOnly />
      <Button variant="secondary" onClick={() => handleLocalFilePreview(value)} disabled={!value || !previewable}>查看</Button>
    </div>
  );
}

function FilePreviewDialog({ preview, handleLocalFilePreviewClose }: { preview: FilePreviewResult; handleLocalFilePreviewClose: () => void }) {
  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="w-full max-w-4xl overflow-hidden rounded-3xl bg-white shadow-2xl">
        <div className="flex items-start justify-between gap-4 border-b border-slate-100 px-6 py-5">
          <div>
            <h3 className="text-lg font-bold text-slate-950">配置文件内容</h3>
            <div className="mt-1 break-all font-mono text-xs text-slate-400">{preview.path}</div>
          </div>
          <Button variant="ghost" onClick={handleLocalFilePreviewClose}>关闭</Button>
        </div>
        {preview.exists ? (
          <pre className="max-h-[70vh] overflow-auto whitespace-pre-wrap bg-slate-950 p-6 text-sm leading-6 text-indigo-100">
            {preview.content}{preview.truncated ? '\n\n...内容过长，已截断预览' : ''}
          </pre>
        ) : (
          <div className="px-6 py-16 text-center text-sm text-slate-400">当前路径文件不存在。</div>
        )}
      </div>
    </div>
  );
}

function DialogFrame({ title, description, handleClose, maxWidth, children }: { title: string; description: string; handleClose: () => void; maxWidth: string; children: ReactNode }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className={`max-h-[88vh] w-full ${maxWidth} overflow-hidden rounded-3xl bg-white p-5 shadow-2xl`}>
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-lg font-bold text-slate-950">{title}</h3>
            <p className="mt-2 text-sm leading-6 text-slate-500">{description}</p>
          </div>
          <button className="text-2xl leading-none text-slate-400 hover:text-slate-700" type="button" onClick={handleClose}>x</button>
        </div>
        <div className="mt-5">{children}</div>
      </div>
    </div>
  );
}

function DialogActions({ handleClose, saving, handleSave }: { handleClose: () => void; saving: boolean; handleSave: () => void }) {
  return (
    <div className="mt-5 flex justify-end gap-3">
      <Button variant="secondary" onClick={handleClose} disabled={saving}>取消</Button>
      <Button onClick={handleSave} disabled={saving}>{saving ? '保存中' : '保存'}</Button>
    </div>
  );
}

function PortInput({ label, value, defaultValue, onChange }: { label: string; value: string; defaultValue: number; onChange: (value: string) => void }) {
  return (
    <label className="block">
      <span className="text-xs font-semibold text-slate-500">{label}</span>
      <input className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-700 outline-none focus:border-indigo-400" type="number" min={1} max={65535} value={value} onChange={(event) => onChange(event.target.value)} placeholder={String(defaultValue)} />
      <span className="mt-1 block text-xs text-slate-400">默认 {defaultValue}</span>
    </label>
  );
}

function ApplicationLogs({
  logs,
  keyword,
  level,
  filteredLogCount,
  setKeyword,
  setLevel,
  handleSearch,
  handleClear,
}: {
  logs: AppOperationLogEntry[];
  keyword: string;
  level: AppOperationLogEntry['level'] | 'all';
  filteredLogCount: number;
  setKeyword: (keyword: string) => void;
  setLevel: (level: AppOperationLogEntry['level'] | 'all') => void;
  handleSearch: () => Promise<void>;
  handleClear: () => Promise<void>;
}) {
  const [expandedLogIds, setExpandedLogIds] = useState<Set<string>>(new Set());
  const toggleLogDetail = (id: string) => {
    setExpandedLogIds((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 overflow-hidden">
      <div className="grid shrink-0 gap-3 lg:grid-cols-[1fr_160px_auto_auto]">
        <input className="rounded-2xl border border-slate-200 px-4 py-3 text-sm outline-none focus:border-indigo-400" value={keyword} onChange={(event) => setKeyword(event.target.value)} placeholder="搜索模块、动作、消息、详情" />
        <select className="rounded-2xl border border-slate-200 px-4 py-3 text-sm outline-none focus:border-indigo-400" value={level} onChange={(event) => setLevel(event.target.value as AppOperationLogEntry['level'] | 'all')}>
          {logLevelOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
        </select>
        <Button onClick={handleSearch}>搜索</Button>
        <Button variant="ghost" onClick={handleClear}>清空日志</Button>
      </div>
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-3 text-sm text-slate-500">
        <span>当前显示 {filteredLogCount} 条</span>
        <span>日志以 JSON Lines 写入本地文件。</span>
      </div>
      <div className="min-h-0 flex-1 space-y-2 overflow-y-auto pr-2">
        {logs.map((log) => (
          <div key={log.id} className="rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm">
            <div className="grid items-start gap-3 lg:grid-cols-[minmax(0,1fr)_auto]">
              <div className="flex min-w-0 flex-wrap items-center gap-2">
                <span className={`rounded-full px-2.5 py-1 text-xs font-semibold ${getLevelClassName(log.level)}`}>{getLevelText(log.level)}</span>
                <span className="font-semibold text-slate-800">{log.module}</span>
                <span className="text-slate-400">/</span>
                <span className="min-w-0 truncate text-slate-700">{log.action}</span>
              </div>
              <div className="flex items-center justify-end gap-2">
                <span className="font-mono text-xs text-slate-400">{formatLogTime(log.time)}</span>
                {log.detail && <button className="rounded-md px-2 py-1 text-xs font-semibold text-indigo-600 hover:bg-indigo-50" type="button" onClick={() => toggleLogDetail(log.id)}>{expandedLogIds.has(log.id) ? '收起' : '详情'}</button>}
              </div>
            </div>
            <p className="mt-2 line-clamp-2 leading-6 text-slate-600">{log.message}</p>
            {log.detail && expandedLogIds.has(log.id) && <pre className="mt-3 max-h-48 overflow-auto whitespace-pre-wrap rounded-lg bg-slate-950 p-3 text-xs leading-5 text-slate-100">{log.detail}</pre>}
          </div>
        ))}
        {logs.length === 0 && <div className="rounded-2xl bg-slate-50 px-4 py-8 text-center text-sm text-slate-400">暂无应用日志。</div>}
      </div>
    </div>
  );
}

function buildConfigPathItems(localConfigPaths: LocalConfigPaths) {
  return [
    { label: 'Codex config.toml', value: localConfigPaths.codex_config_path },
    { label: 'Catalog JSON', value: localConfigPaths.catalog_path },
    { label: 'Router provider config', value: localConfigPaths.provider_config_path },
    { label: 'App settings JSON', value: localConfigPaths.app_settings_path },
    { label: 'App operation log', value: localConfigPaths.app_log_path },
    { label: 'Router debug log', value: localConfigPaths.router_debug_log_path },
  ];
}

function normalizeDisplayVersion(version: string) {
  const normalized = version.trim().replace(/^v/i, '');
  return /^\d+(?:\.\d+){0,3}(?:[-+][0-9A-Za-z.-]+)?$/.test(normalized) ? normalized : APP_VERSION;
}

function normalizePortInput(value: string, defaultValue: number) {
  const port = Number.parseInt(value, 10);
  if (!Number.isFinite(port) || port < 1 || port > 65535) return defaultValue;
  return port;
}

function formatLogTime(value: string) {
  const numericValue = Number(value);
  const date = Number.isFinite(numericValue) ? new Date(numericValue > 1_000_000_000_000 ? numericValue : numericValue * 1000) : new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const pad = (number: number) => String(number).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

function getLevelText(level: AppOperationLogEntry['level']) {
  const levelTextMap: Record<AppOperationLogEntry['level'], string> = {
    info: '信息',
    warn: '警告',
    error: '错误',
  };
  return levelTextMap[level];
}

function getLevelClassName(level: AppOperationLogEntry['level']) {
  const levelClassNameMap: Record<AppOperationLogEntry['level'], string> = {
    info: 'bg-indigo-50 text-indigo-700',
    warn: 'bg-amber-50 text-amber-700',
    error: 'bg-rose-50 text-rose-700',
  };
  return levelClassNameMap[level];
}

function formatUnknownError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
