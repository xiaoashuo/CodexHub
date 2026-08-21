import { useState, type ReactNode } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { Button } from '../../components/ui/Button';
import { invokeCleanMaintenanceData, invokeCreateMigrationBackup, invokeImportMigrationBackup, invokeInspectMigrationBackup } from '../../lib/tauriBridge';
import type { AppSettings, MigrationBackupInspectionResult, MigrationBackupResult, MigrationRestoreResult } from '../../types';

type CleanState = 'idle' | 'running' | 'done';
type BackupState = 'idle' | 'running' | 'done';
type RestoreState = 'idle' | 'running' | 'done';

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

export function MaintenanceToolsPage({ appSettings, handleAppSettingsSave }: { appSettings: AppSettings; handleAppSettingsSave: (settings: AppSettings) => Promise<void>; }) {
  const [cleanState, setCleanState] = useState<CleanState>('idle');
  const [backupState, setBackupState] = useState<BackupState>('idle');
  const [restoreState, setRestoreState] = useState<RestoreState>('idle');
  const [cleanResultDialog, setCleanResultDialog] = useState<CleanResultDialogState | null>(null);
  const [migrationBackupDialog, setMigrationBackupDialog] = useState<MigrationBackupDialogState | null>(null);
  const [migrationRestoreDialog, setMigrationRestoreDialog] = useState<MigrationRestoreDialogState | null>(null);
  const [savingFrequency, setSavingFrequency] = useState(false);

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

  return (
    <div className="h-full min-h-0 overflow-y-auto">
      <div className="border-b border-slate-200 pb-4"><h2 className="text-2xl font-bold text-slate-950">{textMap.maintenance}</h2></div>
      <section className="mt-6">
        <div className="mb-3 px-1 text-sm font-semibold text-slate-700">{textMap.data}</div>
        <div className="overflow-hidden rounded-2xl border border-slate-200 bg-white">
          <SettingsLikeRow title={textMap.migrationBackup} description={textMap.migrationBackupDescription} action={<div className="flex items-center gap-2"><Button className="min-w-24" variant="secondary" disabled={backupState === 'running' || restoreState === 'running'} onClick={handleMigrationBackup}>{backupState === 'running' ? textMap.backingUp : textMap.backup}</Button><Button className="min-w-24" variant="secondary" disabled={backupState === 'running' || restoreState === 'running'} onClick={handleMigrationImport}>{restoreState === 'running' ? textMap.importing : textMap.import}</Button></div>} />
          <SettingsLikeRow title={textMap.cleanData} description={textMap.cleanDescription} action={<Button className="min-w-24" variant="secondary" disabled={cleanState === 'running'} onClick={handleCleanData}>{cleanState === 'running' ? textMap.cleaning : textMap.clean}</Button>} />
          <SettingsLikeRow title={textMap.usageFrequency} description={textMap.usageDescription} action={<div className="flex rounded-full bg-slate-100 p-1">{refreshFrequencyOptions.map((option) => (<button key={option.value} type="button" disabled={savingFrequency} onClick={() => handleFrequencyChange(option.value)} className={`h-8 min-w-16 rounded-full px-3 text-sm transition ${appSettings.account_usage_refresh_seconds === option.value ? 'bg-white font-semibold text-slate-950 shadow-sm' : 'text-slate-500 hover:text-slate-800'} disabled:cursor-not-allowed disabled:opacity-60`}>{option.label}</button>))}</div>} />
        </div>
      </section>
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


function formatBytes(value: number) {
  if (value >= 1024 * 1024 * 1024) return `${(value / 1024 / 1024 / 1024).toFixed(2)} GB`;
  if (value >= 1024 * 1024) return `${(value / 1024 / 1024).toFixed(2)} MB`;
  if (value >= 1024) return `${(value / 1024).toFixed(2)} KB`;
  return `${value} B`;
}





