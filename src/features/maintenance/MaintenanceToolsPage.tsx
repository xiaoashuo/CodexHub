import { useEffect, useRef, useState, type ReactNode } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { Button } from '../../components/ui/Button';
import { invokeAccountProxyRequestLogs, invokeCleanMaintenanceData, invokeClearAccountProxyRequestLogs, invokeCreateMigrationBackup, invokeImportMigrationBackup, invokeInspectMigrationBackup } from '../../lib/tauriBridge';
import type { AccountProxyLogEntry, AppSettings, MigrationBackupInspectionResult, MigrationBackupResult, MigrationRestoreResult } from '../../types';

type CleanState = 'idle' | 'running' | 'done';
type BackupState = 'idle' | 'running' | 'done';
type RestoreState = 'idle' | 'running' | 'done';
type RestartStepStatus = 'pending' | 'running' | 'success' | 'error';

type CleanResultDialogState = {
  title: string;
  message: string;
  success: boolean;
};

type MigrationBackupDialogState = {
  result: MigrationBackupResult;
};

type MigrationRestoreDialogState = {
  result: MigrationRestoreResult;
};

type RestartStep = {
  key: string;
  label: string;
  message: string;
  status: RestartStepStatus;
};

type RestartProgressState = {
  open: boolean;
  running: boolean;
  completed: boolean;
  success: boolean;
  steps: RestartStep[];
  resultMessage: string;
};

const textMap = {
  seconds30: '30 \u79d2',
  minute1: '1 \u5206\u949f',
  minute3: '3 \u5206\u949f',
  minute5: '5 \u5206\u949f',
  prepareRestart: '\u51c6\u5907\u91cd\u542f',
  restartCodex: '\u91cd\u542f Codex',
  finish: '\u5b8c\u6210',
  waitCheck: '\u7b49\u5f85\u68c0\u67e5 Codex \u542f\u52a8\u547d\u4ee4\u3002',
  waitRestart: '\u7b49\u5f85\u5173\u95ed\u65e7\u8fdb\u7a0b\u5e76\u91cd\u65b0\u62c9\u8d77 Codex\u3002',
  waitResult: '\u7b49\u5f85\u91cd\u542f\u7ed3\u679c\u3002',
  checking: '\u6b63\u5728\u68c0\u67e5 Codex \u542f\u52a8\u547d\u4ee4\u3002',
  checked: 'Codex \u542f\u52a8\u547d\u4ee4\u68c0\u67e5\u5b8c\u6210\u3002',
  restarting: '\u6b63\u5728\u5173\u95ed\u65e7\u8fdb\u7a0b\u5e76\u91cd\u65b0\u62c9\u8d77 Codex\u3002',
  restartDone: 'Codex \u91cd\u542f\u5b8c\u6210\u3002',
  restartFailed: 'Codex \u91cd\u542f\u5931\u8d25\u3002',
  maintenance: '\u7ef4\u62a4\u5de5\u5177',
  data: '\u6570\u636e',
  cleanData: '\u6e05\u7406\u6570\u636e',
  cleanDescription: '\u6e05\u7406\u5197\u4f59\u5907\u4efd\u3001\u5931\u6548\u5feb\u7167\u548c\u672c\u5730\u7f13\u5b58\u9879\u3002',
  migrationBackup: '一键备份迁移包',
  migrationBackupDescription: '导出账号、模型、会话、个人技能、技能备份、MCP 与应用设置；插件缓存暂不纳入备份。',
  backingUp: '备份中',
  backup: '备份',
  importing: '导入中',
  import: '导入',
  backupDone: '备份完成',
  backupFailed: '备份失败',
  restoreDone: '恢复完成',
  restoreFailed: '恢复失败',
  restoreBackupPath: '恢复前备份',
  restoredContent: '恢复内容',
  backupPath: '备份路径',
  backupContent: '备份内容',
  skippedItems: '跳过项',
  noSkippedItems: '无跳过项',
  cleaning: '\u6e05\u7406\u4e2d',
  clean: '\u6e05\u7406',
  cleanDone: '\u6e05\u7406\u5b8c\u6210',
  cleanFailed: '\u6e05\u7406\u5931\u8d25',
  usageFrequency: '\u8d26\u53f7\u989d\u5ea6\u5237\u65b0\u9891\u7387',
  usageDescription: '\u63a7\u5236\u989d\u5ea6 API\uff08wham/usage\uff09\u7684\u81ea\u52a8\u5237\u65b0\u9891\u7387\u3002',
  accountProxy: '\u8d26\u53f7\u53cd\u4ee3',
  accountProxyDescription: '\u652f\u6301\u5916\u90e8\u5e94\u7528\u901a\u8fc7\u5f53\u524d\u8d26\u53f7\u8c03\u7528 Codex\uff1b\u4f7f\u7528 127.0.0.1:1455 \u8d26\u53f7\u76d1\u542c\u7aef\u53e3\u3002',
  settings: '\u8bbe\u7f6e',
  restartDescription: '\u91cd\u65b0\u62c9\u8d77 Codex \u5ba2\u6237\u7aef\uff0c\u8ba9\u8d26\u53f7\u6216\u8def\u7531\u914d\u7f6e\u7acb\u5373\u751f\u6548\u3002',
  restartingButton: '\u91cd\u542f\u4e2d',
  restart: '\u91cd\u542f',
  accountProxySettings: '\u8d26\u53f7\u53cd\u4ee3\u8bbe\u7f6e',
  accountProxySettingsDesc: '\u5f00\u542f\u540e\uff0c\u5916\u90e8\u5e94\u7528\u53ef\u901a\u8fc7\u672c\u673a Base URL \u8c03\u7528\u5f53\u524d Codex \u8d26\u53f7\u3002',
  enabled: '\u5df2\u5f00\u542f',
  disabled: '\u672a\u5f00\u542f',
  whetherEnable: '\u662f\u5426\u5f00\u542f',
  localOnly: '\u53ea\u5f00\u653e 127.0.0.1 \u672c\u673a\u8bbf\u95ee\uff0c\u4e0d\u5f71\u54cd OAuth \u767b\u5f55\u56de\u8c03\u3002',
  regenerate: '\u91cd\u65b0\u751f\u6210',
  apiKeyTip: 'API Key \u4f1a\u4ee5 sk_ \u5f00\u5934\u751f\u6210\uff0c\u7528\u4e8e\u5916\u90e8\u5e94\u7528\u586b\u5199 Authorization\uff1b\u7b2c\u4e00\u7248\u5efa\u8bae\u4fdd\u6301\u672c\u673a\u4f7f\u7528\uff0c\u4e0d\u5f00\u653e\u5c40\u57df\u7f51\u3002',
  cancel: '\u53d6\u6d88',
  saving: '\u4fdd\u5b58\u4e2d',
  save: '\u4fdd\u5b58',
  copied: '\u5df2\u590d\u5236',
  copy: '\u590d\u5236',
  runningRestart: '\u6b63\u5728\u91cd\u542f\u4e2d...',
  completed: '\u5df2\u5b8c\u6210',
  close: '\u5173\u95ed',
  restartDialogDesc: '\u6b63\u5728\u91cd\u65b0\u62c9\u8d77 Codex \u5ba2\u6237\u7aef\uff0c\u8bf7\u7b49\u5f85\u5f53\u524d\u64cd\u4f5c\u5b8c\u6210\u3002',
  logs: '\u65e5\u5fd7',
  accountProxyLogs: '\u8d26\u53f7\u53cd\u4ee3\u65e5\u5fd7',
  refresh: '\u5237\u65b0',
  refreshing: '\u5237\u65b0\u4e2d',
  clearLogs: '\u6e05\u7a7a',
  emptyLogs: '\u6682\u65e0\u8bf7\u6c42\u65e5\u5fd7',
};

const refreshFrequencyOptions = [
  { label: textMap.seconds30, value: 30 },
  { label: textMap.minute1, value: 60 },
  { label: textMap.minute3, value: 180 },
  { label: textMap.minute5, value: 300 },
];

const initialRestartSteps: RestartStep[] = [
  { key: 'prepare', label: textMap.prepareRestart, status: 'pending', message: textMap.waitCheck },
  { key: 'restart', label: textMap.restartCodex, status: 'pending', message: textMap.waitRestart },
  { key: 'finish', label: textMap.finish, status: 'pending', message: textMap.waitResult },
];

const initialRestartProgress: RestartProgressState = {
  open: false,
  running: false,
  completed: false,
  success: false,
  steps: initialRestartSteps,
  resultMessage: '',
};

function createAccountProxyApiKey() {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  const token = btoa(String.fromCharCode(...bytes)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
  return `sk_${token}`;
}

function defaultAccountProxyUrl(appSettings: AppSettings) {
  return `http://127.0.0.1:${appSettings.oauth_callback_port || 1455}/v1`;
}

export function MaintenanceToolsPage({ appSettings, handleAppSettingsSave, handleCodexRestart }: { appSettings: AppSettings; handleAppSettingsSave: (settings: AppSettings) => Promise<void>; handleCodexRestart: () => Promise<{ success: boolean; message: string }>; }) {
  const [cleanState, setCleanState] = useState<CleanState>('idle');
  const [backupState, setBackupState] = useState<BackupState>('idle');
  const [restoreState, setRestoreState] = useState<RestoreState>('idle');
  const [cleanResultDialog, setCleanResultDialog] = useState<CleanResultDialogState | null>(null);
  const [migrationBackupDialog, setMigrationBackupDialog] = useState<MigrationBackupDialogState | null>(null);
  const [migrationRestoreDialog, setMigrationRestoreDialog] = useState<MigrationRestoreDialogState | null>(null);
  const [savingFrequency, setSavingFrequency] = useState(false);
  const [restartProgress, setRestartProgress] = useState<RestartProgressState>(initialRestartProgress);
  const [accountProxyDialogOpen, setAccountProxyDialogOpen] = useState(false);
  const [accountProxyLogDialogOpen, setAccountProxyLogDialogOpen] = useState(false);

  const updateRestartStep = (key: string, status: RestartStepStatus, message: string) => {
    setRestartProgress((current) => ({ ...current, steps: current.steps.map((step) => (step.key === key ? { ...step, status, message } : step)) }));
  };

  const handleCleanData = async () => {
    setCleanState('running');
    try {
      const result = await invokeCleanMaintenanceData();
      setCleanResultDialog({
        title: textMap.cleanDone,
        message: `${result.message} \u5907\u4efd ${formatBytes(result.backupDeletedBytes)} / \u7f13\u5b58 ${formatBytes(result.cacheDeletedBytes)} / \u5931\u6548\u5feb\u7167 ${formatBytes(result.invalidSnapshotDeletedBytes)}`,
        success: true,
      });
      setCleanState('done');
    } catch (error) {
      setCleanResultDialog({
        title: textMap.cleanFailed,
        message: error instanceof Error ? error.message : String(error),
        success: false,
      });
      setCleanState('idle');
    }
  };

  const handleMigrationBackup = async () => {
    setBackupState('running');
    try {
      const result = await invokeCreateMigrationBackup();
      setMigrationBackupDialog({ result });
      setBackupState('done');
    } catch (error) {
      setCleanResultDialog({
        title: textMap.backupFailed,
        message: error instanceof Error ? error.message : String(error),
        success: false,
      });
      setBackupState('idle');
    }
  };

  const handleMigrationImport = async () => {
    if (restoreState === 'running') return;
    const selected = await open({
      multiple: false,
      directory: false,
      title: '选择迁移备份 ZIP',
      filters: [{ name: 'Migration Backup', extensions: ['zip'] }],
    });
    if (!selected || Array.isArray(selected)) return;

    setRestoreState('running');
    try {
      const inspection = await invokeInspectMigrationBackup(selected);
      if (!confirmMigrationRestoreInspection(inspection)) {
        setRestoreState('idle');
        return;
      }
      const result = await invokeImportMigrationBackup(selected);
      setMigrationRestoreDialog({ result });
      setRestoreState('done');
    } catch (error) {
      setCleanResultDialog({
        title: textMap.restoreFailed,
        message: error instanceof Error ? error.message : String(error),
        success: false,
      });
      setRestoreState('idle');
    }
  };

  const handleFrequencyChange = async (seconds: number) => {
    if (seconds === appSettings.account_usage_refresh_seconds || savingFrequency) return;
    setSavingFrequency(true);
    try {
      await handleAppSettingsSave({ ...appSettings, account_usage_refresh_seconds: seconds });
    } finally {
      setSavingFrequency(false);
    }
  };

  const handleRestartCodex = async () => {
    if (restartProgress.running) return;

    setRestartProgress({ open: true, running: true, completed: false, success: false, resultMessage: '', steps: initialRestartSteps.map((step) => (step.key === 'prepare' ? { ...step, status: 'running', message: textMap.checking } : step)) });

    window.setTimeout(() => {
      updateRestartStep('prepare', 'success', textMap.checked);
      updateRestartStep('restart', 'running', textMap.restarting);
    }, 180);

    try {
      const result = await handleCodexRestart();
      setRestartProgress((current) => ({
        ...current,
        running: false,
        completed: true,
        success: result.success,
        resultMessage: result.message,
        steps: current.steps.map((step) => {
          if (step.key === 'restart') return { ...step, status: result.success ? 'success' : 'error', message: result.message };
          if (step.key === 'finish') return { ...step, status: result.success ? 'success' : 'error', message: result.success ? textMap.restartDone : textMap.restartFailed };
          return step.status === 'running' ? { ...step, status: 'success', message: textMap.checked } : step;
        }),
      }));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setRestartProgress((current) => ({ ...current, running: false, completed: true, success: false, resultMessage: message, steps: current.steps.map((step) => (step.status === 'running' || step.key === 'finish' ? { ...step, status: 'error', message } : step)) }));
    }
  };

  const handleRestartDialogClose = () => {
    if (restartProgress.running) return;
    setRestartProgress(initialRestartProgress);
  };

  return (
    <div className="h-full min-h-0 overflow-y-auto">
      <div className="border-b border-slate-200 pb-4"><h2 className="text-2xl font-bold text-slate-950">{textMap.maintenance}</h2></div>
      <section className="mt-6">
        <div className="mb-3 px-1 text-sm font-semibold text-slate-700">{textMap.data}</div>
        <div className="overflow-hidden rounded-2xl border border-slate-200 bg-white">
          <SettingsLikeRow title={textMap.migrationBackup} description={textMap.migrationBackupDescription} action={<div className="flex items-center gap-2"><Button className="min-w-24" variant="secondary" disabled={backupState === 'running' || restoreState === 'running'} onClick={handleMigrationBackup}>{backupState === 'running' ? textMap.backingUp : textMap.backup}</Button><Button className="min-w-24" variant="secondary" disabled={backupState === 'running' || restoreState === 'running'} onClick={handleMigrationImport}>{restoreState === 'running' ? textMap.importing : textMap.import}</Button></div>} />
          <SettingsLikeRow title={textMap.cleanData} description={textMap.cleanDescription} action={<Button className="min-w-24" variant="secondary" disabled={cleanState === 'running'} onClick={handleCleanData}>{cleanState === 'running' ? textMap.cleaning : textMap.clean}</Button>} />
          <SettingsLikeRow title={textMap.usageFrequency} description={textMap.usageDescription} action={<div className="flex rounded-full bg-slate-100 p-1">{refreshFrequencyOptions.map((option) => (<button key={option.value} type="button" disabled={savingFrequency} onClick={() => handleFrequencyChange(option.value)} className={`h-8 min-w-16 rounded-full px-3 text-sm transition ${appSettings.account_usage_refresh_seconds === option.value ? 'bg-white font-semibold text-slate-950 shadow-sm' : 'text-slate-500 hover:text-slate-800'} disabled:cursor-not-allowed disabled:opacity-60`}>{option.label}</button>))}</div>} />
          <SettingsLikeRow title={textMap.accountProxy} description={textMap.accountProxyDescription} action={<div className="flex items-center gap-2"><Button className="min-w-24" variant="secondary" onClick={() => setAccountProxyDialogOpen(true)}>{textMap.settings}</Button><Button className="min-w-24" variant="secondary" onClick={() => setAccountProxyLogDialogOpen(true)}>{textMap.logs}</Button></div>} />
          <SettingsLikeRow title={textMap.restartCodex} description={textMap.restartDescription} action={<Button className="min-w-24" variant="secondary" disabled={restartProgress.running} onClick={handleRestartCodex}>{restartProgress.running ? textMap.restartingButton : textMap.restart}</Button>} />
        </div>
      </section>
      {accountProxyDialogOpen && <AccountProxySettingsDialog appSettings={appSettings} handleClose={() => setAccountProxyDialogOpen(false)} handleAppSettingsSave={handleAppSettingsSave} />}
      {accountProxyLogDialogOpen && <AccountProxyLogDialog handleClose={() => setAccountProxyLogDialogOpen(false)} />}
      {restartProgress.open && <RestartCodexProgressDialog state={restartProgress} handleClose={handleRestartDialogClose} />}
      {cleanResultDialog && <CleanResultDialog state={cleanResultDialog} handleClose={() => setCleanResultDialog(null)} />}
      {migrationBackupDialog && <MigrationBackupResultDialog state={migrationBackupDialog} handleClose={() => setMigrationBackupDialog(null)} />}
      {migrationRestoreDialog && <MigrationRestoreResultDialog state={migrationRestoreDialog} handleClose={() => setMigrationRestoreDialog(null)} />}
    </div>
  );
}

function SettingsLikeRow({ title, description, action }: { title: string; description: string; action: ReactNode }) {
  return <div className="flex min-h-[74px] items-center justify-between gap-4 border-b border-slate-200 px-5 py-4 last:border-b-0"><div className="min-w-0 flex-1"><div className="text-sm font-semibold text-slate-950">{title}</div><div className="mt-1 text-sm leading-5 text-slate-500">{description}</div></div><div className="shrink-0 whitespace-nowrap">{action}</div></div>;
}

function CleanResultDialog({ state, handleClose }: { state: CleanResultDialogState; handleClose: () => void }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="box-border w-full max-w-lg rounded-2xl bg-white px-5 py-5 shadow-2xl">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-xl font-bold text-slate-950">{state.title}</h3>
            <p className="mt-2 text-sm leading-6 text-slate-500">{state.message}</p>
          </div>
          <span className={`shrink-0 whitespace-nowrap rounded-full px-3 py-1 text-xs font-semibold ${state.success ? 'bg-emerald-50 text-emerald-700' : 'bg-rose-50 text-rose-700'}`}>
            {state.success ? textMap.completed : textMap.cleanFailed}
          </span>
        </div>
        <div className="mt-7 flex items-center justify-end gap-3">
          <Button variant="secondary" onClick={handleClose}>{textMap.close}</Button>
        </div>
      </div>
    </div>
  );
}

function MigrationBackupResultDialog({ state, handleClose }: { state: MigrationBackupDialogState; handleClose: () => void }) {
  const { result } = state;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="box-border w-full max-w-2xl rounded-2xl bg-white px-5 py-5 shadow-2xl">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-xl font-bold text-slate-950">{textMap.backupDone}</h3>
            <p className="mt-2 text-sm leading-6 text-slate-500">{result.message}</p>
          </div>
          <span className="shrink-0 whitespace-nowrap rounded-full bg-emerald-50 px-3 py-1 text-xs font-semibold text-emerald-700">{textMap.completed}</span>
        </div>
        <div className="mt-5 space-y-3 text-sm">
          <div className="rounded-xl bg-slate-50 px-4 py-3">
            <div className="font-semibold text-slate-700">{textMap.backupPath}</div>
            <div className="mt-1 break-all font-mono text-xs text-slate-500">{result.backupPath}</div>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <div className="rounded-xl bg-slate-50 px-4 py-3">
              <div className="font-semibold text-slate-700">{textMap.backupContent}</div>
              <div className="mt-1 text-slate-500">{result.includedSections.join('、')} · {result.fileCount} 个文件 · {formatBytes(result.totalBytes)}</div>
            </div>
            <div className="rounded-xl bg-slate-50 px-4 py-3">
              <div className="font-semibold text-slate-700">{textMap.skippedItems}</div>
              <div className="mt-1 max-h-24 overflow-auto text-slate-500">{result.skippedItems.length === 0 ? textMap.noSkippedItems : result.skippedItems.join('；')}</div>
            </div>
          </div>
        </div>
        <div className="mt-7 flex items-center justify-end gap-3">
          <Button variant="secondary" onClick={handleClose}>{textMap.close}</Button>
        </div>
      </div>
    </div>
  );
}

function MigrationRestoreResultDialog({ state, handleClose }: { state: MigrationRestoreDialogState; handleClose: () => void }) {
  const { result } = state;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="box-border w-full max-w-2xl rounded-2xl bg-white px-5 py-5 shadow-2xl">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-xl font-bold text-slate-950">{textMap.restoreDone}</h3>
            <p className="mt-2 text-sm leading-6 text-slate-500">{result.message}</p>
          </div>
          <span className="shrink-0 whitespace-nowrap rounded-full bg-emerald-50 px-3 py-1 text-xs font-semibold text-emerald-700">{textMap.completed}</span>
        </div>
        <div className="mt-5 space-y-3 text-sm">
          <div className="rounded-xl bg-slate-50 px-4 py-3">
            <div className="font-semibold text-slate-700">{textMap.restoreBackupPath}</div>
            <div className="mt-1 break-all font-mono text-xs text-slate-500">{result.backupPath}</div>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <div className="rounded-xl bg-slate-50 px-4 py-3">
              <div className="font-semibold text-slate-700">{textMap.restoredContent}</div>
              <div className="mt-1 text-slate-500">{result.restoredSections.join('、') || '-'} · {result.restoredCount} 个文件 · {formatBytes(result.restoredBytes)}</div>
            </div>
            <div className="rounded-xl bg-slate-50 px-4 py-3">
              <div className="font-semibold text-slate-700">{textMap.skippedItems}</div>
              <div className="mt-1 max-h-24 overflow-auto text-slate-500">{result.skippedItems.length === 0 ? textMap.noSkippedItems : result.skippedItems.join('；')}</div>
            </div>
          </div>
        </div>
        <div className="mt-7 flex items-center justify-end gap-3">
          <Button variant="secondary" onClick={handleClose}>{textMap.close}</Button>
        </div>
      </div>
    </div>
  );
}


function AccountProxyLogDialog({ handleClose }: { handleClose: () => void }) {
  const [logs, setLogs] = useState<AccountProxyLogEntry[]>([]);
  const [logAction, setLogAction] = useState<'refresh' | 'clear' | null>(null);
  const logActionRef = useRef<'refresh' | 'clear' | null>(null);
  const loading = logAction !== null;

  const handleRefresh = async () => {
    if (logActionRef.current) return;
    logActionRef.current = 'refresh';
    const startedAt = performance.now();
    setLogAction('refresh');
    try {
      setLogs(await invokeAccountProxyRequestLogs());
    } finally {
      const elapsed = performance.now() - startedAt;
      if (elapsed < 350) {
        await new Promise((resolve) => window.setTimeout(resolve, 350 - elapsed));
      }
      logActionRef.current = null;
      setLogAction(null);
    }
  };

  const handleClear = async () => {
    if (logActionRef.current) return;
    logActionRef.current = 'clear';
    setLogAction('clear');
    try {
      setLogs(await invokeClearAccountProxyRequestLogs());
    } finally {
      logActionRef.current = null;
      setLogAction(null);
    }
  };

  useEffect(() => {
    void handleRefresh();
  }, []);

  return <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm"><div className="box-border flex max-h-[80vh] w-full max-w-5xl flex-col rounded-2xl bg-white px-5 py-5 shadow-2xl"><div className="flex items-center justify-between gap-4"><h3 className="text-xl font-bold text-slate-950">{textMap.accountProxyLogs}</h3><div className="flex items-center gap-2"><Button className={`min-w-24 ${logAction === 'refresh' ? 'cursor-wait' : ''}`} variant="secondary" onClick={handleRefresh} aria-busy={logAction === 'refresh'}>{logAction === 'refresh' && <span className="mr-2 h-3.5 w-3.5 animate-spin rounded-full border-2 border-slate-400/50 border-t-slate-700" />}{logAction === 'refresh' ? textMap.refreshing : textMap.refresh}</Button><Button variant="danger" onClick={handleClear} disabled={loading}>{textMap.clearLogs}</Button><Button variant="secondary" onClick={handleClose}>{textMap.close}</Button></div></div><div className="mt-5 min-h-0 flex-1 overflow-auto rounded-2xl border border-slate-200"><div className="grid min-w-[1120px] grid-cols-[150px_90px_150px_120px_160px_80px_150px_80px_90px_1fr] gap-0 border-b border-slate-200 bg-slate-50 px-3 py-2 text-xs font-semibold text-slate-500"><div>Time</div><div>Status</div><div>Path</div><div>Protocol</div><div>Model</div><div>Stream</div><div>Token</div><div>Cost</div><div>Account</div><div>Error</div></div>{logs.length === 0 ? <div className="px-4 py-8 text-center text-sm text-slate-500">{textMap.emptyLogs}</div> : logs.map((log, index) => <div key={`${log.time}-${index}`} className="grid min-w-[1120px] grid-cols-[150px_90px_150px_120px_160px_80px_150px_80px_90px_1fr] gap-0 border-b border-slate-100 px-3 py-2 text-xs text-slate-700 last:border-b-0"><div>{log.time}</div><div>{log.status}</div><div>{log.path}</div><div>{log.protocol}</div><div className="truncate" title={log.model}>{log.model}</div><div>{log.stream ? 'true' : 'false'}</div><div>{formatAccountProxyTokenUsage(log)}</div><div>{log.cost}</div><div className="truncate" title={log.account}>{log.account}</div><div className="truncate" title={log.error_detail}>{log.error_detail}</div></div>)}</div></div></div>;
}

function formatAccountProxyTokenUsage(log: AccountProxyLogEntry) {
  const input = log.input_tokens ?? 0;
  const output = log.output_tokens ?? 0;
  const cached = log.cached_input_tokens ?? 0;
  const total = log.total_tokens ?? input + output;
  if (input === 0 && output === 0 && cached === 0 && total === 0) {
    return '-';
  }
  return `in ${formatTokenCount(input)} / cache ${formatTokenCount(cached)} / out ${formatTokenCount(output)} / total ${formatTokenCount(total)}`;
}

function confirmMigrationRestoreInspection(inspection: MigrationBackupInspectionResult) {
  if (inspection.missingProjectCount === 0) {
    return true;
  }

  const preview = inspection.missingProjects
    .slice(0, 8)
    .map((item) => `- ${item.cwd}（${item.sessionCount} 个会话）`)
    .join('\n');
  const remaining = inspection.missingProjects.length > 8
    ? `\n...还有 ${inspection.missingProjects.length - 8} 个目录未显示`
    : '';

  return window.confirm([
    inspection.message,
    '',
    '这些会话的项目目录不存在。恢复后历史会话仍会写回，但 Codex Desktop 可能无法按原项目打开或分组。',
    '',
    preview,
    remaining,
    '',
    '是否继续恢复？',
  ].filter(Boolean).join('\n'));
}

function formatTokenCount(value: number) {
  return new Intl.NumberFormat('zh-CN').format(value || 0);
}

function formatBytes(value: number) {
  if (value >= 1024 * 1024 * 1024) return `${(value / 1024 / 1024 / 1024).toFixed(2)} GB`;
  if (value >= 1024 * 1024) return `${(value / 1024 / 1024).toFixed(2)} MB`;
  if (value >= 1024) return `${(value / 1024).toFixed(2)} KB`;
  return `${value} B`;
}

function AccountProxySettingsDialog({ appSettings, handleClose, handleAppSettingsSave }: { appSettings: AppSettings; handleClose: () => void; handleAppSettingsSave: (settings: AppSettings) => Promise<void>; }) {
  const accountProxy = appSettings.account_proxy ?? { account_proxy_enabled: false, account_proxy_url: defaultAccountProxyUrl(appSettings), api_key: createAccountProxyApiKey() };
  const initialApiKey = accountProxy.api_key?.startsWith('sk_') ? accountProxy.api_key : createAccountProxyApiKey();
  const [enabled, setEnabled] = useState(accountProxy.account_proxy_enabled);
  const [accountProxyUrl, setAccountProxyUrl] = useState(accountProxy.account_proxy_url || defaultAccountProxyUrl(appSettings));
  const [apiKey, setApiKey] = useState(initialApiKey);
  const [saving, setSaving] = useState(false);
  const [copied, setCopied] = useState<string | null>(null);

  const handleCopy = async (label: string, value: string) => { await navigator.clipboard.writeText(value); setCopied(label); window.setTimeout(() => setCopied(null), 1200); };
  const handleGenerateApiKey = () => setApiKey(createAccountProxyApiKey());
  const handleSave = async () => {
    if (saving) return;
    setSaving(true);
    try {
      await handleAppSettingsSave({ ...appSettings, account_proxy: { account_proxy_enabled: enabled, account_proxy_url: accountProxyUrl.trim(), api_key: apiKey.trim().startsWith('sk_') ? apiKey.trim() : createAccountProxyApiKey() } });
      handleClose();
    } finally {
      setSaving(false);
    }
  };

  return <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm"><div className="box-border w-full max-w-2xl rounded-2xl bg-white px-5 py-5 shadow-2xl"><div className="flex items-start justify-between gap-4"><div><h3 className="text-xl font-bold text-slate-950">{textMap.accountProxySettings}</h3><p className="mt-2 text-sm leading-6 text-slate-500">{textMap.accountProxySettingsDesc}</p></div><span className={`rounded-full px-3 py-1 text-xs font-semibold ${enabled ? 'bg-emerald-50 text-emerald-700' : 'bg-slate-100 text-slate-500'}`}>{enabled ? textMap.enabled : textMap.disabled}</span></div><label className="mt-6 flex cursor-pointer items-center justify-between rounded-2xl border border-slate-200 px-4 py-3"><div><div className="text-sm font-semibold text-slate-950">{textMap.whetherEnable}</div><div className="mt-1 text-sm text-slate-500">{textMap.localOnly}</div></div><input className="h-5 w-5 accent-indigo-600" type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} /></label><div className="mt-5 space-y-4"><CopyableField label="Base URL" value={accountProxyUrl} copied={copied === 'base_url'} onValueChange={setAccountProxyUrl} onCopy={() => handleCopy('base_url', accountProxyUrl)} /><CopyableField label="API Key" value={apiKey} copied={copied === 'api_key'} onValueChange={setApiKey} onCopy={() => handleCopy('api_key', apiKey)} extraAction={<Button variant="secondary" onClick={handleGenerateApiKey}>{textMap.regenerate}</Button>} /></div><div className="mt-4 rounded-xl bg-amber-50 px-4 py-3 text-sm leading-6 text-amber-700">{textMap.apiKeyTip}</div><div className="mt-7 flex items-center justify-end gap-3"><Button variant="secondary" onClick={handleClose} disabled={saving}>{textMap.cancel}</Button><Button className="min-w-24" onClick={handleSave} disabled={saving}>{saving ? textMap.saving : textMap.save}</Button></div></div></div>;
}

function CopyableField({ label, value, copied, onValueChange, onCopy, extraAction }: { label: string; value: string; copied: boolean; onValueChange: (value: string) => void; onCopy: () => void; extraAction?: ReactNode; }) {
  return <label className="block"><div className="mb-2 text-sm font-semibold text-slate-700">{label}</div><div className="flex gap-2"><input className="min-w-0 flex-1 rounded-xl border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-800 outline-none focus:border-indigo-300 focus:bg-white" value={value} onChange={(event) => onValueChange(event.target.value)} /><Button className="min-w-20" variant="secondary" onClick={onCopy}>{copied ? textMap.copied : textMap.copy}</Button>{extraAction}</div></label>;
}

function RestartCodexProgressDialog({ state, handleClose }: { state: RestartProgressState; handleClose: () => void }) {
  const progressPercent = getRestartProgressPercent(state.steps);
  const currentStep = getCurrentRestartStep(state.steps);
  const buttonText = state.running ? textMap.runningRestart : state.success ? textMap.completed : textMap.close;
  return <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm"><div className="box-border rounded-2xl bg-white px-5 py-5 shadow-2xl" style={{ width: '50vw' }}><h3 className="text-xl font-bold text-slate-950">{textMap.restartCodex}</h3><p className="mt-2 text-sm leading-6 text-slate-500">{textMap.restartDialogDesc}</p><div className="mt-7"><div className="h-1.5 overflow-hidden rounded-full bg-slate-100"><div className={`h-full rounded-full transition-all duration-500 ease-out ${state.completed && !state.success ? 'bg-rose-500' : 'bg-violet-600'}`} style={{ width: `${progressPercent}%` }} /></div><div className="mt-3 min-h-6 text-center text-sm text-slate-500">{currentStep.message}</div></div>{state.resultMessage && !state.running && <div className={`mt-4 min-w-0 break-all rounded-xl px-4 py-3 text-sm leading-6 ${state.success ? 'bg-emerald-50 text-emerald-700' : 'bg-rose-50 text-rose-700'}`}>{state.resultMessage}</div>}<div className="mt-7 flex items-center justify-end gap-3"><Button variant="secondary" onClick={handleClose} disabled={state.running}>{textMap.close}</Button><Button className="min-w-[132px] bg-violet-500 hover:bg-violet-600" onClick={handleClose} disabled={state.running}>{state.running && <span className="mr-2 h-3.5 w-3.5 animate-spin rounded-full border-2 border-white/50 border-t-white" />}{buttonText}</Button></div></div></div>;
}

function getCurrentRestartStep(steps: RestartStep[]) {
  return steps.find((step) => step.status === 'running') ?? steps.find((step) => step.status === 'error') ?? steps.find((step) => step.status === 'pending') ?? steps[steps.length - 1];
}

function getRestartProgressPercent(steps: RestartStep[]) {
  const completedCount = steps.filter((step) => step.status === 'success').length;
  const hasError = steps.some((step) => step.status === 'error');
  const runningIndex = steps.findIndex((step) => step.status === 'running');
  if (completedCount === steps.length) return 100;
  if (hasError) return Math.max(10, Math.round((completedCount / steps.length) * 100));
  if (runningIndex >= 0) return Math.max(10, Math.round(((runningIndex + 0.35) / steps.length) * 100));
  return Math.round((completedCount / steps.length) * 100);
}
