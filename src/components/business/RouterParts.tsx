import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card, CardContent, CardHeader } from '../ui/Card';
import { Info, X } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import type { RouterLogEntry, RouterRuntimeInfo } from '../../types';

const EMPTY_RUNTIME_VALUE = '-';

export function RouterRuntimePanel({ runtimeInfo, children }: { runtimeInfo: RouterRuntimeInfo; children?: ReactNode }) {
  const rows = [
    ['服务', runtimeInfo.service],
    ['版本', runtimeInfo.version],
    ['地址', `${runtimeInfo.host}:${runtimeInfo.port}`],
    ['PID', runtimeInfo.pid ? String(runtimeInfo.pid) : EMPTY_RUNTIME_VALUE],
    ['最大并发', String(runtimeInfo.concurrencyLimit || 8)],
    ['健康检查', runtimeInfo.healthUrl],
    ['运行秒数', `${runtimeInfo.uptimeSeconds}s`],
  ];
  const visibleRows = rows.filter((_, index) => index !== 4);

  return (
    <Card>
      <CardHeader>
        <h3 className="text-lg font-bold text-slate-950">运行信息</h3>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 gap-3">
          {visibleRows.map(([label, value]) => (
            <div key={label} className="rounded-2xl bg-slate-50 px-4 py-3">
              <div className="text-xs text-slate-400">{label}</div>
              <div className="mt-1 break-all font-mono text-sm text-slate-800">{value}</div>
            </div>
          ))}
        </div>
        {children && <div className="mt-4 border-t border-slate-100 pt-4">{children}</div>}
      </CardContent>
    </Card>
  );
}

export function RouterLogs({
  logs,
  handleRouterLogsRefresh,
  handleRouterLogsClear,
}: {
  logs: RouterLogEntry[];
  handleRouterLogsRefresh?: () => Promise<void>;
  handleRouterLogsClear?: () => Promise<void>;
}) {
  const [selectedLog, setSelectedLog] = useState<RouterLogEntry | null>(null);

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-lg font-bold text-slate-950">Router 请求日志</h3>
            <p className="mt-1 text-sm text-slate-500">运行中会自动轮询刷新，手动刷新可立即同步。</p>
          </div>
          <div className="flex gap-2">
            {handleRouterLogsRefresh && (
              <Button variant="secondary" onClick={handleRouterLogsRefresh}>
                立即刷新
              </Button>
            )}
            {handleRouterLogsClear && (
              <Button variant="ghost" onClick={handleRouterLogsClear}>
                清空
              </Button>
            )}
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <div className="max-h-[calc(100vh-360px)] min-h-[260px] space-y-3 overflow-y-auto pr-2">
          {logs.map((log, index) => {
            const logKey = `${log.time}-${log.path}-${index}`;

            return (
              <div key={logKey} className="rounded-2xl bg-slate-50 px-4 py-3 text-sm">
                <div className="grid grid-cols-[1fr_1fr_0.8fr_1fr_0.8fr_auto] items-start gap-3">
                  <LogCell label="时间" value={formatRouterLogTime(log.time)} mono />
                  <LogCell label="来源 IP" value={log.source_ip} mono />
                  <LogCell label="状态" value={log.status} tone={log.status.startsWith('2') ? 'success' : 'danger'} />
                  <LogCell label="Provider" value={log.target_provider} />
                  <LogCell label="耗时" value={log.cost} align="right" />
                  <div className="flex justify-end">
                    <button
                      className="inline-flex h-6 items-center gap-1 rounded-md px-1.5 text-xs font-medium text-slate-400 transition hover:bg-slate-100 hover:text-slate-700"
                      type="button"
                      onClick={() => setSelectedLog(log)}
                    >
                      <Info className="h-3.5 w-3.5" />
                      详情
                    </button>
                  </div>
                </div>
              </div>
            );
          })}
          {logs.length === 0 && <div className="rounded-2xl bg-slate-50 px-4 py-8 text-center text-sm text-slate-400">暂无请求日志。</div>}
        </div>
        {selectedLog && <RouterLogDetailDialog log={selectedLog} onClose={() => setSelectedLog(null)} />}
      </CardContent>
    </Card>
  );
}

function RouterLogDetailDialog({ log, onClose }: { log: RouterLogEntry; onClose: () => void }) {
  const detailRows = [
    ['时间', formatRouterLogTime(log.time)],
    ['原始时间', log.time],
    ['来源 IP', log.source_ip],
    ['方法', log.method],
    ['请求地址', log.path],
    ['状态', log.status],
    ['Provider', log.target_provider],
    ['耗时', log.cost],
    ['Token 消耗', formatTokenUsage(log)],
    ['Usage 来源', log.usage_source || EMPTY_RUNTIME_VALUE],
    ['错误详情', log.error_detail || EMPTY_RUNTIME_VALUE],
  ];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="box-border flex max-h-[82vh] w-full max-w-4xl flex-col rounded-2xl bg-white px-5 py-5 shadow-2xl">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-xl font-bold text-slate-950">请求日志详情</h3>
            <p className="mt-1 break-all font-mono text-xs text-slate-500">{formatRouterLogTime(log.time)} · {log.target_provider || EMPTY_RUNTIME_VALUE}</p>
          </div>
          <button
            className="inline-flex h-8 w-8 items-center justify-center rounded-lg text-slate-400 transition hover:bg-slate-100 hover:text-slate-700"
            type="button"
            onClick={onClose}
            aria-label="关闭"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
        <div className="mt-5 min-h-0 flex-1 overflow-auto">
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3">
            {detailRows.map(([label, value]) => (
              <div key={label} className="min-w-0 rounded-lg bg-slate-50 px-3 py-2">
                <div className="text-xs text-slate-400">{label}</div>
                <div className="mt-1 break-all font-mono text-xs leading-5 text-slate-700">{value || EMPTY_RUNTIME_VALUE}</div>
              </div>
            ))}
          </div>
          <div className="mt-4">
            <div className="mb-1 text-xs font-semibold text-slate-500">完整日志</div>
            <pre className="max-h-80 overflow-auto rounded-lg bg-slate-950 px-3 py-2 text-xs leading-5 text-slate-100">{JSON.stringify(log, null, 2)}</pre>
          </div>
        </div>
        <div className="mt-5 flex justify-end">
          <Button variant="secondary" onClick={onClose}>关闭</Button>
        </div>
      </div>
    </div>
  );
}

function formatTokenUsage(log: RouterLogEntry) {
  const input = log.input_tokens ?? 0;
  const output = log.output_tokens ?? 0;
  const cached = log.cached_input_tokens ?? 0;
  const total = log.total_tokens ?? input + output;
  if (input === 0 && output === 0 && cached === 0 && total === 0) {
    return '-';
  }
  return `in ${formatTokenCount(input)} / cache ${formatTokenCount(cached)} / out ${formatTokenCount(output)} / total ${formatTokenCount(total)}`;
}

function formatRouterLogTime(value: string) {
  const trimmed = value.trim();
  if (!trimmed) return EMPTY_RUNTIME_VALUE;

  const numericValue = Number(trimmed);
  const date = Number.isFinite(numericValue)
    ? new Date(numericValue < 1_000_000_000_000 ? numericValue * 1000 : numericValue)
    : new Date(trimmed);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  const parts = new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  }).formatToParts(date);
  const part = (type: Intl.DateTimeFormatPartTypes) => parts.find((item) => item.type === type)?.value ?? '';

  return `${part('month')}/${part('day')} ${part('hour')}:${part('minute')}:${part('second')}`;
}

function formatTokenCount(value: number) {
  return new Intl.NumberFormat('zh-CN').format(value || 0);
}

function LogCell({
  label,
  value,
  mono,
  strong,
  tone,
  align,
}: {
  label: string;
  value: string;
  mono?: boolean;
  strong?: boolean;
  tone?: 'success' | 'danger';
  align?: 'right';
}) {
  const toneClass = tone === 'success' ? 'text-emerald-700' : tone === 'danger' ? 'text-rose-700' : 'text-slate-700';
  const alignClass = align === 'right' ? 'text-right' : '';
  const fontClass = mono ? 'font-mono' : strong ? 'font-semibold' : '';

  return (
    <div className={alignClass}>
      <div className="text-xs text-slate-400">{label}</div>
      <div className={`mt-1 break-all text-xs leading-5 ${toneClass} ${fontClass}`}>{value || EMPTY_RUNTIME_VALUE}</div>
    </div>
  );
}

export function EndpointRow({ method, path, description }: { method: string; path: string; description: string }) {
  return (
    <div className="grid grid-cols-[80px_1fr_1.4fr] items-center gap-3 rounded-2xl bg-slate-50 px-4 py-3">
      <Badge tone={method === 'GET' ? 'blue' : 'green'}>{method}</Badge>
      <span className="break-all font-mono text-xs text-slate-700">{path}</span>
      <span className="text-slate-500">{description}</span>
    </div>
  );
}
