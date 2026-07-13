import { StatCard } from '../../components/business/PreviewParts';
import { Card, CardContent } from '../../components/ui/Card';
import type { PageContext } from '../../lib/appTypes';
import type { ScanSummary, TokenUsageSummary } from '../../types';

type ListenerStatus = {
  running: boolean;
  host: string;
  port: number;
  callbackUrl: string;
  message: string;
};

const text = {
  accountCount: '\u8d26\u6237\u6570',
  accountSnapshot: '\u8d26\u53f7\u7ba1\u7406\u5feb\u7167',
  configuredModels: '\u5df2\u914d\u7f6e\u6a21\u578b',
  enabledModels: '\u542f\u7528\u6a21\u578b',
  codexSelectable: '\u53ef\u88ab Codex \u9009\u62e9',
  skillCount: '\u6280\u80fd\u6570',
  installedSkills: '\u5df2\u5b89\u88c5 Skills',
  mcpCount: 'MCP \u6570',
  enabledCount: '\u4e2a\u5df2\u542f\u7528',
  waitingRefresh: '\u7b49\u5f85\u81ea\u52a8\u5237\u65b0',
  serviceListen: '\u670d\u52a1\u76d1\u542c',
  serviceDescription: '\u670d\u52a1\u8fd0\u884c\u72b6\u6001\u4e0e Token \u7528\u91cf\u6982\u89c8\u3002',
  routerAddress: '\u672c\u5730 Router',
  routerRunning: 'Router \u8fd0\u884c\u4e2d',
  routerStopped: 'Router \u5df2\u505c\u6b62',
  listenerAddress: '\u8d26\u53f7\u53cd\u4ee3',
  accountProxyOn: '\u8d26\u53f7\u53cd\u4ee3\u5df2\u5f00\u542f',
  accountProxyOff: '\u8d26\u53f7\u53cd\u4ee3\u5df2\u5173\u95ed',
  input: '\u8f93\u5165',
  output: '\u8f93\u51fa',
  threadCount: '\u5df2\u6709\u4f1a\u8bdd',
};

export function DashboardPage({ models, enabledModels, appSettings, routerStatus, routerRuntimeInfo, routerUrl, dashboardSnapshot }: PageContext) {
  const { threadSummary, accountCount, skillCount, mcpSummary, oauthListenerStatus, tokenUsageSummary, lastUpdatedAt } = dashboardSnapshot;
  const hasLoaded = lastUpdatedAt !== null;

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-4 gap-4">
        <StatCard label={text.accountCount} value={hasLoaded ? String(accountCount) : '-'} helper={text.accountSnapshot} />
        <StatCard label={text.configuredModels} value={String(models.length)} helper="Catalog models" />
        <StatCard label={text.enabledModels} value={String(enabledModels)} helper={text.codexSelectable} />
        <ThreadStatCard summary={threadSummary} hasLoaded={hasLoaded} />
      </div>
      <div className="grid grid-cols-2 gap-4">
        <StatCard label={text.skillCount} value={hasLoaded ? String(skillCount) : '-'} helper={text.installedSkills} />
        <StatCard label={text.mcpCount} value={hasLoaded ? String(mcpSummary.total) : '-'} helper={hasLoaded ? `${mcpSummary.enabled} ${text.enabledCount}` : text.waitingRefresh} />
      </div>
      <ServiceStatusPanel
        routerStatus={routerStatus}
        routerRuntimeInfo={routerRuntimeInfo}
        routerUrl={routerUrl}
        routerPort={appSettings.router_port}
        oauthListenerStatus={oauthListenerStatus}
        accountProxyEnabled={appSettings.account_proxy?.account_proxy_enabled === true}
        tokenUsageSummary={tokenUsageSummary}
        hasLoaded={hasLoaded}
      />
    </div>
  );
}

function ServiceStatusPanel({
  routerStatus,
  routerRuntimeInfo,
  routerUrl,
  routerPort,
  oauthListenerStatus,
  accountProxyEnabled,
  tokenUsageSummary,
  hasLoaded,
}: {
  routerStatus: PageContext['routerStatus'];
  routerRuntimeInfo: PageContext['routerRuntimeInfo'];
  routerUrl: string;
  routerPort: number;
  oauthListenerStatus: ListenerStatus | null;
  accountProxyEnabled: boolean;
  tokenUsageSummary: TokenUsageSummary;
  hasLoaded: boolean;
}) {
  const routerRunning = routerStatus === 'running';
  const listenerHost = oauthListenerStatus?.host || '127.0.0.1';
  const listenerPort = oauthListenerStatus?.port || 1455;
  const listenerUrl = `http://${listenerHost}:${listenerPort}/v1`;
  const displayRouterUrl = routerUrl || `http://${routerRuntimeInfo.host || '127.0.0.1'}:${routerRuntimeInfo.port || routerPort}`;
  return (
    <Card>
      <CardContent>
        <div className="mb-4 flex items-center justify-between gap-3">
          <div>
            <h3 className="text-base font-bold text-slate-950">{text.serviceListen}</h3>
            <p className="mt-1 text-sm text-slate-500">{text.serviceDescription}</p>
          </div>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <ServiceListenItem
            label={text.routerAddress}
            value={displayRouterUrl}
            running={routerRunning}
            statusText={routerRunning ? text.routerRunning : text.routerStopped}
            inputTokens={hasLoaded ? tokenUsageSummary.router_input_tokens : null}
            cachedInputTokens={hasLoaded ? tokenUsageSummary.router_cached_input_tokens : null}
            outputTokens={hasLoaded ? tokenUsageSummary.router_output_tokens : null}
          />
          <ServiceListenItem
            label={text.listenerAddress}
            value={listenerUrl}
            running={oauthListenerStatus?.running === true}
            statusText={accountProxyEnabled ? text.accountProxyOn : text.accountProxyOff}
            helper={oauthListenerStatus?.message}
            inputTokens={hasLoaded ? tokenUsageSummary.account_proxy_input_tokens : null}
            cachedInputTokens={hasLoaded ? tokenUsageSummary.account_proxy_cached_input_tokens : null}
            outputTokens={hasLoaded ? tokenUsageSummary.account_proxy_output_tokens : null}
          />
        </div>
      </CardContent>
    </Card>
  );
}

function ServiceListenItem({
  label,
  value,
  running,
  statusText,
  helper,
  inputTokens,
  cachedInputTokens,
  outputTokens,
}: {
  label: string;
  value: string;
  running: boolean;
  statusText: string;
  helper?: string;
  inputTokens: number | null;
  cachedInputTokens: number | null;
  outputTokens: number | null;
}) {
  return (
    <div className={`rounded-lg border px-4 py-3 ${running ? 'border-emerald-200 bg-emerald-50' : 'border-amber-200 bg-amber-50'}`}>
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span className={`h-2.5 w-2.5 shrink-0 rounded-full ${running ? 'bg-emerald-500' : 'bg-amber-500'}`} />
            <span className="font-semibold text-slate-900">{label}</span>
          </div>
          <div className="mt-2 truncate font-mono text-xs text-slate-600">{value}</div>
          <div className={`mt-1 text-xs ${running ? 'text-emerald-700' : 'text-amber-700'}`}>{statusText}</div>
          <div className="mt-3 grid grid-cols-3 gap-2 text-xs">
            <TokenUsagePill label={text.input} value={inputTokens} tone={running ? 'emerald' : 'amber'} />
            <TokenUsagePill label="Cache" value={cachedInputTokens} tone={running ? 'emerald' : 'amber'} />
            <TokenUsagePill label={text.output} value={outputTokens} tone={running ? 'emerald' : 'amber'} />
          </div>
          {helper && <div className="mt-1 truncate text-xs text-slate-500">{helper}</div>}
        </div>
      </div>
    </div>
  );
}

function TokenUsagePill({ label, value, tone }: { label: string; value: number | null; tone: 'emerald' | 'amber' }) {
  const toneClassName = tone === 'emerald' ? 'border-emerald-100 bg-white/80 text-emerald-700' : 'border-amber-100 bg-white/80 text-amber-700';

  return (
    <div className={`min-w-0 rounded-lg border px-2.5 py-2 ${toneClassName}`}>
      <div className="text-[11px] font-medium text-slate-500">{label}</div>
      <div className="mt-0.5 truncate font-mono text-sm font-semibold text-slate-950">{formatTokenCount(value)}</div>
    </div>
  );
}

function ThreadStatCard({ summary, hasLoaded }: { summary: ScanSummary | null; hasLoaded: boolean }) {
  return (
    <div className="rounded-3xl border border-slate-200 bg-white shadow-sm">
      <div className="px-6 py-5">
        <div className="text-sm font-medium text-slate-500">{text.threadCount}</div>
        <div className="mt-3">
          <div>
            <div className="text-xl font-bold text-slate-950">{hasLoaded ? String(summary?.totalThreads ?? 0) : '-'}</div>
            <div className="mt-1 text-xs text-slate-400">{hasLoaded ? formatBytes(summary?.totalSize ?? 0) : '-'}</div>
          </div>
        </div>
      </div>
    </div>
  );
}

function formatTokenCount(value: number | null) {
  if (value === null) return '-';
  return new Intl.NumberFormat('zh-CN').format(value || 0);
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
