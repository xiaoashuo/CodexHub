import { useEffect, useMemo, useState } from 'react';
import { Badge } from '../../components/ui/Badge';
import { Button } from '../../components/ui/Button';
import { Card, CardContent, CardHeader } from '../../components/ui/Card';
import { invokeCheckRestoreCodexThreadIndex, invokeDeleteCodexThreadFiles, invokeRestoreCodexThreadIndex, invokeScanCodexThreads } from '../../lib/tauriBridge';
import type { ProjectGroup, ThreadRestoreCheckResult, ThreadScanResult, ThreadSession } from '../../types';

const EMPTY_SCAN: ThreadScanResult = {
  summary: {
    totalThreads: 0,
    totalSize: 0,
    activeDays: 0,
    averageThreadsPerDay: 0,
    indexedThreads: 0,
    missingFromIndex: 0,
    archivedThreads: 0,
    projectCount: 0,
    scannedAt: '',
  },
  projects: [],
};

let cachedThreadScanResult: ThreadScanResult | null = null;
let cachedExpandedProjectKeys = new Set<string>();
let initialThreadScanPromise: Promise<ThreadScanResult> | null = null;
const CODEX_VISIBLE_THREAD_LIMIT_HINT = '提示：Codex 侧边栏通常只稳定加载最近约 50 条会话。只有勾选“同步到最近”时，才会把恢复的会话刷新到最近位置。';

function restartRestoreWaitingProgress(moveToRecent: boolean) {
  return `等待确认。确认后会关闭 Codex，恢复会话索引，然后重新启动 Codex。\n${moveToRecent ? '本次会同步到最近。' : '本次会保留原始时间。'}\n${CODEX_VISIBLE_THREAD_LIMIT_HINT}`;
}

type RestartRestoreDialogState = {
  filePaths: string[];
  restoreAll: boolean;
  moveToRecent: boolean;
  check: ThreadRestoreCheckResult;
  running: boolean;
  completed: boolean;
  error: string;
  progress: string;
};

type SessionFilter = 'all' | 'active' | 'archived' | 'recoverable' | 'abnormal';

export function ThreadManagerPage() {
  const [scanResult, setScanResult] = useState<ThreadScanResult>(cachedThreadScanResult ?? EMPTY_SCAN);
  const [expandedProjects, setExpandedProjects] = useState<Set<string>>(new Set(cachedExpandedProjectKeys));
  const [selectedProjects, setSelectedProjects] = useState<Set<string>>(new Set());
  const [selectedSessions, setSelectedSessions] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [deleteRequestPaths, setDeleteRequestPaths] = useState<string[] | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [restartRestoreDialog, setRestartRestoreDialog] = useState<RestartRestoreDialogState | null>(null);
  const [showSubagentSessions, setShowSubagentSessions] = useState(false);
  const [sessionFilter, setSessionFilter] = useState<SessionFilter>('all');
  const [sessionKeyword, setSessionKeyword] = useState('');

  useEffect(() => {
    if (cachedThreadScanResult) {
      return;
    }

    handleInitialScan();
  }, []);

  useEffect(() => {
    setSelectedProjects(new Set());
    setSelectedSessions(new Set());
  }, [showSubagentSessions]);

  const visibleProjects = useMemo(
    () => filterSessionProjects(filterSubagentSessions(scanResult.projects, showSubagentSessions), sessionFilter, sessionKeyword),
    [scanResult.projects, showSubagentSessions, sessionFilter, sessionKeyword],
  );
  const sortedProjects = useMemo(
    () => [...visibleProjects].sort(compareProjectsByName),
    [visibleProjects],
  );
  const selectedCount = selectedSessions.size;
  const applyScanResult = (result: ThreadScanResult, expandedProjectKeys?: Set<string>) => {
    const nextProjects = [...result.projects].sort(compareProjectsByName);
    const nextScanResult = { ...result, projects: nextProjects };
    const nextExpandedProjects = expandedProjectKeys ?? new Set<string>();

    cachedThreadScanResult = nextScanResult;
    cachedExpandedProjectKeys = new Set(nextExpandedProjects);
    setScanResult(nextScanResult);
    setExpandedProjects(new Set(nextExpandedProjects));
    setSelectedProjects(new Set());
    setSelectedSessions(new Set());
  };

  const collectSelectedSessionPaths = () => {
    return visibleProjects
      .flatMap((project) => project.sessions)
      .filter((session) => selectedSessions.has(getSessionKey(session)))
      .map((session) => session.filePath);
  };

  async function handleInitialScan() {
    setLoading(true);
    setError('');
    setNotice('');

    try {
      if (!initialThreadScanPromise) {
        initialThreadScanPromise = invokeScanCodexThreads().finally(() => {
          initialThreadScanPromise = null;
        });
      }

      const result = await initialThreadScanPromise;
      applyScanResult(result);
    } catch (scanError) {
      setError(formatUnknownError(scanError));
    } finally {
      setLoading(false);
    }
  }

  const handleRefresh = async () => {
    setLoading(true);
    setError('');
    setNotice('');

    try {
      const result = await invokeScanCodexThreads();
      applyScanResult(result, expandedProjects);
    } catch (scanError) {
      setError(formatUnknownError(scanError));
    } finally {
      setLoading(false);
    }
  };

  const toggleProject = (project: ProjectGroup) => {
    const key = getProjectKey(project);
    setExpandedProjects((current) => {
      const next = new Set(current);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      cachedExpandedProjectKeys = new Set(next);
      return next;
    });
  };

  const toggleSelectedProject = (project: ProjectGroup) => {
    const key = getProjectKey(project);
    const sessionKeys = project.sessions.map(getSessionKey);
    const allSelected = sessionKeys.length > 0 && sessionKeys.every((sessionKey) => selectedSessions.has(sessionKey));

    setSelectedSessions((current) => {
      const next = new Set(current);
      if (allSelected) {
        sessionKeys.forEach((sessionKey) => next.delete(sessionKey));
      } else {
        sessionKeys.forEach((sessionKey) => next.add(sessionKey));
      }
      return next;
    });
    setSelectedProjects((current) => {
      const next = new Set(current);
      if (allSelected) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  };

  const toggleSelectedSession = (session: ThreadSession) => {
    const key = getSessionKey(session);
    setSelectedSessions((current) => {
      const next = new Set(current);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      setSelectedProjects((currentProjects) => {
        const nextProjects = new Set(currentProjects);
        for (const project of visibleProjects) {
          if (!project.sessions.some((item) => getSessionKey(item) === key)) {
            continue;
          }
          const projectKey = getProjectKey(project);
          const allProjectSessionsSelected = project.sessions.length > 0 && project.sessions.every((item) => next.has(getSessionKey(item)));
          if (allProjectSessionsSelected) {
            nextProjects.add(projectKey);
          } else {
            nextProjects.delete(projectKey);
          }
          break;
        }
        return nextProjects;
      });
      return next;
    });
  };

  const handleDeleteSelected = async () => {
    if (selectedCount === 0) {
      return;
    }

    const filePaths = collectSelectedSessionPaths();
    if (filePaths.length === 0) {
      return;
    }

    setDeleteRequestPaths(filePaths);
  };

  const confirmDeleteSelected = async () => {
    const filePaths = deleteRequestPaths ?? [];
    if (filePaths.length === 0 || deleting) {
      return;
    }

    setDeleting(true);
    setLoading(true);
    setError('');
    setNotice('');

    try {
      const result = await invokeDeleteCodexThreadFiles(filePaths);
      const nextProjects = [...result.projects].sort(compareProjectsByName);
      const nextExpandedProjects = new Set([...expandedProjects].filter((key) => nextProjects.some((project) => getProjectKey(project) === key)));
      applyScanResult({ ...result, projects: nextProjects }, nextExpandedProjects);
      setDeleteRequestPaths(null);
      setNotice(`已提交异步删除并完成扫描刷新：${filePaths.length} 个会话文件。`);
    } catch (deleteError) {
      setError(formatUnknownError(deleteError));
    } finally {
      setDeleting(false);
      setLoading(false);
    }
  };

  const handleRestoreSelected = async () => {
    if (selectedCount === 0) {
      return;
    }

    const filePaths = collectSelectedSessionPaths();
    await restoreIndex(filePaths, false, false);
  };

  const restoreIndex = async (filePaths: string[], restoreAll: boolean, moveToRecent: boolean) => {
    setLoading(true);
    setError('');
    setNotice('');

    try {
      const check = await invokeCheckRestoreCodexThreadIndex(filePaths, restoreAll);

      if (check.requiresCodexRestart) {
        setRestartRestoreDialog({
          filePaths,
          restoreAll,
          moveToRecent,
          check,
          running: false,
          completed: false,
          error: '',
          progress: restartRestoreWaitingProgress(moveToRecent),
        });
        return;
      }

      const result = await invokeRestoreCodexThreadIndex(filePaths, restoreAll, false, moveToRecent);
      applyScanResult(result.scan, expandedProjects);
      setNotice(result.backupPath ? `${result.message} 备份：${result.backupPath}` : result.message);
    } catch (restoreError) {
      setError(formatUnknownError(restoreError));
    } finally {
      setLoading(false);
    }
  };


  const handleCancelRestartRestore = () => {
    if (restartRestoreDialog?.running) {
      return;
    }

    setRestartRestoreDialog(null);
  };

  const handleConfirmRestartRestore = async () => {
    if (!restartRestoreDialog || restartRestoreDialog.running) {
      return;
    }

    setLoading(true);
    setError('');
    setNotice('');
    setRestartRestoreDialog((current) => current ? {
      ...current,
      running: true,
      completed: false,
      error: '',
      progress: '正在关闭 Codex，并准备写入会话恢复状态...',
    } : current);

    const progressTimers = [
      window.setTimeout(() => {
        setRestartRestoreDialog((current) => current?.running ? {
          ...current,
          progress: '正在恢复会话索引，并写入 Codex 侧边栏状态...',
        } : current);
      }, 900),
      window.setTimeout(() => {
        setRestartRestoreDialog((current) => current?.running ? {
          ...current,
          progress: '正在启动 Codex，并等待恢复结果刷新...',
        } : current);
      }, 2200),
    ];

    try {
      const result = await invokeRestoreCodexThreadIndex(
        restartRestoreDialog.filePaths,
        restartRestoreDialog.restoreAll,
        true,
        restartRestoreDialog.moveToRecent,
      );

      setRestartRestoreDialog((current) => current ? {
        ...current,
        running: false,
        completed: true,
        progress: '已完成：Codex 已重启，会话索引恢复结果已刷新。',
      } : current);
      applyScanResult(result.scan, expandedProjects);
      setNotice(result.backupPath ? `${result.message} 备份：${result.backupPath}` : result.message);
    } catch (restoreError) {
      setRestartRestoreDialog((current) => current ? {
        ...current,
        running: false,
        completed: false,
        error: formatUnknownError(restoreError),
        progress: '恢复流程已中断，请根据错误信息处理后重试。',
      } : current);
    } finally {
      progressTimers.forEach((timer) => window.clearTimeout(timer));
      setLoading(false);
    }
  };

  return (
    <div className="flex h-full min-h-0 w-full max-w-full flex-col gap-4 overflow-hidden">
      <section className="shrink-0 flex flex-wrap items-start justify-between gap-4">
        <div>
          <h2 className="text-2xl font-bold text-slate-950">会话管理</h2>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2">
          <div className="relative">
            <input
              value={sessionKeyword}
              onChange={(event) => setSessionKeyword(event.target.value)}
              placeholder="搜索项目或会话"
              className="h-10 w-44 rounded-lg border border-slate-200 bg-white px-3 text-sm text-slate-700 shadow-sm outline-none transition placeholder:text-slate-400 focus:border-indigo-400"
            />
          </div>
          <div className="flex flex-wrap gap-1 rounded-lg border border-slate-200 bg-white p-1 shadow-sm">
            {([
              ['all', '全部'],
              ['active', '活跃'],
              ['archived', '归档'],
              ['recoverable', '可恢复'],
              ['abnormal', '异常'],
            ] as const).map(([value, label]) => (
              <button
                key={value}
                type="button"
                onClick={() => setSessionFilter(value)}
                className={`rounded-md px-2.5 py-1.5 text-xs font-semibold transition ${sessionFilter === value ? 'bg-indigo-600 text-white' : 'text-slate-500 hover:bg-slate-50 hover:text-slate-800'}`}
              >
                {label}
              </button>
            ))}
          </div>
          <label className="flex items-center gap-2 rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-600 shadow-sm">
            <input
              checked={showSubagentSessions}
              className="h-4 w-4 rounded border-slate-300 text-indigo-600"
              onChange={(event) => setShowSubagentSessions(event.target.checked)}
              type="checkbox"
            />
            显示子代理会话
          </label>
          <Button variant="secondary" disabled={loading || selectedCount === 0} onClick={handleRestoreSelected}>
            恢复选中
          </Button>
          <Button variant="danger" disabled={selectedCount === 0} onClick={handleDeleteSelected}>
            删除选中{selectedCount > 0 ? ` (${selectedCount})` : ''}
          </Button>
          <Button onClick={handleRefresh} disabled={loading}>{loading ? '扫描中...' : '刷新扫描'}</Button>
        </div>
      </section>

      {error && (
        <div className="rounded-lg border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
          {error}
        </div>
      )}

      {notice && (
        <div className="rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-700">
          {notice}
        </div>
      )}

      <div className="grid shrink-0 grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
        <SummaryCard label="总线程数" value={scanResult.summary.totalThreads.toString()} hint={`${scanResult.summary.projectCount} 个项目`} />
        <SummaryCard label="总大小" value={formatBytes(scanResult.summary.totalSize)} hint={`${scanResult.summary.archivedThreads} 个归档`} />
        <SummaryCard label="活跃天数" value={scanResult.summary.activeDays.toString()} hint={`日均 ${scanResult.summary.averageThreadsPerDay.toFixed(1)} 个`} />
        <SummaryCard label="当前选中" value={selectedCount.toString()} hint="选中后统一恢复" />
      </div>

      <Card className="flex min-h-0 max-w-full flex-1 flex-col overflow-hidden">
        <CardHeader className="shrink-0">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <h3 className="text-lg font-bold text-slate-950">项目 / 线程列表</h3>
              <p className="mt-1 text-sm text-slate-500">
                {scanResult.summary.scannedAt ? `上次扫描：${formatDateTime(scanResult.summary.scannedAt)}` : '尚未完成扫描'}
              </p>
            </div>
            <Badge tone="blue">{`${sortedProjects.length} 个项目`}</Badge>
          </div>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 space-y-2 overflow-y-auto overflow-x-hidden pr-2">
          {sortedProjects.length === 0 && !loading ? (
            <div className="px-4 py-8 text-center text-sm text-slate-500">未扫描到 Codex 会话文件。</div>
          ) : (
            sortedProjects.map((project) => {
              const projectKey = getProjectKey(project);
              const expanded = expandedProjects.has(projectKey);
              const selected = project.sessions.length > 0 && project.sessions.every((session) => selectedSessions.has(getSessionKey(session)));

              return (
                <div key={projectKey} className="min-w-0 overflow-hidden border-b border-slate-100 last:border-b-0">
                   <div className="flex w-full items-center gap-3 rounded-xl px-2 py-3 transition hover:bg-slate-50">
                    <input
                      aria-label={`选择 ${project.projectName}`}
                      checked={selected}
                      className="h-4 w-4 shrink-0 rounded border-slate-300 text-indigo-600"
                      onChange={() => toggleSelectedProject(project)}
                      type="checkbox"
                    />
                    <button
                      aria-label={expanded ? `折叠 ${project.projectName}` : `展开 ${project.projectName}`}
                      className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-md border text-sm font-bold leading-none transition ${
                        expanded ? 'border-slate-900 bg-slate-900 text-white' : 'border-slate-300 bg-white text-slate-700 hover:bg-slate-100'
                      }`}
                      onClick={() => toggleProject(project)}
                      type="button"
                    >
                      {expanded ? '-' : '+'}
                    </button>
                    <div className="min-w-0 flex-1 text-left">
                       <div className="truncate font-semibold text-slate-900">{project.projectName}</div>
                       <div className="mt-0.5 truncate text-xs text-slate-500" title={project.cwd}>{project.cwd || 'Unknown Project'}</div>
                    </div>
                    <div className="flex shrink-0 flex-wrap justify-end gap-x-3 gap-y-1">
                       <span className="rounded-full bg-slate-100 px-2 py-1 text-xs font-medium text-slate-600">{project.threadCount} 个会话</span>
                       <span className="text-xs text-slate-400">{formatBytes(project.totalSize)}</span>
                    </div>
                  </div>
                  {expanded && (
                    <div className="min-w-0 overflow-x-hidden border-l border-slate-100 pl-10">
                      {project.sessions.map((session) => (
                        <ThreadRow
                          key={`${session.source}-${session.filePath}`}
                          selected={selectedSessions.has(getSessionKey(session))}
                          session={session}
                          toggleSelectedSession={toggleSelectedSession}
                        />
                      ))}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </CardContent>
      </Card>

      {deleteRequestPaths && (
        <DeleteConfirmDialog
          count={deleteRequestPaths.length}
          deleting={deleting}
          onCancel={() => !deleting && setDeleteRequestPaths(null)}
          onConfirm={confirmDeleteSelected}
        />
      )}

      {restartRestoreDialog && (
        <RestartRestoreDialog
          state={restartRestoreDialog}
          onCancel={handleCancelRestartRestore}
          onConfirm={handleConfirmRestartRestore}
          onMoveToRecentChange={(moveToRecent) => setRestartRestoreDialog((current) => current ? {
            ...current,
            moveToRecent,
            progress: current.running || current.completed ? current.progress : restartRestoreWaitingProgress(moveToRecent),
          } : current)}
        />
      )}
    </div>
  );
}


function RestartRestoreDialog({
  state,
  onCancel,
  onConfirm,
  onMoveToRecentChange,
}: {
  state: RestartRestoreDialogState;
  onCancel: () => void;
  onConfirm: () => void;
  onMoveToRecentChange: (moveToRecent: boolean) => void;
}) {
  const projectRoots = state.check.projectRoots;
  const shownProjectRoots = projectRoots.slice(0, 6);
  const hiddenProjectCount = Math.max(0, projectRoots.length - shownProjectRoots.length);
  const confirmLabel = state.running ? '恢复中...' : state.completed ? '已完成' : '确认并恢复';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/45 px-4 py-6 backdrop-blur-sm">
      <div className="w-full max-w-2xl overflow-hidden rounded-xl bg-white shadow-2xl shadow-slate-950/20">
        <div className="border-b border-slate-200 px-6 py-5">
          <h3 className="text-xl font-bold text-slate-950">需要重启 Codex 后恢复会话</h3>
          <p className="mt-2 text-sm leading-6 text-slate-600">
            为避免运行中的 Codex 用内存状态覆盖恢复结果，本次恢复会先关闭 Codex，再写入会话索引和侧边栏状态，最后重新启动 Codex。
          </p>
        </div>

        <div className="space-y-4 px-6 py-5">
          <div className="rounded-lg border border-slate-200 bg-slate-50 px-4 py-3">
            <div className="text-sm font-semibold text-slate-800">涉及项目</div>
            <div className="mt-2 space-y-1">
              {shownProjectRoots.map((projectRoot) => (
                <div key={projectRoot} className="break-all font-mono text-xs leading-5 text-slate-600">
                  {projectRoot}
                </div>
              ))}
              {shownProjectRoots.length === 0 && (
                <div className="text-xs text-slate-500">本次恢复的会话没有项目目录。</div>
              )}
              {hiddenProjectCount > 0 && (
                <div className="text-xs text-slate-500">以及 {hiddenProjectCount} 个项目目录</div>
              )}
            </div>
          </div>

          <label className={`flex items-start gap-3 rounded-lg border border-slate-200 bg-slate-50 px-4 py-3 ${state.running || state.completed ? 'cursor-not-allowed opacity-70' : 'cursor-pointer'}`}>
            <input
              checked={state.moveToRecent}
              className="mt-1 h-4 w-4 shrink-0 rounded border-slate-300 text-indigo-600"
              disabled={state.running || state.completed}
              onChange={(event) => onMoveToRecentChange(event.target.checked)}
              type="checkbox"
            />
            <span>
              <span className="block text-sm font-semibold text-slate-900">同步到最近</span>
              <span className="mt-1 block text-sm leading-6 text-slate-600">
                默认不勾选会保留原始时间和历史排序；勾选后会刷新排序时间，让恢复的会话进入 Codex 最近列表前面。
              </span>
            </span>
          </label>

          <div className="rounded-lg border border-indigo-100 bg-indigo-50 px-4 py-3">
            <div className="flex items-start gap-3">
              {state.running && <span className="mt-0.5 h-4 w-4 shrink-0 animate-spin rounded-full border-2 border-indigo-300 border-t-indigo-700" />}
              <div>
                <div className="text-sm font-semibold text-indigo-900">进度</div>
                <div className="mt-1 text-sm leading-6 text-indigo-800">{state.progress}</div>
              </div>
            </div>
          </div>

          {state.error && (
            <div className="break-all rounded-lg border border-rose-200 bg-rose-50 px-4 py-3 text-sm leading-6 text-rose-700">
              {state.error}
            </div>
          )}
        </div>

        <div className="flex flex-wrap justify-end gap-3 bg-slate-50 px-6 py-4">
          <Button variant="secondary" onClick={onCancel} disabled={state.running}>
            {state.completed ? '关闭' : '取消'}
          </Button>
          <Button onClick={onConfirm} disabled={state.running || state.completed}>
            {confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}

function DeleteConfirmDialog({
  count,
  deleting,
  onCancel,
  onConfirm,
}: {
  count: number;
  deleting: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/45 px-4 py-6 backdrop-blur-sm">
      <div className="w-full max-w-lg overflow-hidden rounded-xl bg-white shadow-2xl shadow-slate-950/20">
        <div className="border-b border-amber-100 bg-amber-50 px-6 py-4 text-sm leading-6 text-amber-800">
          删除会异步执行，确认后界面会等待后端删除完成并重新扫描会话列表。
        </div>
        <div className="px-6 py-5">
          <h3 className="text-xl font-bold text-slate-950">删除选中会话</h3>
          <p className="mt-3 text-sm leading-6 text-slate-600">
            将删除 {count} 个 Codex 会话文件。该操作会移除本地 jsonl 原始文件，删除后不能从当前工具内撤销。
          </p>
        </div>
        <div className="flex flex-wrap justify-end gap-3 bg-slate-50 px-6 py-4">
          <Button variant="secondary" onClick={onCancel} disabled={deleting}>取消</Button>
          <Button variant="danger" onClick={onConfirm} disabled={deleting}>{deleting ? '删除中...' : '确认删除'}</Button>
        </div>
      </div>
    </div>
  );
}

function SummaryCard({ label, value, hint }: { label: string; value: string; hint: string }) {
  return (
    <div className="rounded-lg border border-slate-200 bg-white px-5 py-4 shadow-sm">
      <div className="text-sm text-slate-500">{label}</div>
      <div className="mt-2 text-2xl font-bold text-slate-950">{value}</div>
      <div className="mt-1 text-xs text-slate-400">{hint}</div>
    </div>
  );
}

function ThreadRow({
  selected,
  session,
  toggleSelectedSession,
}: {
  selected: boolean;
  session: ThreadSession;
  toggleSelectedSession: (session: ThreadSession) => void;
}) {
  return (
    <div className="flex min-w-0 gap-3 px-4 py-3 text-sm transition hover:bg-slate-50">
      <input
        aria-label={`选择 ${session.title}`}
        checked={selected}
        className="mt-1 h-4 w-4 shrink-0 rounded border-slate-300 text-indigo-600"
        onChange={() => toggleSelectedSession(session)}
        type="checkbox"
      />
      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 flex-wrap items-start justify-between gap-3">
          <div className="min-w-0 flex-1">
            <div className="truncate font-medium text-slate-800" title={session.filePath}>{session.title}</div>
            {session.firstUserText && session.firstUserText !== session.title && (
              <div className="mt-1 truncate text-xs text-slate-500">{session.firstUserText}</div>
            )}
          </div>
          <div className="flex min-w-0 flex-wrap justify-end gap-1">
            {session.parseErrors > 0 && <Badge tone="rose">解析异常</Badge>}
            {session.archived && <Badge tone="amber">归档</Badge>}
            {isSubagentSession(session) && <Badge tone="amber">子代理</Badge>}
            {!session.archived && isRecoverableSession(session) && <Badge tone="amber">可恢复</Badge>}
            {!session.archived && !isRecoverableSession(session) && session.parseErrors === 0 && <Badge tone="green">正常</Badge>}
          </div>
        </div>
        <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-slate-500">
          <span>{formatDateTime(session.updatedAt || session.createdAt)}</span>
          <span className="text-slate-300">/</span>
          <span>{formatBytes(session.fileSize)}</span>
          <span className="text-slate-300">/</span>
          <span>{session.messageCount} 条消息</span>
        </div>
      </div>
    </div>
  );
}

function filterSessionProjects(projects: ProjectGroup[], filter: SessionFilter, keyword: string) {
  const normalizedKeyword = keyword.trim().toLowerCase();
  return projects
    .map((project) => {
      const sessions = project.sessions.filter((session) => {
        const matchesFilter = filter === 'all'
          || (filter === 'archived' && session.archived)
          || (filter === 'active' && !session.archived)
          || (filter === 'recoverable' && isRecoverableSession(session))
          || (filter === 'abnormal' && session.parseErrors > 0);
        if (!matchesFilter) return false;
        if (!normalizedKeyword) return true;
        return `${project.projectName} ${project.cwd ?? ''} ${session.title} ${session.firstUserText ?? ''}`.toLowerCase().includes(normalizedKeyword);
      });
      return { ...project, sessions, threadCount: sessions.length, totalSize: sessions.reduce((sum, session) => sum + session.fileSize, 0) };
    })
    .filter((project) => project.sessions.length > 0);
}

function isRecoverableSession(session: ThreadSession) {
  return session.missingFromIndex || session.sidebarMissing || session.stateNeedsRepair;
}

function filterSubagentSessions(projects: ProjectGroup[], showSubagentSessions: boolean) {
  if (showSubagentSessions) {
    return projects;
  }

  return projects
    .map((project) => {
      const sessions = project.sessions.filter((session) => !isSubagentSession(session));
      return {
        ...project,
        sessions,
        threadCount: sessions.length,
        totalSize: sessions.reduce((sum, session) => sum + session.fileSize, 0),
        activeDays: new Set(
          sessions
            .map((session) => activeDayFromSession(session))
            .filter((day): day is string => Boolean(day)),
        ).size,
      };
    })
    .filter((project) => project.sessions.length > 0);
}

function isSubagentSession(session: ThreadSession) {
  return session.threadSource === 'subagent';
}

function compareProjectsByName(left: ProjectGroup, right: ProjectGroup) {
  const leftIsDialog = isDialogProjectName(left.projectName);
  const rightIsDialog = isDialogProjectName(right.projectName);

  if (leftIsDialog && !rightIsDialog) {
    return -1;
  }

  if (rightIsDialog && !leftIsDialog) {
    return 1;
  }

  return left.projectName.localeCompare(right.projectName, 'zh-CN') || right.threadCount - left.threadCount;
}

function isDialogProjectName(projectName: string) {
  return projectName === '对话' || projectName === '瀵硅瘽';
}

function buildScanResultFromProjects(current: ThreadScanResult, projects: ProjectGroup[]): ThreadScanResult {
  const sessions = projects.flatMap((project) => project.sessions);
  const activeDays = new Set(
    sessions
      .map((session) => activeDayFromSession(session))
      .filter((day): day is string => Boolean(day)),
  );
  const totalThreads = sessions.length;
  const totalSize = sessions.reduce((sum, session) => sum + session.fileSize, 0);
  const indexedThreads = sessions.filter((session) => session.indexed).length;
  const missingFromIndex = sessions.filter((session) => session.missingFromIndex).length;
  const archivedThreads = sessions.filter((session) => session.archived).length;

  return {
    summary: {
      ...current.summary,
      totalThreads,
      totalSize,
      activeDays: activeDays.size,
      averageThreadsPerDay: activeDays.size === 0 ? 0 : totalThreads / activeDays.size,
      indexedThreads,
      missingFromIndex,
      archivedThreads,
      projectCount: projects.length,
    },
    projects,
  };
}

function activeDayFromSession(session: ThreadSession) {
  const value = session.createdAt || session.updatedAt;

  if (!value) {
    return undefined;
  }

  if (value.length >= 10 && value.charAt(4) === '-') {
    return value.slice(0, 10);
  }

  const date = /^\d+$/.test(value) ? new Date(Number(value) * 1000) : new Date(value);

  if (Number.isNaN(date.getTime())) {
    return undefined;
  }

  return date.toISOString().slice(0, 10);
}

function getProjectKey(project: ProjectGroup) {
  return `${project.projectName}:${project.cwd || ''}`;
}

function getSessionKey(session: ThreadSession) {
  return `${session.source}:${session.filePath}`;
}

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024 * 1024) {
    return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
  }

  if (bytes >= 1024 * 1024) {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }

  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${bytes} B`;
}

function formatDateTime(value?: string) {
  if (!value) {
    return '-';
  }

  const date = /^\d+$/.test(value) ? new Date(Number(value) * 1000) : new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleString('zh-CN', { hour12: false });
}

function formatUnknownError(error: unknown) {
  if (error instanceof Error) {
    if (error.message.includes('invoke')) {
      return '当前浏览器预览无法访问 Tauri 后端，请在桌面应用中执行会话扫描。';
    }

    return error.message;
  }

  if (typeof error === 'string') {
    return error;
  }

  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}
