import { useState, type ReactNode } from 'react';
import { Button } from '../../components/ui/Button';
import { Card, CardContent, CardHeader } from '../../components/ui/Card';
import type { AppSettings, FilePreviewResult, LocalConfigPaths, ProxyTestResult } from '../../types';
import { APP_VERSION } from '../../lib/constants';
import { invokeDetectCodexExePath, invokeDetectProxyConnection, invokeTestProxyConnection, invokeToggleCodexTokenAutoRenew } from '../../lib/tauriBridge';
import { MaintenanceToolsPage } from '../maintenance/MaintenanceToolsPage';

type ProxyMode = 'direct' | 'manual';

const DEFAULT_PROXY_URL = 'http://127.0.0.1:4002';

function SystemSettingsPage({
  appSettings,
  localConfigPaths,
  filePreview,
  handleLocalFilePreview,
  handleLocalFilePreviewClose,
  handleAppSettingsSave,
  handleCodexRestart,
}: {
  appSettings: AppSettings;
  localConfigPaths: LocalConfigPaths;
  filePreview: FilePreviewResult | null;
  handleLocalFilePreview: (path: string) => Promise<void>;
  handleLocalFilePreviewClose: () => void;
  handleAppSettingsSave: (settings: AppSettings) => Promise<void>;
  handleCodexRestart: () => Promise<{ success: boolean; message: string }>;
}) {
  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
      <Card className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <CardHeader className="shrink-0 py-4">
          <h3 className="text-lg font-bold text-slate-950">配置路径</h3>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 overflow-hidden pt-4">
          <PathSettings
            appSettings={appSettings}
            localConfigPaths={localConfigPaths}
            filePreview={filePreview}
            handleLocalFilePreview={handleLocalFilePreview}
            handleLocalFilePreviewClose={handleLocalFilePreviewClose}
            handleAppSettingsSave={handleAppSettingsSave}
            handleCodexRestart={handleCodexRestart}
          />
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
  handleCodexRestart,
}: {
  appSettings: AppSettings;
  localConfigPaths: LocalConfigPaths;
  filePreview: FilePreviewResult | null;
  handleLocalFilePreview: (path: string) => Promise<void>;
  handleLocalFilePreviewClose: () => void;
  handleAppSettingsSave: (settings: AppSettings) => Promise<void>;
  handleCodexRestart: () => Promise<{ success: boolean; message: string }>;
}) {
  const [codexDialogOpen, setCodexDialogOpen] = useState(false);
  const [proxyDialogOpen, setProxyDialogOpen] = useState(false);
  const [coreFilesOpen, setCoreFilesOpen] = useState(false);

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 overflow-hidden">
      <div className="min-h-0 overflow-y-auto rounded-xl border border-slate-200 bg-white">
        <SettingsListRow title="Codex 启动命令" actionLabel="设置" onAction={() => setCodexDialogOpen(true)} />
        <SettingsListRow
          title="重启ChatGPT"
          actionLabel="重启"
          onAction={() => void handleCodexRestart()}
        />
        <SettingsListRow
          title="代理配置"
          description="仅用于官方 Codex API 请求。"
          value={appSettings.official_proxy_url || '直连'}
          actionLabel="设置"
          onAction={() => setProxyDialogOpen(true)}
        />
        <SettingsListRow title="核心文件" description="当前工作目录、配置、Catalog、日志等路径。" actionLabel="查看" onAction={() => setCoreFilesOpen(true)} />
        <TokenAutoRenewRow enabled={appSettings.token_auto_renew_enabled || false} onToggle={async (enabled: boolean) => { await invokeToggleCodexTokenAutoRenew(enabled); await handleAppSettingsSave({ ...appSettings, token_auto_renew_enabled: enabled }); }} />
        <SettingsListRow title="当前版本" value={normalizeDisplayVersion(appSettings.system_version)} valueVariant="plain" valuePlacement="right" />
      </div>

      {codexDialogOpen && <CodexPathDialog appSettings={appSettings} handleClose={() => setCodexDialogOpen(false)} handleAppSettingsSave={handleAppSettingsSave} />}
      {proxyDialogOpen && <ProxyConfigDialog appSettings={appSettings} handleClose={() => setProxyDialogOpen(false)} handleAppSettingsSave={handleAppSettingsSave} />}
      {coreFilesOpen && <CoreFilesDialog localConfigPaths={localConfigPaths} handleClose={() => setCoreFilesOpen(false)} handleLocalFilePreview={handleLocalFilePreview} />}
      {filePreview && <FilePreviewDialog preview={filePreview} handleLocalFilePreviewClose={handleLocalFilePreviewClose} />}
    </div>
  );
}

export function SettingsPage(props: {
  appSettings: AppSettings;
  localConfigPaths: LocalConfigPaths;
  filePreview: FilePreviewResult | null;
  handleLocalFilePreview: (path: string) => Promise<void>;
  handleLocalFilePreviewClose: () => void;
  handleAppSettingsSave: (settings: AppSettings) => Promise<void>;
  handleCodexRestart: () => Promise<{ success: boolean; message: string }>;
}) {
  const [activeTab, setActiveTab] = useState<'system' | 'maintenance' | 'audit'>('system');

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
      <div className="mb-5 shrink-0">
        <div className="flex items-center justify-between gap-4">
          <h2 className="text-2xl font-bold leading-tight text-slate-950">设置</h2>
        <div className="flex gap-1 rounded-2xl bg-slate-100 p-1">
          <button type="button" onClick={() => setActiveTab('system')} className={`rounded-xl px-5 py-2.5 text-sm font-semibold transition ${activeTab === 'system' ? 'bg-white text-indigo-700 shadow-sm' : 'text-slate-500 hover:text-slate-800'}`}>系统设置</button>
          <button type="button" onClick={() => setActiveTab('audit')} className={`rounded-xl px-5 py-2.5 text-sm font-semibold transition ${activeTab === 'audit' ? 'bg-white text-indigo-700 shadow-sm' : 'text-slate-500 hover:text-slate-800'}`}>审计配置</button>
          <button type="button" onClick={() => setActiveTab('maintenance')} className={`rounded-xl px-5 py-2.5 text-sm font-semibold transition ${activeTab === 'maintenance' ? 'bg-white text-indigo-700 shadow-sm' : 'text-slate-500 hover:text-slate-800'}`}>维护工具</button>
        </div>
        </div>
        <p className="mt-1.5 text-sm text-slate-500">管理 Codex 启动、代理、审计与应用维护选项。</p>
      </div>
      <div className="min-h-0 flex-1 overflow-hidden">
        {activeTab === 'system' ? <SystemSettingsPage {...props} /> : activeTab === 'audit' ? <AuditSettingsPage {...props} /> : <MaintenanceToolsPage appSettings={props.appSettings} handleAppSettingsSave={props.handleAppSettingsSave} />}
      </div>
    </div>
  );
}

function AuditSettingsPage({
  appSettings,
  handleAppSettingsSave,
}: {
  appSettings: AppSettings;
  handleAppSettingsSave: (settings: AppSettings) => Promise<void>;
}) {
  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
      <Card className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <CardHeader className="shrink-0 py-4">
          <h3 className="text-lg font-bold text-slate-950">审计配置</h3>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 overflow-hidden pt-4">
          <div className="min-h-0 overflow-y-auto rounded-xl border border-slate-200 bg-white">
            <SettingsToggleRow
              title="审计请求体"
              description="开启后，路由日志会记录转发到上游模型的完整请求体。"
              enabled={appSettings.audit_request_enabled ?? true}
              onToggle={async (enabled) => {
                await handleAppSettingsSave({ ...appSettings, audit_request_enabled: enabled });
              }}
            />
            <SettingsToggleRow
              title="审计响应体"
              description="开启后，路由日志会记录上游模型返回的完整响应体。"
              enabled={appSettings.audit_response_enabled ?? true}
              onToggle={async (enabled) => {
                await handleAppSettingsSave({ ...appSettings, audit_response_enabled: enabled });
              }}
            />
            <SettingsListRow title="说明" description="关闭对应开关后，新的路由日志将不再记录该部分内容；已记录的日志不受影响。" valueVariant="plain" />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function SettingsToggleRow({
  title,
  description,
  enabled,
  onToggle,
}: {
  title: string;
  description: string;
  enabled: boolean;
  onToggle: (enabled: boolean) => Promise<void>;
}) {
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
        <div className="text-sm font-semibold text-slate-900">{title}</div>
        <div className="mt-1 text-xs leading-5 text-slate-500">{description}</div>
      </div>
      <div />
      <button
        type="button"
        aria-label={`${enabled ? '关闭' : '打开'}${title}`}
        disabled={toggling}
        onClick={handleToggle}
        className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 disabled:opacity-50 ${enabled ? 'bg-indigo-600' : 'bg-slate-300'}`}
      >
        <span className={`inline-block h-4 w-4 transform rounded-full bg-white shadow-sm transition-transform ${enabled ? 'translate-x-6' : 'translate-x-1'}`} />
      </button>
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
        <div className="text-sm font-semibold text-slate-900">token 自动续期</div>
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

function buildConfigPathItems(localConfigPaths: LocalConfigPaths) {
  return [
    { label: 'Codex config.toml', value: localConfigPaths.codex_config_path },
    { label: 'Catalog JSON', value: localConfigPaths.catalog_path },
    { label: 'Router provider config', value: localConfigPaths.provider_config_path },
    { label: 'App settings JSON', value: localConfigPaths.app_settings_path },
  ];
}

function normalizeDisplayVersion(version: string) {
  const normalized = version.trim().replace(/^v/i, '');
  return /^\d+(?:\.\d+){0,3}(?:[-+][0-9A-Za-z.-]+)?$/.test(normalized) ? normalized : APP_VERSION;
}

function formatUnknownError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
