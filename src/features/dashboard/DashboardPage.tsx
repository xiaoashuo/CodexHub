import { Cpu, MessageSquare, Plug, Sparkles, Users } from 'lucide-react';
import { Card, CardContent } from '../../components/ui/Card';
import type { PageContext } from '../../lib/appTypes';
import type { TokenUsageSummary } from '../../types';

const text = {
  overview: '总览',
  overviewDescription: '本地 Router 与 Codex 运行状态概览。',
  accountCount: '账户数',
  accountSnapshot: '账号管理快照',
  configuredModels: '已配置模型',
  enabledModels: '启用模型',
  codexSelectable: '可被 Codex 选择',
  threadCount: '已有会话',
  skillCount: '技能数',
  installedSkills: '已安装 Skills',
  mcpCount: 'MCP 数',
  enabledCount: '个已启用',
  waitingRefresh: '等待自动刷新',
  guide: '使用说明',
  guideNote: '若首次安装 ChatGPT，需启动登录一次。',
  proxyHint: '若需访问 OpenAI 官方代理或境外服务，请在设置中打开代理。',
  steps: ['账户管理新增账户', '模型管理配置中转站', '路由管理启动路由'],
  tokenUsage: 'Token 用量',
  cumulativeToken: '累计 Token',
  todayToken: '今日 Token',
  input: '输入',
  cached: '缓存输入',
  output: '输出',
  noData: '暂无数据',
};

export function DashboardPage({ enabledModels, dashboardSnapshot }: PageContext) {
  const { threadSummary, accountCount, skillCount, mcpSummary, tokenUsageSummary, lastUpdatedAt } = dashboardSnapshot;
  const hasLoaded = lastUpdatedAt !== null;
  const routerTokens = tokenUsageSummary.router_input_tokens + tokenUsageSummary.router_cached_input_tokens + tokenUsageSummary.router_output_tokens;
  const cumulativeTokens = routerTokens;
  const todayTokens = routerTokens;

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-2xl font-bold leading-tight text-slate-950">{text.overview}</h2>
        <p className="mt-1.5 text-sm text-slate-500">{text.overviewDescription}</p>
      </div>

      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        <DashboardStatCard icon={<Users className="h-5 w-5" />} value={hasLoaded ? String(accountCount) : '-'} label={text.accountCount} helper={text.accountSnapshot} />
        <DashboardStatCard icon={<Cpu className="h-5 w-5" />} value={String(enabledModels)} label={text.enabledModels} helper={text.codexSelectable} />
        <DashboardStatCard icon={<MessageSquare className="h-5 w-5" />} value={hasLoaded ? String(threadSummary?.totalThreads ?? 0) : '-'} label={text.threadCount} helper={hasLoaded ? formatBytes(threadSummary?.totalSize ?? 0) : text.waitingRefresh} />
        <DashboardStatCard icon={<Sparkles className="h-5 w-5" />} value={hasLoaded ? String(skillCount) : '-'} label={text.skillCount} helper={text.installedSkills} />
        <DashboardStatCard icon={<Plug className="h-5 w-5" />} value={hasLoaded ? String(mcpSummary.total) : '-'} label={text.mcpCount} helper={hasLoaded ? `${mcpSummary.enabled} ${text.enabledCount}` : text.waitingRefresh} />
      </div>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
        <GuideCard />
        <TokenPanel
          cumulativeTokens={cumulativeTokens}
          todayTokens={todayTokens}
          summary={tokenUsageSummary}
          hasLoaded={hasLoaded}
        />
      </div>
    </div>
  );
}

function GuideCard() {
  return (
    <Card>
      <CardContent>
        <h3 className="text-base font-bold text-slate-950">{text.guide}</h3>
        <div className="mt-3 rounded-xl bg-amber-50 px-3 py-2.5 text-sm leading-6 text-amber-700">{text.guideNote}</div>
        <div className="mt-3 rounded-xl bg-sky-50 px-3 py-2.5 text-sm leading-6 text-sky-700">{text.proxyHint}</div>
        <ol className="mt-4 space-y-3">
          {text.steps.map((step, index) => (
            <li key={step} className="flex items-center gap-3">
              <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-indigo-50 text-sm font-semibold text-indigo-600">{index + 1}</span>
              <span className="text-sm text-slate-700">{step}</span>
            </li>
          ))}
        </ol>
      </CardContent>
    </Card>
  );
}

function TokenPanel({ cumulativeTokens, todayTokens, summary, hasLoaded }: { cumulativeTokens: number; todayTokens: number; summary: TokenUsageSummary; hasLoaded: boolean }) {
  return (
    <Card>
      <CardContent>
        <h3 className="text-base font-bold text-slate-950">{text.tokenUsage}</h3>
        <div className="mt-4 grid grid-cols-2 gap-4">
          <div className="rounded-2xl bg-slate-50 px-4 py-4">
            <div className="text-xs text-slate-500">{text.cumulativeToken}</div>
            <div className="mt-2 text-2xl font-bold text-slate-950">{hasLoaded ? formatTokenCount(cumulativeTokens) : text.noData}</div>
          </div>
          <div className="rounded-2xl bg-slate-50 px-4 py-4">
            <div className="text-xs text-slate-500">{text.todayToken}</div>
            <div className="mt-2 text-2xl font-bold text-slate-950">{hasLoaded ? formatTokenCount(todayTokens) : text.noData}</div>
          </div>
        </div>
        <div className="mt-4 grid grid-cols-3 gap-3">
          <TokenBreakdownPill label={text.input} value={hasLoaded ? summary.router_input_tokens : null} />
          <TokenBreakdownPill label={text.cached} value={hasLoaded ? summary.router_cached_input_tokens : null} />
          <TokenBreakdownPill label={text.output} value={hasLoaded ? summary.router_output_tokens : null} />
        </div>
      </CardContent>
    </Card>
  );
}

function TokenBreakdownPill({ label, value }: { label: string; value: number | null }) {
  return (
    <div className="min-w-0 rounded-xl border border-slate-100 bg-white px-3 py-2.5">
      <div className="text-[11px] font-medium text-slate-500">{label}</div>
      <div className="mt-0.5 truncate font-mono text-sm font-semibold text-slate-950">{value === null ? '-' : formatTokenCount(value)}</div>
    </div>
  );
}

function DashboardStatCard({ icon, value, label, helper }: { icon: React.ReactNode; value: string; label: string; helper: string }) {
  return (
    <div className="flex flex-col items-center rounded-2xl border border-slate-200 bg-white px-3 py-3 text-center shadow-sm">
      <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-indigo-50 text-indigo-600">{icon}</div>
      <div className="mt-2 text-2xl font-bold leading-none text-slate-950">{value}</div>
      <div className="mt-1.5 text-xs font-semibold text-slate-800">{label}</div>
      <div className="mt-1 truncate text-[11px] text-slate-400">{helper}</div>
    </div>
  );
}

function formatTokenCount(value: number) {
  const v = value || 0;
  if (v >= 1000000) {
    return `${trimTrailingZero(v / 1000000)}M`;
  }
  if (v >= 10000) {
    return `${trimTrailingZero(v / 1000)}k`;
  }
  return new Intl.NumberFormat('zh-CN').format(v);
}

function trimTrailingZero(n: number) {
  return n
    .toFixed(1)
    .replace(/\.0$/, '');
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
