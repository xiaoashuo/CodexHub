import { useState } from 'react';
import { Button } from '../../components/ui/Button';
import { Card, CardContent, CardHeader } from '../../components/ui/Card';
import { RouterLogs, RouterRuntimePanel } from '../../components/business/RouterParts';
import { ROUTER_BASE_PATH, ROUTER_HEALTH_PATH, ROUTER_HOST, ROUTER_PORT } from '../../lib/constants';
import { DEFAULT_ROUTER_START_CODEX_RESTART_MODE } from '../../lib/routerDefaults';
import type { AppSettings, CodexRestartMode, RouterCommandProgressState, RouterLogEntry, RouterRuntimeInfo, RouterStartupChecklistState, RouterStartupChecklistStep, RouterStatus } from '../../types';

type RouterTab = 'runtime' | 'logs';

const text = {
  runtime: '运行信息',
  logs: '请求日志',
  title: '路由管理',
  currentStatus: '当前状态',
  running: '运行中',
  stopped: '已停止',
  restartCodexAfterStart: '启动后重启 Codex',
  runtimeSettings: '运行参数',
  concurrencyLimit: '最大并发请求数',
  concurrencyHint: '控制 Router 同时转发的请求数量，保存后下次启动或重启 Router 生效。',
  concurrencySaved: '并发设置已保存，重启 Router 后生效。',
  save: '保存',
  saving: '保存中...',
  yes: '是',
  no: '否',
  restartHint: '默认关闭：只启动本地 Router 并写入 Codex 配置，避免中断当前 Codex 任务。需要立即刷新模型菜单时可手动开启；如果 Codex 启动时插件远端缓存刷新失败，Router 仍会保持运行。',
  startRouter: '启动 Router',
  stopRouter: '停止 Router',
  restartRouter: '重启 Router',
  check: '检查',
  startDialogTitle: '启用 Codex 伙伴智能路由',
  startIntro: '启用前请了解：',
  stopIntro: '停止前请了解：',
  starting: '正在启用中...',
  started: '已启用',
  stopping: '正在停止中...',
  stoppedDone: '已停止',
  close: '关闭',
  cancel: '取消',
};

const routerTabs: { key: RouterTab; label: string }[] = [
  { key: 'runtime', label: text.runtime },
  { key: 'logs', label: text.logs },
];

const startRouterNotes = [
  'Codex 伙伴需要保持运行，关闭后 Codex 将暂时无法通过本地路由对话。',
  '启用后 Codex 模型菜单会同时显示官方模型和中转模型，选择哪个就使用哪个。',
  '默认不会自动重启 Codex，以免中断当前任务；需要立即加载新配置时，可以打开“启动后重启 Codex”。',
  '如果重启 Codex 时出现插件远端缓存刷新失败，这通常不是 Router 启动失败；可先保持 Router 运行，稍后手动重启 Codex。',
  '随时可以关闭此开关，Codex 会恢复为仅使用官方模型。',
];

const stopRouterNotes = [
  '停止后 Codex 会恢复为官方模型列表，不再经过本地智能路由。',
  '应用会还原 Codex 配置，并尝试重启客户端以尽快刷新模型菜单。',
  '历史对话记录不会删除，恢复后的会话仍会保留。',
  '如果 IDE 插件仍显示旧模型，请手动重启 IDE 内的 Codex 插件。',
];

export function RouterPage({
  routerStatus,
  routerRuntimeInfo,
  appSettings,
  routerLogs,
  handleRouterToggle,
  handleRouterRestart,
  handleRouterHealthCheck,
  handleRouterLogsRefresh,
  handleRouterLogsClear,
  handleAppSettingsSave,
}: {
  routerStatus: RouterStatus;
  routerRuntimeInfo: RouterRuntimeInfo;
  appSettings: AppSettings;
  routerLogs: RouterLogEntry[];
  handleRouterToggle: (codexRestartMode?: CodexRestartMode) => Promise<void>;
  handleRouterRestart: (codexRestartMode?: CodexRestartMode) => Promise<void>;
  handleRouterHealthCheck: () => Promise<void>;
  handleRouterLogsRefresh: () => Promise<void>;
  handleRouterLogsClear: () => Promise<void>;
  handleAppSettingsSave: (settings: AppSettings) => Promise<void>;
}) {
  const [activeTab, setActiveTab] = useState<RouterTab>('runtime');
  const [codexRestartMode, setCodexRestartMode] = useState<CodexRestartMode>(DEFAULT_ROUTER_START_CODEX_RESTART_MODE);
  const [concurrencyLimit, setConcurrencyLimit] = useState(String(appSettings.router_concurrency_limit || 8));
  const [concurrencySaving, setConcurrencySaving] = useState(false);
  const [concurrencyMessage, setConcurrencyMessage] = useState('');
  const displayPort = routerStatus === 'running' ? routerRuntimeInfo.port : appSettings.router_port || ROUTER_PORT;
  const displayHealthUrl = `http://${ROUTER_HOST}:${displayPort}${ROUTER_HEALTH_PATH}`;
  const displayRuntimeInfo = routerStatus === 'running'
    ? routerRuntimeInfo
    : {
        ...routerRuntimeInfo,
        host: ROUTER_HOST,
        port: displayPort,
        healthUrl: displayHealthUrl,
        concurrencyLimit: appSettings.router_concurrency_limit || 8,
      };

  const handleConcurrencySave = async () => {
    if (concurrencySaving) return;
    const parsed = Number.parseInt(concurrencyLimit, 10);
    const nextLimit = Number.isFinite(parsed) ? Math.min(Math.max(parsed, 1), 64) : 8;
    setConcurrencySaving(true);
    setConcurrencyMessage('');
    try {
      await handleAppSettingsSave({ ...appSettings, router_concurrency_limit: nextLimit });
      setConcurrencyLimit(String(nextLimit));
      setConcurrencyMessage(text.concurrencySaved);
    } finally {
      setConcurrencySaving(false);
    }
  };

  return (
    <div className="grid grid-cols-12 gap-6">
      <section className="col-span-4 space-y-6">
        <Card>
          <CardHeader>
            <h3 className="text-lg font-bold text-slate-950">{text.title}</h3>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="rounded-3xl bg-slate-950 p-6 text-white">
              <div className="text-sm text-slate-300">{text.currentStatus}</div>
              <div className="mt-3 text-3xl font-bold">{routerStatus === 'running' ? text.running : text.stopped}</div>
              <div className="mt-3 break-all font-mono text-sm leading-6 text-indigo-200">{ROUTER_HOST}:{displayPort}{ROUTER_BASE_PATH}</div>
            </div>
            <Button className="w-full" onClick={() => handleRouterToggle(codexRestartMode)}>{routerStatus === 'running' ? text.stopRouter : text.startRouter}</Button>
            <Button className="w-full" variant="secondary" disabled={routerStatus !== 'running'} onClick={() => handleRouterRestart(codexRestartMode)}>{text.restartRouter}</Button>
            <Button className="w-full" variant="secondary" onClick={handleRouterHealthCheck}>{text.check} {ROUTER_HEALTH_PATH}</Button>
          </CardContent>
        </Card>
      </section>
      <section className="col-span-8 min-h-0 space-y-6">
        <Card className="flex max-h-[calc(100vh-170px)] min-h-0 flex-col">
          <CardHeader>
            <div className="flex gap-2 rounded-2xl bg-slate-100 p-1">
              {routerTabs.map((tab) => (
                <button
                  key={tab.key}
                  className={`flex-1 rounded-xl px-4 py-2 text-sm font-semibold transition ${activeTab === tab.key ? 'bg-white text-indigo-700 shadow-sm' : 'text-slate-500 hover:text-slate-900'}`}
                  onClick={() => setActiveTab(tab.key)}
                >
                  {tab.label}
                </button>
              ))}
            </div>
          </CardHeader>
          <CardContent className="min-h-0 overflow-hidden">
            {activeTab === 'runtime' && (
              <RouterRuntimePanel runtimeInfo={displayRuntimeInfo}>
                <RouterRuntimeSettings
                  codexRestartMode={codexRestartMode}
                  concurrencyLimit={concurrencyLimit}
                  concurrencyMessage={concurrencyMessage}
                  concurrencySaving={concurrencySaving}
                  handleConcurrencySave={handleConcurrencySave}
                  routerStatus={routerStatus}
                  setCodexRestartMode={setCodexRestartMode}
                  setConcurrencyLimit={setConcurrencyLimit}
                />
              </RouterRuntimePanel>
            )}
            {activeTab === 'logs' && <RouterLogs logs={routerLogs} handleRouterLogsRefresh={handleRouterLogsRefresh} handleRouterLogsClear={handleRouterLogsClear} />}
          </CardContent>
        </Card>
      </section>
    </div>
  );
}

function RouterRuntimeSettings({
  codexRestartMode,
  concurrencyLimit,
  concurrencyMessage,
  concurrencySaving,
  handleConcurrencySave,
  routerStatus,
  setCodexRestartMode,
  setConcurrencyLimit,
}: {
  codexRestartMode: CodexRestartMode;
  concurrencyLimit: string;
  concurrencyMessage: string;
  concurrencySaving: boolean;
  handleConcurrencySave: () => Promise<void>;
  routerStatus: RouterStatus;
  setCodexRestartMode: (value: CodexRestartMode) => void;
  setConcurrencyLimit: (value: string) => void;
}) {
  const restartDisabled = routerStatus === 'running';

  return (
    <div>
      <div className="mb-2 text-sm font-bold text-slate-900">{text.runtimeSettings}</div>
      <div className="grid gap-3 lg:grid-cols-2">
        <div className="grid grid-cols-[minmax(0,1fr)_140px] items-center gap-3">
          <LabelWithHelp label={text.restartCodexAfterStart} help={text.restartHint} />
          <button
            className={`relative h-6 w-11 rounded-full transition ${codexRestartMode === 'restart' ? 'bg-indigo-600' : 'bg-slate-300'} ${restartDisabled ? 'cursor-not-allowed opacity-60' : 'cursor-pointer'}`}
            disabled={restartDisabled}
            onClick={() => setCodexRestartMode(codexRestartMode === 'restart' ? 'skip' : 'restart')}
            type="button"
          >
            <span className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition ${codexRestartMode === 'restart' ? 'left-5' : 'left-0.5'}`} />
          </button>
        </div>
        <div className="grid grid-cols-[minmax(0,1fr)_170px] items-center gap-3">
          <LabelWithHelp label={text.concurrencyLimit} help={text.concurrencyHint} />
          <div className="grid grid-cols-[1fr_auto] gap-1">
            <input
              className="min-w-0 rounded-lg border border-slate-200 px-2.5 py-1.5 text-xs font-semibold text-slate-800 outline-none focus:border-indigo-400"
              inputMode="numeric"
              min={1}
              max={64}
              type="number"
              value={concurrencyLimit}
              onChange={(event) => setConcurrencyLimit(event.target.value)}
            />
            <Button variant="secondary" onClick={handleConcurrencySave}>
              {concurrencySaving ? text.saving : text.save}
            </Button>
          </div>
        </div>
      </div>
      {concurrencyMessage && <div className="mt-2 text-xs font-semibold text-emerald-700">{concurrencyMessage}</div>}
    </div>
  );
}

function LabelWithHelp({ label, help }: { label: string; help: string }) {
  return (
    <div className="flex items-center gap-1.5">
      <span className="text-xs font-semibold text-slate-700">{label}</span>
      <span className="group relative inline-flex">
        <span className="inline-flex h-4 w-4 cursor-help items-center justify-center rounded-full border border-slate-300 bg-white text-[10px] font-bold leading-none text-slate-500">?</span>
        <span className="pointer-events-none absolute bottom-6 left-1/2 z-20 hidden w-64 -translate-x-1/2 rounded-lg border border-slate-200 bg-white px-3 py-2 text-xs leading-5 text-slate-600 shadow-xl group-hover:block">
          {help}
        </span>
      </span>
    </div>
  );
}

export function RouterStartupChecklistDialog({ state, handleClose }: { state: RouterStartupChecklistState; handleClose: () => void }) {
  return (
    <RouterOperationDialog
      title={text.startDialogTitle}
      intro={text.startIntro}
      notes={startRouterNotes}
      steps={state.steps}
      running={state.running}
      completed={state.completed}
      runningText={text.starting}
      completedText={text.started}
      handleClose={handleClose}
    />
  );
}

export function RouterCommandProgressDialog({ state, handleClose }: { state: RouterCommandProgressState; handleClose: () => void }) {
  return (
    <RouterOperationDialog
      title={state.title}
      intro={text.stopIntro}
      notes={stopRouterNotes}
      steps={state.steps}
      running={state.running}
      completed={state.completed}
      runningText={text.stopping}
      completedText={text.stoppedDone}
      handleClose={handleClose}
    />
  );
}

function RouterOperationDialog({
  title,
  intro,
  notes,
  steps,
  running,
  completed,
  runningText,
  completedText,
  handleClose,
}: {
  title: string;
  intro: string;
  notes: string[];
  steps: RouterStartupChecklistStep[];
  running: boolean;
  completed: boolean;
  runningText: string;
  completedText: string;
  handleClose: () => void;
}) {
  const progressPercent = getOperationProgressPercent(steps);
  const currentStep = getCurrentProgressStep(steps);
  const primaryText = running ? runningText : completed ? completedText : text.close;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="w-full max-w-[512px] rounded-2xl bg-white px-6 py-5 shadow-2xl">
        <h3 className="text-xl font-bold text-slate-950">{title}</h3>
        <div className="mt-4 text-sm leading-6 text-slate-600">
          <div>{intro}</div>
          <ul className="mt-2 list-disc space-y-1 pl-5">
            {notes.map((note) => (
              <li key={note}>{note}</li>
            ))}
          </ul>
        </div>
        <div className="mt-7">
          <div className="h-1.5 overflow-hidden rounded-full bg-slate-100">
            <div className="h-full rounded-full bg-violet-600 transition-all duration-500 ease-out" style={{ width: `${progressPercent}%` }} />
          </div>
          <div className="mt-3 min-h-6 text-center text-sm text-slate-500">{currentStep.message}</div>
        </div>
        <div className="mt-7 flex items-center justify-end gap-3">
          <Button variant="secondary" onClick={handleClose} disabled={running}>{text.cancel}</Button>
          <Button className="min-w-[132px] bg-violet-500 hover:bg-violet-600" onClick={handleClose} disabled={running}>
            {running && <span className="mr-2 h-3.5 w-3.5 animate-spin rounded-full border-2 border-white/50 border-t-white" />}
            {primaryText}
          </Button>
        </div>
      </div>
    </div>
  );
}

function getCurrentProgressStep(steps: RouterStartupChecklistStep[]) {
  return steps.find((step) => step.status === 'running') ?? steps.find((step) => step.status === 'error') ?? steps.find((step) => step.status === 'pending') ?? steps[steps.length - 1];
}

function getOperationProgressPercent(steps: RouterStartupChecklistStep[]) {
  if (steps.length === 0) {
    return 0;
  }

  const completedStepCount = steps.filter((step) => step.status === 'success' || step.status === 'warning').length;
  const runningStepIndex = steps.findIndex((step) => step.status === 'running');
  const hasError = steps.some((step) => step.status === 'error');

  if (completedStepCount === steps.length) {
    return 100;
  }

  if (hasError) {
    return Math.max(8, Math.round((completedStepCount / steps.length) * 100));
  }

  if (runningStepIndex >= 0) {
    return Math.max(8, Math.round(((runningStepIndex + 0.35) / steps.length) * 100));
  }

  return Math.round((completedStepCount / steps.length) * 100);
}
